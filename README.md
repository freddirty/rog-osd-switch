# ROG OSD Switch

A tiny, open-source background utility that controls the **native monitor OSD** of the ASUS ROG Strix OLED XG32UCWMG. It replaces the unreliable EZ OSD hotkey handling in ASUS DisplayWidget Center for the input-switching workflow.

The application draws no overlay of its own. It sends the ASUS vendor DDC/CI command `0xEB` to the monitor, so the OSD remains visible even while another computer's HDMI input is on screen.

## Windows app

The first MVP targets Windows 10/11 and the XG32UCWMG.

| Shortcut | Action |
| --- | --- |
| `Ctrl+Alt+Right` | Send joystick Right: open Input Select when the OSD is closed, or confirm when it is open |
| `Ctrl+Alt+Up` | Send joystick Up |
| `Ctrl+Alt+Down` | Send joystick Down |
| `Ctrl+Alt+Left` | Send joystick Left: cancel Input Select or navigate left in another OSD |

Shortcuts are registered through the native Windows hotkey API. By default, `Ctrl+Win+Arrow` is never registered and is ignored by this application.

The application runs in the Windows notification area (system tray). Double-click its icon to open Settings, right-click it for the menu, or start the executable again to bring the existing instance's Settings window forward. Settings lets you:

- choose any combination of `Ctrl`, `Alt`, `Shift`, and `Win` as the common shortcut modifier;
- turn per-user **Start with Windows** on or off;
- exit the background application from the tray menu.

The four arrow keys remain fixed because they directly mirror the monitor joystick. Configuration is stored in `%LOCALAPPDATA%\RogOsdSwitch\config.ini`. Autostart uses the current user's standard `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` entry and does not require administrator rights.

## Run

1. Exit ASUS DisplayWidget Center so both programs do not control the OSD at once.
2. Start `rog-osd-switch.exe`. Its icon appears in the notification area.
3. Use the shortcuts above. Double-click the tray icon to configure the app; right-click it to open the menu or exit.

Run `rog-osd-switch.exe --probe` to display the detected current input source.

The diagnostic log is written to `%LOCALAPPDATA%\RogOsdSwitch\rog-osd-switch.log`.

## Linux (X11)

Linux has a native Rust DDC/CI/X11 backend and a GTK tray app. The tray icon's
menu opens **Settings**, rescans the monitor, or exits the app. Settings provides
the same controls as Windows: Ctrl, Alt, Shift, and Win/Super modifier checkboxes,
and per-user login autostart. Changes take effect immediately; the settings
window can be closed while the app remains in the tray. Launching the app again
opens the existing instance's settings instead of starting a second backend.

Default shortcuts are **Ctrl+Super+Arrow** (Super is the Windows key). Cinnamon
uses Ctrl+Alt+Arrow and Ctrl+Alt+Shift+Arrow for workspace actions, so the settings
window checks Cinnamon's built-in and custom shortcuts before saving. If native
shortcut registration or saving fails, the previous settings are restored.
Right opens Input Select or confirms; Up/Down navigate; Left cancels. The arrow
keys remain fixed. Caps Lock and Num Lock are supported.

Requirements: Rust 1.85+ and a C linker to build; libX11, Python 3, PyGObject,
GTK 3, an X11 session, and read/write access to the monitor's I²C device to run.
Ayatana AppIndicator is preferred for the tray, with a GTK status icon fallback.
Linux Mint 22.3 Cinnamon already had all runtime dependencies and user I²C ACLs.
On Debian/Ubuntu, the GUI packages are `python3-gi`, `gir1.2-gtk-3.0`, and
`gir1.2-ayatanaappindicator3-0.1`.

```sh
cargo test
cargo build --release
./target/release/rog-osd-switch --probe
./scripts/install-linux.sh
~/.local/bin/rog-osd-switch-start
# Open settings, including when an instance is already running:
~/.local/bin/rog-osd-switch-start --settings
```

GitHub Actions also produces a `rog-osd-switch-linux-x64` artifact containing
the compiled backend, tray app, and installer. Extract its tarball and run
`./scripts/install-linux.sh`; Rust is only needed when building from source.

The installer installs for the current user, enables login autostart on the first
installation, and preserves saved shortcuts and the autostart choice on upgrades.
The application menu entry opens settings. Exit the tray app before upgrading.
No administrator rights are needed when the display I²C device is accessible.
If I²C devices are missing or denied, an administrator must enable `i2c-dev` and
provide access to the display adapter's device. Do not run the app as root.

Configuration is stored in
`${XDG_CONFIG_HOME:-~/.config}/rog-osd-switch/config.json`; autostart uses the
standard `autostart/rog-osd-switch.desktop` file under the same config directory.
The rotating diagnostic log is
`${XDG_STATE_HOME:-~/.local/state}/rog-osd-switch/rog-osd-switch.log`.

The Rust backend discovers XG32UCWMG by reading and validating display adapter
EDIDs. It does not send ASUS commands to other models or scan motherboard SMBus
adapters. For diagnostics, the backend can be used independently:

```sh
~/.local/bin/rog-osd-switch --probe
~/.local/bin/rog-osd-switch --bus 9 --probe
~/.local/bin/rog-osd-switch --send right
~/.local/bin/rog-osd-switch --send left
# Bare backend without the tray, after exiting the tray app:
~/.local/bin/rog-osd-switch --modifiers ctrl+super
```

`--probe` and `--send` also work on Wayland; global shortcuts currently require
X11. DDC write success confirms transport completion, not the monitor's visual
response. Use **Rescan monitor** after reconnecting the display.

GUI/settings tests:

```sh
/usr/bin/python3 -m unittest discover -s tests -v
# Optional actual GTK/backend lifecycle test: exit the tray app first.
ROG_OSD_INTEGRATION=1 /usr/bin/python3 -m unittest discover -s tests -v
```

To uninstall, exit the tray app, disable autostart in Settings (or remove its
autostart desktop file), and remove `~/.local/bin/rog-osd-switch`,
`~/.local/bin/rog-osd-switch-start`, `~/.local/lib/rog-osd-switch`,
`~/.local/share/applications/rog-osd-switch.desktop`, and
`~/.local/share/icons/hicolor/scalable/apps/rog-osd-switch.svg`. Saved configuration
and logs can be retained for reinstallation.

## Protocol notes

DisplayWidget Center 1.5.1.3 uses the standard Windows `Dxva2.dll` monitor API. Its native OSD command is VCP `0xEB`:

| Value | Operation |
| ---: | --- |
| `0` | Close |
| `1` | Show |
| `2` | Up |
| `3` | Down |
| `4` | Right |
| `5` | Left |
| `6` | Enter |
| `7` | Back |

The application does not send `Show` and does not track whether a menu is open. It maps every shortcut directly to a virtual joystick direction. With the OSD closed, the XG32UCWMG firmware interprets `Right` as its dedicated Input Select shortcut; with an OSD open, the same commands navigate that OSD.

On the tested XG32UCWMG, DDC/CI remains reachable from the inactive DisplayPort connection while HDMI 1 is displayed.

## Build

```powershell
cargo test
cargo build --release
```

The release executable is created at `target\release\rog-osd-switch.exe`.

## Roadmap

- XDG Global Shortcuts support for Wayland
- Configurable monitor matching and shortcuts

## License

MIT
