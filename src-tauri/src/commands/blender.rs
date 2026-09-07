//! Bridge to the Dum-E Bridge add-on running inside Blender
//! (`blender_bridge/dume_bridge.py`). Same file-based request/response
//! polling protocol as `commands::premiere`, for the same reason: bpy is
//! only callable from *inside* Blender's own embedded Python -- there's no
//! externally-attachable scripting module like DaVinci Resolve ships, so
//! there's no subprocess to spawn here either.

use crate::types::{BlenderActionResult, BlenderImportResult, BlenderSceneInfo, BlenderStatus};
use serde::de::DeserializeOwned;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;

/// Fixed, literal path -- deliberately NOT under the user's home directory,
/// same reasoning as the Premiere bridge. Both sides (this file and
/// `blender_bridge/dume_bridge.py`) hardcode this exact same path.
const BRIDGE_DIR: &str = "/tmp/dume_blender_bridge";
const POLL_INTERVAL: Duration = Duration::from_millis(150);
const TIMEOUT: Duration = Duration::from_secs(5);
/// Used only for the passive reachability check in `get_about_info` -- see
/// `commands::premiere::REACHABILITY_TIMEOUT` for why this needs to be much
/// shorter than `TIMEOUT`.
const REACHABILITY_TIMEOUT: Duration = Duration::from_millis(750);

fn bridge_dir() -> PathBuf {
    PathBuf::from(BRIDGE_DIR)
}

/// Writes `request.json`, polls for the matching `response-<id>.json` up to
/// `timeout`, and returns its parsed `data` field. Returns `Ok(None)` (not
/// an error) if nothing responds in time -- the add-on isn't
/// installed/enabled, or Blender isn't running, mirroring how the
/// Resolve/Premiere bridges treat "app not running" as a normal, clean
/// state rather than a failure.
async fn call_bridge<T: DeserializeOwned>(
    command: &str,
    args: Option<serde_json::Value>,
    timeout: Duration,
) -> Result<Option<T>, String> {
    let dir = bridge_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't create bridge directory: {e}"))?;

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
                .map_err(|e| format!("Couldn't parse add-on response: {e} ({raw})"))?;
            return if envelope.ok {
                Ok(envelope.data)
            } else {
                Err(envelope.error.unwrap_or_else(|| "Add-on reported an error".to_string()))
            };
        }
        sleep(POLL_INTERVAL).await;
    }

    // Timed out -- clean up the request file so a late-arriving add-on
    // doesn't process a stale request; Blender not running/the add-on not
    // being enabled is the expected case here, not a bug.
    let _ = std::fs::remove_file(&request_path);
    Ok(None)
}

#[derive(serde::Deserialize)]
struct StatusData {
    version: Option<String>,
    file_path: Option<String>,
    scene_name: Option<String>,
}

#[tauri::command]
pub async fn blender_status() -> Result<BlenderStatus, String> {
    match call_bridge::<StatusData>("status", None, TIMEOUT).await? {
        Some(data) => Ok(BlenderStatus {
            running: true,
            version: data.version,
            file_path: data.file_path,
            scene_name: data.scene_name,
        }),
        None => Ok(BlenderStatus { running: false, version: None, file_path: None, scene_name: None }),
    }
}

/// Same call as `blender_status` but with a short timeout, for passive
/// reachability checks (`get_about_info`) -- see
/// `commands::premiere::premiere_reachable`, which this mirrors.
pub async fn blender_reachable() -> bool {
    call_bridge::<StatusData>("status", None, REACHABILITY_TIMEOUT).await.ok().flatten().is_some()
}

#[tauri::command]
pub async fn blender_scene_info() -> Result<Option<BlenderSceneInfo>, String> {
    call_bridge("scene_info", None, TIMEOUT).await
}

#[derive(serde::Deserialize)]
struct AddMarkerData {
    frame: i64,
}

/// First write action for the Blender bridge, mirroring the Resolve and
/// Premiere bridges' own first steps: one safe, easily-undoable action.
/// No color parameter -- Blender's timeline markers don't have one (unlike
/// Resolve's 16 or Premiere's 7), just a name and a frame.
#[tauri::command]
pub async fn blender_add_marker(name: String) -> Result<BlenderActionResult, String> {
    let args = serde_json::json!({ "name": name });
    match call_bridge::<AddMarkerData>("add_marker", Some(args), TIMEOUT).await? {
        Some(data) => {
            Ok(BlenderActionResult { ok: true, message: format!("Added marker \"{name}\" at frame {}", data.frame) })
        }
        None => Ok(BlenderActionResult {
            ok: false,
            message: "Dum-E Bridge add-on isn't enabled in Blender".to_string(),
        }),
    }
}

#[derive(serde::Deserialize)]
struct ImportMediaData {
    imported_count: i64,
    scene_created: bool,
    scene_name: String,
}

/// Imports footage into Blender's VSE, mirroring the Resolve/Premiere
/// bridges' own `import_media` -- a `scene_name` optionally builds a new
/// Scene from exactly what was imported, the closest Blender equivalent to
/// a new timeline/sequence.
#[tauri::command]
pub async fn blender_import_media(paths: Vec<String>, scene_name: Option<String>) -> Result<BlenderImportResult, String> {
    let args = serde_json::json!({ "paths": paths, "scene_name": scene_name });
    match call_bridge::<ImportMediaData>("import_media", Some(args), TIMEOUT).await? {
        Some(data) => {
            let message = if data.scene_created {
                format!(
                    "Imported {} clip{} and created scene \"{}\"",
                    data.imported_count,
                    if data.imported_count == 1 { "" } else { "s" },
                    data.scene_name
                )
            } else {
                format!("Imported {} clip{}", data.imported_count, if data.imported_count == 1 { "" } else { "s" })
            };
            Ok(BlenderImportResult { ok: true, message, imported_count: data.imported_count, scene_created: data.scene_created })
        }
        None => Ok(BlenderImportResult {
            ok: false,
            message: "Dum-E Bridge add-on isn't enabled in Blender".to_string(),
            imported_count: 0,
            scene_created: false,
        }),
    }
}
