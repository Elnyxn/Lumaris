#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod app;
pub mod commands;
pub mod error;
pub mod events;
pub mod hotkey;
pub mod i18n;
pub mod logging;
pub mod monitor;
pub mod platform;
pub mod settings;
pub mod startup;
pub mod state;
pub mod tray;
pub mod update;
pub mod utils;
pub mod window;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    app::run();
}
