//! Desktop-integration glue that turns the binary into a resident application:
//! a single instance, a system-tray icon, a global show/hide hotkey, and a
//! start-at-login entry.
//!
//! Everything in here is best-effort. If the tray cannot be created, the hotkey
//! is already taken, or the session is Wayland (where `global-hotkey` cannot
//! grab keys at all), the app still runs as a plain window and simply reports
//! what it could not set up.

use std::{
    fs,
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
};
use tray_icon::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIconBuilder, TrayIconEvent,
};

/// Stable identifier for the Wayland `app_id` / X11 `WM_CLASS`, the desktop
/// entry's `StartupWMClass`, and eframe's on-disk storage directory.
pub const APP_ID: &str = "todo_app";

/// Human-readable name shown in the window title bar and the tray tooltip.
pub const APP_NAME: &str = "Todo App";

/// How the global hotkey is spelled for the user. `:` is Shift + `;` on the
/// usual layouts, which is why the registration below uses `Code::Semicolon`.
pub const HOTKEY_LABEL: &str = "Ctrl+Shift+:";

/// Loopback port a second launch uses to hand its command to the instance that
/// already owns the tray icon and the global hotkey.
const CONTROL_PORT: u16 = 47351;

const MENU_SHOW: &str = "todo_app.show";
const MENU_AUTOSTART: &str = "todo_app.autostart";
const MENU_QUIT: &str = "todo_app.quit";

/// The 64px render is a good compromise: large enough for a HiDPI tray, small
/// enough not to bloat the binary.
const TRAY_ICON_PNG: &[u8] = include_bytes!("../assets/icons/64x64.png");
const WINDOW_ICON_PNG: &[u8] = include_bytes!("../assets/icons/256x256.png");

/// Something that should happen to the window, whatever the source: the global
/// hotkey, the tray, or a second launch of the binary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Show,
    Hide,
    Toggle,
    ToggleAutostart,
    Quit,
}

impl Command {
    fn as_byte(self) -> u8 {
        match self {
            Self::Show => b'S',
            Self::Hide => b'H',
            Self::Toggle => b'T',
            Self::ToggleAutostart => b'A',
            Self::Quit => b'Q',
        }
    }

    fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            b'S' => Some(Self::Show),
            b'H' => Some(Self::Hide),
            b'T' => Some(Self::Toggle),
            b'A' => Some(Self::ToggleAutostart),
            b'Q' => Some(Self::Quit),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Command line
// ---------------------------------------------------------------------------

pub struct Cli {
    /// What a second launch should ask the running instance to do.
    pub command: Option<Command>,
    /// Start straight into the tray without showing the window (used by the
    /// start-at-login entry).
    pub background: bool,
}

pub fn parse_args() -> Cli {
    let mut cli = Cli {
        command: None,
        background: false,
    };

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--show" => cli.command = Some(Command::Show),
            "--hide" => cli.command = Some(Command::Hide),
            "--toggle" => cli.command = Some(Command::Toggle),
            "--quit" => cli.command = Some(Command::Quit),
            "--background" => cli.background = true,
            "--version" | "-V" => {
                println!("{APP_NAME} {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => eprintln!("todo_app: ignoring unknown argument {other:?}"),
        }
    }

    cli
}

fn print_help() {
    println!(
        "{APP_NAME} {version}

Usage: todo_app [OPTIONS]

The app lives in the system tray and is shown or hidden with {HOTKEY_LABEL}.
Launching it again while it is running forwards the option below to the
instance that is already there instead of starting a second copy.

Options:
      --toggle      Show the window if it is hidden, hide it otherwise
      --show        Show and focus the window
      --hide        Hide the window to the tray
      --quit        Quit the running instance
      --background  Start into the tray without showing the window
  -h, --help        Print this help
  -V, --version     Print the version",
        version = env!("CARGO_PKG_VERSION")
    );
}

// ---------------------------------------------------------------------------
// Single instance
// ---------------------------------------------------------------------------

pub enum Startup {
    /// This process owns the tray, the hotkey and the window.
    Primary(TcpListener),
    /// Another instance was already running and has been told what to do.
    Forwarded,
}

/// Claims the single-instance lock by binding a loopback port. Binding doubles
/// as the lock and as the channel a second launch talks over, which keeps this
/// to one mechanism per platform instead of three.
pub fn acquire(command: Option<Command>) -> Startup {
    let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, CONTROL_PORT);

    match TcpListener::bind(address) {
        Ok(listener) => Startup::Primary(listener),
        Err(_) => {
            // Default to showing the window: double-clicking the launcher while
            // the app sits in the tray should bring it up, not toggle it away.
            let command = command.unwrap_or(Command::Show);
            match TcpStream::connect(address)
                .and_then(|mut stream| stream.write_all(&[command.as_byte()]))
            {
                Ok(()) => {}
                Err(err) => eprintln!("todo_app: could not reach the running instance: {err}"),
            }
            Startup::Forwarded
        }
    }
}

// ---------------------------------------------------------------------------
// Storage migration
// ---------------------------------------------------------------------------

/// App ids earlier releases stored their data under. Before the viewport got an
/// explicit `app_id`, eframe fell back to the window title.
const LEGACY_APP_IDS: &[&str] = &["Todo App"];

/// Setting `app_id` moves eframe's storage directory, which would silently
/// orphan every existing user's projects. Copy the old state across once, before
/// `run_native` opens the new location.
pub fn migrate_legacy_storage() {
    let Some(current_dir) = eframe::storage_dir(APP_ID) else {
        return;
    };
    let current_file = current_dir.join("app.ron");
    if current_file.exists() {
        return;
    }

    for legacy_id in LEGACY_APP_IDS {
        let Some(legacy_file) = eframe::storage_dir(legacy_id).map(|dir| dir.join("app.ron"))
        else {
            continue;
        };
        if !legacy_file.exists() {
            continue;
        }

        let copied = fs::create_dir_all(&current_dir)
            .and_then(|()| fs::copy(&legacy_file, &current_file).map(|_| ()));

        match copied {
            Ok(()) => println!(
                "todo_app: migrated saved data from {}",
                legacy_file.display()
            ),
            Err(err) => eprintln!(
                "todo_app: could not migrate saved data from {}: {err}",
                legacy_file.display()
            ),
        }
        return;
    }
}

// ---------------------------------------------------------------------------
// Icons
// ---------------------------------------------------------------------------

fn decode_png(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let image = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (width, height) = image.dimensions();
    Some((image.into_raw(), width, height))
}

pub fn window_icon() -> egui::IconData {
    match decode_png(WINDOW_ICON_PNG) {
        Some((rgba, width, height)) => egui::IconData {
            rgba,
            width,
            height,
        },
        None => egui::IconData::default(),
    }
}

fn tray_icon() -> Option<tray_icon::Icon> {
    let (rgba, width, height) = decode_png(TRAY_ICON_PNG)?;
    tray_icon::Icon::from_rgba(rgba, width, height).ok()
}

// ---------------------------------------------------------------------------
// Showing and hiding the window
// ---------------------------------------------------------------------------
//
// eframe only processes viewport commands while it is painting, and a window
// that the compositor has hidden never paints. So a genuinely hidden window
// cannot bring itself back: something outside the paint loop has to make it
// visible first. On Windows that is a `ShowWindow` call. Everywhere else the
// window is minimised instead of hidden, which keeps it painting - the taskbar
// entry stays, but the tray icon and the hotkey behave the same.

#[cfg(windows)]
mod window_control {
    use std::sync::atomic::{AtomicIsize, Ordering};

    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, TRUE},
        UI::WindowsAndMessaging::{
            EnumWindows, GetWindowTextW, GetWindowThreadProcessId, SetForegroundWindow, ShowWindow,
            SW_HIDE, SW_SHOW, SW_SHOWNA,
        },
    };

    static WINDOW: AtomicIsize = AtomicIsize::new(0);

    /// The winit window is the only top-level window this process gives a title
    /// to; the tray and IME helper windows are all untitled.
    unsafe extern "system" fn visit(window: HWND, _: LPARAM) -> i32 {
        let mut process = 0u32;
        unsafe { GetWindowThreadProcessId(window, &mut process) };
        if process != std::process::id() {
            return TRUE;
        }

        let mut title = [0u16; 64];
        let length = unsafe { GetWindowTextW(window, title.as_mut_ptr(), title.len() as i32) };
        if length <= 0 {
            return TRUE;
        }

        if String::from_utf16_lossy(&title[..length as usize]) == super::APP_NAME {
            WINDOW.store(window as isize, Ordering::Relaxed);
            return 0; // stop enumerating
        }
        TRUE
    }

    fn handle() -> Option<HWND> {
        let cached = WINDOW.load(Ordering::Relaxed);
        if cached != 0 {
            return Some(cached as HWND);
        }

        unsafe { EnumWindows(Some(visit), 0) };

        match WINDOW.load(Ordering::Relaxed) {
            0 => None,
            found => Some(found as HWND),
        }
    }

    /// Takes the window off screen and out of the taskbar entirely.
    pub fn hide() -> bool {
        match handle() {
            Some(window) => {
                unsafe { ShowWindow(window, SW_HIDE) };
                true
            }
            None => false,
        }
    }

    pub fn show() {
        if let Some(window) = handle() {
            unsafe {
                ShowWindow(window, SW_SHOW);
                SetForegroundWindow(window);
            }
        }
    }

    /// Makes a hidden window paintable again without stealing focus, so the UI
    /// loop starts running and can act on the command that is about to arrive.
    pub fn wake() {
        if let Some(window) = handle() {
            unsafe { ShowWindow(window, SW_SHOWNA) };
        }
    }
}

#[cfg(not(windows))]
mod window_control {
    pub fn hide() -> bool {
        false
    }
    pub fn show() {}
    pub fn wake() {}
}

pub fn hide_window(ctx: &egui::Context) {
    if window_control::hide() {
        return;
    }
    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
}

pub fn show_window(ctx: &egui::Context) {
    window_control::show();
    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
}

// ---------------------------------------------------------------------------
// The resident desktop presence
// ---------------------------------------------------------------------------

/// Shared with the listener threads so they can wake a hidden window before
/// handing the command on to the UI loop, which only runs while it paints.
#[derive(Clone, Default)]
pub struct Visibility(std::sync::Arc<std::sync::atomic::AtomicBool>);

impl Visibility {
    pub fn set(&self, visible: bool) {
        self.0.store(visible, std::sync::atomic::Ordering::Relaxed);
    }

    fn get(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// Hands one command to the UI loop, waking the window first if it is hidden.
/// Returns false once the app has gone away, which ends the listener thread.
fn notify(
    tx: &Sender<Command>,
    ctx: &egui::Context,
    visibility: &Visibility,
    command: Command,
) -> bool {
    if !visibility.get() {
        window_control::wake();
    }
    if tx.send(command).is_err() {
        return false;
    }
    ctx.request_repaint();
    true
}

pub struct Desktop {
    events: Receiver<Command>,
    /// Kept in step with the window by the UI loop.
    pub visibility: Visibility,
    autostart: Option<auto_launch::AutoLaunch>,
    /// Whether the app is currently registered to start at login.
    pub autostart_enabled: bool,
    /// Whether the global hotkey was actually grabbed. False on Wayland, or if
    /// another application already owns the combination.
    pub hotkey_active: bool,
    /// Held so the registration outlives this struct's construction.
    _hotkeys: Option<GlobalHotKeyManager>,
    #[cfg(not(target_os = "linux"))]
    _tray: Option<tray_icon::TrayIcon>,
}

impl Desktop {
    pub fn new(ctx: &egui::Context, listener: TcpListener, visible: bool) -> Self {
        let (tx, events) = mpsc::channel();
        let visibility = Visibility::default();
        visibility.set(visible);

        spawn_control_listener(listener, tx.clone(), ctx.clone(), visibility.clone());

        let autostart = build_autostart();
        let autostart_enabled = autostart
            .as_ref()
            .and_then(|auto| auto.is_enabled().ok())
            .unwrap_or(false);

        let (hotkeys, hotkey_active) = register_hotkey();
        spawn_hotkey_listener(tx.clone(), ctx.clone(), visibility.clone());
        spawn_menu_listener(tx.clone(), ctx.clone(), visibility.clone());
        spawn_tray_click_listener(tx, ctx.clone(), visibility.clone());

        #[cfg(target_os = "linux")]
        spawn_linux_tray(autostart_enabled);

        #[cfg(not(target_os = "linux"))]
        let tray = build_tray(autostart_enabled);

        Self {
            events,
            visibility,
            autostart,
            autostart_enabled,
            hotkey_active,
            _hotkeys: hotkeys,
            #[cfg(not(target_os = "linux"))]
            _tray: tray,
        }
    }

    pub fn poll(&self) -> Option<Command> {
        self.events.try_recv().ok()
    }

    pub fn set_autostart(&mut self, enabled: bool) {
        let Some(auto) = self.autostart.as_ref() else {
            return;
        };

        let result = if enabled {
            auto.enable()
        } else {
            auto.disable()
        };
        match result {
            Ok(()) => self.autostart_enabled = enabled,
            Err(err) => eprintln!("todo_app: could not change start-at-login: {err}"),
        }
    }
}

fn build_autostart() -> Option<auto_launch::AutoLaunch> {
    let executable = std::env::current_exe()
        .map_err(|err| eprintln!("todo_app: cannot locate the executable: {err}"))
        .ok()?;

    // auto-launch writes the Windows Run entry as `{path} {args}` with no
    // quoting, so a path containing a space (`C:\Users\First Last\...`) would
    // never launch. Quote it ourselves; the other platforms pass the path as a
    // separate argv entry, where the quotes would become part of the filename.
    let executable = executable.to_string_lossy();
    let app_path = if cfg!(windows) {
        format!("\"{executable}\"")
    } else {
        executable.to_string()
    };

    auto_launch::AutoLaunchBuilder::new()
        .set_app_name(APP_NAME)
        .set_app_path(&app_path)
        .set_args(&["--background"])
        // A LaunchAgent avoids the AppleScript automation prompt macOS shows
        // for the login-items route.
        .set_macos_launch_mode(auto_launch::MacOSLaunchMode::LaunchAgent)
        .build()
        .map_err(|err| eprintln!("todo_app: could not set up start-at-login: {err}"))
        .ok()
}

fn register_hotkey() -> (Option<GlobalHotKeyManager>, bool) {
    let manager = match GlobalHotKeyManager::new() {
        Ok(manager) => manager,
        Err(err) => {
            eprintln!("todo_app: global hotkeys are unavailable ({err}).");
            eprintln!("todo_app: on Wayland, run scripts/install-hotkey-linux.sh instead.");
            return (None, false);
        }
    };

    // `:` is Shift + `;`, so the physical key to grab is Semicolon.
    let hotkey = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Semicolon);

    match manager.register(hotkey) {
        Ok(()) => (Some(manager), true),
        Err(err) => {
            eprintln!("todo_app: could not register {HOTKEY_LABEL}: {err}");
            (Some(manager), false)
        }
    }
}

fn build_tray(autostart_enabled: bool) -> Option<tray_icon::TrayIcon> {
    let menu = Menu::new();

    let show = MenuItem::with_id(MENU_SHOW, "Show / hide", true, None);
    let autostart = CheckMenuItem::with_id(
        MENU_AUTOSTART,
        "Start at login",
        true,
        autostart_enabled,
        None,
    );
    let quit = MenuItem::with_id(MENU_QUIT, "Quit", true, None);

    let build = || -> tray_icon::menu::Result<()> {
        menu.append(&show)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&autostart)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&quit)
    };

    if let Err(err) = build() {
        eprintln!("todo_app: could not build the tray menu: {err}");
        return None;
    }

    let mut builder = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(format!("{APP_NAME} - {HOTKEY_LABEL}"));

    if let Some(icon) = tray_icon() {
        builder = builder.with_icon(icon);
    }

    builder
        .build()
        .map_err(|err| eprintln!("todo_app: could not create the tray icon: {err}"))
        .ok()
}

/// egui/winit does not run a GTK loop on Linux, but the tray backend
/// (libappindicator) needs one, so it gets a thread of its own.
#[cfg(target_os = "linux")]
fn spawn_linux_tray(autostart_enabled: bool) {
    thread::spawn(move || {
        if let Err(err) = gtk::init() {
            eprintln!("todo_app: no GTK, so no tray icon: {err}");
            return;
        }
        let _tray = build_tray(autostart_enabled);
        gtk::main();
    });
}

fn spawn_control_listener(
    listener: TcpListener,
    tx: Sender<Command>,
    ctx: egui::Context,
    visibility: Visibility,
) {
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));

            let mut byte = [0u8; 1];
            if stream.read_exact(&mut byte).is_err() {
                continue;
            }

            let Some(command) = Command::from_byte(byte[0]) else {
                continue;
            };
            if !notify(&tx, &ctx, &visibility, command) {
                break;
            }
        }
    });
}

fn spawn_hotkey_listener(tx: Sender<Command>, ctx: egui::Context, visibility: Visibility) {
    thread::spawn(move || {
        let receiver = GlobalHotKeyEvent::receiver();
        while let Ok(event) = receiver.recv() {
            // Both press and release arrive; acting on release too would undo
            // the toggle immediately.
            if event.state != HotKeyState::Pressed {
                continue;
            }
            if !notify(&tx, &ctx, &visibility, Command::Toggle) {
                break;
            }
        }
    });
}

fn spawn_menu_listener(tx: Sender<Command>, ctx: egui::Context, visibility: Visibility) {
    thread::spawn(move || {
        let receiver = MenuEvent::receiver();
        while let Ok(event) = receiver.recv() {
            let command = match event.id.0.as_str() {
                MENU_SHOW => Command::Toggle,
                MENU_AUTOSTART => Command::ToggleAutostart,
                MENU_QUIT => Command::Quit,
                _ => continue,
            };
            if !notify(&tx, &ctx, &visibility, command) {
                break;
            }
        }
    });
}

fn spawn_tray_click_listener(tx: Sender<Command>, ctx: egui::Context, visibility: Visibility) {
    thread::spawn(move || {
        let receiver = TrayIconEvent::receiver();
        while let Ok(event) = receiver.recv() {
            let TrayIconEvent::Click {
                button,
                button_state,
                ..
            } = event
            else {
                continue;
            };
            if button != tray_icon::MouseButton::Left
                || button_state != tray_icon::MouseButtonState::Down
            {
                continue;
            }
            if !notify(&tx, &ctx, &visibility, Command::Toggle) {
                break;
            }
        }
    });
}
