#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod automation;
mod key_hook;
mod mouse;
mod platform;
mod renderer;
mod ui;

fn main() {
    if let Err(error) = app::run(app::AppConfig::from_env()) {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
