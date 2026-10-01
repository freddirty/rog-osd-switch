"""Run with /usr/bin/python3 -m unittest discover -s tests -v.

Set ROG_OSD_INTEGRATION=1 for the actual GTK/backend lifecycle checks; stop the
installed tray first. These tests register shortcuts but do not send OSD commands.
"""
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import time
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("tray", Path(__file__).resolve().parents[1] / "linux/tray.py")
tray = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tray)


class SettingsTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        root = Path(self.directory.name)
        self.override = patch.multiple(tray, CONFIG_FILE=root / "config.json",
                                      AUTOSTART_FILE=root / "autostart/app.desktop")
        self.override.start()

    def tearDown(self):
        self.override.stop()
        self.directory.cleanup()

    def test_persist_modifiers_and_toggle_autostart(self):
        tray.save_settings(["alt", "super"], True)
        self.assertEqual(tray.load_modifiers(), ["alt", "super"])
        self.assertTrue(tray.autostart_enabled())
        tray.save_settings(["ctrl", "super"], False)
        self.assertFalse(tray.autostart_enabled())
        self.assertFalse(tray.AUTOSTART_FILE.exists())

    def test_disabled_desktop_entry_is_respected(self):
        tray.atomic_write(tray.AUTOSTART_FILE, tray.desktop_entry() + "Hidden=true\n")
        self.assertFalse(tray.autostart_enabled())

    def test_bad_config_is_rejected(self):
        for values in ([], ["unknown"], "ctrl+super"):
            tray.CONFIG_FILE.write_text(json.dumps({"modifiers": values}))
            with self.assertRaises(ValueError):
                tray.load_modifiers()

    def test_failed_autostart_write_restores_previous_config(self):
        tray.save_settings(["ctrl", "super"], False)
        real_write = tray.atomic_write
        def fail_autostart(path, contents):
            if path == tray.AUTOSTART_FILE:
                raise PermissionError("test write failure")
            real_write(path, contents)
        with patch.object(tray, "atomic_write", side_effect=fail_autostart):
            with self.assertRaises(PermissionError):
                tray.save_settings(["alt", "super"], True)
        self.assertEqual(tray.load_modifiers(), ["ctrl", "super"])
        self.assertFalse(tray.autostart_enabled())

    def test_cinnamon_shortcut_conflicts(self):
        if not tray.Gtk.init_check()[0]:
            self.skipTest("X11 display unavailable")
        source = tray.Gio.SettingsSchemaSource.get_default()
        if source.lookup("org.cinnamon.desktop.keybindings.wm", True) is None:
            self.skipTest("Cinnamon schemas unavailable")
        settings = tray.Gio.Settings.new("org.cinnamon.desktop.keybindings.wm")
        for values, key in ((["ctrl", "alt"], "switch-to-workspace-left"),
                            (["ctrl", "alt", "shift"], "move-to-workspace-left")):
            if settings.get_strv(key):
                self.assertTrue(tray.shortcut_conflicts(values))
        self.assertFalse(tray.shortcut_conflicts(["ctrl", "super"]))


@unittest.skipUnless(os.environ.get("ROG_OSD_INTEGRATION") == "1", "opt-in hardware/GTK lifecycle test")
class LifecycleTests(unittest.TestCase):
    setUp = SettingsTests.setUp
    tearDown = SettingsTests.tearDown

    def test_apply_conflict_rollback_and_exit(self):
        self.app = tray.TrayApp()
        self.app.set_application_id(tray.APP_ID + ".Test")
        self.app.register(None)
        self.app.values = ["ctrl", "super"]
        self.app.make_tray()
        try:
            self.app.start_worker(self.app.values)
            self.wait_for(lambda: self.app.ready)
            self.app.show_settings()
            old_pid = self.app.worker.pid
            self.select(["ctrl", "alt"])
            self.app.apply_settings()
            self.assertIn("Cinnamon", self.app.message_label.get_text())
            self.assertEqual(self.app.worker.pid, old_pid)
            self.select([])
            self.app.apply_settings()
            self.assertIn("legalább", self.app.message_label.get_text())
            self.select(["alt", "super"])
            self.app.autostart_check.set_active(False)
            self.app.apply_settings()
            self.wait_for(lambda: self.app.ready and self.app.pending is None)
            self.assertEqual(tray.load_modifiers(), ["alt", "super"])
            self.assertFalse(tray.autostart_enabled())
            self.select(["ctrl", "super"])
            self.app.autostart_check.set_active(True)
            self.app.apply_settings()
            self.wait_for(lambda: self.app.ready and self.app.pending is None)
            self.assertEqual(tray.load_modifiers(), ["ctrl", "super"])
            self.assertTrue(tray.autostart_enabled())
            self.app.hide_settings()
            self.assertIsNone(self.app.window)
            self.assertIsNone(self.app.worker.poll())
            # A failing backend must not save new shortcuts, and must restore
            # the previous worker with the same settings.
            self.app.show_settings()
            self.select(["alt", "super"])
            actual_start = self.app.start_worker
            def fail_once(values):
                self.app.start_worker = actual_start
                self.app.worker_failed("test registration failure")
            self.app.start_worker = fail_once
            self.app.apply_settings()
            self.wait_for(lambda: self.app.ready and self.app.pending is None)
            self.assertEqual(tray.load_modifiers(), ["ctrl", "super"])
            self.assertIn("visszaállítottam", self.app.message_label.get_text())
            worker = self.app.worker
            self.app.exit_app()
            self.assertIsNotNone(worker.poll())
        finally:
            self.app.stop_worker()
            if self.app.window:
                self.app.window.destroy()
            self.app.quit()

    def select(self, values):
        for name, checkbox in self.app.checks.items():
            checkbox.set_active(name in values)

    def wait_for(self, condition):
        deadline = time.monotonic() + 5
        context = tray.GLib.MainContext.default()
        while time.monotonic() < deadline:
            while context.pending():
                context.iteration(False)
            if condition():
                return
            time.sleep(.02)
        self.fail("Backend did not reach the expected state")
