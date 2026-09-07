use crate::types::{AboutInfo, ModelStatus};
use crate::AppState;
use crate::{db, ollama, search};
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn get_about_info(app: AppHandle, state: State<'_, AppState>) -> Result<AboutInfo, String> {
    let cfg = state.config.read().map_err(|e| e.to_string())?.clone();
    let http = state.http.clone();

    let ollama_reachable = ollama::is_reachable(&http).await;
    let (available, running) = if ollama_reachable {
        (
            ollama::list_tags(&http).await.unwrap_or_default(),
            ollama::list_running(&http).await.unwrap_or_default(),
        )
    } else {
        (Vec::new(), Vec::new())
    };

    let status_for = |name: &str| ModelStatus {
        name: name.to_string(),
        available: available.iter().any(|m| m == name),
        loaded_in_memory: running.iter().any(|m| m == name),
    };

    let index_stats = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::index_stats(&conn).map_err(|e| e.to_string())?
    };

    let searxng_reachable = search::is_reachable(&http, &cfg.searxng_url).await;
    let resolve_reachable = crate::commands::resolve::resolve_status()
        .await
        .map(|s| s.running)
        .unwrap_or(false);
    let premiere_reachable = crate::commands::premiere::premiere_reachable().await;
    let blender_reachable = crate::commands::blender::blender_reachable().await;

    Ok(AboutInfo {
        app_version: app.package_info().version.to_string(),
        ollama_reachable,
        chat_model: status_for(&cfg.chat_model),
        embedding_model: status_for(&cfg.embedding_model),
        index_stats,
        fallback_enabled: cfg.fallback_enabled,
        searxng_reachable,
        resolve_reachable,
        premiere_reachable,
        blender_reachable,
    })
}
