//! The erebus frontend for Windows, built on WinUI 3 through windows-reactor.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod inbox;
mod logs;
mod ui;

fn main() {
    if let Err(err) = windows_reactor::App::run_component::<ui::Erebus>(()) {
        eprintln!("failed to start: {err}");
    }
}
