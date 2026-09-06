use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// Command-R7B is Cohere's small model purpose-trained for retrieval-augmented
/// generation (answer strictly from supplied documents, cite them, decline
/// when they don't cover the question) — exactly Dum-E's core behavior.
pub const DEFAULT_CHAT_MODEL: &str = "command-r7b:latest";
pub const DEFAULT_EMBEDDING_MODEL: &str = "nomic-embed-text:latest";

/// A self-hosted SearXNG instance (open-source metasearch engine) — no API
/// key, no billing, no third-party vendor, just a local service.
pub const DEFAULT_SEARXNG_URL: &str = "http://127.0.0.1:8888";

/// Models too large to run acceptably on a 16GB machine; never offered as a
/// selectable chat/embedding model even if present locally.
pub const EXCLUDED_MODELS: &[&str] = &["llama3.3:latest", "llama3.3"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub chat_model: String,
    pub embedding_model: String,
    /// Internet search fallback is opt-in: off by default so no query text
    /// ever leaves the machine until the user explicitly enables it.
    pub fallback_enabled: bool,
    pub searxng_url: String,
    pub top_k: usize,
    pub similarity_threshold_primary: f32,
    pub similarity_threshold_secondary: f32,
    /// Path to a local checkout of the (separate, unmodified) B-Roll
    /// Analyzer project -- e.g. `~/Developer/Blair/B-Roll Analyzer`. Not
    /// bundled with Dum-E; `None` until the user points at their own copy
    /// in Settings. See `broll_bridge/README.md`.
    pub broll_analyzer_path: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            chat_model: DEFAULT_CHAT_MODEL.to_string(),
            embedding_model: DEFAULT_EMBEDDING_MODEL.to_string(),
            fallback_enabled: false,
            searxng_url: DEFAULT_SEARXNG_URL.to_string(),
            top_k: 6,
            similarity_threshold_primary: 0.55,
            similarity_threshold_secondary: 0.45,
            broll_analyzer_path: None,
        }
    }
}

pub fn app_data_dir(app: &AppHandle) -> anyhow::Result<PathBuf> {
    let dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn db_path(app: &AppHandle) -> anyhow::Result<PathBuf> {
    Ok(app_data_dir(app)?.join("dume.db"))
}

fn config_path(app: &AppHandle) -> anyhow::Result<PathBuf> {
    Ok(app_data_dir(app)?.join("config.json"))
}

pub fn load_config(app: &AppHandle) -> AppConfig {
    let path = match config_path(app) {
        Ok(p) => p,
        Err(_) => return AppConfig::default(),
    };
    match std::fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

pub fn save_config(app: &AppHandle, config: &AppConfig) -> anyhow::Result<()> {
    let path = config_path(app)?;
    let raw = serde_json::to_string_pretty(config)?;
    std::fs::write(path, raw)?;
    Ok(())
}
