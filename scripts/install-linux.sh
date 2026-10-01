#!/bin/sh
# Install the Linux tray app for the current user, without sudo.
set -eu
project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
binary="$project_dir/target/release/rog-osd-switch"
if [ ! -x "$binary" ]; then
    echo 'Build first: cargo build --release' >&2
    exit 1
fi
/usr/bin/python3 -c 'import gi; gi.require_version("Gtk", "3.0"); from gi.repository import Gtk' || {
    echo 'The tray requires python3-gi and gir1.2-gtk-3.0 (optional: gir1.2-ayatanaappindicator3-0.1).' >&2
    exit 1
}
config_dir="${XDG_CONFIG_HOME:-$HOME/.config}"
app_dir="$HOME/.local/lib/rog-osd-switch"
mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications" "$config_dir/autostart" "$app_dir/icons" "$HOME/.local/share/icons/hicolor/scalable/apps"
install -m 755 "$binary" "$HOME/.local/bin/rog-osd-switch"
install -m 644 "$project_dir/linux/tray.py" "$app_dir/tray.py"
install -m 644 "$project_dir/linux/icons/rog-osd-switch.svg" "$app_dir/icons/rog-osd-switch.svg"
install -m 644 "$project_dir/linux/icons/rog-osd-switch.svg" "$HOME/.local/share/icons/hicolor/scalable/apps/rog-osd-switch.svg"
cat > "$HOME/.local/bin/rog-osd-switch-start" <<'WRAPPER'
#!/bin/sh
set -eu
exec /usr/bin/python3 "$HOME/.local/lib/rog-osd-switch/tray.py" "$@"
WRAPPER
chmod 755 "$HOME/.local/bin/rog-osd-switch-start"
cat > "$HOME/.local/share/applications/rog-osd-switch.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=ROG OSD Switch
Comment=ASUS monitor OSD shortcuts and settings
Exec="$HOME/.local/bin/rog-osd-switch-start" --settings
Icon=rog-osd-switch
Terminal=false
Categories=Utility;
StartupNotify=false
DESKTOP
# Preserve existing shortcut settings and the user's autostart choice on upgrades.
/usr/bin/python3 - "$app_dir/tray.py" <<'PY'
import importlib.util
import sys
spec = importlib.util.spec_from_file_location("rog_tray", sys.argv[1])
tray = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tray)
first_install = not tray.CONFIG_FILE.exists()
enabled = tray.autostart_enabled() or (first_install and not tray.AUTOSTART_FILE.exists())
tray.save_settings(tray.load_modifiers(), enabled)
PY
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" >/dev/null 2>&1 || true
fi
echo "Installed tray, settings, and backend in $HOME/.local."
echo 'Start: ~/.local/bin/rog-osd-switch-start'
echo 'Settings: ~/.local/bin/rog-osd-switch-start --settings'
