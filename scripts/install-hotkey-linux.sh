#!/usr/bin/env bash
#
# Registers Ctrl+Shift+: as a desktop-environment shortcut that runs
# `todo_app --toggle`.
#
# The app grabs the hotkey itself on X11, but Wayland compositors do not let
# applications grab keys - the compositor owns them. So on Wayland the shortcut
# has to be registered with the desktop environment, which then runs
# `todo_app --toggle`; that hands the toggle to the instance already sitting in
# the tray rather than starting a second copy.
#
# Usage: scripts/install-hotkey-linux.sh
set -euo pipefail

COMMAND="todo_app --toggle"
MARKER="# Todo App global hotkey (installed by install-hotkey-linux.sh)"

say() { printf '%s\n' "$*"; }
warn() { printf '%s\n' "$*" >&2; }

if ! command -v todo_app >/dev/null 2>&1; then
    warn "todo_app is not on your PATH. Install the package first, or adjust"
    warn "COMMAND in this script to the full path of the binary."
    exit 1
fi

# ---------------------------------------------------------------------------

install_gnome() {
    local schema=org.gnome.settings-daemon.plugins.media-keys
    local path=/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/todo-app/
    local existing

    existing=$(gsettings get "$schema" custom-keybindings)

    if [[ "$existing" != *"$path"* ]]; then
        if [[ "$existing" == "@as []" || "$existing" == "[]" ]]; then
            gsettings set "$schema" custom-keybindings "['$path']"
        else
            gsettings set "$schema" custom-keybindings \
                "${existing%]*}, '$path']"
        fi
    fi

    gsettings set "$schema.custom-keybinding:$path" name 'Todo App'
    gsettings set "$schema.custom-keybinding:$path" command "$COMMAND"
    gsettings set "$schema.custom-keybinding:$path" binding '<Control><Shift>semicolon'

    say "GNOME: bound <Control><Shift>semicolon to '$COMMAND'."
}

append_once() {
    local file=$1 line=$2
    mkdir -p "$(dirname "$file")"
    touch "$file"

    if grep -qF "$MARKER" "$file"; then
        say "Already present in $file - nothing to do."
        return
    fi

    printf '\n%s\n%s\n' "$MARKER" "$line" >>"$file"
    say "Appended the binding to $file."
}

# ---------------------------------------------------------------------------

desktop=${XDG_CURRENT_DESKTOP:-}
say "Session: ${XDG_SESSION_TYPE:-unknown} / ${desktop:-unknown}"

case "${desktop,,}" in
*gnome*)
    install_gnome
    ;;
*hyprland*)
    append_once "${HOME}/.config/hypr/hyprland.conf" \
        "bind = CTRL SHIFT, semicolon, exec, $COMMAND"
    say "Reload Hyprland (hyprctl reload) to pick it up."
    ;;
*sway*)
    append_once "${HOME}/.config/sway/config" \
        "bindsym Ctrl+Shift+semicolon exec $COMMAND"
    say "Reload sway (swaymsg reload) to pick it up."
    ;;
*)
    warn "No automatic support for '${desktop:-unknown}'."
    warn
    warn "Add a custom keyboard shortcut in your desktop environment's settings:"
    warn "    Shortcut: Ctrl+Shift+:"
    warn "    Command:  $COMMAND"
    warn
    warn "On KDE Plasma this is System Settings -> Keyboard -> Shortcuts ->"
    warn "Add New -> Command or Script."
    exit 1
    ;;
esac
