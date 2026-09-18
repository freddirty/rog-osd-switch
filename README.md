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

- Linux DDC/CI backend
- XDG Global Shortcuts support for Wayland, with X11 fallback
- Configurable monitor matching and shortcuts

## License

MIT
