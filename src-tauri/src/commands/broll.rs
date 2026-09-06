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

use crate::db;
use crate::types::{BrollClipResult, ChatMessage, DEFAULT_CHAT_ID};
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
            Ok(clips) => format_summary(&folder, &clips),
            Err(err) => format!("Couldn't score that folder: {err}"),
        },
    };

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::insert_message(&conn, DEFAULT_CHAT_ID, "assistant", &answer, false, &[]).map_err(|e| e.to_string())
}
