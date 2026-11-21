#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]
#[cfg(any(windows, test))]
mod settings;
#[cfg(any(windows, test))]
mod window_catalog;
#[cfg(windows)]
mod ui;
#[cfg(windows)]
fn main() {
    ui::run();
}
#[cfg(not(windows))]
fn main() {
    eprintln!("Kokoda Host requires Windows.");
}
