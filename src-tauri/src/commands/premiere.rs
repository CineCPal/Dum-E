//! Bridge to the Dum-E Bridge UXP plugin running inside Adobe Premiere Pro
//! (`premiere_plugin/`). Unlike DaVinci Resolve, Premiere has no locally
//! callable scripting module -- automation has to run as a plugin loaded
//! *inside* Premiere, so there's no subprocess to spawn here. Instead this
//! polls a shared directory of request/response JSON files that the plugin
//! is independently polling too (see `premiere_plugin/index.js`).
//!
//! UXP's network permission model has real, documented problems with
//! localhost/IP connections (an open Adobe GitHub issue reports exactly
//! this failing), so a WebSocket bridge was ruled out in favor of this
//! simpler, better-precedented file-based one. See PLAN.md for the research
//! behind that choice.

use crate::types::{
    PremiereActionResult, PremiereImportResult, PremiereRenderResult, PremiereStatus, PremiereTimelineClip,
};
use serde::de::DeserializeOwned;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;

/// Fixed, literal path -- deliberately NOT under the user's home directory,
/// since resolving "~" from the UXP plugin's sandboxed JS environment isn't
/// reliably available. Both sides hardcode this exact same path.
const BRIDGE_DIR: &str = "/tmp/dume_premiere_bridge";
const POLL_INTERVAL: Duration = Duration::from_millis(150);
const TIMEOUT: Duration = Duration::from_secs(5);
/// Used only for the passive reachability check in `get_about_info` -- long
/// enough to catch a couple of the plugin's own 500ms poll cycles, but far
/// short of the full TIMEOUT so opening the About dialog doesn't stall for
/// 5 seconds every time the plugin just isn't loaded (the common case).
const REACHABILITY_TIMEOUT: Duration = Duration::from_millis(750);
/// `render`'s EXPORT_IMMEDIATELY blocks the plugin's response until the
/// whole export finishes (see `render` in `premiere_plugin/index.js`) --
/// long enough for a real export, not just the usual instant point query.
const RENDER_TIMEOUT: Duration = Duration::from_secs(60 * 60);

fn bridge_dir() -> PathBuf {
    PathBuf::from(BRIDGE_DIR)
}

/// Writes `request.json`, polls for the matching `response-<id>.json` up to
/// `TIMEOUT`, and returns its parsed `data` field. Returns `Ok(None)` (not an
/// error) if nothing responds in time -- the plugin isn't loaded/running,
/// mirroring how the Resolve bridge treats "app not running" as a normal,
/// clean state rather than a failure.
async fn call_plugin<T: DeserializeOwned>(
    command: &str,
    args: Option<serde_json::Value>,
    timeout: Duration,
) -> Result<Option<T>, String> {
    let dir = bridge_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't create bridge directory: {e}"))?;

    // No real concurrency to worry about here (Dum-E only ever has one of
    // these calls in flight at a time from the UI), so a timestamp+pid pair
    // is unique enough without pulling in a uuid crate.
    let id = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );

    let request = serde_json::json!({ "id": id, "command": command, "args": args }).to_string();
    let request_path = dir.join("request.json");
    std::fs::write(&request_path, request).map_err(|e| format!("Couldn't write request file: {e}"))?;

    let response_path = dir.join(format!("response-{id}.json"));
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        if response_path.exists() {
            let raw = std::fs::read_to_string(&response_path)
                .map_err(|e| format!("Couldn't read response file: {e}"))?;
            let _ = std::fs::remove_file(&response_path);

            #[derive(serde::Deserialize)]
            struct Envelope<T> {
                ok: bool,
                data: Option<T>,
                error: Option<String>,
            }
            let envelope: Envelope<T> = serde_json::from_str(&raw)
                .map_err(|e| format!("Couldn't parse plugin response: {e} ({raw})"))?;
            return if envelope.ok {
                Ok(envelope.data)
            } else {
                Err(envelope.error.unwrap_or_else(|| "Plugin reported an error".to_string()))
            };
        }
        sleep(POLL_INTERVAL).await;
    }

    // Timed out -- clean up the request file so a late-arriving plugin
    // doesn't process a stale request; the plugin not being loaded/running
    // is the expected case here, not a bug.
    let _ = std::fs::remove_file(&request_path);
    Ok(None)
}

#[derive(serde::Deserialize)]
struct GetStatusData {
    app_version: Option<String>,
    active_project_name: Option<String>,
}

#[tauri::command]
pub async fn premiere_status() -> Result<PremiereStatus, String> {
    match call_plugin::<GetStatusData>("get_status", None, TIMEOUT).await? {
        Some(data) => Ok(PremiereStatus {
            connected: true,
            app_version: data.app_version,
            active_project_name: data.active_project_name,
        }),
        None => Ok(PremiereStatus { connected: false, app_version: None, active_project_name: None }),
    }
}

/// Same call as `premiere_status` but with a short timeout, for passive
/// reachability checks (`get_about_info`) where the plugin is expected to
/// already be idle-polling if it's connected at all -- not exposed as a
/// Tauri command since it's an internal detail of that check, not something
/// the frontend calls directly.
pub async fn premiere_reachable() -> bool {
    call_plugin::<GetStatusData>("get_status", None, REACHABILITY_TIMEOUT)
        .await
        .ok()
        .flatten()
        .is_some()
}

/// First write action for the Premiere bridge, mirroring the Resolve
/// bridge's own first step: one safe, easily-undoable action (a marker at
/// the playhead) to prove out write commands before building anything more
/// ambitious. `color_index` maps to `Constants.MarkerColor` (0=green,
/// 1=red, 2=magenta, 3=orange, 4=yellow, 5=blue, 6=cyan) -- `Markers
/// .createAddMarkerAction` itself has no color parameter, so the plugin
/// sets it as a second transaction against whichever marker now matches
/// this one's name+start time (see `premiere_plugin/index.js`).
#[tauri::command]
pub async fn premiere_add_marker(
    name: String,
    note: String,
    color_index: Option<i64>,
) -> Result<PremiereActionResult, String> {
    let args = serde_json::json!({ "name": name, "note": note, "color_index": color_index });
    match call_plugin::<bool>("add_marker", Some(args), TIMEOUT).await? {
        Some(_) => Ok(PremiereActionResult { ok: true, message: format!("Added marker \"{name}\" at the playhead") }),
        None => Ok(PremiereActionResult {
            ok: false,
            message: "Dum-E Bridge panel isn't loaded in Premiere".to_string(),
        }),
    }
}

#[derive(serde::Deserialize)]
struct ImportMediaData {
    imported_count: i64,
    sequence_created: bool,
}

/// Second write action, mirroring the Resolve bridge's own step 2: import
/// footage into the project panel and optionally build a new sequence from
/// exactly what was imported, in one call (see `import_media` in
/// `premiere_plugin/index.js` for why import and sequence creation can't
/// be split across two separate bridge calls).
#[tauri::command]
pub async fn premiere_import_media(
    paths: Vec<String>,
    sequence_name: Option<String>,
) -> Result<PremiereImportResult, String> {
    let args = serde_json::json!({ "paths": paths, "sequence_name": sequence_name });
    match call_plugin::<ImportMediaData>("import_media", Some(args), TIMEOUT).await? {
        Some(data) => {
            let message = if data.sequence_created {
                format!(
                    "Imported {} clip{} and created sequence \"{}\"",
                    data.imported_count,
                    if data.imported_count == 1 { "" } else { "s" },
                    sequence_name.unwrap_or_default()
                )
            } else {
                format!("Imported {} clip{}", data.imported_count, if data.imported_count == 1 { "" } else { "s" })
            };
            Ok(PremiereImportResult {
                ok: true,
                message,
                imported_count: data.imported_count,
                sequence_created: data.sequence_created,
            })
        }
        None => Ok(PremiereImportResult {
            ok: false,
            message: "Dum-E Bridge panel isn't loaded in Premiere".to_string(),
            imported_count: 0,
            sequence_created: false,
        }),
    }
}

#[derive(serde::Deserialize)]
struct TimelineClipsData {
    clips: Vec<PremiereTimelineClip>,
}

#[tauri::command]
pub async fn premiere_list_timeline_clips() -> Result<Vec<PremiereTimelineClip>, String> {
    match call_plugin::<TimelineClipsData>("list_timeline_clips", None, TIMEOUT).await? {
        Some(data) => Ok(data.clips),
        None => Ok(Vec::new()),
    }
}

/// Mirrors Resolve's own timeline-clip commands: no stable cross-call clip
/// ID exists here either, so every mutating call addresses a clip by
/// position (`track_type`/`track_index`/`item_index`) plus `expected_name`,
/// which the plugin re-verifies against a fresh lookup before acting.
/// Unlike Resolve, Premiere's scripting API exposes no way to delete a
/// track item at all (no `createRemove*Action` on any track-item type) --
/// enable/disable is the only mutation available here, so that's this
/// command's ceiling, not an arbitrary scope cut. See PLAN.md.
#[tauri::command]
pub async fn premiere_set_timeline_clip_enabled(
    track_type: String,
    track_index: i64,
    item_index: i64,
    expected_name: String,
    enabled: bool,
) -> Result<PremiereActionResult, String> {
    let args = serde_json::json!({
        "track_type": track_type,
        "track_index": track_index,
        "item_index": item_index,
        "expected_name": expected_name,
        "enabled": enabled,
    });
    match call_plugin::<bool>("set_timeline_clip_enabled", Some(args), TIMEOUT).await? {
        Some(_) => Ok(PremiereActionResult {
            ok: true,
            message: format!("{} \"{expected_name}\"", if enabled { "Enabled" } else { "Disabled" }),
        }),
        None => Ok(PremiereActionResult {
            ok: false,
            message: "Dum-E Bridge panel isn't loaded in Premiere".to_string(),
        }),
    }
}

#[derive(serde::Deserialize)]
struct RenderData {
    output_file: String,
}

/// Deliberately simpler than Resolve's `resolve_start_render`: this blocks
/// on the plugin's response for the whole export (`RENDER_TIMEOUT`) instead
/// of streaming progress events over a long-running subprocess, since the
/// file-based bridge has no equivalent channel for the plugin to push
/// updates back through between polls. `EXPORT_IMMEDIATELY` makes Premiere
/// show its own native export progress UI in the meantime, which is where
/// CLAUDE.md's progress-feedback mandate actually gets satisfied here.
#[tauri::command]
pub async fn premiere_start_render(
    preset_file: String,
    target_dir: String,
    custom_name: Option<String>,
) -> Result<PremiereRenderResult, String> {
    let args = serde_json::json!({
        "preset_file": preset_file,
        "target_dir": target_dir,
        "custom_name": custom_name,
    });
    match call_plugin::<RenderData>("render", Some(args), RENDER_TIMEOUT).await? {
        Some(data) => Ok(PremiereRenderResult {
            ok: true,
            message: format!("Rendered to {}", data.output_file),
            output_file: Some(data.output_file),
        }),
        None => Ok(PremiereRenderResult {
            ok: false,
            message: "Dum-E Bridge panel isn't loaded in Premiere".to_string(),
            output_file: None,
        }),
    }
}
