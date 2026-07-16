pub mod commands;
pub mod db;
pub mod domain;
pub mod error;
pub mod indicator;
pub mod provider;
pub mod repository;
pub mod signal;
pub mod state;

use commands::scan_presets::{
    create_scan_preset, delete_scan_preset, get_scan_preset, list_scan_presets, update_scan_preset,
};
use commands::watchlists::{
    create_watchlist, delete_watchlist, get_watchlist, list_watchlists, update_watchlist,
};
use db::Database;
use state::AppState;
use tauri::Manager;

#[tauri::command]
fn health_check() -> &'static str {
    "ok"
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let database_path = app.path().app_data_dir()?.join("stock.sqlite3");
            let database = Database::open(database_path)?;
            app.manage(AppState::new(database));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            health_check,
            list_watchlists,
            get_watchlist,
            create_watchlist,
            update_watchlist,
            delete_watchlist,
            list_scan_presets,
            get_scan_preset,
            create_scan_preset,
            update_scan_preset,
            delete_scan_preset
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}
