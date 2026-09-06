use crate::types::{
    RenderProgress, ResolveActionResult, ResolveClipInfo, ResolveImportResult, ResolveProjectInfo,
    ResolveStatus, ResolveTimelineClip, ResolveTimelineSummary,
};
use std::path::PathBuf;
use std::process::Stdio;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Resolved at compile time from this crate's location, since
/// `resolve_bridge/` lives one directory above `src-tauri/` (same convention
/// as `reindex.rs`'s `ingestion/`).
fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri always has a parent directory")
        .to_path_buf()
}

fn python_command() -> String {
    std::env::var("DUME_PYTHON").unwrap_or_else(|_| "python3".to_string())
}

/// Spawns `resolve_bridge/resolve_bridge.py <command> [json_args]` and parses
/// its single-line JSON stdout into `T`. Unlike `reindex.rs` (a long-running
/// job that streams progress events), Resolve calls here are instant point
/// queries/actions, so this just awaits the child process and returns its
/// parsed result directly.
async fn run_bridge<T: serde::de::DeserializeOwned>(
    command: &str,
    json_args: Option<String>,
) -> Result<T, String> {
    let script = project_root().join("resolve_bridge").join("resolve_bridge.py");
    if !script.exists() {
        return Err(format!("Resolve bridge script not found at {}", script.display()));
    }

    let mut cmd = Command::new(python_command());
    cmd.arg(&script).arg(command);
    if let Some(args) = json_args {
        cmd.arg(args);
    }

    let output = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("Failed to run Resolve bridge: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Resolve bridge produced no output: {stderr}"));
    }

    serde_json::from_str(trimmed).map_err(|e| format!("Couldn't parse Resolve bridge output: {e} ({trimmed})"))
}

#[tauri::command]
pub async fn resolve_status() -> Result<ResolveStatus, String> {
    run_bridge("status", None).await
}

#[tauri::command]
pub async fn resolve_project_info() -> Result<ResolveProjectInfo, String> {
    run_bridge("project", None).await
}

#[tauri::command]
pub async fn resolve_add_marker(color: String, name: String, note: String) -> Result<ResolveActionResult, String> {
    let args = serde_json::json!({ "color": color, "name": name, "note": note }).to_string();
    run_bridge("add_marker", Some(args)).await
}

/// Deserialization shape for `list_timelines`'s `{"timelines": [...]}`
/// wrapper -- not part of the Rust<->TS contract, just internal plumbing.
#[derive(serde::Deserialize)]
struct TimelinesResponse {
    timelines: Vec<ResolveTimelineSummary>,
}

#[tauri::command]
pub async fn resolve_list_timelines() -> Result<Vec<ResolveTimelineSummary>, String> {
    let resp: TimelinesResponse = run_bridge("list_timelines", None).await?;
    Ok(resp.timelines)
}

#[derive(serde::Deserialize)]
struct ClipsResponse {
    clips: Vec<ResolveClipInfo>,
}

#[tauri::command]
pub async fn resolve_list_media_pool_clips() -> Result<Vec<ResolveClipInfo>, String> {
    let resp: ClipsResponse = run_bridge("list_media_pool_clips", None).await?;
    Ok(resp.clips)
}

#[tauri::command]
pub async fn resolve_import_media(
    paths: Vec<String>,
    timeline_name: Option<String>,
) -> Result<ResolveImportResult, String> {
    let args = serde_json::json!({ "paths": paths, "timeline_name": timeline_name }).to_string();
    run_bridge("import_media", Some(args)).await
}

#[derive(serde::Deserialize)]
struct TimelineClipsResponse {
    clips: Vec<ResolveTimelineClip>,
}

#[tauri::command]
pub async fn resolve_list_timeline_clips() -> Result<Vec<ResolveTimelineClip>, String> {
    let resp: TimelineClipsResponse = run_bridge("list_timeline_clips", None).await?;
    Ok(resp.clips)
}

/// Every mutating timeline-clip command takes the same addressing tuple:
/// `track_type`/`track_index`/`item_index` (Resolve exposes no stable
/// cross-call clip ID) plus `expected_name` -- the bridge re-fetches the
/// item at that position and refuses to act if the name no longer matches,
/// since the list could have changed between when the UI last displayed it
/// and when the user clicked.
#[tauri::command]
pub async fn resolve_set_timeline_clip_enabled(
    track_type: String,
    track_index: i64,
    item_index: i64,
    expected_name: String,
    enabled: bool,
) -> Result<ResolveActionResult, String> {
    let args = serde_json::json!({
        "track_type": track_type,
        "track_index": track_index,
        "item_index": item_index,
        "expected_name": expected_name,
        "enabled": enabled,
    })
    .to_string();
    run_bridge("set_timeline_clip_enabled", Some(args)).await
}

/// **Irreversible via the scripting API** -- Resolve exposes no undo here.
/// The frontend must confirm with the user (native dialog) before ever
/// calling this.
#[tauri::command]
pub async fn resolve_delete_timeline_clip(
    track_type: String,
    track_index: i64,
    item_index: i64,
    expected_name: String,
    ripple: bool,
) -> Result<ResolveActionResult, String> {
    let args = serde_json::json!({
        "track_type": track_type,
        "track_index": track_index,
        "item_index": item_index,
        "expected_name": expected_name,
        "ripple": ripple,
    })
    .to_string();
    run_bridge("delete_timeline_clip", Some(args)).await
}

#[derive(serde::Deserialize)]
struct RenderPresetsResponse {
    presets: Vec<String>,
}

#[tauri::command]
pub async fn resolve_list_render_presets() -> Result<Vec<String>, String> {
    let resp: RenderPresetsResponse = run_bridge("list_render_presets", None).await?;
    Ok(resp.presets)
}

#[tauri::command]
pub async fn resolve_stop_render() -> Result<ResolveActionResult, String> {
    run_bridge("stop_render", None).await
}

/// Spawns `resolve_bridge/resolve_bridge.py render <json_args>` and streams
/// its JSON progress lines as `render://progress` events, mirroring
/// `reindex.rs::run_reindex` exactly -- a render can take minutes, and
/// CLAUDE.md requires progress feedback for long-running media exports, so
/// this is the one Resolve command that doesn't fit `run_bridge`'s
/// single-shot-result pattern.
#[tauri::command]
pub async fn resolve_start_render(
    app: AppHandle,
    preset_name: String,
    target_dir: String,
    custom_name: Option<String>,
) -> Result<(), String> {
    let script = project_root().join("resolve_bridge").join("resolve_bridge.py");
    if !script.exists() {
        let err = format!("Resolve bridge script not found at {}", script.display());
        let _ = app.emit("render://error", &err);
        return Err(err);
    }

    let args = serde_json::json!({
        "preset_name": preset_name,
        "target_dir": target_dir,
        "custom_name": custom_name,
    })
    .to_string();

    let mut child = Command::new(python_command())
        .arg(&script)
        .arg("render")
        .arg(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start render process: {e}"))?;

    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");
    let app_stdout = app.clone();

    let stdout_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        let mut last_error: Option<String> = None;
        let mut saw_done = false;
        while let Ok(Some(line)) = lines.next_line().await {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            match serde_json::from_str::<RenderProgress>(trimmed) {
                Ok(progress) => {
                    if progress.stage == "error" {
                        last_error = progress.message.clone();
                    } else if progress.stage == "done" {
                        saw_done = true;
                    }
                    let _ = app_stdout.emit("render://progress", &progress);
                }
                Err(_) => {
                    last_error = Some(trimmed.to_string());
                }
            }
        }
        (saw_done, last_error)
    });

    let stderr_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        let mut collected = String::new();
        while let Ok(Some(line)) = lines.next_line().await {
            collected.push_str(&line);
            collected.push('\n');
        }
        collected
    });

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Render process failed to run: {e}"))?;
    let (saw_done, last_error) = stdout_task.await.unwrap_or((false, None));
    let stderr_output = stderr_task.await.unwrap_or_default();

    if status.success() && saw_done && last_error.is_none() {
        let _ = app.emit("render://done", ());
        Ok(())
    } else {
        let detail = last_error
            .or_else(|| (!stderr_output.trim().is_empty()).then(|| stderr_output.trim().to_string()))
            .unwrap_or_else(|| "render did not complete successfully".to_string());
        let _ = app.emit("render://error", &detail);
        Err(detail)
    }
}
