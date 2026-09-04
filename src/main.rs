// A release build is a desktop application, not a console program: without this
// Windows opens a console window behind it every time it is launched.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod desktop;

/// Accent colour used for the title, the primary buttons and the floating
/// notes button.
const ACCENT: egui::Color32 = egui::Color32::from_rgb(124, 58, 237);
/// Lighter tint of [`ACCENT`] for text and glyphs, which need more contrast
/// against the dark background than the fill colour has.
const ACCENT_TEXT: egui::Color32 = egui::Color32::from_rgb(167, 139, 250);
/// Muted accent for icon chips sitting on the panel background.
const ACCENT_MUTED: egui::Color32 = egui::Color32::from_rgb(52, 40, 84);
/// Background of the new-project text field; the default `extreme_bg_color` is
/// pure black, which reads as a hole next to the panel.
const FIELD_BG: egui::Color32 = egui::Color32::from_rgb(20, 20, 26);

// Notes canvas palette. Pure black with mid-grey grid lines was harsh and made
// the note cards float; these sit closer together so the cards read as raised.
const CANVAS_BG: egui::Color32 = egui::Color32::from_rgb(15, 15, 20);
const GRID_MINOR: egui::Color32 = egui::Color32::from_rgb(29, 29, 37);
const GRID_MAJOR: egui::Color32 = egui::Color32::from_rgb(44, 44, 57);
const CARD_BG: egui::Color32 = egui::Color32::from_rgb(26, 26, 33);
const CARD_BORDER: egui::Color32 = egui::Color32::from_rgb(52, 52, 66);
const MENU_BG: egui::Color32 = egui::Color32::from_rgb(30, 30, 38);
const MENU_HOVER: egui::Color32 = egui::Color32::from_rgb(52, 40, 84);
/// Body text inside a note card. egui's default reads washed out against the
/// dark card, and the notes are the one place with real prose in them.
const CARD_TEXT: egui::Color32 = egui::Color32::from_rgb(223, 225, 238);
const CARD_TITLE: egui::Color32 = egui::Color32::from_rgb(240, 241, 249);
const CARD_BODY_SIZE: f32 = 15.0;

/// Row height and horizontal padding of a context-menu item. The width is
/// measured from the labels, because an `Area` is not width-constrained and
/// `available_width()` inside one runs to the edge of the screen.
const MENU_ROW_HEIGHT: f32 = 32.0;
const MENU_PADDING: f32 = 12.0;
const MENU_FONT_SIZE: f32 = 14.5;

/// Every fifth grid line is drawn brighter, which gives the canvas a sense of
/// scale while panning.
const GRID_SPACING: f32 = 50.0;
const GRID_MAJOR_EVERY: i32 = 5;

/// Window geometry. eframe restores the last size from storage, so these are
/// both the defaults and the floor the restored size is checked against.
const DEFAULT_WINDOW_SIZE: [f32; 2] = [800.0, 600.0];
const MIN_WINDOW_SIZE: [f32; 2] = [400.0, 300.0];

use chrono::prelude::*;
use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use egui_material_icons as icons;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};

// Notes Canvas Data Structures
#[derive(Clone, Serialize, Deserialize)]
struct TextBox {
    id: usize,
    title: String,
    position: egui::Pos2,
    size: egui::Vec2,

    // New Markdown content
    #[serde(default)]
    content: String,

    #[serde(default = "default_auto_height")]
    auto_height: bool,

    min_size: egui::Vec2,
}

fn default_auto_height() -> bool {
    true
}

#[derive(Serialize, Deserialize)]
struct NotesCanvas {
    text_boxes: Vec<TextBox>,
    next_textbox_id: usize,
    scene_rect: egui::Rect,
}

impl Default for NotesCanvas {
    fn default() -> Self {
        Self {
            text_boxes: Vec::new(),
            next_textbox_id: 1,
            scene_rect: egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(5000.0, 5000.0)),
        }
    }
}

// Todo App Data Structures
#[derive(Clone, Serialize, Deserialize)]
struct Task {
    id: usize,
    text: String,
    completed: bool,
}

#[derive(Clone, Serialize, Deserialize)]
struct Project {
    id: usize,
    name: String,
    tasks: Vec<Task>,
    expanded: bool,
}

#[derive(Serialize, Deserialize)]
struct TodoApp {
    projects: Vec<Project>,
    next_project_id: usize,
    next_task_id: usize,
    #[serde(skip)]
    new_project_name: String,
    #[serde(skip)]
    editing_project: Option<usize>,
    #[serde(skip)]
    editing_task: Option<(usize, usize)>, // (project_id, task_id)
    #[serde(skip)]
    new_task_texts: HashMap<usize, String>, // project_id -> new task text
    #[serde(skip)]
    edit_project_text: String,
    #[serde(skip)]
    edit_task_text: String,
    #[serde(skip)]
    adding_task_to_project: Option<usize>, // Project ID for right-click task creation
    #[serde(skip)]
    right_click_task_text: HashMap<usize, String>, // Task text for each project's right-click creation
    // Notes canvas fields
    notes_canvas: NotesCanvas,
    #[serde(skip)]
    show_notes: bool,
    #[serde(skip)]
    context_menu_pos: Option<egui::Pos2>,

    // Editing state
    #[serde(skip)]
    editing_textbox: Option<usize>, // ID of textbox being edited
    #[serde(skip)]
    commonmark_cache: CommonMarkCache, // Cache for markdown rendering

    #[serde(skip)]
    editing_title: Option<usize>, // ID of textbox whose title is being edited
    #[serde(skip)]
    temp_title_text: String,

    // Desktop integration: tray icon, global hotkey, start-at-login
    /// False until the app has decided once whether to start at login, so the
    /// default is only applied on a genuinely fresh install.
    #[serde(default)]
    autostart_configured: bool,
    #[serde(skip)]
    desktop: Option<desktop::Desktop>,
    #[serde(skip)]
    window_visible: bool,
    /// Cleared whenever the window is hidden so the size is re-checked the next
    /// time it comes back.
    #[serde(skip)]
    window_size_checked: bool,
    /// Set when the user really means to exit; otherwise closing the window
    /// only hides it to the tray.
    #[serde(skip)]
    quitting: bool,
}

impl Default for TodoApp {
    fn default() -> Self {
        Self {
            projects: Vec::new(),
            next_project_id: 1,
            next_task_id: 1,
            new_project_name: String::new(),
            editing_project: None,
            editing_task: None,
            new_task_texts: HashMap::new(),
            edit_project_text: String::new(),
            edit_task_text: String::new(),
            adding_task_to_project: None,
            right_click_task_text: HashMap::new(),
            notes_canvas: NotesCanvas::default(),
            show_notes: false,
            context_menu_pos: None,
            editing_textbox: None,
            commonmark_cache: CommonMarkCache::default(),
            editing_title: None,
            temp_title_text: String::new(),
            autostart_configured: false,
            desktop: None,
            window_visible: true,
            window_size_checked: false,
            quitting: false,
        }
    }
}

impl TodoApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Load data from storage if available

        // Customize fonts

        // --- Persistence Loading Strategy ---
        // Release: Use eframe's default storage (OS standard paths)
        // Debug: Use local file "todo_data.json" in CWD

        let loaded_app: Option<Self> = if cfg!(debug_assertions) {
            // Debug Mode: Try loading from local file
            if let Ok(file) = File::open("todo_data.json") {
                let reader = BufReader::new(file);
                serde_json::from_reader(reader).ok()
            } else {
                None
            }
        } else {
            // Release Mode: Use eframe storage
            if let Some(storage) = cc.storage {
                eframe::get_value(storage, eframe::APP_KEY)
            } else {
                None
            }
        };

        if let Some(mut app) = loaded_app {
            // Restore transient/runtime state
            app.commonmark_cache = CommonMarkCache::default();
            app.editing_textbox = None;
            app.editing_title = None;
            app.adding_task_to_project = None;
            app.editing_task = None;
            app.right_click_task_text = HashMap::new();
            app.new_task_texts = HashMap::new();
            app.context_menu_pos = None;
            app.temp_title_text = String::new();
            app.edit_task_text = String::new();

            // Ensure auto_height is set correctly for old data if needed (though serde default handles it)
            // Fix text boxes that might have come from older saves without auto_height
            // (already handled by serde default)

            return app;
        }

        // If load failed or no save exists, return default
        Default::default()
    }

    /// Hands the app its tray icon / hotkey / autostart handle once the event
    /// loop is up, and applies the first-run start-at-login default.
    fn attach_desktop(&mut self, mut integration: desktop::Desktop, visible: bool) {
        // Only a real install should register itself to start at login; a debug
        // build would point the login entry at target/debug.
        if !self.autostart_configured && !cfg!(debug_assertions) {
            integration.set_autostart(true);
            self.autostart_configured = true;
        }

        self.desktop = Some(integration);
        self.set_window_visible(visible);
    }

    /// Drains whatever the tray, the global hotkey or a second launch asked for,
    /// and turns a window close into a hide.
    fn handle_desktop_commands(&mut self, ctx: &egui::Context) {
        while let Some(command) = self.desktop.as_ref().and_then(desktop::Desktop::poll) {
            match command {
                desktop::Command::Show => self.show_window(ctx),
                desktop::Command::Hide => self.hide_window(ctx),
                desktop::Command::Toggle => {
                    if self.window_visible {
                        self.hide_window(ctx);
                    } else {
                        self.show_window(ctx);
                    }
                }
                desktop::Command::ToggleAutostart => {
                    if let Some(integration) = self.desktop.as_mut() {
                        let enabled = !integration.autostart_enabled;
                        integration.set_autostart(enabled);
                    }
                    self.autostart_configured = true;
                }
                desktop::Command::Quit => {
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }

        // Closing the window keeps the app alive in the tray, which is what
        // makes the global hotkey work at all - it needs the process running.
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.hide_window(ctx);
        }
    }

    /// Whether the global hotkey was actually grabbed by this process. It is not
    /// on Wayland, or when another application already owns the combination.
    fn hotkey_active(&self) -> bool {
        self.desktop
            .as_ref()
            .is_some_and(|integration| integration.hotkey_active)
    }

    /// eframe saves the window size on every autosave, and a window that the
    /// platform has minimised reports a degenerate one - which would then be
    /// restored on the next launch, leaving the app a few pixels wide. Put it
    /// back rather than making the user drag it out again.
    fn recover_degenerate_window_size(&mut self, ctx: &egui::Context) {
        if self.window_size_checked || !self.window_visible {
            return;
        }

        let Some(size) = ctx.input(|i| i.viewport().inner_rect.map(|rect| rect.size())) else {
            return;
        };
        self.window_size_checked = true;

        if size.x < MIN_WINDOW_SIZE[0] || size.y < MIN_WINDOW_SIZE[1] {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                DEFAULT_WINDOW_SIZE[0],
                DEFAULT_WINDOW_SIZE[1],
            )));
        }
    }

    fn show_window(&mut self, ctx: &egui::Context) {
        desktop::show_window(ctx);
        self.set_window_visible(true);
    }

    fn hide_window(&mut self, ctx: &egui::Context) {
        desktop::hide_window(ctx);
        self.set_window_visible(false);
    }

    /// The listener threads need this too: a hidden window has to be woken from
    /// outside the paint loop before it can act on anything.
    fn set_window_visible(&mut self, visible: bool) {
        if !visible {
            self.window_size_checked = false;
        }
        self.window_visible = visible;
        if let Some(integration) = self.desktop.as_ref() {
            integration.visibility.set(visible);
        }
    }
}

impl eframe::App for TodoApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // --- Persistence Saving Strategy ---
        // Release: Use eframe's default storage
        // Debug: Use local file "todo_data.json"

        if cfg!(debug_assertions) {
            // Debug Mode: Save to local file
            if let Ok(file) = File::create("todo_data.json") {
                let writer = BufWriter::new(file);
                // Pretty print for easier debugging
                let _ = serde_json::to_writer_pretty(writer, self);
            }
        } else {
            // Release Mode: Save to eframe storage
            eframe::set_value(storage, eframe::APP_KEY, self);
        }
    }

    /// Closing the window is cancelled and turned into a hide, so eframe's
    /// save-on-exit may never run. Autosaving more eagerly keeps the window of
    /// possible data loss small.
    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(5)
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_desktop_commands(ctx);
        self.recover_degenerate_window_size(ctx);

        // Render floating pen button (always visible)
        self.render_floating_pen_button(ctx);

        // Conditionally render main content
        if self.show_notes {
            self.render_notes_canvas(ctx);
        } else {
            self.render_todo_view(ctx);
        }
    }
}

impl TodoApp {
    fn render_floating_pen_button(&mut self, ctx: &egui::Context) {
        egui::Area::new(egui::Id::new("floating_pen_button"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-20.0, -20.0))
            .show(ctx, |ui| {
                let button_size = 60.0;
                let pen_icon = if self.show_notes {
                    icons::icons::ICON_CLOSE // X to close notes
                } else {
                    icons::icons::ICON_EDIT // Pen to open notes
                };

                if ui
                    .add_sized(
                        [button_size, button_size],
                        egui::Button::new(
                            egui::RichText::new(pen_icon)
                                .size(24.0)
                                .color(egui::Color32::WHITE),
                        )
                        .fill(ACCENT)
                        .corner_radius(30.0),
                    )
                    .clicked()
                {
                    self.show_notes = !self.show_notes;
                }
            });
    }

    fn render_todo_view(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Fixed font sizes
            let heading_size = 24.0;
            let project_title_size = 20.0; // Larger font for project titles
            let label_size = 16.0;
            let button_size = 14.0;
            let text_size = 16.0; // Increased task text size for better visibility

            // Header card: user on the left, title in the middle, date on the
            // right, with the new-project row in its own panel underneath.
            egui::Frame::group(ui.style())
                .corner_radius(12)
                .inner_margin(egui::Margin::symmetric(16, 12))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new("User")
                                    .size(11.0)
                                    .color(ui.visuals().weak_text_color()),
                            );
                            ui.label(
                                egui::RichText::new(
                                    whoami::username()
                                        .unwrap_or_else(|_| "Unknown".to_string()),
                                )
                                .size(label_size)
                                .strong(),
                            );
                        });

                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                let now = Local::now();
                                ui.label(
                                    egui::RichText::new(now.format("%d/%m/%Y").to_string())
                                        .size(label_size),
                                );
                                ui.add_space(6.0);
                                ui.label(
                                    egui::RichText::new(icons::icons::ICON_CALENDAR_MONTH)
                                        .size(label_size)
                                        .color(ui.visuals().weak_text_color()),
                                );

                                // Centred in whatever is left between the two blocks.
                                ui.with_layout(
                                    egui::Layout::centered_and_justified(
                                        egui::Direction::LeftToRight,
                                    ),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new("Todo App")
                                                .size(heading_size)
                                                .strong()
                                                .color(ACCENT_TEXT),
                                        );
                                    },
                                );
                            },
                        );
                    });

                    ui.add_space(10.0);

                    // Add new project section
                    egui::Frame::group(ui.style())
                        .corner_radius(10)
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());

                            ui.horizontal(|ui| {
                                egui::Frame::new()
                                    .fill(ACCENT_MUTED)
                                    .corner_radius(8)
                                    .inner_margin(egui::Margin::same(6))
                                    .show(ui, |ui| {
                                        ui.label(
                                            egui::RichText::new(icons::icons::ICON_FOLDER_OPEN)
                                                .size(18.0)
                                                .color(ACCENT_TEXT),
                                        );
                                    });

                                ui.add_space(6.0);
                                ui.label(
                                    egui::RichText::new("New Project")
                                        .size(label_size)
                                        .strong(),
                                );
                                ui.add_space(6.0);

                                // The field's height comes from its font and
                                // margin, not from a size argument, so match the
                                // button by padding it rather than sizing it.
                                // A single-line TextEdit takes its height from
                                // its font and margin, not from a size argument,
                                // so pad it to the button's height rather than
                                // trying to size it directly.
                                let control_height = 34.0;
                                let field_font_size = 15.0;
                                let field_row_height = ui
                                    .painter()
                                    .layout_no_wrap(
                                        "Ag".to_owned(),
                                        egui::FontId::proportional(field_font_size),
                                        egui::Color32::WHITE,
                                    )
                                    .size()
                                    .y;
                                let field_padding =
                                    (((control_height - field_row_height) / 2.0) as i8).max(2);

                                // Round the field's corners to match the button.
                                let corner = egui::CornerRadius::same(8);
                                let visuals = ui.visuals_mut();
                                visuals.extreme_bg_color = FIELD_BG;
                                visuals.widgets.inactive.corner_radius = corner;
                                visuals.widgets.hovered.corner_radius = corner;
                                visuals.widgets.active.corner_radius = corner;

                                let add_button = egui::Button::new(
                                    egui::RichText::new(format!(
                                        "{} Add Project",
                                        icons::icons::ICON_ADD
                                    ))
                                    .size(button_size)
                                    .strong()
                                    .color(egui::Color32::WHITE),
                                )
                                .fill(ACCENT)
                                .corner_radius(8);

                                // Reserve room for the button so the field can
                                // take everything else without pushing it out.
                                let button_width = 150.0;
                                let field_width =
                                    (ui.available_width() - button_width - 12.0).max(120.0);

                                let response = ui.add(
                                    egui::TextEdit::singleline(&mut self.new_project_name)
                                        .hint_text("Enter project name...")
                                        .font(egui::FontId::proportional(field_font_size))
                                        .margin(egui::Margin::symmetric(10, field_padding))
                                        .desired_width(field_width),
                                );

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .add_sized([button_width, control_height], add_button)
                                            .clicked()
                                            || (response.lost_focus()
                                                && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                                        {
                                            self.add_project();
                                        }
                                    },
                                );
                            });
                        });
                });

            ui.add_space(16.0);

            // Simplified instruction for users
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "Right-click on the expand/collapse button ({}/{}) to add tasks directly!",
                        icons::icons::ICON_CHEVRON_RIGHT,
                        icons::icons::ICON_EXPAND_MORE
                    ))
                    .size(label_size)
                    .color(egui::Color32::GRAY),
                );
            });

            ui.horizontal(|ui| {
                let hotkey_hint = if self.hotkey_active() {
                    format!(
                        "Press {} anywhere to show or hide this window.",
                        desktop::HOTKEY_LABEL
                    )
                } else {
                    format!(
                        "{} could not be registered. On Wayland, run                          /usr/share/todo_app/install-hotkey-linux.sh to bind it                          in your desktop environment.",
                        desktop::HOTKEY_LABEL
                    )
                };
                ui.label(
                    egui::RichText::new(hotkey_hint)
                        .size(label_size)
                        .color(egui::Color32::GRAY),
                );
            });

            ui.add_space(16.0);

            ui.separator();

            // Display projects in a scroll area
            let (projects_to_remove, project_actions, task_actions) = egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    let mut projects_to_remove = Vec::new();
                    let mut project_actions = Vec::new(); // Store actions to perform after iteration
                    let mut task_actions = Vec::new(); // Store task actions

                    for (project_idx, project) in self.projects.iter_mut().enumerate() {
                        ui.push_id(project.id, |ui| {
                            let _frame_response = egui::Frame::group(ui.style())
                                .inner_margin(egui::Margin::same(16))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    // Project header
                                    ui.horizontal(|ui| {
                                        // Expand/collapse button with right-click to add task
                                        let expand_icon = if project.expanded {
                                            icons::icons::ICON_EXPAND_MORE
                                        } else {
                                            icons::icons::ICON_CHEVRON_RIGHT
                                        };
                                        let expand_response = ui.button(
                                            egui::RichText::new(expand_icon).size(button_size),
                                        );

                                        if expand_response.clicked() {
                                            project.expanded = !project.expanded;
                                        }

                                        // Right-click on expand button to add task
                                        if expand_response.secondary_clicked() {
                                            project_actions.push((
                                                "add_task",
                                                project.id,
                                                String::new(),
                                            ));
                                        }

                                        // Project name and controls
                                        if self.editing_project == Some(project.id) {
                                            // Editing mode: show text input with confirmation buttons
                                            let edit_width =
                                                (ui.available_width() - 2.0 * button_size - 48.0)
                                                    .max(80.0);
                                            let response = ui.add(
                                                egui::TextEdit::singleline(
                                                    &mut self.edit_project_text,
                                                )
                                                .desired_width(edit_width),
                                            );
                                            if response.lost_focus()
                                                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                            {
                                                if !self.edit_project_text.trim().is_empty() {
                                                    project.name = self.edit_project_text.clone();
                                                }
                                                project_actions.push((
                                                    "stop_edit",
                                                    project.id,
                                                    String::new(),
                                                ));
                                            } else if response.lost_focus()
                                                && ui.input(|i| i.key_pressed(egui::Key::Escape))
                                            {
                                                project_actions.push((
                                                    "stop_edit",
                                                    project.id,
                                                    String::new(),
                                                ));
                                            }

                                            if ui.button(icons::icons::ICON_CHECK).clicked() {
                                                if !self.edit_project_text.trim().is_empty() {
                                                    project.name = self.edit_project_text.clone();
                                                }
                                                project_actions.push((
                                                    "stop_edit",
                                                    project.id,
                                                    String::new(),
                                                ));
                                            }
                                            if ui.button(icons::icons::ICON_CLOSE).clicked() {
                                                project_actions.push((
                                                    "stop_edit",
                                                    project.id,
                                                    String::new(),
                                                ));
                                            }
                                        } else {
                                            // Display mode: the action buttons are laid out first
                                            // so they keep their space at the right edge; the
                                            // project name is truncated into what is left over.
                                            let name_color = ui.visuals().text_color();

                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    // Delete project button
                                                    if ui
                                                        .button(
                                                            egui::RichText::new(
                                                                icons::icons::ICON_DELETE,
                                                            )
                                                            .size(button_size),
                                                        )
                                                        .clicked()
                                                    {
                                                        projects_to_remove.push(project_idx);
                                                    }

                                                    // Edit project button
                                                    if ui
                                                        .button(
                                                            egui::RichText::new(
                                                                icons::icons::ICON_EDIT,
                                                            )
                                                            .size(button_size),
                                                        )
                                                        .clicked()
                                                    {
                                                        project_actions.push((
                                                            "start_edit",
                                                            project.id,
                                                            project.name.clone(),
                                                        ));
                                                    }

                                                    truncating_label(
                                                        ui,
                                                        &project.name,
                                                        project_title_size,
                                                        name_color,
                                                    );
                                                },
                                            );
                                        }
                                    });

                                    // Tasks (only shown when expanded)
                                    if project.expanded {
                                        ui.indent("tasks", |ui| {
                                            let mut tasks_to_remove = Vec::new();

                                            for (task_idx, task) in
                                                project.tasks.iter_mut().enumerate()
                                            {
                                                ui.add_space(8.0);
                                                ui.horizontal(|ui| {
                                                    // Checkbox for completion
                                                    ui.checkbox(&mut task.completed, "");

                                                    // Task text and controls
                                                    if self.editing_task
                                                        == Some((project.id, task.id))
                                                    {
                                                        // Editing mode: show text input with confirmation buttons
                                                        let edit_width =
                                                            (ui.available_width() - 80.0).max(80.0);
                                                        let response = ui.add(
                                                            egui::TextEdit::singleline(
                                                                &mut self.edit_task_text,
                                                            )
                                                            .desired_width(edit_width),
                                                        );
                                                        if response.lost_focus()
                                                            && ui.input(|i| {
                                                                i.key_pressed(egui::Key::Enter)
                                                            })
                                                        {
                                                            if !self
                                                                .edit_task_text
                                                                .trim()
                                                                .is_empty()
                                                            {
                                                                task.text =
                                                                    self.edit_task_text.clone();
                                                            }
                                                            task_actions.push((
                                                                "stop_edit",
                                                                project.id,
                                                                task.id,
                                                                String::new(),
                                                            ));
                                                        } else if response.lost_focus()
                                                            && ui.input(|i| {
                                                                i.key_pressed(egui::Key::Escape)
                                                            })
                                                        {
                                                            task_actions.push((
                                                                "stop_edit",
                                                                project.id,
                                                                task.id,
                                                                String::new(),
                                                            ));
                                                        }

                                                        if ui
                                                            .button(icons::icons::ICON_CHECK)
                                                            .clicked()
                                                        {
                                                            if !self
                                                                .edit_task_text
                                                                .trim()
                                                                .is_empty()
                                                            {
                                                                task.text =
                                                                    self.edit_task_text.clone();
                                                            }
                                                            task_actions.push((
                                                                "stop_edit",
                                                                project.id,
                                                                task.id,
                                                                String::new(),
                                                            ));
                                                        }
                                                        if ui
                                                            .button(icons::icons::ICON_CLOSE)
                                                            .clicked()
                                                        {
                                                            task_actions.push((
                                                                "stop_edit",
                                                                project.id,
                                                                task.id,
                                                                String::new(),
                                                            ));
                                                        }
                                                    } else {
                                                        // Display mode: the action buttons are
                                                        // laid out first so they keep their space
                                                        // at the right edge; the task text is then
                                                        // truncated into whatever is left over.
                                                        let text_color = if task.completed {
                                                            ui.visuals().weak_text_color()
                                                        } else {
                                                            ui.visuals().text_color()
                                                        };

                                                        ui.with_layout(
                                                            egui::Layout::right_to_left(
                                                                egui::Align::Center,
                                                            ),
                                                            |ui| {
                                                                // Delete task button
                                                                if ui
                                                                    .button(
                                                                        icons::icons::ICON_DELETE,
                                                                    )
                                                                    .clicked()
                                                                {
                                                                    tasks_to_remove.push(task_idx);
                                                                }

                                                                // Edit task button
                                                                if ui
                                                                    .button(icons::icons::ICON_EDIT)
                                                                    .clicked()
                                                                {
                                                                    task_actions.push((
                                                                        "start_edit",
                                                                        project.id,
                                                                        task.id,
                                                                        task.text.clone(),
                                                                    ));
                                                                }

                                                                truncating_label(
                                                                    ui, &task.text, text_size,
                                                                    text_color,
                                                                );
                                                            },
                                                        );
                                                    }
                                                });
                                            }

                                            // Remove tasks
                                            for &idx in tasks_to_remove.iter().rev() {
                                                project.tasks.remove(idx);
                                            }

                                            // Show inline task creation UI when this project is selected for task addition
                                            if self.adding_task_to_project == Some(project.id) {
                                                ui.add_space(8.0);
                                                ui.horizontal(|ui| {
                                                    ui.label("New Task:");
                                                    let task_text = self
                                                        .right_click_task_text
                                                        .get_mut(&project.id)
                                                        .unwrap();
                                                    let response =
                                                        ui.text_edit_singleline(task_text);

                                                    if ui.button(icons::icons::ICON_CHECK).clicked()
                                                        || (response.lost_focus()
                                                            && ui.input(|i| {
                                                                i.key_pressed(egui::Key::Enter)
                                                            }))
                                                    {
                                                        if !task_text.trim().is_empty() {
                                                            project_actions.push((
                                                                "create_task",
                                                                project.id,
                                                                task_text.clone(),
                                                            ));
                                                        }
                                                        project_actions.push((
                                                            "cancel_add_task",
                                                            project.id,
                                                            String::new(),
                                                        ));
                                                    }

                                                    if ui.button(icons::icons::ICON_CLOSE).clicked()
                                                        || (response.lost_focus()
                                                            && ui.input(|i| {
                                                                i.key_pressed(egui::Key::Escape)
                                                            }))
                                                    {
                                                        project_actions.push((
                                                            "cancel_add_task",
                                                            project.id,
                                                            String::new(),
                                                        ));
                                                    }
                                                });
                                            }
                                        });
                                    }
                                });
                        });
                        ui.add_space(16.0);
                    }

                    (projects_to_remove, project_actions, task_actions)
                })
                .inner;

            // Process project actions
            for (action, project_id, text) in project_actions {
                match action {
                    "start_edit" => {
                        self.editing_project = Some(project_id);
                        self.edit_project_text = text;
                    }
                    "stop_edit" => {
                        self.editing_project = None;
                    }
                    "add_task" => {
                        self.adding_task_to_project = Some(project_id);
                        // Initialize the text field for this project if it doesn't exist
                        self.right_click_task_text.entry(project_id).or_default();
                    }
                    "create_task" => {
                        self.add_task_to_project(project_id, text);
                    }
                    "cancel_add_task" => {
                        self.adding_task_to_project = None;
                        if let Some(task_text) = self.right_click_task_text.get_mut(&project_id) {
                            task_text.clear();
                        }
                    }
                    _ => {}
                }
            }

            // Process task actions
            for (action, project_id, task_id, text) in task_actions {
                match action {
                    "start_edit" => {
                        self.editing_task = Some((project_id, task_id));
                        self.edit_task_text = text;
                    }
                    "stop_edit" => {
                        self.editing_task = None;
                    }
                    _ => {}
                }
            }

            // Remove projects
            for &idx in projects_to_remove.iter().rev() {
                let project_id = self.projects[idx].id;
                self.projects.remove(idx);
                self.new_task_texts.remove(&project_id);
            }
        });
    }

    fn render_notes_canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let rect = ui.max_rect();

            // Detect middle mouse or two-finger drag for panning
            let canvas_response = ui.interact(
                rect,
                egui::Id::new("canvas_interact"),
                egui::Sense::click_and_drag(),
            );

            // Pan with middle mouse button
            if canvas_response.dragged_by(egui::PointerButton::Middle) {
                self.notes_canvas.scene_rect = self
                    .notes_canvas
                    .scene_rect
                    .translate(canvas_response.drag_delta());
            }

            // Pan with two-finger trackpad drag
            // translation_delta() provides panning from scrolling or pan gestures
            let pan_delta = ui.input(|i| i.translation_delta());
            if pan_delta != egui::Vec2::ZERO {
                self.notes_canvas.scene_rect = self.notes_canvas.scene_rect.translate(pan_delta);
            }

            // Render background with pan offset
            self.render_canvas_background(ui);

            // Render text boxes with pan offset
            self.render_text_boxes(ui);

            // Handle right-click context menu
            self.handle_context_menu(ui);
        });

        // Center button in top-right corner
        egui::Area::new(egui::Id::new("center_button"))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-20.0, 20.0))
            .show(ctx, |ui| {
                let button_size = 40.0;
                if ui
                    .add_sized(
                        [button_size, button_size],
                        egui::Button::new(
                            egui::RichText::new(icons::icons::ICON_CENTER_FOCUS_STRONG).size(20.0),
                        )
                        .corner_radius(5.0),
                    )
                    .on_hover_text("Reset view to center")
                    .clicked()
                {
                    // Reset to center
                    self.notes_canvas.scene_rect =
                        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(5000.0, 5000.0));
                }
            });
    }

    fn render_canvas_background(&self, ui: &mut egui::Ui) {
        let painter = ui.painter();
        let rect = ui.max_rect();
        let pan_offset = self.notes_canvas.scene_rect.min.to_vec2();

        painter.rect_filled(rect, 0.0, CANVAS_BG);

        // Grid indices are counted from the canvas origin rather than from the
        // viewport, so the brighter major lines stay put while panning.
        let first_col = ((rect.min.x - pan_offset.x) / GRID_SPACING).floor() as i32;
        let last_col = ((rect.max.x - pan_offset.x) / GRID_SPACING).ceil() as i32;
        let first_row = ((rect.min.y - pan_offset.y) / GRID_SPACING).floor() as i32;
        let last_row = ((rect.max.y - pan_offset.y) / GRID_SPACING).ceil() as i32;

        let line_for = |index: i32| {
            if index.rem_euclid(GRID_MAJOR_EVERY) == 0 {
                egui::Stroke::new(1.0_f32, GRID_MAJOR)
            } else {
                egui::Stroke::new(1.0_f32, GRID_MINOR)
            }
        };

        for col in first_col..=last_col {
            let x = col as f32 * GRID_SPACING + pan_offset.x;
            if rect.x_range().contains(x) {
                painter.line_segment(
                    [egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)],
                    line_for(col),
                );
            }
        }

        for row in first_row..=last_row {
            let y = row as f32 * GRID_SPACING + pan_offset.y;
            if rect.y_range().contains(y) {
                painter.line_segment(
                    [egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)],
                    line_for(row),
                );
            }
        }

        // Empty-canvas hint, at the viewport centre.
        if self.notes_canvas.text_boxes.is_empty() {
            let center = rect.center();
            painter.text(
                center - egui::vec2(0.0, 18.0),
                egui::Align2::CENTER_CENTER,
                "Right-click anywhere to create a note",
                egui::FontId::proportional(22.0),
                ACCENT_TEXT,
            );
            painter.text(
                center + egui::vec2(0.0, 16.0),
                egui::Align2::CENTER_CENTER,
                "Middle-click or drag with two fingers to pan",
                egui::FontId::proportional(15.0),
                egui::Color32::from_gray(110),
            );
        }
    }

    fn render_text_boxes(&mut self, ui: &mut egui::Ui) {
        let mut actions: Vec<(&str, usize)> = Vec::new(); // (action, textbox_id)
        let pan_offset = self.notes_canvas.scene_rect.min.to_vec2();

        // Iterate through text boxes (render in order for z-ordering)
        for text_box_idx in 0..self.notes_canvas.text_boxes.len() {
            let text_box = &mut self.notes_canvas.text_boxes[text_box_idx];
            let id = egui::Id::new("textbox").with(text_box.id);

            // Apply pan offset to text box position
            let screen_position = text_box.position + pan_offset;

            // Calculate frame rect
            // If auto-height, we use a flexible height initially (Infinity) so it can grow
            // If manual, we use the stored size
            let current_size = if text_box.auto_height {
                egui::vec2(text_box.size.x, f32::INFINITY)
            } else {
                text_box.size
            };

            let frame_rect = egui::Rect::from_min_size(screen_position, current_size);
            let textbox_id = text_box.id;

            // Create a child UI at the text box position
            // We use a builder but we DON'T strictly force max_rect if we want auto-growth
            // However, for the frame background to be drawn correctly, we wrap it.
            let mut child_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(frame_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );

            let is_active =
                self.editing_textbox == Some(textbox_id) || self.editing_title == Some(textbox_id);
            let frame = egui::Frame::new()
                .fill(CARD_BG)
                .stroke(egui::Stroke::new(
                    1.0_f32,
                    if is_active { ACCENT } else { CARD_BORDER },
                ))
                .corner_radius(14)
                .inner_margin(egui::Margin::same(14))
                .shadow(egui::epaint::Shadow {
                    offset: [0, 10],
                    blur: 24,
                    spread: 0,
                    color: egui::Color32::from_black_alpha(130),
                });

            let inner_response = frame.show(&mut child_ui, |ui| {
                ui.set_width(text_box.size.x - 20.0);

                // If MANUAL mode, force the frame to be at least the user-defined height
                if !text_box.auto_height {
                    ui.set_min_height(text_box.size.y - 20.0); // -20 for padding/margins approximation
                }

                // Header with title and delete button
                // Use a Right-to-Left layout to position the delete button first (on the right)
                // Then use the remaining space for the Title, making it the drag handle.
                let mut delta = egui::Vec2::ZERO;

                // Allocate a fixed height strip for the header so it doesn't float in the middle
                // of the box due to Align::Center
                let header_height = 30.0;
                let header_rect = ui
                    .allocate_exact_size(
                        egui::vec2(ui.available_width(), header_height),
                        egui::Sense::hover(),
                    )
                    .0;

                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(header_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // 1. Delete Button (Right aligned)
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(icons::icons::ICON_DELETE)
                                            .size(15.0)
                                            .color(egui::Color32::from_gray(140)),
                                    )
                                    .fill(egui::Color32::TRANSPARENT)
                                    .corner_radius(7),
                                )
                                .on_hover_text("Delete this note")
                                .clicked()
                            {
                                actions.push(("delete", textbox_id));
                            }

                            // 2. Title (Fills remaining space to the left)
                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    // Grip, so it is obvious the header is the
                                    // handle you drag the note around by.
                                    ui.label(
                                        egui::RichText::new(icons::icons::ICON_DRAG_INDICATOR)
                                            .size(15.0)
                                            .color(egui::Color32::from_gray(90)),
                                    );
                                    // Check if this title is being edited
                                    if self.editing_title == Some(textbox_id) {
                                        let response = ui.add(
                                            egui::TextEdit::singleline(&mut self.temp_title_text)
                                                .desired_width(ui.available_width())
                                                .font(egui::FontId::proportional(15.0)),
                                        );

                                        if response.lost_focus() {
                                            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                                actions.push(("save_title", textbox_id));
                                            } else if ui.input(|i| i.key_pressed(egui::Key::Escape))
                                            {
                                                actions.push(("cancel_title_edit", textbox_id));
                                            }
                                        }
                                    } else {
                                        // Display Title - drag handle and rename target
                                        let title_response = ui
                                            .allocate_ui_with_layout(
                                                egui::vec2(ui.available_width(), 24.0),
                                                egui::Layout::left_to_right(egui::Align::Center),
                                                |ui| {
                                                    ui.add(
                                                        egui::Label::new(
                                                            egui::RichText::new(&text_box.title)
                                                                .strong()
                                                                .size(16.0)
                                                                .color(CARD_TITLE),
                                                        )
                                                        .truncate()
                                                        .sense(egui::Sense::click_and_drag()),
                                                    )
                                                },
                                            )
                                            .inner
                                            .on_hover_cursor(egui::CursorIcon::Grab)
                                            .on_hover_text("Drag to move, double-click to rename");

                                        if title_response.double_clicked() {
                                            actions.push(("edit_title", textbox_id));
                                        }

                                        if title_response.dragged_by(egui::PointerButton::Primary) {
                                            delta = title_response.drag_delta();
                                            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                                        }

                                        // Bring the note the user just touched to
                                        // the front of the stack.
                                        if title_response.drag_started() || title_response.clicked()
                                        {
                                            actions.push(("raise", textbox_id));
                                        }
                                    }
                                },
                            );
                        });
                    },
                );

                ui.scope(|ui| {
                    ui.visuals_mut().widgets.noninteractive.bg_stroke =
                        egui::Stroke::new(1.0_f32, CARD_BORDER);
                    ui.separator();
                });

                // Everything below the separator is the note body, so the style
                // change applies to exactly that.
                ui.style_mut().text_styles.insert(
                    egui::TextStyle::Body,
                    egui::FontId::proportional(CARD_BODY_SIZE),
                );
                ui.visuals_mut().override_text_color = Some(CARD_TEXT);

                // Main Content Area
                let is_editing = self.editing_textbox == Some(textbox_id);

                if is_editing {
                    // Edit Mode
                    // Use ui.add with desired_width instead of add_sized to allow height to grow naturally
                    let response = ui.add(
                        egui::TextEdit::multiline(&mut text_box.content)
                            .frame(false)
                            .hint_text("Markdown supported. Click outside to finish.")
                            .font(egui::FontId::proportional(15.0))
                            .desired_width(ui.available_width()),
                    );

                    if response.clicked_elsewhere() {
                        actions.push(("stop_editing", textbox_id));
                    }
                } else {
                    // View Mode
                    let scroll_area =
                        egui::ScrollArea::vertical().id_salt(format!("scroll_{}", textbox_id));

                    // In auto-height mode, we don't want the scroll area to shrink or scroll, just expand.
                    // In manual mode, we want it to scroll if content is too big.
                    let scroll_area = if text_box.auto_height {
                        scroll_area.auto_shrink([false, true]) // Let it grow vertically
                    } else {
                        scroll_area.auto_shrink([false, false]) // Fill available space
                    };

                    let scroll_response = scroll_area.show(ui, |ui| {
                        if text_box.content.is_empty() {
                            if ui
                                .add(
                                    egui::Label::new(
                                        egui::RichText::new("Double-click to write something...")
                                            .italics()
                                            .color(egui::Color32::from_gray(105)),
                                    )
                                    .sense(egui::Sense::click()),
                                )
                                .on_hover_cursor(egui::CursorIcon::Text)
                                .double_clicked()
                            {
                                actions.push(("start_editing", textbox_id));
                            }
                        } else {
                            CommonMarkViewer::new().show(
                                ui,
                                &mut self.commonmark_cache,
                                &text_box.content,
                            );
                        }
                    });

                    if !text_box.content.is_empty()
                        && ui
                            .interact(
                                scroll_response.inner_rect,
                                id.with("view_interact"),
                                egui::Sense::click(),
                            )
                            .double_clicked()
                    {
                        actions.push(("start_editing", textbox_id));
                    }
                }

                delta
            });

            let header_drag_delta = inner_response.inner;
            let response = inner_response.response;

            // Handle dragging (Position Update)
            let text_box = &mut self.notes_canvas.text_boxes[text_box_idx];
            // Only update position if we got a delta from the header
            if header_drag_delta != egui::Vec2::ZERO {
                text_box.position += header_drag_delta;
            }

            // Note: We removed the old drag logic associated with 'response' (the whole frame)

            // If we are in auto_height mode, update the size to match the frame size
            if text_box.auto_height {
                // FIXED: Only update HEIGHT. Preserve width to prevent shrinking loop!
                text_box.size.y = response.rect.height();
            }

            // Resize handle
            let resize_handle_size = 15.0;
            // The handle should ALWAYS be at the bottom-right of the actual rendered frame
            let resize_handle_pos = egui::pos2(
                response.rect.max.x - resize_handle_size,
                response.rect.max.y - resize_handle_size,
            );
            let resize_handle_rect = egui::Rect::from_min_size(
                resize_handle_pos,
                egui::vec2(resize_handle_size, resize_handle_size),
            );

            let resize_response = ui.interact(
                resize_handle_rect,
                id.with("resize_handle"),
                egui::Sense::drag(),
            );

            // Double-click resize handle to reset to auto-height
            // double_clicked() defaults to primary button
            if resize_response.double_clicked() {
                text_box.auto_height = true;
            }

            if resize_response.dragged_by(egui::PointerButton::Primary) {
                let delta = resize_response.drag_delta();

                // If dragging height, disable auto_height
                if delta.y.abs() > 0.0 {
                    text_box.auto_height = false;
                }

                text_box.size += delta;
                text_box.size = text_box.size.max(text_box.min_size);
            }

            // A grip of diagonal strokes reads as "drag me" far better than the
            // plain square did, and the cursor now changes to match.
            if resize_response.hovered() || resize_response.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
            }
            let grip_color = if resize_response.hovered() || resize_response.dragged() {
                ACCENT_TEXT
            } else {
                egui::Color32::from_gray(85)
            };
            let corner = resize_handle_rect.max - egui::vec2(3.0, 3.0);
            for offset in [3.0_f32, 7.0, 11.0] {
                ui.painter().line_segment(
                    [
                        egui::pos2(corner.x - offset, corner.y),
                        egui::pos2(corner.x, corner.y - offset),
                    ],
                    egui::Stroke::new(1.6_f32, grip_color),
                );
            }
        }

        // Process actions
        for (action, textbox_id) in actions {
            match action {
                "delete" => {
                    self.notes_canvas
                        .text_boxes
                        .retain(|tb| tb.id != textbox_id);
                }
                "edit_title" => {
                    if let Some(textbox) = self
                        .notes_canvas
                        .text_boxes
                        .iter()
                        .find(|tb| tb.id == textbox_id)
                    {
                        self.editing_title = Some(textbox_id);
                        self.temp_title_text = textbox.title.clone();
                    }
                }
                "save_title" => {
                    if let Some(textbox) = self
                        .notes_canvas
                        .text_boxes
                        .iter_mut()
                        .find(|tb| tb.id == textbox_id)
                    {
                        if !self.temp_title_text.trim().is_empty() {
                            textbox.title = self.temp_title_text.clone();
                        }
                    }
                    self.editing_title = None;
                    self.temp_title_text.clear();
                }
                "cancel_title_edit" => {
                    self.editing_title = None;
                    self.temp_title_text.clear();
                }
                "start_editing" => {
                    self.editing_textbox = Some(textbox_id);
                }
                "stop_editing" => {
                    self.editing_textbox = None;
                }
                "raise" => {
                    // Notes are painted in vector order, so moving one to the end
                    // puts it on top of the others.
                    if let Some(index) = self
                        .notes_canvas
                        .text_boxes
                        .iter()
                        .position(|tb| tb.id == textbox_id)
                    {
                        let raised = self.notes_canvas.text_boxes.remove(index);
                        self.notes_canvas.text_boxes.push(raised);
                    }
                }
                _ => {}
            }
        }
    }

    fn handle_context_menu(&mut self, ui: &mut egui::Ui) {
        let pan_offset = self.notes_canvas.scene_rect.min.to_vec2();

        // Detect right-click
        ui.input(|i| {
            if i.pointer.secondary_clicked() {
                if let Some(screen_pos) = i.pointer.interact_pos() {
                    // Convert screen position to canvas position
                    let canvas_pos = screen_pos - pan_offset;

                    // Check if click is not on any text box
                    let on_textbox = self.notes_canvas.text_boxes.iter().any(|tb| {
                        egui::Rect::from_min_size(tb.position, tb.size).contains(canvas_pos)
                    });

                    if !on_textbox {
                        self.context_menu_pos = Some(screen_pos);
                    }
                }
            }
        });

        // Show context menu
        if let Some(menu_screen_pos) = self.context_menu_pos {
            egui::Area::new(egui::Id::new("context_menu"))
                .fixed_pos(menu_screen_pos)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style())
                        .fill(MENU_BG)
                        .stroke(egui::Stroke::new(1.0_f32, CARD_BORDER))
                        .corner_radius(10)
                        .inner_margin(egui::Margin::same(6))
                        .shadow(egui::epaint::Shadow {
                            offset: [0, 8],
                            blur: 20,
                            spread: 0,
                            color: egui::Color32::from_black_alpha(140),
                        })
                        .show(ui, |ui| {
                            let items = [
                                (icons::icons::ICON_ADD_BOX, "Create note", true),
                                (icons::icons::ICON_CLOSE, "Cancel", false),
                            ];
                            let width = menu_width(ui, &items);

                            for (icon, label, accent) in items {
                                if menu_item(ui, width, icon, label, accent).clicked() {
                                    if accent {
                                        let canvas_pos = menu_screen_pos - pan_offset;
                                        self.create_text_box_at(canvas_pos);
                                    }
                                    self.context_menu_pos = None;
                                }
                            }
                        });
                });
        }

        // Close menu on click elsewhere
        ui.input(|i| {
            if i.pointer.primary_clicked() && self.context_menu_pos.is_some() {
                self.context_menu_pos = None;
            }
        });
    }

    fn create_text_box_at(&mut self, pos: egui::Pos2) {
        let text_box = TextBox {
            id: self.notes_canvas.next_textbox_id,
            title: format!("Note {}", self.notes_canvas.next_textbox_id),
            position: pos,
            size: egui::vec2(400.0, 250.0), // Fixed default size
            content: String::new(),
            auto_height: false, // Start in manual mode as requested
            min_size: egui::vec2(150.0, 80.0),
        };

        self.notes_canvas.text_boxes.push(text_box);
        self.notes_canvas.next_textbox_id += 1;
    }

    // Todo methods
    fn add_project(&mut self) {
        if !self.new_project_name.trim().is_empty() {
            let project = Project {
                id: self.next_project_id,
                name: self.new_project_name.clone(),
                tasks: Vec::new(),
                expanded: true,
            };
            self.projects.push(project);
            self.next_project_id += 1;
            self.new_project_name.clear();
        }
    }

    fn add_task_to_project(&mut self, project_id: usize, task_text: String) {
        if let Some(project) = self.projects.iter_mut().find(|p| p.id == project_id) {
            if !task_text.trim().is_empty() {
                let task = Task {
                    id: self.next_task_id,
                    text: task_text.trim().to_string(),
                    completed: false,
                };
                project.tasks.push(task);
                self.next_task_id += 1;
            }
        }
    }
}

/// Draws `text` on a single line, elided with an ellipsis when it does not fit
/// the width still available in `ui`.
///
/// `ui.label` / `ui.colored_label` inherit `Ui::wrap_mode()`, which resolves to
/// `TextWrapMode::Extend` inside a non-wrapping `ui.horizontal`, i.e. "lay out on
/// one line and ignore the available width". A long unbroken word therefore runs
/// straight past the surrounding frame. `Label::truncate` opts out of that, and the
/// full string is offered as a tooltip only when it actually had to be shortened.
fn menu_label(icon: &str, label: &str) -> String {
    format!("{icon}   {label}")
}

/// Width of the widest row plus padding, so the popup wraps its content instead
/// of stretching across the screen.
fn menu_width(ui: &egui::Ui, items: &[(&str, &str, bool)]) -> f32 {
    items
        .iter()
        .map(|(icon, label, _)| {
            ui.painter()
                .layout_no_wrap(
                    menu_label(icon, label),
                    egui::FontId::proportional(MENU_FONT_SIZE),
                    egui::Color32::WHITE,
                )
                .size()
                .x
        })
        .fold(0.0_f32, f32::max)
        + MENU_PADDING * 2.0
}

/// One row of the canvas context menu, left-aligned with a hover highlight.
/// `egui::Button` centres its label, which reads wrong in a menu, so the row is
/// painted directly.
fn menu_item(
    ui: &mut egui::Ui,
    width: f32,
    icon: &str,
    label: &str,
    accent: bool,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, MENU_ROW_HEIGHT), egui::Sense::click());

    if ui.is_rect_visible(rect) {
        if response.hovered() {
            ui.painter().rect_filled(rect, 8.0, MENU_HOVER);
        }

        let color = match (accent, response.hovered()) {
            (true, _) => ACCENT_TEXT,
            (false, true) => egui::Color32::from_gray(220),
            (false, false) => egui::Color32::from_gray(160),
        };

        ui.painter().text(
            rect.left_center() + egui::vec2(MENU_PADDING, 0.0),
            egui::Align2::LEFT_CENTER,
            menu_label(icon, label),
            egui::FontId::proportional(MENU_FONT_SIZE),
            color,
        );
    }

    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn truncating_label(ui: &mut egui::Ui, text: &str, size: f32, color: egui::Color32) {
    let available = ui.available_width().max(1.0);
    let height = ui.spacing().interact_size.y.max(size * 1.4);

    let full_width = ui
        .painter()
        .layout_no_wrap(text.to_owned(), egui::FontId::proportional(size), color)
        .size()
        .x;

    // Claim the whole remaining width and lay out left-to-right inside it, so a
    // short label still starts at the left edge instead of being centred.
    let response = ui
        .allocate_ui_with_layout(
            egui::vec2(available, height),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add(
                    egui::Label::new(egui::RichText::new(text).size(size).color(color)).truncate(),
                )
            },
        )
        .inner;

    if full_width > available + 0.5 {
        response.on_hover_text(text);
    }
}

fn main() -> Result<(), eframe::Error> {
    let cli = desktop::parse_args();

    // A second launch is not a second app: it hands its command to the instance
    // that already owns the tray icon and the global hotkey, then exits.
    let listener = match desktop::acquire(cli.command) {
        desktop::Startup::Primary(listener) => listener,
        desktop::Startup::Forwarded => return Ok(()),
    };

    // Setting app_id below moves eframe's storage directory, so bring any state
    // written by an earlier release along with it.
    desktop::migrate_legacy_storage();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            // Also fixes the Wayland app_id / X11 WM_CLASS, which the desktop
            // entry's StartupWMClass and the window rules in the README match on.
            .with_app_id(desktop::APP_ID)
            .with_icon(desktop::window_icon())
            .with_inner_size(DEFAULT_WINDOW_SIZE)
            .with_min_inner_size(MIN_WINDOW_SIZE)
            .with_visible(!cli.background),
        ..Default::default()
    };

    eframe::run_native(
        desktop::APP_NAME,
        options,
        Box::new(move |cc| {
            // Initialize the material icons - this sets up the icon fonts
            egui_material_icons::initialize(&cc.egui_ctx);

            // Don't override fonts at all to preserve material icons
            let mut app = TodoApp::new(cc);
            let visible = !cli.background;
            app.attach_desktop(
                desktop::Desktop::new(&cc.egui_ctx, listener, visible),
                visible,
            );
            Ok(Box::new(app))
        }),
    )
}
