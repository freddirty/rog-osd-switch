#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(windows)]
mod windows;

#[cfg(windows)]
fn main() {
    windows::run();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Linux support is planned but is not part of the Windows MVP yet.");
}
