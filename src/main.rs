#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod assets;
mod export;
mod record_format;
mod platform;
mod ui;
mod util;
mod state;
mod storage;
mod views;

fn main() {
    app::run();
}
