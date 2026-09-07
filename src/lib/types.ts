export type Citation =
  | { kind: "manual"; title: string; page_start: number; page_end: number }
  | { kind: "web"; title: string; url: string };

export interface ChatMessage {
  id: number;
  chat_id: number;
  role: "user" | "assistant" | string;
  content: string;
  used_fallback: boolean;
  citations: Citation[];
  created_at: string;
}

export interface IndexStats {
  manual_count: number;
  chunk_count: number;
  last_indexed_at: string | null;
}

export interface ModelStatus {
  name: string;
  available: boolean;
  loaded_in_memory: boolean;
}

export interface AboutInfo {
  app_version: string;
  ollama_reachable: boolean;
  chat_model: ModelStatus;
  embedding_model: ModelStatus;
  index_stats: IndexStats;
  fallback_enabled: boolean;
  searxng_reachable: boolean;
  resolve_reachable: boolean;
  premiere_reachable: boolean;
  blender_reachable: boolean;
}

export interface AppConfig {
  chat_model: string;
  embedding_model: string;
  fallback_enabled: boolean;
  searxng_url: string;
  top_k: number;
  similarity_threshold_primary: number;
  similarity_threshold_secondary: number;
  broll_analyzer_path: string | null;
}

export interface ConfigUpdate {
  chat_model?: string;
  embedding_model?: string;
  fallback_enabled?: boolean;
  searxng_url?: string;
  top_k?: number;
  broll_analyzer_path?: string;
}

export interface ReindexProgress {
  stage: string;
  manual: string | null;
  chunk: number | null;
  total_chunks: number | null;
  message: string | null;
}

export interface ChatTokenEvent {
  chat_id: number;
  delta: string;
}

export interface ChatDoneEvent {
  message: ChatMessage;
}

export interface ChatErrorEvent {
  chat_id: number;
  error: string;
}

export interface ResolveStatus {
  running: boolean;
  product_name: string | null;
  version: string | null;
  current_page: string | null;
}

export interface ResolveTimelineInfo {
  name: string;
  fps: string;
  start_timecode: string;
  current_timecode: string | null;
  video_tracks: number;
  audio_tracks: number;
}

export interface ResolveProjectInfo {
  has_project: boolean;
  project_name: string | null;
  timeline_count: number | null;
  current_timeline: ResolveTimelineInfo | null;
}

export interface ResolveActionResult {
  ok: boolean;
  message: string;
}

export interface ResolveTimelineSummary {
  name: string;
  index: number;
}

export interface PremiereStatus {
  connected: boolean;
  app_version: string | null;
  active_project_name: string | null;
}

export interface PremiereActionResult {
  ok: boolean;
  message: string;
}

export interface PremiereImportResult {
  ok: boolean;
  message: string;
  imported_count: number;
  sequence_created: boolean;
}

// Order matches Constants.MarkerColor in the premierepro UXP API exactly
// (index sent to the backend as color_index) -- only 7 colors, unlike
// Resolve's 16.
export const PREMIERE_MARKER_COLORS = [
  "Green", "Red", "Magenta", "Orange", "Yellow", "Blue", "Cyan",
] as const;

export interface PremiereTimelineClip {
  name: string;
  track_type: string;
  track_index: number;
  item_index: number;
  enabled: boolean;
}

export interface PremiereRenderResult {
  ok: boolean;
  message: string;
  output_file: string | null;
}

export interface BlenderStatus {
  running: boolean;
  version: string | null;
  file_path: string | null;
  scene_name: string | null;
}

export interface BlenderSceneInfo {
  scene_name: string;
  frame_start: number;
  frame_end: number;
  frame_current: number;
  fps: number;
  render_engine: string;
  object_count: number;
}

export interface BlenderActionResult {
  ok: boolean;
  message: string;
}

export interface BlenderImportResult {
  ok: boolean;
  message: string;
  imported_count: number;
  scene_created: boolean;
}

export interface ResolveClipInfo {
  name: string;
  file_name: string | null;
  frames: string | null;
  folder_path: string;
}

export interface ResolveImportResult {
  ok: boolean;
  message: string;
  imported_count: number;
  timeline_created: boolean;
}

export const RESOLVE_MARKER_COLORS = [
  "Blue", "Cyan", "Green", "Yellow", "Rose", "Purple", "Fuchsia", "Sky",
  "Mint", "Lemon", "Sand", "Cocoa", "Cream", "Pink", "Lavender", "Red",
] as const;

export interface ResolveTimelineClip {
  name: string;
  track_type: string;
  track_index: number;
  item_index: number;
  start: number;
  end: number;
  enabled: boolean;
}

export interface RenderProgress {
  stage: string;
  job_id: string | null;
  job_status: string | null;
  completion_percentage: number | null;
  message: string | null;
}
