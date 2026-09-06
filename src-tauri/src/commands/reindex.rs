use crate::types::ReindexProgress;
use std::path::PathBuf;
use std::process::Stdio;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Resolved at compile time from this crate's location, since `ingestion/`
/// and `books/` live one directory above `src-tauri/`. This holds for the
/// `npm run tauri dev` workflow this MVP targets; bundling these as app
/// resources for a distributable build is a follow-up (see README).
fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri always has a parent directory")
        .to_path_buf()
}

fn python_command() -> String {
    std::env::var("DUME_PYTHON").unwrap_or_else(|_| "python3".to_string())
}

/// Spawns `ingestion/ingest.py` as a child process, streaming each JSON
/// progress line it prints as a `reindex://progress` event, then emits
/// `reindex://done` (success) or `reindex://error` (failure).
#[tauri::command]
pub async fn run_reindex(app: AppHandle) -> Result<(), String> {
    let root = project_root();
    let script = root.join("ingestion").join("ingest.py");
    let books_dir = root.join("books");
    let db_path = crate::config::db_path(&app).map_err(|e| e.to_string())?;

    if !script.exists() {
        let err = format!("Ingestion script not found at {}", script.display());
        let _ = app.emit("reindex://error", &err);
        return Err(err);
    }

    let mut child = Command::new(python_command())
        .arg(&script)
        .arg("--books-dir")
        .arg(&books_dir)
        .arg("--db-path")
        .arg(&db_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start ingestion process: {e}"))?;

    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");
    let app_stdout = app.clone();

    let stdout_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        let mut last_error: Option<String> = None;
        while let Ok(Some(line)) = lines.next_line().await {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            match serde_json::from_str::<ReindexProgress>(trimmed) {
                Ok(progress) => {
                    let _ = app_stdout.emit("reindex://progress", &progress);
                }
                Err(_) => {
                    last_error = Some(trimmed.to_string());
                }
            }
        }
        last_error
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
        .map_err(|e| format!("Ingestion process failed to run: {e}"))?;
    let last_non_json_line = stdout_task.await.unwrap_or(None);
    let stderr_output = stderr_task.await.unwrap_or_default();

    if status.success() {
        let _ = app.emit("reindex://done", ());
        Ok(())
    } else {
        let detail = if !stderr_output.trim().is_empty() {
            stderr_output.trim().to_string()
        } else {
            last_non_json_line.unwrap_or_else(|| "no error output".to_string())
        };
        let err = format!("Ingestion exited with {status}: {detail}");
        let _ = app.emit("reindex://error", &err);
        Err(err)
    }
}
