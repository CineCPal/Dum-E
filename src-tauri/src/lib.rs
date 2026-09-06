mod commands;
mod config;
mod db;
mod ollama;
mod rag;
mod search;
mod types;

use rusqlite::Connection;
use std::sync::{Mutex, RwLock};
use tauri::Manager;

pub struct AppState {
    pub db: Mutex<Connection>,
    pub config: RwLock<config::AppConfig>,
    pub http: reqwest::Client,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let db_path = config::db_path(&handle)?;
            let conn = db::open(&db_path)?;
            let cfg = config::load_config(&handle);

            app.manage(AppState {
                db: Mutex::new(conn),
                config: RwLock::new(cfg),
                http: reqwest::Client::new(),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::chat::list_messages,
            commands::chat::ask_question,
            commands::chat::clear_chat,
            commands::settings::get_config,
            commands::settings::update_config,
            commands::settings::list_ollama_models,
            commands::about::get_about_info,
            commands::reindex::run_reindex,
            commands::resolve::resolve_status,
            commands::resolve::resolve_project_info,
            commands::resolve::resolve_add_marker,
            commands::resolve::resolve_list_timelines,
            commands::resolve::resolve_list_media_pool_clips,
            commands::resolve::resolve_import_media,
            commands::resolve::resolve_list_timeline_clips,
            commands::resolve::resolve_set_timeline_clip_enabled,
            commands::resolve::resolve_delete_timeline_clip,
            commands::resolve::resolve_list_render_presets,
            commands::resolve::resolve_stop_render,
            commands::resolve::resolve_start_render,
            commands::premiere::premiere_status,
            commands::premiere::premiere_add_marker,
            commands::premiere::premiere_import_media,
            commands::premiere::premiere_list_timeline_clips,
            commands::premiere::premiere_set_timeline_clip_enabled,
            commands::premiere::premiere_start_render,
            commands::broll::broll_score_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
