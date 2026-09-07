import { invoke } from "@tauri-apps/api/core";
import type {
  AboutInfo,
  AppConfig,
  ChatMessage,
  ConfigUpdate,
  ResolveActionResult,
  ResolveClipInfo,
  ResolveImportResult,
  ResolveProjectInfo,
  ResolveStatus,
  ResolveTimelineClip,
  ResolveTimelineSummary,
  PremiereStatus,
  PremiereActionResult,
  PremiereImportResult,
  PremiereTimelineClip,
  PremiereRenderResult,
  BlenderStatus,
  BlenderSceneInfo,
  BlenderActionResult,
  BlenderImportResult,
} from "./types";

export const api = {
  listMessages: () => invoke<ChatMessage[]>("list_messages"),
  clearChat: () => invoke<void>("clear_chat"),
  askQuestion: (text: string) => invoke<ChatMessage>("ask_question", { text }),
  scoreBrollFolder: (text: string, folder: string) =>
    invoke<ChatMessage>("broll_score_folder", { text, folder }),
  sendBrollToResolve: (text: string, count: number, timelineName: string | null) =>
    invoke<ChatMessage>("broll_send_to_resolve", { text, count, timelineName }),
  sendBrollToPremiere: (text: string, count: number, sequenceName: string | null) =>
    invoke<ChatMessage>("broll_send_to_premiere", { text, count, sequenceName }),

  getConfig: () => invoke<AppConfig>("get_config"),
  updateConfig: (update: ConfigUpdate) => invoke<AppConfig>("update_config", { update }),
  listOllamaModels: () => invoke<string[]>("list_ollama_models"),

  getAboutInfo: () => invoke<AboutInfo>("get_about_info"),
  runReindex: () => invoke<void>("run_reindex"),

  getResolveStatus: () => invoke<ResolveStatus>("resolve_status"),
  getResolveProjectInfo: () => invoke<ResolveProjectInfo>("resolve_project_info"),
  addResolveMarker: (color: string, name: string, note: string) =>
    invoke<ResolveActionResult>("resolve_add_marker", { color, name, note }),
  getResolveTimelines: () => invoke<ResolveTimelineSummary[]>("resolve_list_timelines"),
  getResolveMediaPoolClips: () => invoke<ResolveClipInfo[]>("resolve_list_media_pool_clips"),
  importResolveMedia: (paths: string[], timelineName: string | null) =>
    invoke<ResolveImportResult>("resolve_import_media", { paths, timelineName }),

  getResolveTimelineClips: () => invoke<ResolveTimelineClip[]>("resolve_list_timeline_clips"),
  setResolveTimelineClipEnabled: (
    trackType: string,
    trackIndex: number,
    itemIndex: number,
    expectedName: string,
    enabled: boolean,
  ) =>
    invoke<ResolveActionResult>("resolve_set_timeline_clip_enabled", {
      trackType,
      trackIndex,
      itemIndex,
      expectedName,
      enabled,
    }),
  deleteResolveTimelineClip: (
    trackType: string,
    trackIndex: number,
    itemIndex: number,
    expectedName: string,
    ripple: boolean,
  ) =>
    invoke<ResolveActionResult>("resolve_delete_timeline_clip", {
      trackType,
      trackIndex,
      itemIndex,
      expectedName,
      ripple,
    }),

  getResolveRenderPresets: () => invoke<string[]>("resolve_list_render_presets"),
  stopResolveRender: () => invoke<ResolveActionResult>("resolve_stop_render"),
  startResolveRender: (presetName: string, targetDir: string, customName: string | null) =>
    invoke<void>("resolve_start_render", { presetName, targetDir, customName }),

  getPremiereStatus: () => invoke<PremiereStatus>("premiere_status"),
  addPremiereMarker: (name: string, note: string, colorIndex: number | null) =>
    invoke<PremiereActionResult>("premiere_add_marker", { name, note, colorIndex }),
  importPremiereMedia: (paths: string[], sequenceName: string | null) =>
    invoke<PremiereImportResult>("premiere_import_media", { paths, sequenceName }),

  getPremiereTimelineClips: () => invoke<PremiereTimelineClip[]>("premiere_list_timeline_clips"),
  setPremiereTimelineClipEnabled: (
    trackType: string,
    trackIndex: number,
    itemIndex: number,
    expectedName: string,
    enabled: boolean,
  ) =>
    invoke<PremiereActionResult>("premiere_set_timeline_clip_enabled", {
      trackType,
      trackIndex,
      itemIndex,
      expectedName,
      enabled,
    }),

  startPremiereRender: (presetFile: string, targetDir: string, customName: string | null) =>
    invoke<PremiereRenderResult>("premiere_start_render", { presetFile, targetDir, customName }),

  getBlenderStatus: () => invoke<BlenderStatus>("blender_status"),
  getBlenderSceneInfo: () => invoke<BlenderSceneInfo | null>("blender_scene_info"),
  addBlenderMarker: (name: string) => invoke<BlenderActionResult>("blender_add_marker", { name }),
  importBlenderMedia: (paths: string[], sceneName: string | null) =>
    invoke<BlenderImportResult>("blender_import_media", { paths, sceneName }),
};
