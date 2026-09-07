//! Bridge to the (separate, unmodified) B-Roll Analyzer project --
//! `broll_bridge/broll_bridge.py` imports that project's own `analyzer.py`
//! directly (read-only) to score a folder of b-roll clips on technical
//! quality. Chat-triggered: the frontend detects a request like "score the
//! b-roll in this folder", opens a native folder picker, then calls
//! `broll_score_folder` with the chosen path -- no LLM tool-calling
//! involved in this first slice (see PLAN.md for why a deterministic
//! trigger was chosen over trusting the model to decide).
//!
//! Deliberately simpler than the Resolve/Premiere render commands: this
//! blocks on the whole analysis rather than streaming progress events,
//! since proving the "chat text -> external tool -> chat text" pipeline
//! was the goal of this slice, not a polished progress UI.

use crate::commands::premiere::premiere_import_media;
use crate::commands::resolve::resolve_import_media;
use crate::db;
use crate::types::{BrollClipResult, ChatMessage, LastBrollScore, DEFAULT_CHAT_ID};
use crate::AppState;
use std::path::PathBuf;
use std::process::Stdio;
use tauri::State;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// `broll_bridge/` lives one directory above `src-tauri/`, same convention
/// as `resolve_bridge/`/`ingestion/`.
fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri always has a parent directory")
        .to_path_buf()
}

fn bridge_python() -> PathBuf {
    project_root().join("broll_bridge").join(".venv").join("bin").join("python3")
}

fn bridge_script() -> PathBuf {
    project_root().join("broll_bridge").join("broll_bridge.py")
}

#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum BridgeLine {
    Progress {
        #[allow(dead_code)]
        current: i64,
        #[allow(dead_code)]
        total: i64,
        #[allow(dead_code)]
        file: String,
    },
    Result {
        clips: Vec<BrollClipResult>,
    },
    Error {
        message: String,
    },
}

async fn run_broll_bridge(analyzer_dir: &str, folder: &str) -> Result<Vec<BrollClipResult>, String> {
    let python = bridge_python();
    if !python.exists() {
        return Err(format!(
            "The b-roll bridge isn't set up yet -- see broll_bridge/README.md ({} not found)",
            python.display()
        ));
    }

    let args = serde_json::json!({ "analyzer_dir": analyzer_dir, "folder": folder }).to_string();
    let mut child = Command::new(&python)
        .arg(bridge_script())
        .arg(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start the b-roll bridge: {e}"))?;

    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");

    let stdout_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        let mut clips: Option<Vec<BrollClipResult>> = None;
        let mut error: Option<String> = None;
        while let Ok(Some(line)) = lines.next_line().await {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            match serde_json::from_str::<BridgeLine>(trimmed) {
                Ok(BridgeLine::Result { clips: c }) => clips = Some(c),
                Ok(BridgeLine::Error { message }) => error = Some(message),
                Ok(BridgeLine::Progress { .. }) => {}
                Err(_) => {}
            }
        }
        (clips, error)
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

    let status = child.wait().await.map_err(|e| format!("The b-roll bridge failed to run: {e}"))?;
    let (clips, bridge_error) = stdout_task.await.unwrap_or((None, None));
    let stderr_output = stderr_task.await.unwrap_or_default();

    if let Some(msg) = bridge_error {
        return Err(msg);
    }
    if !status.success() {
        let detail =
            if !stderr_output.trim().is_empty() { stderr_output.trim().to_string() } else { "no error output".to_string() };
        return Err(format!("The b-roll bridge exited with {status}: {detail}"));
    }
    clips.ok_or_else(|| "The b-roll bridge produced no result".to_string())
}

fn format_summary(folder: &str, clips: &[BrollClipResult]) -> String {
    if clips.is_empty() {
        return format!("No video clips found in `{folder}`.");
    }

    let mut sorted: Vec<&BrollClipResult> = clips.iter().collect();
    sorted.sort_by(|a, b| b.overall_score.partial_cmp(&a.overall_score).unwrap_or(std::cmp::Ordering::Equal));

    let mut lines = vec![format!(
        "Scored {} clip{} in `{folder}` on technical quality (sharpness/exposure/stability):\n",
        clips.len(),
        if clips.len() == 1 { "" } else { "s" }
    )];
    for clip in sorted.iter().take(10) {
        if let Some(err) = &clip.error {
            lines.push(format!("- **{}** — couldn't analyze ({err})", clip.filename));
        } else {
            lines.push(format!(
                "- **{}** — {:.0}/100, best segment {:.1}s–{:.1}s",
                clip.filename, clip.overall_score, clip.best_window_start, clip.best_window_end
            ));
        }
    }
    if sorted.len() > 10 {
        lines.push(format!("...and {} more.", sorted.len() - 10));
    }
    lines.join("\n")
}

/// Persists the triggering chat text as a user message (mirroring
/// `ask_question`'s own persistence, so this looks like a normal chat
/// turn in history), runs the analysis, and persists+returns the
/// assistant's summary -- including on failure (missing config, bridge
/// error), so the chat always gets a reply rather than a raw thrown error.
#[tauri::command]
pub async fn broll_score_folder(state: State<'_, AppState>, text: String, folder: String) -> Result<ChatMessage, String> {
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::insert_message(&conn, DEFAULT_CHAT_ID, "user", &text, false, &[]).map_err(|e| e.to_string())?;
    }

    let analyzer_dir = state.config.read().map_err(|e| e.to_string())?.broll_analyzer_path.clone();

    let answer = match analyzer_dir {
        None => "Set the B-Roll Analyzer path in Settings first, then try again.".to_string(),
        Some(dir) => match run_broll_bridge(&dir, &folder).await {
            Ok(clips) => {
                let summary = format_summary(&folder, &clips);
                *state.last_broll.lock().map_err(|e| e.to_string())? =
                    Some(LastBrollScore { folder: folder.clone(), clips });
                summary
            }
            Err(err) => format!("Couldn't score that folder: {err}"),
        },
    };

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::insert_message(&conn, DEFAULT_CHAT_ID, "assistant", &answer, false, &[]).map_err(|e| e.to_string())
}

/// Shared by `broll_send_to_resolve`/`broll_send_to_premiere`: the
/// `count` highest-`overall_score` clips from a scored batch, skipping any
/// that failed to analyze.
fn top_clips(clips: &[BrollClipResult], count: i64) -> Vec<&BrollClipResult> {
    let mut ranked: Vec<&BrollClipResult> = clips.iter().filter(|c| c.error.is_none()).collect();
    ranked.sort_by(|a, b| b.overall_score.partial_cmp(&a.overall_score).unwrap_or(std::cmp::Ordering::Equal));
    ranked.into_iter().take(count.max(1) as usize).collect()
}

/// Closes the loop opened by `broll_score_folder`: takes the `count`
/// highest-`overall_score` clips from the *last* scored folder (kept in
/// `AppState::last_broll`, not re-scored or re-picked here) and imports
/// them into DaVinci Resolve via `resolve_import_media` -- the exact same
/// bridge call Settings' own "Import Footage" panel uses. Same deterministic
/// chat-trigger philosophy as `broll_score_folder`: `useChat.ts` matches on
/// text before this is ever called, no LLM tool-calling involved.
#[tauri::command]
pub async fn broll_send_to_resolve(
    state: State<'_, AppState>,
    text: String,
    count: i64,
    timeline_name: Option<String>,
) -> Result<ChatMessage, String> {
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::insert_message(&conn, DEFAULT_CHAT_ID, "user", &text, false, &[]).map_err(|e| e.to_string())?;
    }

    let last = state.last_broll.lock().map_err(|e| e.to_string())?.clone();

    let answer = match last {
        None => {
            "I haven't scored any b-roll yet this session -- ask me to score a folder first, \
             then ask me to send the top clips to Resolve."
                .to_string()
        }
        Some(LastBrollScore { folder, clips }) => {
            let picked = top_clips(&clips, count);

            if picked.is_empty() {
                format!("None of the clips scored in `{folder}` analyzed cleanly enough to send.")
            } else {
                let paths: Vec<String> = picked.iter().map(|c| c.path.clone()).collect();
                let name = timeline_name.or_else(|| Some("B-Roll Picks".to_string()));
                match resolve_import_media(paths, name).await {
                    Ok(result) if result.ok => {
                        let mut lines = vec![format!(
                            "Sent the top {} clip{} from `{folder}` to Resolve -- {}",
                            picked.len(),
                            if picked.len() == 1 { "" } else { "s" },
                            result.message
                        )];
                        for clip in &picked {
                            lines.push(format!("- **{}** — {:.0}/100", clip.filename, clip.overall_score));
                        }
                        lines.join("\n")
                    }
                    Ok(result) => format!("Resolve couldn't import those clips: {}", result.message),
                    Err(err) => format!("Couldn't reach Resolve: {err}"),
                }
            }
        }
    };

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::insert_message(&conn, DEFAULT_CHAT_ID, "assistant", &answer, false, &[]).map_err(|e| e.to_string())
}

/// Same idea as `broll_send_to_resolve`, targeting Premiere Pro instead --
/// imports via the UXP bridge's `premiere_import_media` (Phase 2 step 4),
/// which mirrors `resolve_import_media`'s import+optional-sequence-creation
/// shape exactly.
#[tauri::command]
pub async fn broll_send_to_premiere(
    state: State<'_, AppState>,
    text: String,
    count: i64,
    sequence_name: Option<String>,
) -> Result<ChatMessage, String> {
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::insert_message(&conn, DEFAULT_CHAT_ID, "user", &text, false, &[]).map_err(|e| e.to_string())?;
    }

    let last = state.last_broll.lock().map_err(|e| e.to_string())?.clone();

    let answer = match last {
        None => {
            "I haven't scored any b-roll yet this session -- ask me to score a folder first, \
             then ask me to send the top clips to Premiere."
                .to_string()
        }
        Some(LastBrollScore { folder, clips }) => {
            let picked = top_clips(&clips, count);

            if picked.is_empty() {
                format!("None of the clips scored in `{folder}` analyzed cleanly enough to send.")
            } else {
                let paths: Vec<String> = picked.iter().map(|c| c.path.clone()).collect();
                let name = sequence_name.or_else(|| Some("B-Roll Picks".to_string()));
                match premiere_import_media(paths, name).await {
                    Ok(result) if result.ok => {
                        let mut lines = vec![format!(
                            "Sent the top {} clip{} from `{folder}` to Premiere -- {}",
                            picked.len(),
                            if picked.len() == 1 { "" } else { "s" },
                            result.message
                        )];
                        for clip in &picked {
                            lines.push(format!("- **{}** — {:.0}/100", clip.filename, clip.overall_score));
                        }
                        lines.join("\n")
                    }
                    Ok(result) => format!("Premiere couldn't import those clips: {}", result.message),
                    Err(err) => format!("Couldn't reach Premiere: {err}"),
                }
            }
        }
    };

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::insert_message(&conn, DEFAULT_CHAT_ID, "assistant", &answer, false, &[]).map_err(|e| e.to_string())
}
