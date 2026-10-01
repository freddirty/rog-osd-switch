//! Linux I²C DDC/CI transport and X11 global shortcuts. No root daemon required.
use rog_osd_switch::{Hotkey, command_for_hotkey};
use std::{
    ffi::{CString, c_char, c_int, c_long, c_uint, c_ulong, c_void},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::fd::AsRawFd,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const I2C_SLAVE: c_ulong = 0x0703;
static X_ERROR: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    fn flock(fd: c_int, operation: c_int) -> c_int;
}

#[repr(C)]
struct KeyEvent {
    kind: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut c_void,
    window: c_ulong,
    root: c_ulong,
    subwindow: c_ulong,
    time: c_ulong,
    x: c_int,
    y: c_int,
    x_root: c_int,
    y_root: c_int,
    state: c_uint,
    keycode: c_uint,
    same_screen: c_int,
}

#[link(name = "libX11.so.6", kind = "dylib", modifiers = "+verbatim")]
unsafe extern "C" {
    fn XOpenDisplay(name: *const c_char) -> *mut c_void;
    fn XCloseDisplay(display: *mut c_void) -> c_int;
    fn XDefaultRootWindow(display: *mut c_void) -> c_ulong;
    fn XStringToKeysym(name: *const c_char) -> c_ulong;
    fn XKeysymToKeycode(display: *mut c_void, keysym: c_ulong) -> u8;
    fn XGrabKey(
        display: *mut c_void,
        key: c_int,
        modifiers: c_uint,
        window: c_ulong,
        owner_events: c_int,
        pointer_mode: c_int,
        keyboard_mode: c_int,
    ) -> c_int;
    fn XSync(display: *mut c_void, discard: c_int) -> c_int;
    fn XNextEvent(display: *mut c_void, event: *mut c_long) -> c_int;
    fn XSetErrorHandler(
        handler: Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int>,
    ) -> Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int>;
    fn XGetModifierMapping(display: *mut c_void) -> *mut ModifierMap;
    fn XFreeModifiermap(map: *mut ModifierMap) -> c_int;
}

#[repr(C)]
struct ModifierMap {
    max_keypermod: c_int,
    modifiermap: *mut u8,
}

unsafe extern "C" fn on_x_error(_: *mut c_void, _: *mut c_void) -> c_int {
    X_ERROR.store(true, Ordering::SeqCst);
    0
}

fn slave(file: &File, address: c_int) -> Result<()> {
    if unsafe { ioctl(file.as_raw_fd(), I2C_SLAVE, address) } < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

fn monitor_name(edid: &[u8]) -> Option<String> {
    if edid.len() != 128
        || edid[..8] != [0, 255, 255, 255, 255, 255, 255, 0]
        || edid.iter().fold(0u8, |s, b| s.wrapping_add(*b)) != 0
    {
        return None;
    }
    edid[54..126].chunks_exact(18).find_map(|d| {
        (d[..5] == [0, 0, 0, 0xfc, 0]).then(|| String::from_utf8_lossy(&d[5..18]).trim().to_owned())
    })
}

fn find_monitor(bus: Option<&str>) -> Result<PathBuf> {
    let mut paths = Vec::new();
    if let Some(bus) = bus {
        paths.push(PathBuf::from(format!("/dev/i2c-{}", bus.parse::<u32>()?)));
    } else {
        for entry in fs::read_dir("/sys/class/i2c-dev")? {
            let path = entry?.path();
            let name = fs::read_to_string(path.join("name"))?;
            // Only display adapters, never arbitrary motherboard/SMBus devices.
            if ["NVIDIA", "AMDGPU", "drm", "DPDDC", "i915", "GMBUS", "gmbus"]
                .iter()
                .any(|part| name.contains(part))
            {
                paths.push(PathBuf::from("/dev").join(path.file_name().unwrap()));
            }
        }
    }
    let mut denied = false;
    for path in paths {
        let mut file = match OpenOptions::new().read(true).write(true).open(&path) {
            Ok(f) => f,
            Err(e) => {
                denied |= e.kind() == std::io::ErrorKind::PermissionDenied;
                continue;
            }
        };
        let mut edid = [0; 128];
        if slave(&file, 0x50).is_ok()
            && file.write_all(&[0]).is_ok()
            && file.read_exact(&mut edid).is_ok()
            && monitor_name(&edid).as_deref() == Some("XG32UCWMG")
        {
            return Ok(path);
        }
    }
    Err(if denied { "Cannot access display I²C devices. Grant this user read/write access to the monitor's /dev/i2c-N device." }
        else { "XG32UCWMG was not found. Check the monitor connection and DDC/CI; try --bus N for a display adapter." }.into())
}

fn packet(payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0x51, 0x80 | payload.len() as u8];
    bytes.extend_from_slice(payload);
    bytes.push(bytes.iter().fold(0x6e, |sum, b| sum ^ b));
    bytes
}

fn open_ddc(path: &PathBuf) -> Result<File> {
    let file = OpenOptions::new().read(true).write(true).open(path)?;
    slave(&file, 0x37)?;
    Ok(file)
}

fn send(path: &PathBuf, hotkey: Hotkey) -> Result<()> {
    let value = command_for_hotkey(hotkey) as u8;
    open_ddc(path)?.write_all(&packet(&[0x03, 0xeb, 0, value]))?;
    thread::sleep(Duration::from_millis(50));
    Ok(())
}

fn parse_reply(reply: &[u8], code: u8) -> Result<u16> {
    if reply.len() != 11
        || reply[0..3] != [0x6e, 0x88, 0x02]
        || reply[3] != 0
        || reply[4] != code
        || reply.iter().fold(0x50, |s, b| s ^ b) != 0
    {
        return Err(format!("Invalid or unsupported DDC/CI VCP reply: {reply:02x?}").into());
    }
    Ok(u16::from_be_bytes([reply[8], reply[9]]))
}

fn probe(path: &PathBuf) -> Result<u16> {
    let mut file = open_ddc(path)?;
    file.write_all(&packet(&[0x01, 0x60]))?;
    thread::sleep(Duration::from_millis(80));
    let mut reply = [0; 11];
    file.read_exact(&mut reply)?;
    parse_reply(&reply, 0x60)
}

fn modifiers(value: &str) -> Result<c_uint> {
    let mut mask = 0;
    for name in value.split('+') {
        mask |= match name.to_ascii_lowercase().as_str() {
            "ctrl" => 4,
            "alt" => 8,
            "shift" => 1,
            "super" => 64,
            _ => {
                return Err(
                    "Modifiers must be a + separated combination of ctrl, alt, shift, super".into(),
                );
            }
        };
    }
    Ok(mask)
}

pub fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut bus = None;
    let mut action = None;
    let mut do_probe = false;
    let mut mask = 68;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bus" => bus = Some(args.next().ok_or("--bus requires a number")?),
            "--modifiers" => mask = modifiers(&args.next().ok_or("--modifiers requires a value")?)?,
            "--probe" => do_probe = true,
            "--send" => {
                action = Some(match args.next().as_deref() {
                    Some("up") => Hotkey::Up,
                    Some("down") => Hotkey::Down,
                    Some("right") => Hotkey::Right,
                    Some("left") => Hotkey::Left,
                    _ => return Err("--send requires up, down, right or left".into()),
                })
            }
            "--help" | "-h" => {
                println!(
                    "ROG OSD Switch (Linux X11)\nDefault: Ctrl+Super+Arrow monitor joystick shortcuts.\nOptions: --probe | --send up|down|right|left | --bus N | --modifiers ctrl+alt\nDaemon: stop with Ctrl+C or SIGTERM. Requires read/write access to display I²C.\nTray/settings: rog-osd-switch-start [--settings]. Wayland global shortcuts are not supported."
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
    }
    if do_probe && action.is_some() {
        return Err("Use --probe or --send, separately".into());
    }
    let path = find_monitor(bus.as_deref())?;
    if do_probe {
        println!(
            "XG32UCWMG on {}: input source 0x{:02X}",
            path.display(),
            probe(&path)?
        );
        return Ok(());
    }
    if let Some(action) = action {
        return send(&path, action);
    }
    if std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("wayland") {
        return Err(
            "Global shortcuts require an X11 session; --probe and --send also work on Wayland."
                .into(),
        );
    }
    let runtime =
        PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR is unset")?);
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(runtime.join("rog-osd-switch.lock"))?;
    if unsafe { flock(lock.as_raw_fd(), 2 | 4) } != 0 {
        return Err("Another instance is already running".into());
    }
    unsafe {
        let display = XOpenDisplay(std::ptr::null());
        if display.is_null() {
            return Err("Cannot connect to the X11 display".into());
        }
        let root = XDefaultRootWindow(display);
        let old_handler = XSetErrorHandler(Some(on_x_error));
        X_ERROR.store(false, Ordering::SeqCst);
        let keys = [
            ("Up", Hotkey::Up),
            ("Down", Hotkey::Down),
            ("Right", Hotkey::Right),
            ("Left", Hotkey::Left),
        ]
        .map(|(name, action)| {
            (
                XKeysymToKeycode(
                    display,
                    XStringToKeysym(CString::new(name).unwrap().as_ptr()),
                ),
                action,
            )
        });
        let num_key = XKeysymToKeycode(display, XStringToKeysym(c"Num_Lock".as_ptr()));
        let map = XGetModifierMapping(display);
        let mut num_mask = 0;
        if !map.is_null() {
            for modifier in 0..8 {
                for slot in 0..(*map).max_keypermod as usize {
                    if num_key != 0
                        && *(*map)
                            .modifiermap
                            .add(modifier * (*map).max_keypermod as usize + slot)
                            == num_key
                    {
                        num_mask |= 1 << modifier;
                    }
                }
            }
            XFreeModifiermap(map);
        }
        let mut lock_masks = vec![0, 2, num_mask, 2 | num_mask];
        lock_masks.sort_unstable();
        lock_masks.dedup();
        for (code, _) in keys {
            if code == 0 {
                X_ERROR.store(true, Ordering::SeqCst);
                continue;
            }
            for locks in &lock_masks {
                XGrabKey(display, code.into(), mask | locks, root, 0, 1, 1);
            }
        }
        XSync(display, 0);
        XSetErrorHandler(old_handler);
        if X_ERROR.load(Ordering::SeqCst) {
            XCloseDisplay(display);
            return Err("Cannot register shortcuts: another app or Cinnamon may own them. Try a different --modifiers combination.".into());
        }
        eprintln!(
            "ROG OSD Switch active on {}; shortcut modifier mask {mask}. Ctrl+C/SIGTERM exits.",
            path.display()
        );
        loop {
            let mut event = [0 as c_long; 24];
            XNextEvent(display, event.as_mut_ptr());
            let key = &*(event.as_ptr() as *const KeyEvent);
            if key.kind == 2 {
                if let Some((_, action)) = keys
                    .iter()
                    .find(|(code, _)| c_uint::from(*code) == key.keycode)
                {
                    match send(&path, *action) {
                        Ok(()) => eprintln!("OSD {action:?}: DDC/CI write succeeded"),
                        Err(error) => eprintln!("DDC/CI command failed: {error}"),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn set_vcp_packet_matches_ddc_wire_format() {
        assert_eq!(packet(&[3, 0xeb, 0, 4]), [0x51, 0x84, 3, 0xeb, 0, 4, 0x57]);
    }
    #[test]
    fn validates_monitor_reply_and_checksum() {
        let reply = [0x6e, 0x88, 2, 0, 0x60, 0, 0, 0x1a, 0, 0x0f, 0xc1];
        assert_eq!(parse_reply(&reply, 0x60).unwrap(), 15);
        let mut corrupt = reply;
        corrupt[9] = 17;
        assert!(parse_reply(&corrupt, 0x60).is_err());
        assert!(parse_reply(&reply, 0xeb).is_err());
        assert!(parse_reply(&reply[..10], 0x60).is_err());
    }
    #[test]
    fn invalid_modifiers_are_rejected() {
        assert_eq!(modifiers("ctrl+alt+shift").unwrap(), 13);
        assert!(modifiers("ctrll+alt").is_err());
    }
}
