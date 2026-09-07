use serde::{Deserialize, Serialize};

pub const DEFAULT_CHAT_ID: i64 = 1;
pub const EMBEDDING_DIMENSIONS: usize = 768;

/// One clip's technical-quality score from the (separate, unmodified)
/// B-Roll Analyzer project's `analyzer.py`, via `broll_bridge/`. See
/// `commands::broll`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrollClipResult {
    pub path: String,
    pub filename: String,
    pub duration: f64,
    pub fps: f64,
    pub width: i64,
    pub height: i64,
    pub overall_score: f64,
    pub best_window_start: f64,
    pub best_window_end: f64,
    pub error: Option<String>,
}

/// The most recent successful `broll_score_folder` result, kept in
/// `AppState` (in-memory only, not persisted to `dume.db`) so a follow-up
/// chat message like "send the top 3 to Resolve" has something to act on
/// without re-scoring or re-picking a folder. Resets on app restart --
/// acceptable for this first slice, same scoping discipline as the rest of
/// Phase 2.
#[derive(Debug, Clone)]
pub struct LastBrollScore {
    pub folder: String,
    pub clips: Vec<BrollClipResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Citation {
    Manual {
        title: String,
        page_start: i64,
        page_end: i64,
    },
    Web {
        title: String,
        url: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: i64,
    pub chat_id: i64,
    pub role: String,
    pub content: String,
    pub used_fallback: bool,
    pub citations: Vec<Citation>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RetrievedChunk {
    pub chunk_id: i64,
    pub manual_title: String,
    pub product: String,
    pub page_start: i64,
    pub page_end: i64,
    pub text: String,
    pub similarity: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexStats {
    pub manual_count: i64,
    pub chunk_count: i64,
    pub last_indexed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelStatus {
    pub name: String,
    pub available: bool,
    pub loaded_in_memory: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AboutInfo {
    pub app_version: String,
    pub ollama_reachable: bool,
    pub chat_model: ModelStatus,
    pub embedding_model: ModelStatus,
    pub index_stats: IndexStats,
    pub fallback_enabled: bool,
    pub searxng_reachable: bool,
    pub resolve_reachable: bool,
    pub premiere_reachable: bool,
    pub blender_reachable: bool,
}

/// Emitted repeatedly on the `chat://token` event while an answer streams in.
#[derive(Debug, Clone, Serialize)]
pub struct ChatTokenEvent {
    pub chat_id: i64,
    pub delta: String,
}

/// Emitted once on `chat://done` when a full answer (with citations) is ready.
#[derive(Debug, Clone, Serialize)]
pub struct ChatDoneEvent {
    pub message: ChatMessage,
}

/// Emitted on `chat://error` if answering fails outright.
#[derive(Debug, Clone, Serialize)]
pub struct ChatErrorEvent {
    pub chat_id: i64,
    pub error: String,
}

/// Emitted repeatedly on `reindex://progress` while ingestion runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReindexProgress {
    pub stage: String,
    pub manual: Option<String>,
    pub chunk: Option<i64>,
    pub total_chunks: Option<i64>,
    pub message: Option<String>,
}

/// Whether DaVinci Resolve is running and reachable via its local scripting
/// API (see `resolve_bridge/resolve_bridge.py`'s `status` command).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveStatus {
    pub running: bool,
    pub product_name: Option<String>,
    pub version: Option<String>,
    pub current_page: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveTimelineInfo {
    pub name: String,
    pub fps: String,
    pub start_timecode: String,
    pub current_timecode: Option<String>,
    pub video_tracks: i64,
    pub audio_tracks: i64,
}

/// The currently open project/timeline in Resolve, if any (see
/// `resolve_bridge/resolve_bridge.py`'s `project` command).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveProjectInfo {
    pub has_project: bool,
    pub project_name: Option<String>,
    pub timeline_count: Option<i64>,
    pub current_timeline: Option<ResolveTimelineInfo>,
}

/// Result of a Resolve write action (currently just adding a marker at the
/// playhead) -- see `resolve_bridge/resolve_bridge.py`'s `add_marker` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveActionResult {
    pub ok: bool,
    pub message: String,
}

/// One timeline in the current project (see `resolve_bridge/resolve_bridge.py`'s
/// `list_timelines` command). `index` is 1-based, matching Resolve's own
/// `GetTimelineByIndex` convention.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveTimelineSummary {
    pub name: String,
    pub index: i64,
}

/// One clip anywhere in the media pool (see
/// `resolve_bridge/resolve_bridge.py`'s `list_media_pool_clips` command,
/// which now recurses into subfolders). `folder_path` is empty for
/// root-level clips, e.g. "Interviews" for one level deep.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveClipInfo {
    pub name: String,
    pub file_name: Option<String>,
    pub frames: Option<String>,
    pub folder_path: String,
}

/// One clip on the current timeline (see `resolve_bridge/resolve_bridge.py`'s
/// `list_timeline_clips` command). `track_index` and `item_index` (position
/// within that track, 0-based) are how mutating commands
/// (`set_timeline_clip_enabled`/`delete_timeline_clip`) address a specific
/// clip -- Resolve's scripting API exposes no stable cross-call clip ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveTimelineClip {
    pub name: String,
    pub track_type: String,
    pub track_index: i64,
    pub item_index: i64,
    pub start: i64,
    pub end: i64,
    pub enabled: bool,
}

/// One line of progress from a running render -- see
/// `resolve_bridge/resolve_bridge.py`'s `render` command, which streams these
/// (mirroring `ReindexProgress`/`reindex.rs`) since renders can run long and
/// CLAUDE.md requires progress feedback for long-running media exports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderProgress {
    pub stage: String,
    pub job_id: Option<String>,
    pub job_status: Option<String>,
    pub completion_percentage: Option<i64>,
    pub message: Option<String>,
}

/// Result of importing media (and optionally building a timeline from it) --
/// see `resolve_bridge/resolve_bridge.py`'s `import_media` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveImportResult {
    pub ok: bool,
    pub message: String,
    pub imported_count: i64,
    pub timeline_created: bool,
}

/// Live status from the Dum-E Bridge UXP plugin running inside Premiere Pro
/// (see `premiere_plugin/index.js` and `commands/premiere.rs`). Unlike
/// Resolve, there's no subprocess to call -- `connected: false` means the
/// plugin isn't loaded/running (no response file appeared before the
/// timeout), not an error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PremiereStatus {
    pub connected: bool,
    pub app_version: Option<String>,
    pub active_project_name: Option<String>,
}

/// Result of a Premiere write action (currently just adding a marker at the
/// playhead) -- see `premiere_plugin/index.js`'s `add_marker` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PremiereActionResult {
    pub ok: bool,
    pub message: String,
}

/// Result of importing footage into Premiere's project panel, and
/// optionally building a new sequence from exactly what was imported --
/// see `premiere_plugin/index.js`'s `import_media` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PremiereImportResult {
    pub ok: bool,
    pub message: String,
    pub imported_count: i64,
    pub sequence_created: bool,
}

/// One clip on the active sequence (see `premiere_plugin/index.js`'s
/// `list_timeline_clips` command). `track_index`/`item_index` are how
/// `premiere_set_timeline_clip_enabled` addresses a specific clip --
/// Premiere's scripting API exposes no stable cross-call clip ID, same
/// situation as Resolve's equivalent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PremiereTimelineClip {
    pub name: String,
    pub track_type: String,
    pub track_index: i64,
    pub item_index: i64,
    pub enabled: bool,
}

/// Result of a render -- see `premiere_plugin/index.js`'s `render` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PremiereRenderResult {
    pub ok: bool,
    pub message: String,
    pub output_file: Option<String>,
}

/// Live status from the Dum-E Bridge add-on running inside Blender (see
/// `blender_bridge/dume_bridge.py` and `commands/blender.rs`). Same
/// file-based bridge shape as `PremiereStatus` -- `running: false` means
/// the add-on isn't installed/enabled, or Blender isn't open, not an error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlenderStatus {
    pub running: bool,
    pub version: Option<String>,
    pub file_path: Option<String>,
    pub scene_name: Option<String>,
}

/// Scene details for the currently open .blend file -- see
/// `blender_bridge/dume_bridge.py`'s `scene_info` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlenderSceneInfo {
    pub scene_name: String,
    pub frame_start: i64,
    pub frame_end: i64,
    pub frame_current: i64,
    pub fps: i64,
    pub render_engine: String,
    pub object_count: i64,
}

/// Result of a Blender write action (currently just adding a timeline
/// marker at the current frame) -- see `blender_bridge/dume_bridge.py`'s
/// `add_marker` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlenderActionResult {
    pub ok: bool,
    pub message: String,
}

/// Result of importing footage into Blender's VSE, and optionally building
/// a new Scene from exactly what was imported (the closest Blender
/// equivalent to a new Resolve timeline or Premiere sequence) -- see
/// `blender_bridge/dume_bridge.py`'s `import_media` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlenderImportResult {
    pub ok: bool,
    pub message: String,
    pub imported_count: i64,
    pub scene_created: bool,
}
