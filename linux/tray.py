#!/usr/bin/python3
"""GTK tray/settings shell for the native Rust X11/DDC backend."""
import json
import logging
from logging.handlers import RotatingFileHandler
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

import gi

gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
from gi.repository import Gdk, Gio, GLib, Gtk

try:
    gi.require_version("AyatanaAppIndicator3", "0.1")
    from gi.repository import AyatanaAppIndicator3 as AppIndicator
except (ValueError, ImportError):
    AppIndicator = None

APP_ID = "io.github.freddirty.RogOsdSwitch"
MODIFIERS = ("ctrl", "alt", "shift", "super")
DEFAULT_MODIFIERS = ["ctrl", "super"]
LABELS = {"ctrl": "Ctrl", "alt": "Alt", "shift": "Shift", "super": "Win / Super"}
CONFIG_HOME = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config"))
CONFIG_FILE = CONFIG_HOME / "rog-osd-switch" / "config.json"
AUTOSTART_FILE = CONFIG_HOME / "autostart" / "rog-osd-switch.desktop"
INSTALL_DIR = Path(__file__).resolve().parent
ICON = str(INSTALL_DIR / "icons" / "rog-osd-switch.svg")
BACKEND = Path(os.environ.get("ROG_OSD_BACKEND", Path.home() / ".local/bin/rog-osd-switch"))
LAUNCHER = Path.home() / ".local/bin/rog-osd-switch-start"
LOG = logging.getLogger("rog-osd-switch")


def atomic_write(path, contents):
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix=".rog-osd-", dir=path.parent)
    try:
        with os.fdopen(fd, "w") as stream:
            stream.write(contents)
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def load_modifiers():
    if not CONFIG_FILE.exists():
        return DEFAULT_MODIFIERS.copy()
    values = json.loads(CONFIG_FILE.read_text())["modifiers"]
    if not isinstance(values, list) or not values or any(v not in MODIFIERS for v in values):
        raise ValueError("Érvénytelen gyorsbillentyű-beállítás.")
    return [name for name in MODIFIERS if name in values]


def autostart_enabled():
    if not AUTOSTART_FILE.exists():
        return False
    desktop = GLib.KeyFile()
    try:
        desktop.load_from_file(str(AUTOSTART_FILE), GLib.KeyFileFlags.NONE)
        keys = desktop.get_keys("Desktop Entry")[0]
        for key in ("Hidden",):
            if key in keys and desktop.get_boolean("Desktop Entry", key):
                return False
        key = "X-GNOME-Autostart-enabled"
        return key not in keys or desktop.get_boolean("Desktop Entry", key)
    except GLib.Error:
        return False


def desktop_entry():
    # Quote according to the Desktop Entry Exec grammar (not shell quoting).
    executable = str(LAUNCHER).replace("\\", "\\\\").replace('"', '\\"').replace("`", "\\`").replace("$", "\\$").replace("%", "%%")
    return ("[Desktop Entry]\nType=Application\nName=ROG OSD Switch\n"
            f'Exec="{executable}"\nIcon=rog-osd-switch\nTerminal=false\n'
            "X-GNOME-Autostart-enabled=true\nX-GNOME-Autostart-Delay=8\n")


def save_settings(values, autostart):
    """Roll back both files if either write fails."""
    old = {p: p.read_bytes() if p.exists() else None for p in (CONFIG_FILE, AUTOSTART_FILE)}
    try:
        atomic_write(CONFIG_FILE, json.dumps({"modifiers": values}, indent=2) + "\n")
        if autostart:
            atomic_write(AUTOSTART_FILE, desktop_entry())
        else:
            AUTOSTART_FILE.unlink(missing_ok=True)
    except OSError:
        for path, content in old.items():
            if content is None:
                path.unlink(missing_ok=True)
            else:
                atomic_write(path, content.decode())
        raise


def shortcut_conflicts(values):
    """Cinnamon owns XI2 grabs that a core XGrabKey cannot reliably detect."""
    requested = sum({"ctrl": int(Gdk.ModifierType.CONTROL_MASK),
                     "alt": int(Gdk.ModifierType.MOD1_MASK),
                     "shift": int(Gdk.ModifierType.SHIFT_MASK),
                     "super": int(Gdk.ModifierType.SUPER_MASK)}[v] for v in values)
    arrows = {Gdk.KEY_Up, Gdk.KEY_Down, Gdk.KEY_Left, Gdk.KEY_Right}
    source = Gio.SettingsSchemaSource.get_default()
    if source is None:
        return []
    schemas, _ = source.list_schemas(True)
    settings_list = []
    for name in schemas:
        if name.startswith("org.cinnamon.desktop.keybindings"):
            schema = source.lookup(name, True)
            if schema and schema.get_path() is not None:
                settings_list.append(Gio.Settings.new_full(schema, None, None))
    custom_schema = source.lookup("org.cinnamon.desktop.keybindings.custom-keybinding", True)
    base_schema = source.lookup("org.cinnamon.desktop.keybindings", True)
    if custom_schema and base_schema:
        base = Gio.Settings.new_full(base_schema, None, None)
        for name in base.get_strv("custom-list"):
            if name != "__dummy__" and "/" not in name:
                settings_list.append(Gio.Settings.new_full(custom_schema, None,
                    f"/org/cinnamon/desktop/keybindings/custom-keybindings/{name}/"))
    conflicts = []
    for settings in settings_list:
        for key in settings.props.settings_schema.list_keys():
            value = settings.get_value(key)
            if value.get_type_string() != "as":
                continue
            for shortcut in value.unpack():
                keyval, modifiers = Gtk.accelerator_parse(shortcut)
                if keyval in arrows and int(modifiers) == requested:
                    conflicts.append(shortcut)
    return sorted(set(conflicts))


class TrayApp(Gtk.Application):
    def __init__(self):
        super().__init__(application_id=APP_ID, flags=Gio.ApplicationFlags.HANDLES_COMMAND_LINE)
        self.worker = None
        self.started = False
        self.window = None
        self.status = "Indítás…"
        self.watchdog = None
        self.pipe_buffer = b""
        self.ready = False
        self.last_error = ""
        self.pending = None
        self.stopping = False

    def do_startup(self):
        Gtk.Application.do_startup(self)
        self.hold()
        self.set_property("register-session", True)
        for name, callback in (("settings", self.show_settings), ("quit", self.exit_app)):
            action = Gio.SimpleAction.new(name, None)
            action.connect("activate", lambda _a, _p, cb=callback: cb())
            self.add_action(action)
        GLib.unix_signal_add(GLib.PRIORITY_DEFAULT, signal.SIGTERM, self.exit_app)
        GLib.unix_signal_add(GLib.PRIORITY_DEFAULT, signal.SIGINT, self.exit_app)

    def do_command_line(self, command_line):
        arguments = command_line.get_arguments()[1:]
        if any(arg not in ("--settings",) for arg in arguments):
            command_line.printerr("Használat: rog-osd-switch-start [--settings]\n")
            if not self.started:
                self.quit()
            return 1
        was_started = self.started
        if not self.started:
            self.started = True
            self.make_tray()
            try:
                self.values = load_modifiers()
            except (OSError, ValueError, KeyError, TypeError) as error:
                LOG.exception("Cannot load settings")
                self.values = DEFAULT_MODIFIERS.copy()
                self.show_settings()
                self.message(f"A mentett beállítás nem olvasható: {error}")
            conflicts = shortcut_conflicts(self.values)
            if conflicts:
                self.set_status("Gyorsbillentyű-ütközés")
                self.show_settings()
                self.message("A Cinnamon már használja ezt: " + ", ".join(conflicts))
            else:
                self.start_worker(self.values)
        if was_started or "--settings" in arguments:
            self.show_settings()
        return 0

    def make_tray(self):
        menu = Gtk.Menu()
        self.status_item = Gtk.MenuItem(label=self.status)
        self.status_item.set_sensitive(False)
        menu.append(self.status_item)
        menu.append(Gtk.SeparatorMenuItem())
        for label, callback in (("Beállítások…", self.show_settings),
                                ("Monitor újrakeresése", self.restart_worker),
                                ("Kilépés", self.exit_app)):
            item = Gtk.MenuItem(label=label)
            item.connect("activate", lambda _item, cb=callback: cb())
            menu.append(item)
            if label == "Beállítások…":
                settings_item = item
        menu.show_all()
        self.menu = menu
        if AppIndicator is not None:
            self.indicator = AppIndicator.Indicator.new("rog-osd-switch", ICON,
                AppIndicator.IndicatorCategory.HARDWARE)
            self.indicator.set_title("ROG OSD Switch")
            self.indicator.set_menu(menu)
            self.indicator.set_secondary_activate_target(settings_item)
            self.indicator.set_status(AppIndicator.IndicatorStatus.ACTIVE)
        else:
            self.indicator = Gtk.StatusIcon.new_from_file(ICON)
            self.indicator.set_tooltip_text("ROG OSD Switch")
            self.indicator.connect("activate", lambda _icon: self.show_settings())
            self.indicator.connect("popup-menu", lambda icon, button, timestamp:
                self.menu.popup(None, None, Gtk.StatusIcon.position_menu, icon, button, timestamp))
            self.indicator.set_visible(True)

    def set_status(self, text):
        self.status = text
        self.status_item.set_label(text)
        if self.window:
            self.status_label.set_text(text)

    def start_worker(self, values):
        self.last_error = ""
        self.pipe_buffer = b""
        self.ready = False
        self.worker_started = time.monotonic()
        self.set_status("Monitor keresése…")
        try:
            self.worker = subprocess.Popen([str(BACKEND), "--modifiers", "+".join(values)],
                stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
            os.set_blocking(self.worker.stdout.fileno(), False)
            self.watchdog = GLib.timeout_add(100, self.check_worker, self.worker)
        except OSError as error:
            self.worker_failed(str(error))

    def check_worker(self, worker):
        if self.worker is not worker:
            return GLib.SOURCE_REMOVE
        while True:
            try:
                chunk = os.read(worker.stdout.fileno(), 65536)
            except BlockingIOError:
                break
            if not chunk:
                break
            self.pipe_buffer += chunk
        while b"\n" in self.pipe_buffer:
            line, self.pipe_buffer = self.pipe_buffer.split(b"\n", 1)
            text = line.decode("utf-8", "replace")
            LOG.info(text)
            if "ROG OSD Switch active on " in text:
                self.ready = True
                self.worker_ready()
            elif "ROG OSD Switch:" in text or "failed:" in text:
                self.last_error = text
                if "DDC/CI command failed:" in text:
                    self.set_status("Monitorvezérlési hiba – újrakeresés szükséges")
            if self.worker is not worker:
                return GLib.SOURCE_REMOVE
        if worker.poll() is None:
            if not self.ready and time.monotonic() - self.worker_started > 10:
                self.stop_worker()
                self.worker_failed("A monitor keresése túllépte a várakozási időt.")
                return GLib.SOURCE_REMOVE
            return GLib.SOURCE_CONTINUE
        self.watchdog = None
        worker.stdout.close()
        self.worker = None
        self.worker_failed(self.last_error or "A monitorvezérlés leállt.")
        return GLib.SOURCE_REMOVE

    def stop_worker(self):
        if self.watchdog is not None:
            GLib.source_remove(self.watchdog)
            self.watchdog = None
        if self.worker is not None:
            if self.worker.poll() is None:
                self.worker.terminate()
                try:
                    self.worker.wait(timeout=1)
                except subprocess.TimeoutExpired:
                    self.worker.kill()
                    self.worker.wait()
            self.worker.stdout.close()
            self.worker = None

    def worker_ready(self):
        if self.pending is not None:
            values, autostart = self.pending
            self.pending = None
            try:
                save_settings(values, autostart)
            except OSError as error:
                self.stop_worker()
                self.start_worker(self.values)
                self.apply_button.set_sensitive(True)
                self.message(f"Nem sikerült menteni; a korábbi beállítás maradt érvényben.\n{error}")
                return
            self.values = values
            self.apply_button.set_sensitive(True)
            self.message("Beállítások mentve.", error=False)
        shortcut = "+".join(LABELS[name].replace("Win / Super", "Win") for name in self.values)
        self.set_status(f"Aktív: {shortcut} + nyilak")

    def worker_failed(self, error):
        LOG.error(error)
        self.set_status("A monitorvezérlés nem aktív")
        if self.pending is not None:
            self.pending = None
            self.start_worker(self.values)
            self.apply_button.set_sensitive(True)
            self.message(f"A változtatás nem alkalmazható; visszaállítottam a korábbi gyorsbillentyűket.\n{error}")
        else:
            self.show_settings()
            self.message(error)

    def restart_worker(self):
        if self.pending is None:
            self.stop_worker()
            self.start_worker(self.values)

    def show_settings(self):
        if self.window is not None:
            self.window.present()
            return
        window = Gtk.ApplicationWindow(application=self, title="ROG OSD Switch – Beállítások")
        window.set_icon_from_file(ICON)
        window.set_default_size(490, 360)
        window.set_resizable(False)
        window.set_border_width(22)
        window.connect("delete-event", self.hide_settings)
        self.window = window
        layout = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=16)
        window.add(layout)
        title = Gtk.Label(xalign=0)
        title.set_markup('<b>ASUS ROG Strix OLED XG32UCWMG</b>')
        layout.pack_start(title, False, False, 0)
        self.status_label = Gtk.Label(label=self.status, xalign=0)
        layout.pack_start(self.status_label, False, False, 0)
        layout.pack_start(Gtk.Separator(), False, False, 0)
        label = Gtk.Label(label="Gyorsbillentyűk módosítóbillentyűi:", xalign=0)
        layout.pack_start(label, False, False, 0)
        row = Gtk.Box(spacing=22)
        self.checks = {}
        for name in MODIFIERS:
            check = Gtk.CheckButton(label=LABELS[name])
            check.set_active(name in self.values)
            row.pack_start(check, False, False, 0)
            self.checks[name] = check
        layout.pack_start(row, False, False, 0)
        help_text = Gtk.Label(label="Jobb: bemenetválasztó / jóváhagyás • Fel/Le: navigálás\n"
                                   "Bal: vissza / bezárás. A nyílbillentyűk nem módosíthatók.", xalign=0)
        help_text.set_line_wrap(True)
        layout.pack_start(help_text, False, False, 0)
        self.autostart_check = Gtk.CheckButton(label="Automatikus indítás bejelentkezéskor")
        self.autostart_check.set_active(autostart_enabled())
        layout.pack_start(self.autostart_check, False, False, 0)
        self.message_label = Gtk.Label(xalign=0)
        self.message_label.set_line_wrap(True)
        self.message_label.set_max_width_chars(64)
        self.message_label.set_selectable(True)
        layout.pack_start(self.message_label, False, False, 0)
        buttons = Gtk.Box(spacing=10)
        close = Gtk.Button(label="Bezárás")
        close.connect("clicked", self.hide_settings)
        self.apply_button = Gtk.Button(label="Mentés")
        self.apply_button.get_style_context().add_class("suggested-action")
        self.apply_button.connect("clicked", self.apply_settings)
        buttons.pack_end(self.apply_button, False, False, 0)
        buttons.pack_end(close, False, False, 0)
        layout.pack_end(buttons, False, False, 0)
        window.show_all()
        window.present()

    def hide_settings(self, *_):
        if self.pending is not None:
            return True
        self.window.destroy()
        self.window = None
        return True

    def message(self, text, error=True):
        self.show_settings()
        self.message_label.set_text(text)
        style = self.message_label.get_style_context()
        style.remove_class("error")
        if error:
            style.add_class("error")

    def apply_settings(self, *_):
        if self.pending is not None:
            return
        values = [name for name in MODIFIERS if self.checks[name].get_active()]
        if not values:
            self.message("Válassz legalább egy módosítóbillentyűt.")
            return
        conflicts = shortcut_conflicts(values)
        if conflicts:
            self.message("Ezt a kombinációt a Cinnamon már használja:\n" + ", ".join(conflicts)
                         + "\nVálassz másikat; a jelenlegi gyorsbillentyű továbbra is aktív.")
            return
        autostart = self.autostart_check.get_active()
        if values == self.values and self.ready and self.worker and self.worker.poll() is None:
            try:
                save_settings(values, autostart)
                self.message("Beállítások mentve.", error=False)
            except OSError as error:
                self.message(f"Nem sikerült menteni: {error}")
            return
        self.pending = (values, autostart)
        self.apply_button.set_sensitive(False)
        self.message_label.set_text("Gyorsbillentyűk alkalmazása…")
        self.stop_worker()
        self.start_worker(values)

    def exit_app(self, *_):
        if not self.stopping:
            self.stopping = True
            self.stop_worker()
            self.quit()
        return GLib.SOURCE_REMOVE

    def do_shutdown(self):
        self.stop_worker()
        Gtk.Application.do_shutdown(self)


def main():
    state = Path(os.environ.get("XDG_STATE_HOME", Path.home() / ".local/state")) / "rog-osd-switch"
    state.mkdir(parents=True, exist_ok=True)
    handler = RotatingFileHandler(state / "rog-osd-switch.log", maxBytes=512_000, backupCount=2)
    handler.setFormatter(logging.Formatter("%(asctime)s %(levelname)s %(message)s"))
    LOG.addHandler(handler)
    LOG.setLevel(logging.INFO)
    return TrayApp().run(sys.argv)


if __name__ == "__main__":
    sys.exit(main())
