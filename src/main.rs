#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(windows)]
fn main() {
    windows::run();
}

#[cfg(target_os = "linux")]
fn main() {
    if let Err(error) = linux::run() {
        eprintln!("ROG OSD Switch: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
fn main() {
    eprintln!("This platform is not supported.");
}
