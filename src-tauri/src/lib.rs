// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod engine;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::rules_list,
            commands::scan,
            commands::clean,
            commands::check_blockers,
            commands::check_autoruns,
            commands::settings_get,
            commands::settings_set,
            commands::export_builtin_rules,
            commands::open_in_explorer,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
