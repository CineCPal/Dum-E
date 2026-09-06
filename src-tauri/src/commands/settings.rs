use crate::config::{self, AppConfig, EXCLUDED_MODELS};
use crate::ollama;
use crate::AppState;
use serde::Deserialize;
use tauri::{AppHandle, State};

#[derive(Debug, Deserialize)]
pub struct ConfigUpdate {
    pub chat_model: Option<String>,
    pub embedding_model: Option<String>,
    pub fallback_enabled: Option<bool>,
    pub searxng_url: Option<String>,
    pub top_k: Option<usize>,
    pub broll_analyzer_path: Option<String>,
}

#[tauri::command]
pub fn get_config(state: State<AppState>) -> Result<AppConfig, String> {
    state.config.read().map(|c| c.clone()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_config(app: AppHandle, state: State<AppState>, update: ConfigUpdate) -> Result<AppConfig, String> {
    let mut cfg = state.config.write().map_err(|e| e.to_string())?;

    if let Some(model) = update.chat_model {
        if EXCLUDED_MODELS.contains(&model.as_str()) {
            return Err(format!("{model} is too large to run acceptably on this machine"));
        }
        cfg.chat_model = model;
    }
    if let Some(model) = update.embedding_model {
        cfg.embedding_model = model;
    }
    if let Some(enabled) = update.fallback_enabled {
        cfg.fallback_enabled = enabled;
    }
    if let Some(url) = update.searxng_url {
        cfg.searxng_url = url;
    }
    if let Some(top_k) = update.top_k {
        cfg.top_k = top_k;
    }
    if let Some(path) = update.broll_analyzer_path {
        cfg.broll_analyzer_path = if path.trim().is_empty() { None } else { Some(path) };
    }

    config::save_config(&app, &cfg).map_err(|e| e.to_string())?;
    Ok(cfg.clone())
}

#[tauri::command]
pub async fn list_ollama_models(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let http = state.http.clone();
    let models = ollama::list_tags(&http).await?;
    Ok(models
        .into_iter()
        .filter(|m| !EXCLUDED_MODELS.contains(&m.as_str()))
        .collect())
}
