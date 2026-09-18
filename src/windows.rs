use rog_osd_switch::{Hotkey, OsdCommand, command_for_hotkey};
use std::env;
use std::ffi::{OsStr, c_void};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::mem;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::ptr;
use std::time::{SystemTime, UNIX_EPOCH};

type Bool = i32;
type Handle = *mut c_void;
type Hbrush = *mut c_void;
type Hcursor = *mut c_void;
type Hdc = *mut c_void;
type Hicon = *mut c_void;
type Hinstance = *mut c_void;
type Hkey = isize;
type Hmenu = *mut c_void;
type Hmonitor = *mut c_void;
type Hwnd = *mut c_void;

const APP_NAME: &str = "ROG OSD Switch";
const MAIN_WINDOW_CLASS: &str = "RogOsdSwitch.MainWindow";
const SETTINGS_WINDOW_CLASS: &str = "RogOsdSwitch.SettingsWindow";
const MODEL_FILTER: &str = "XG32UCWMG";
const VCP_INPUT_SOURCE: u8 = 0x60;
const VCP_ASUS_EZ_OSD: u8 = 0xEB;

const WM_DESTROY: u32 = 0x0002;
const WM_CLOSE: u32 = 0x0010;
const WM_SETFONT: u32 = 0x0030;
const WM_COMMAND: u32 = 0x0111;
const WM_CONTEXTMENU: u32 = 0x007B;
const WM_LBUTTONDBLCLK: u32 = 0x0203;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_HOTKEY: u32 = 0x0312;
const WM_APP: u32 = 0x8000;
const WM_TRAY_ICON: u32 = WM_APP + 1;

const MOD_ALT: u32 = 0x0001;
const MOD_CONTROL: u32 = 0x0002;
const MOD_SHIFT: u32 = 0x0004;
const MOD_WIN: u32 = 0x0008;
const MOD_ALLOWED: u32 = MOD_ALT | MOD_CONTROL | MOD_SHIFT | MOD_WIN;
const MOD_NOREPEAT: u32 = 0x4000;
const VK_LEFT: u32 = 0x25;
const VK_UP: u32 = 0x26;
const VK_RIGHT: u32 = 0x27;
const VK_DOWN: u32 = 0x28;

const ERROR_ALREADY_EXISTS: u32 = 183;
const ERROR_FILE_NOT_FOUND: i32 = 2;
const MB_ICONERROR: u32 = 0x10;
const MB_OK: u32 = 0;

const HOTKEY_UP: i32 = 1;
const HOTKEY_DOWN: i32 = 2;
const HOTKEY_RIGHT: i32 = 3;
const HOTKEY_LEFT: i32 = 4;

const ID_CTRL: i32 = 1001;
const ID_ALT: i32 = 1002;
const ID_SHIFT: i32 = 1003;
const ID_WIN: i32 = 1004;
const ID_AUTOSTART: i32 = 1005;
const ID_SAVE: i32 = 1006;
const ID_CANCEL: i32 = 1007;

const CMD_SETTINGS: usize = 40001;
const CMD_AUTOSTART: usize = 40002;
const CMD_EXIT: usize = 40003;
const TRAY_ICON_ID: u32 = 1;

const GWLP_USERDATA: i32 = -21;
const WS_CHILD: u32 = 0x4000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_TABSTOP: u32 = 0x0001_0000;
const WS_CAPTION: u32 = 0x00C0_0000;
const WS_SYSMENU: u32 = 0x0008_0000;
const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
const BS_DEFPUSHBUTTON: u32 = 0x0000_0001;
const BS_AUTOCHECKBOX: u32 = 0x0000_0003;
const BST_UNCHECKED: usize = 0;
const BST_CHECKED: usize = 1;
const COLOR_WINDOW: usize = 5;
const DEFAULT_GUI_FONT: i32 = 17;
const SW_HIDE: i32 = 0;
const SW_SHOWNORMAL: i32 = 1;
const SWP_NOZORDER: u32 = 0x0004;
const SM_CXSCREEN: i32 = 0;
const SM_CYSCREEN: i32 = 1;

const MF_STRING: u32 = 0;
const MF_CHECKED: u32 = 0x0008;
const MF_SEPARATOR: u32 = 0x0800;
const TPM_RIGHTBUTTON: u32 = 0x0002;
const TPM_RETURNCMD: u32 = 0x0100;

const NIM_ADD: u32 = 0;
const NIM_DELETE: u32 = 2;
const NIF_MESSAGE: u32 = 0x0001;
const NIF_ICON: u32 = 0x0002;
const NIF_TIP: u32 = 0x0004;

const RRF_RT_REG_SZ: u32 = 0x0000_0002;
const REG_SZ: u32 = 1;
const HKEY_CURRENT_USER: Hkey = -2_147_483_647isize;
const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_VALUE: &str = "ROG OSD Switch";

const IDI_APPLICATION: *const u16 = 32512usize as *const u16;
const IDC_ARROW: *const u16 = 32512usize as *const u16;

#[repr(C)]
#[derive(Clone, Copy)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct Msg {
    hwnd: Hwnd,
    message: u32,
    w_param: usize,
    l_param: isize,
    time: u32,
    point: Point,
    private: u32,
}

#[repr(C)]
struct WndClassExW {
    size: u32,
    style: u32,
    wnd_proc: Option<unsafe extern "system" fn(Hwnd, u32, usize, isize) -> isize>,
    class_extra: i32,
    window_extra: i32,
    instance: Hinstance,
    icon: Hicon,
    cursor: Hcursor,
    background: Hbrush,
    menu_name: *const u16,
    class_name: *const u16,
    icon_small: Hicon,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

#[repr(C)]
struct NotifyIconDataW {
    size: u32,
    hwnd: Hwnd,
    id: u32,
    flags: u32,
    callback_message: u32,
    icon: Hicon,
    tip: [u16; 128],
    state: u32,
    state_mask: u32,
    info: [u16; 256],
    timeout_or_version: u32,
    info_title: [u16; 64],
    info_flags: u32,
    guid_item: Guid,
    balloon_icon: Hicon,
}

#[repr(C)]
struct PhysicalMonitor {
    handle: Handle,
    description: [u16; 128],
}

#[link(name = "user32")]
unsafe extern "system" {
    fn RegisterClassExW(window_class: *const WndClassExW) -> u16;
    fn CreateWindowExW(
        extended_style: u32,
        class_name: *const u16,
        window_name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: Hwnd,
        menu: Hmenu,
        instance: Hinstance,
        parameter: *mut c_void,
    ) -> Hwnd;
    fn DefWindowProcW(hwnd: Hwnd, message: u32, w_param: usize, l_param: isize) -> isize;
    fn DestroyWindow(hwnd: Hwnd) -> Bool;
    fn ShowWindow(hwnd: Hwnd, command: i32) -> Bool;
    fn SetForegroundWindow(hwnd: Hwnd) -> Bool;
    fn SetWindowPos(
        hwnd: Hwnd,
        insert_after: Hwnd,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> Bool;
    fn GetSystemMetrics(index: i32) -> i32;
    fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: Hwnd, index: i32, value: isize) -> isize;
    fn FindWindowW(class_name: *const u16, window_name: *const u16) -> Hwnd;
    fn PostMessageW(hwnd: Hwnd, message: u32, w_param: usize, l_param: isize) -> Bool;
    fn RegisterHotKey(hwnd: Hwnd, id: i32, modifiers: u32, virtual_key: u32) -> Bool;
    fn UnregisterHotKey(hwnd: Hwnd, id: i32) -> Bool;
    fn GetMessageW(message: *mut Msg, hwnd: Hwnd, min: u32, max: u32) -> Bool;
    fn TranslateMessage(message: *const Msg) -> Bool;
    fn DispatchMessageW(message: *const Msg) -> isize;
    fn PostQuitMessage(exit_code: i32);
    fn SendMessageW(hwnd: Hwnd, message: u32, w_param: usize, l_param: isize) -> isize;
    fn MessageBoxW(hwnd: Hwnd, text: *const u16, caption: *const u16, kind: u32) -> i32;
    fn LoadIconW(instance: Hinstance, icon_name: *const u16) -> Hicon;
    fn LoadCursorW(instance: Hinstance, cursor_name: *const u16) -> Hcursor;
    fn CreatePopupMenu() -> Hmenu;
    fn AppendMenuW(menu: Hmenu, flags: u32, id: usize, text: *const u16) -> Bool;
    fn TrackPopupMenu(
        menu: Hmenu,
        flags: u32,
        x: i32,
        y: i32,
        reserved: i32,
        hwnd: Hwnd,
        rect: *const Rect,
    ) -> i32;
    fn DestroyMenu(menu: Hmenu) -> Bool;
    fn GetCursorPos(point: *mut Point) -> Bool;
    fn CheckDlgButton(hwnd: Hwnd, id: i32, check: u32) -> Bool;
    fn IsDlgButtonChecked(hwnd: Hwnd, id: i32) -> u32;
    fn RegisterWindowMessageW(text: *const u16) -> u32;
    fn EnumDisplayMonitors(
        hdc: Hdc,
        clip: *const Rect,
        callback: Option<unsafe extern "system" fn(Hmonitor, Hdc, *mut Rect, isize) -> Bool>,
        data: isize,
    ) -> Bool;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateMutexW(attributes: *const c_void, initial_owner: Bool, name: *const u16) -> Handle;
    fn GetLastError() -> u32;
    fn CloseHandle(handle: Handle) -> Bool;
    fn GetModuleHandleW(module_name: *const u16) -> Hinstance;
}

#[link(name = "shell32")]
unsafe extern "system" {
    fn Shell_NotifyIconW(message: u32, data: *mut NotifyIconDataW) -> Bool;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    fn GetStockObject(object: i32) -> Handle;
}

#[link(name = "advapi32")]
unsafe extern "system" {
    fn RegSetKeyValueW(
        key: Hkey,
        subkey: *const u16,
        value_name: *const u16,
        value_type: u32,
        data: *const c_void,
        data_size: u32,
    ) -> i32;
    fn RegGetValueW(
        key: Hkey,
        subkey: *const u16,
        value_name: *const u16,
        flags: u32,
        value_type: *mut u32,
        data: *mut c_void,
        data_size: *mut u32,
    ) -> i32;
    fn RegDeleteKeyValueW(key: Hkey, subkey: *const u16, value_name: *const u16) -> i32;
}

#[link(name = "dxva2")]
unsafe extern "system" {
    fn GetNumberOfPhysicalMonitorsFromHMONITOR(monitor: Hmonitor, count: *mut u32) -> Bool;
    fn GetPhysicalMonitorsFromHMONITOR(
        monitor: Hmonitor,
        count: u32,
        physical_monitors: *mut PhysicalMonitor,
    ) -> Bool;
    fn DestroyPhysicalMonitors(count: u32, physical_monitors: *mut PhysicalMonitor) -> Bool;
    fn SetVCPFeature(monitor: Handle, code: u8, value: u32) -> Bool;
    fn GetVCPFeatureAndVCPFeatureReply(
        monitor: Handle,
        code: u8,
        code_type: *mut u32,
        current: *mut u32,
        maximum: *mut u32,
    ) -> Bool;
}

#[derive(Clone, Copy)]
struct Config {
    modifiers: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            modifiers: MOD_CONTROL | MOD_ALT,
        }
    }
}

struct AppState {
    main_hwnd: Hwnd,
    settings_hwnd: Hwnd,
    icon: Hicon,
    config: Config,
    taskbar_created_message: u32,
}

struct InstanceMutex(Handle);

impl Drop for InstanceMutex {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

enum InstanceStatus {
    Acquired(InstanceMutex),
    ActivatedExisting,
}

enum MonitorOperation {
    SetOsd(u32),
    Probe,
}

struct MonitorContext {
    operation: MonitorOperation,
    matched: usize,
    succeeded: usize,
    current_sources: Vec<u32>,
    errors: Vec<u32>,
}

pub fn run() {
    if env::args().any(|arg| arg == "--probe") {
        let message = match probe_monitor() {
            Ok(source) => {
                format!("Found ASUS ROG Strix XG32UCWMG.\nCurrent input source: 0x{source:02X}")
            }
            Err(error) => error,
        };
        show_message(ptr::null_mut(), "ROG OSD Switch probe", &message, MB_OK);
        return;
    }

    let _instance = match acquire_single_instance() {
        Ok(InstanceStatus::Acquired(instance)) => instance,
        Ok(InstanceStatus::ActivatedExisting) => return,
        Err(message) => {
            show_error(ptr::null_mut(), &message);
            return;
        }
    };

    if let Err(error) = run_desktop_app() {
        show_error(ptr::null_mut(), &error);
    }
}

fn run_desktop_app() -> Result<(), String> {
    let instance = unsafe { GetModuleHandleW(ptr::null()) };
    if instance.is_null() {
        return Err(format!(
            "Could not access the application module (error {}).",
            unsafe { GetLastError() }
        ));
    }

    let icon = unsafe { LoadIconW(ptr::null_mut(), IDI_APPLICATION) };
    let cursor = unsafe { LoadCursorW(ptr::null_mut(), IDC_ARROW) };
    register_window_class(
        instance,
        MAIN_WINDOW_CLASS,
        Some(main_window_proc),
        icon,
        cursor,
    )?;
    register_window_class(
        instance,
        SETTINGS_WINDOW_CLASS,
        Some(settings_window_proc),
        icon,
        cursor,
    )?;

    let mut state = Box::new(AppState {
        main_hwnd: ptr::null_mut(),
        settings_hwnd: ptr::null_mut(),
        icon,
        config: load_config(),
        taskbar_created_message: unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) },
    });

    let main_class = wide(MAIN_WINDOW_CLASS);
    let main_title = wide(APP_NAME);
    let main_hwnd = unsafe {
        CreateWindowExW(
            0,
            main_class.as_ptr(),
            main_title.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null_mut(),
        )
    };
    if main_hwnd.is_null() {
        return Err(format!(
            "Could not create the background window (error {}).",
            unsafe { GetLastError() }
        ));
    }
    state.main_hwnd = main_hwnd;
    unsafe {
        SetWindowLongPtrW(
            main_hwnd,
            GWLP_USERDATA,
            (&mut *state as *mut AppState) as isize,
        );
    }

    state.settings_hwnd = match create_settings_window(instance, &mut state) {
        Ok(hwnd) => hwnd,
        Err(error) => {
            unsafe {
                DestroyWindow(main_hwnd);
            }
            return Err(error);
        }
    };
    if let Err(error) = register_hotkeys(main_hwnd, state.config.modifiers) {
        unsafe {
            DestroyWindow(main_hwnd);
        }
        return Err(error);
    }
    if !add_tray_icon(&state) {
        unsafe {
            DestroyWindow(main_hwnd);
        }
        return Err("Could not add the notification-area icon.".to_string());
    }

    log_line(&format!(
        "started; registered {}+Arrow hotkeys",
        modifier_label(state.config.modifiers)
    ));
    let mut message: Msg = unsafe { mem::zeroed() };
    loop {
        let result = unsafe { GetMessageW(&mut message, ptr::null_mut(), 0, 0) };
        if result <= 0 {
            break;
        }
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    log_line("stopped");
    Ok(())
}

fn register_window_class(
    instance: Hinstance,
    name: &str,
    wnd_proc: Option<unsafe extern "system" fn(Hwnd, u32, usize, isize) -> isize>,
    icon: Hicon,
    cursor: Hcursor,
) -> Result<(), String> {
    let class_name = wide(name);
    let window_class = WndClassExW {
        size: mem::size_of::<WndClassExW>() as u32,
        style: 0,
        wnd_proc,
        class_extra: 0,
        window_extra: 0,
        instance,
        icon,
        cursor,
        background: (COLOR_WINDOW + 1) as Hbrush,
        menu_name: ptr::null(),
        class_name: class_name.as_ptr(),
        icon_small: icon,
    };
    if unsafe { RegisterClassExW(&window_class) } == 0 {
        return Err(format!(
            "Could not register the '{name}' window class (error {}).",
            unsafe { GetLastError() }
        ));
    }
    Ok(())
}

fn create_settings_window(instance: Hinstance, state: &mut AppState) -> Result<Hwnd, String> {
    let class_name = wide(SETTINGS_WINDOW_CLASS);
    let title = wide("ROG OSD Switch settings");
    let width = 470;
    let height = 335;
    let x = (unsafe { GetSystemMetrics(SM_CXSCREEN) } - width) / 2;
    let y = (unsafe { GetSystemMetrics(SM_CYSCREEN) } - height) / 2;
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_CAPTION | WS_SYSMENU,
            x,
            y,
            width,
            height,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null_mut(),
        )
    };
    if hwnd.is_null() {
        return Err(format!(
            "Could not create the settings window (error {}).",
            unsafe { GetLastError() }
        ));
    }
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, (state as *mut AppState) as isize);
    }

    create_label(
        hwnd,
        instance,
        "ASUS ROG Strix OLED XG32UCWMG",
        24,
        20,
        410,
        22,
    )?;
    create_label(
        hwnd,
        instance,
        "Shortcut modifiers (the arrow keys always control the monitor OSD):",
        24,
        55,
        420,
        22,
    )?;
    create_checkbox(hwnd, instance, "Ctrl", ID_CTRL, 28, 85, 85, 25)?;
    create_checkbox(hwnd, instance, "Alt", ID_ALT, 118, 85, 85, 25)?;
    create_checkbox(hwnd, instance, "Shift", ID_SHIFT, 208, 85, 85, 25)?;
    create_checkbox(hwnd, instance, "Win", ID_WIN, 298, 85, 85, 25)?;
    create_label(
        hwnd,
        instance,
        "Right opens Input Select; Up/Down choose; Right confirms; Left cancels.",
        24,
        124,
        420,
        38,
    )?;
    create_checkbox(
        hwnd,
        instance,
        "Start ROG OSD Switch automatically when I sign in",
        ID_AUTOSTART,
        28,
        178,
        400,
        26,
    )?;
    create_label(
        hwnd,
        instance,
        "The app stays in the notification area and uses no administrator rights.",
        24,
        216,
        420,
        22,
    )?;
    create_button(hwnd, instance, "Save", ID_SAVE, 268, 255, 82, 30, true)?;
    create_button(hwnd, instance, "Cancel", ID_CANCEL, 360, 255, 82, 30, false)?;

    Ok(hwnd)
}

fn create_label(
    parent: Hwnd,
    instance: Hinstance,
    text: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<(), String> {
    create_control(
        parent,
        instance,
        "STATIC",
        text,
        WS_CHILD | WS_VISIBLE,
        0,
        x,
        y,
        width,
        height,
    )
}

#[allow(clippy::too_many_arguments)]
fn create_checkbox(
    parent: Hwnd,
    instance: Hinstance,
    text: &str,
    id: i32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<(), String> {
    create_control(
        parent,
        instance,
        "BUTTON",
        text,
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_AUTOCHECKBOX,
        id,
        x,
        y,
        width,
        height,
    )
}

#[allow(clippy::too_many_arguments)]
fn create_button(
    parent: Hwnd,
    instance: Hinstance,
    text: &str,
    id: i32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    default: bool,
) -> Result<(), String> {
    create_control(
        parent,
        instance,
        "BUTTON",
        text,
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | if default { BS_DEFPUSHBUTTON } else { 0 },
        id,
        x,
        y,
        width,
        height,
    )
}

#[allow(clippy::too_many_arguments)]
fn create_control(
    parent: Hwnd,
    instance: Hinstance,
    class_name: &str,
    text: &str,
    style: u32,
    id: i32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<(), String> {
    let class_name = wide(class_name);
    let text = wide(text);
    let control = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            text.as_ptr(),
            style,
            x,
            y,
            width,
            height,
            parent,
            id as usize as Hmenu,
            instance,
            ptr::null_mut(),
        )
    };
    if control.is_null() {
        return Err(format!(
            "Could not create a settings control (error {}).",
            unsafe { GetLastError() }
        ));
    }
    unsafe {
        SendMessageW(
            control,
            WM_SETFONT,
            GetStockObject(DEFAULT_GUI_FONT) as usize,
            1,
        );
    }
    Ok(())
}

unsafe extern "system" fn main_window_proc(
    hwnd: Hwnd,
    message: u32,
    w_param: usize,
    l_param: isize,
) -> isize {
    let state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState };
    if !state_ptr.is_null() {
        let state = unsafe { &mut *state_ptr };
        if message == state.taskbar_created_message {
            add_tray_icon(state);
            return 0;
        }
    }

    match message {
        WM_HOTKEY if !state_ptr.is_null() => {
            handle_hotkey(w_param as i32);
            0
        }
        WM_TRAY_ICON if !state_ptr.is_null() => {
            let state = unsafe { &mut *state_ptr };
            match l_param as u32 {
                WM_LBUTTONDBLCLK => show_settings(state),
                WM_RBUTTONUP | WM_CONTEXTMENU => show_tray_menu(state),
                _ => {}
            }
            0
        }
        WM_COMMAND if !state_ptr.is_null() => {
            let state = unsafe { &mut *state_ptr };
            match w_param & 0xFFFF {
                CMD_SETTINGS => show_settings(state),
                CMD_AUTOSTART => toggle_autostart(state.main_hwnd),
                CMD_EXIT => unsafe {
                    DestroyWindow(hwnd);
                },
                _ => {}
            }
            0
        }
        WM_DESTROY if !state_ptr.is_null() => {
            let state = unsafe { &mut *state_ptr };
            unregister_hotkeys(hwnd);
            remove_tray_icon(state);
            if !state.settings_hwnd.is_null() {
                unsafe {
                    DestroyWindow(state.settings_hwnd);
                }
                state.settings_hwnd = ptr::null_mut();
            }
            unsafe {
                PostQuitMessage(0);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, message, w_param, l_param) },
    }
}

unsafe extern "system" fn settings_window_proc(
    hwnd: Hwnd,
    message: u32,
    w_param: usize,
    l_param: isize,
) -> isize {
    let state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState };
    match message {
        WM_COMMAND if !state_ptr.is_null() => {
            let state = unsafe { &mut *state_ptr };
            match (w_param & 0xFFFF) as i32 {
                ID_SAVE => save_settings(state),
                ID_CANCEL => unsafe {
                    ShowWindow(hwnd, SW_HIDE);
                },
                _ => {}
            }
            0
        }
        WM_CLOSE => {
            unsafe {
                ShowWindow(hwnd, SW_HIDE);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, message, w_param, l_param) },
    }
}

fn handle_hotkey(id: i32) {
    let hotkey = match id {
        HOTKEY_UP => Hotkey::Up,
        HOTKEY_DOWN => Hotkey::Down,
        HOTKEY_RIGHT => Hotkey::Right,
        HOTKEY_LEFT => Hotkey::Left,
        _ => return,
    };
    let command = command_for_hotkey(hotkey);
    match send_osd(command) {
        Ok(()) => log_line(&format!("sent OSD command {command:?}")),
        Err(error) => log_line(&format!("OSD command {command:?} failed: {error}")),
    }
}

fn register_hotkeys(hwnd: Hwnd, modifiers: u32) -> Result<(), String> {
    let registrations = [
        (HOTKEY_UP, VK_UP),
        (HOTKEY_DOWN, VK_DOWN),
        (HOTKEY_RIGHT, VK_RIGHT),
        (HOTKEY_LEFT, VK_LEFT),
    ];
    let mut registered = Vec::new();
    for (id, key) in registrations {
        if unsafe { RegisterHotKey(hwnd, id, modifiers | MOD_NOREPEAT, key) } == 0 {
            for previous in registered {
                unsafe {
                    UnregisterHotKey(hwnd, previous);
                }
            }
            return Err(format!(
                "Could not register {}+Arrow. Close ASUS DisplayWidget Center or another app using that shortcut, then try again.",
                modifier_label(modifiers)
            ));
        }
        registered.push(id);
    }
    Ok(())
}

fn unregister_hotkeys(hwnd: Hwnd) {
    for id in [HOTKEY_UP, HOTKEY_DOWN, HOTKEY_RIGHT, HOTKEY_LEFT] {
        unsafe {
            UnregisterHotKey(hwnd, id);
        }
    }
}

fn apply_hotkeys(state: &mut AppState, modifiers: u32) -> Result<(), String> {
    let previous = state.config.modifiers;
    unregister_hotkeys(state.main_hwnd);
    if let Err(error) = register_hotkeys(state.main_hwnd, modifiers) {
        let restore_result = register_hotkeys(state.main_hwnd, previous);
        return match restore_result {
            Ok(()) => Err(error),
            Err(restore_error) => Err(format!(
                "{error}\n\nThe previous shortcut could not be restored either: {restore_error}"
            )),
        };
    }
    state.config.modifiers = modifiers;
    Ok(())
}

fn show_settings(state: &mut AppState) {
    set_checkbox(
        state.settings_hwnd,
        ID_CTRL,
        state.config.modifiers & MOD_CONTROL != 0,
    );
    set_checkbox(
        state.settings_hwnd,
        ID_ALT,
        state.config.modifiers & MOD_ALT != 0,
    );
    set_checkbox(
        state.settings_hwnd,
        ID_SHIFT,
        state.config.modifiers & MOD_SHIFT != 0,
    );
    set_checkbox(
        state.settings_hwnd,
        ID_WIN,
        state.config.modifiers & MOD_WIN != 0,
    );
    set_checkbox(state.settings_hwnd, ID_AUTOSTART, is_autostart_enabled());
    unsafe {
        SetWindowPos(
            state.settings_hwnd,
            ptr::null_mut(),
            (GetSystemMetrics(SM_CXSCREEN) - 470) / 2,
            (GetSystemMetrics(SM_CYSCREEN) - 335) / 2,
            470,
            335,
            SWP_NOZORDER,
        );
        ShowWindow(state.settings_hwnd, SW_SHOWNORMAL);
        SetForegroundWindow(state.settings_hwnd);
    }
}

fn save_settings(state: &mut AppState) {
    let mut modifiers = 0;
    if is_checked(state.settings_hwnd, ID_CTRL) {
        modifiers |= MOD_CONTROL;
    }
    if is_checked(state.settings_hwnd, ID_ALT) {
        modifiers |= MOD_ALT;
    }
    if is_checked(state.settings_hwnd, ID_SHIFT) {
        modifiers |= MOD_SHIFT;
    }
    if is_checked(state.settings_hwnd, ID_WIN) {
        modifiers |= MOD_WIN;
    }
    if modifiers == 0 {
        show_error(
            state.settings_hwnd,
            "Select at least one modifier key for the arrow shortcuts.",
        );
        return;
    }

    if modifiers != state.config.modifiers
        && let Err(error) = apply_hotkeys(state, modifiers)
    {
        show_error(state.settings_hwnd, &error);
        return;
    }

    if let Err(error) = set_autostart(is_checked(state.settings_hwnd, ID_AUTOSTART)) {
        show_error(state.settings_hwnd, &error);
        return;
    }
    if let Err(error) = save_config(state.config) {
        show_error(state.settings_hwnd, &error);
        return;
    }

    log_line(&format!(
        "settings saved; hotkeys are {}+Arrow; autostart={}",
        modifier_label(state.config.modifiers),
        is_autostart_enabled()
    ));
    unsafe {
        ShowWindow(state.settings_hwnd, SW_HIDE);
    }
}

fn set_checkbox(hwnd: Hwnd, id: i32, checked: bool) {
    unsafe {
        CheckDlgButton(
            hwnd,
            id,
            if checked {
                BST_CHECKED as u32
            } else {
                BST_UNCHECKED as u32
            },
        );
    }
}

fn is_checked(hwnd: Hwnd, id: i32) -> bool {
    unsafe { IsDlgButtonChecked(hwnd, id) as usize == BST_CHECKED }
}

fn show_tray_menu(state: &mut AppState) {
    let menu = unsafe { CreatePopupMenu() };
    if menu.is_null() {
        return;
    }
    let settings = wide("Settings...");
    let autostart = wide("Start with Windows");
    let exit = wide("Exit");
    unsafe {
        AppendMenuW(menu, MF_STRING, CMD_SETTINGS, settings.as_ptr());
        AppendMenuW(
            menu,
            MF_STRING
                | if is_autostart_enabled() {
                    MF_CHECKED
                } else {
                    0
                },
            CMD_AUTOSTART,
            autostart.as_ptr(),
        );
        AppendMenuW(menu, MF_SEPARATOR, 0, ptr::null());
        AppendMenuW(menu, MF_STRING, CMD_EXIT, exit.as_ptr());
    }

    let mut point = Point { x: 0, y: 0 };
    unsafe {
        GetCursorPos(&mut point);
        SetForegroundWindow(state.main_hwnd);
    }
    let command = unsafe {
        TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_RETURNCMD,
            point.x,
            point.y,
            0,
            state.main_hwnd,
            ptr::null(),
        )
    } as usize;
    unsafe {
        DestroyMenu(menu);
    }
    match command {
        CMD_SETTINGS => show_settings(state),
        CMD_AUTOSTART => toggle_autostart(state.main_hwnd),
        CMD_EXIT => unsafe {
            DestroyWindow(state.main_hwnd);
        },
        _ => {}
    }
}

fn add_tray_icon(state: &AppState) -> bool {
    let mut data: NotifyIconDataW = unsafe { mem::zeroed() };
    data.size = mem::size_of::<NotifyIconDataW>() as u32;
    data.hwnd = state.main_hwnd;
    data.id = TRAY_ICON_ID;
    data.flags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.callback_message = WM_TRAY_ICON;
    data.icon = state.icon;
    let tip = wide("ROG OSD Switch - monitor Input Select hotkeys");
    let copy_length = tip.len().saturating_sub(1).min(data.tip.len() - 1);
    data.tip[..copy_length].copy_from_slice(&tip[..copy_length]);
    unsafe { Shell_NotifyIconW(NIM_ADD, &mut data) != 0 }
}

fn remove_tray_icon(state: &AppState) {
    let mut data: NotifyIconDataW = unsafe { mem::zeroed() };
    data.size = mem::size_of::<NotifyIconDataW>() as u32;
    data.hwnd = state.main_hwnd;
    data.id = TRAY_ICON_ID;
    unsafe {
        Shell_NotifyIconW(NIM_DELETE, &mut data);
    }
}

fn load_config() -> Config {
    let Some(path) = config_path() else {
        return Config::default();
    };
    let Ok(contents) = fs::read_to_string(path) else {
        return Config::default();
    };
    for line in contents.lines() {
        if let Some(value) = line.trim().strip_prefix("modifiers=")
            && let Ok(modifiers) = value.parse::<u32>()
            && modifiers != 0
            && modifiers & !MOD_ALLOWED == 0
        {
            return Config { modifiers };
        }
    }
    Config::default()
}

fn save_config(config: Config) -> Result<(), String> {
    let path = config_path().ok_or_else(|| "LOCALAPPDATA is not available.".to_string())?;
    let directory = path
        .parent()
        .ok_or_else(|| "The configuration directory is invalid.".to_string())?;
    fs::create_dir_all(directory)
        .map_err(|error| format!("Could not create the configuration directory: {error}"))?;
    fs::write(&path, format!("modifiers={}\n", config.modifiers))
        .map_err(|error| format!("Could not save {}: {error}", path.display()))
}

fn config_path() -> Option<PathBuf> {
    env::var_os("LOCALAPPDATA")
        .map(|path| PathBuf::from(path).join("RogOsdSwitch").join("config.ini"))
}

fn is_autostart_enabled() -> bool {
    let subkey = wide(RUN_KEY);
    let value_name = wide(RUN_VALUE);
    let mut size = 0;
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            value_name.as_ptr(),
            RRF_RT_REG_SZ,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut size,
        ) == 0
    }
}

fn set_autostart(enabled: bool) -> Result<(), String> {
    let subkey = wide(RUN_KEY);
    let value_name = wide(RUN_VALUE);
    if enabled {
        let executable = env::current_exe()
            .map_err(|error| format!("Could not locate the application executable: {error}"))?;
        let command = wide(&format!("\"{}\"", executable.display()));
        let result = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                value_name.as_ptr(),
                REG_SZ,
                command.as_ptr().cast(),
                (command.len() * mem::size_of::<u16>()) as u32,
            )
        };
        if result != 0 {
            return Err(format!(
                "Could not enable Windows autostart (error {result})."
            ));
        }
    } else {
        let result =
            unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, subkey.as_ptr(), value_name.as_ptr()) };
        if result != 0 && result != ERROR_FILE_NOT_FOUND {
            return Err(format!(
                "Could not disable Windows autostart (error {result})."
            ));
        }
    }
    Ok(())
}

fn toggle_autostart(owner: Hwnd) {
    let enabled = !is_autostart_enabled();
    match set_autostart(enabled) {
        Ok(()) => log_line(&format!("autostart set to {enabled}")),
        Err(error) => show_error(owner, &error),
    }
}

fn modifier_label(modifiers: u32) -> String {
    let mut names = Vec::new();
    if modifiers & MOD_CONTROL != 0 {
        names.push("Ctrl");
    }
    if modifiers & MOD_ALT != 0 {
        names.push("Alt");
    }
    if modifiers & MOD_SHIFT != 0 {
        names.push("Shift");
    }
    if modifiers & MOD_WIN != 0 {
        names.push("Win");
    }
    names.join("+")
}

fn acquire_single_instance() -> Result<InstanceStatus, String> {
    let name = wide("Local\\RogOsdSwitch.XG32UCWMG");
    let handle = unsafe { CreateMutexW(ptr::null(), 1, name.as_ptr()) };
    if handle.is_null() {
        return Err(format!(
            "Could not create the application mutex (error {}).",
            unsafe { GetLastError() }
        ));
    }
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe {
            CloseHandle(handle);
        }
        let class_name = wide(MAIN_WINDOW_CLASS);
        let existing = unsafe { FindWindowW(class_name.as_ptr(), ptr::null()) };
        if !existing.is_null()
            && unsafe { PostMessageW(existing, WM_COMMAND, CMD_SETTINGS, 0) } != 0
        {
            return Ok(InstanceStatus::ActivatedExisting);
        }
        return Err(
            "ROG OSD Switch is already starting. Try opening it again in a moment.".to_string(),
        );
    }
    Ok(InstanceStatus::Acquired(InstanceMutex(handle)))
}

fn send_osd(command: OsdCommand) -> Result<(), String> {
    let context = enumerate_monitors(MonitorOperation::SetOsd(command as u32))?;
    if context.matched == 0 {
        return Err(format!("monitor containing '{MODEL_FILTER}' was not found"));
    }
    if context.succeeded != context.matched {
        return Err(format!(
            "DDC write succeeded for {}/{} matching monitors; errors: {:?}",
            context.succeeded, context.matched, context.errors
        ));
    }
    Ok(())
}

fn probe_monitor() -> Result<u32, String> {
    let context = enumerate_monitors(MonitorOperation::Probe)?;
    if context.matched == 0 {
        return Err(format!(
            "Monitor containing '{MODEL_FILTER}' was not found."
        ));
    }
    context.current_sources.first().copied().ok_or_else(|| {
        format!(
            "The monitor was found, but VCP 0x60 could not be read: {:?}",
            context.errors
        )
    })
}

fn enumerate_monitors(operation: MonitorOperation) -> Result<MonitorContext, String> {
    let mut context = MonitorContext {
        operation,
        matched: 0,
        succeeded: 0,
        current_sources: Vec::new(),
        errors: Vec::new(),
    };
    let ok = unsafe {
        EnumDisplayMonitors(
            ptr::null_mut(),
            ptr::null(),
            Some(monitor_callback),
            (&mut context as *mut MonitorContext) as isize,
        )
    };
    if ok == 0 {
        return Err(format!(
            "EnumDisplayMonitors failed with error {}.",
            unsafe { GetLastError() }
        ));
    }
    Ok(context)
}

unsafe extern "system" fn monitor_callback(
    monitor: Hmonitor,
    _hdc: Hdc,
    _rect: *mut Rect,
    data: isize,
) -> Bool {
    let context = unsafe { &mut *(data as *mut MonitorContext) };
    let mut count = 0;
    if unsafe { GetNumberOfPhysicalMonitorsFromHMONITOR(monitor, &mut count) } == 0 || count == 0 {
        context.errors.push(unsafe { GetLastError() });
        return 1;
    }

    let mut physical_monitors: Vec<PhysicalMonitor> =
        (0..count).map(|_| unsafe { mem::zeroed() }).collect();
    if unsafe { GetPhysicalMonitorsFromHMONITOR(monitor, count, physical_monitors.as_mut_ptr()) }
        == 0
    {
        context.errors.push(unsafe { GetLastError() });
        return 1;
    }

    for physical in &physical_monitors {
        let end = physical
            .description
            .iter()
            .position(|character| *character == 0)
            .unwrap_or(physical.description.len());
        let description = String::from_utf16_lossy(&physical.description[..end]);
        if !description.to_ascii_uppercase().contains(MODEL_FILTER) {
            continue;
        }

        context.matched += 1;
        match context.operation {
            MonitorOperation::SetOsd(value) => {
                if unsafe { SetVCPFeature(physical.handle, VCP_ASUS_EZ_OSD, value) } != 0 {
                    context.succeeded += 1;
                } else {
                    context.errors.push(unsafe { GetLastError() });
                }
            }
            MonitorOperation::Probe => {
                let mut current = 0;
                let mut maximum = 0;
                if unsafe {
                    GetVCPFeatureAndVCPFeatureReply(
                        physical.handle,
                        VCP_INPUT_SOURCE,
                        ptr::null_mut(),
                        &mut current,
                        &mut maximum,
                    )
                } != 0
                {
                    context.succeeded += 1;
                    context.current_sources.push(current);
                } else {
                    context.errors.push(unsafe { GetLastError() });
                }
            }
        }
    }

    unsafe {
        DestroyPhysicalMonitors(count, physical_monitors.as_mut_ptr());
    }
    1
}

fn wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

fn show_error(owner: Hwnd, message: &str) {
    show_message(owner, APP_NAME, message, MB_OK | MB_ICONERROR);
}

fn show_message(owner: Hwnd, caption: &str, message: &str, kind: u32) {
    let caption = wide(caption);
    let message = wide(message);
    unsafe {
        MessageBoxW(owner, message.as_ptr(), caption.as_ptr(), kind);
    }
}

fn log_line(message: &str) {
    let Some(local_app_data) = env::var_os("LOCALAPPDATA") else {
        return;
    };
    let directory = PathBuf::from(local_app_data).join("RogOsdSwitch");
    if fs::create_dir_all(&directory).is_err() {
        return;
    }
    let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join("rog-osd-switch.log"))
    else {
        return;
    };
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let _ = writeln!(file, "{seconds} {message}");
}
