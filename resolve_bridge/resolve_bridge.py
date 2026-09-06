#!/usr/bin/env python3
"""Bridge between Dum-E's Rust backend and DaVinci Resolve's local scripting
API (see src-tauri/src/commands/resolve.rs, which spawns this as a
short-lived subprocess per call -- not a long-running daemon).

Usage: resolve_bridge.py <command> [json-args]
Commands: status | project | add_marker | list_timelines |
          list_media_pool_clips | import_media | list_timeline_clips |
          set_timeline_clip_enabled | delete_timeline_clip |
          list_render_presets | stop_render | render

Every command except `render` prints exactly one JSON object to stdout and
exits 0. Failures are reported as {"error": "..."} in that same payload
rather than via stderr/nonzero exit, so the Rust side has one uniform parse
path. `render` is the exception: it's long-running, so it prints one JSON
object per line as progress happens (mirroring ingestion/ingest.py's
protocol) rather than a single final object.

No pip dependencies -- stdlib plus the DaVinciResolveScript module that
ships inside the Resolve installation itself.
"""

import json
import os
import sys
import time

MODULES_DIR = (
    "/Library/Application Support/Blackmagic Design/DaVinci Resolve/"
    "Developer/Scripting/Modules/"
)

# Resolve's fixed set of named marker colors (Timeline.AddMarker rejects
# anything else). Kept in sync with the color dropdown in SettingsDialog.tsx.
VALID_MARKER_COLORS = {
    "Blue", "Cyan", "Green", "Yellow", "Rose", "Purple", "Fuchsia", "Sky",
    "Mint", "Lemon", "Sand", "Cocoa", "Cream", "Pink", "Lavender", "Red",
}


def emit(payload: dict) -> None:
    print(json.dumps(payload))
    sys.exit(0)


def get_resolve():
    """Returns the Resolve scripting object, or None if Resolve isn't
    running or the scripting API can't be reached -- never raises."""
    if not sys.path or MODULES_DIR not in sys.path:
        sys.path.insert(0, MODULES_DIR)
    try:
        import DaVinciResolveScript as dvr
    except ImportError:
        return None
    try:
        return dvr.scriptapp("Resolve")
    except Exception:
        return None


def cmd_status(resolve) -> dict:
    if resolve is None:
        return {"running": False, "product_name": None, "version": None, "current_page": None}
    try:
        return {
            "running": True,
            "product_name": resolve.GetProductName(),
            "version": resolve.GetVersionString(),
            "current_page": resolve.GetCurrentPage(),
        }
    except Exception as exc:
        return {"error": f"Resolve status query failed: {exc}"}


def _current_project(resolve):
    """Returns the current project, or None if Resolve isn't running, or no
    project manager/project is available -- never raises."""
    if resolve is None:
        return None
    project_manager = resolve.GetProjectManager()
    return project_manager.GetCurrentProject() if project_manager else None


def cmd_project(resolve) -> dict:
    empty = {"has_project": False, "project_name": None, "timeline_count": None, "current_timeline": None}
    project = _current_project(resolve)
    if project is None:
        return empty
    try:
        current_timeline = None
        timeline = project.GetCurrentTimeline()
        if timeline is not None:
            current_timeline = {
                "name": timeline.GetName(),
                # Despite the API docs promising a string, GetSetting has been
                # observed returning a raw float (e.g. 24.0) -- str() it so
                # the Rust side's `fps: String` field never fails to parse.
                "fps": str(project.GetSetting("timelineFrameRate")),
                "start_timecode": timeline.GetStartTimecode(),
                "current_timecode": timeline.GetCurrentTimecode() or None,
                "video_tracks": timeline.GetTrackCount("video"),
                "audio_tracks": timeline.GetTrackCount("audio"),
            }

        return {
            "has_project": True,
            "project_name": project.GetName(),
            "timeline_count": project.GetTimelineCount(),
            "current_timeline": current_timeline,
        }
    except Exception as exc:
        return {"error": f"Resolve project query failed: {exc}"}


def _timecode_to_frames(timecode: str, fps: float) -> int:
    """Converts an HH:MM:SS:FF (or drop-frame HH:MM:SS;FF) timecode into a
    raw frame count from 00:00:00:00. Does not apply drop-frame correction --
    exactly correct for non-drop-frame rates (23.976/24/25/30/50/60, the
    common case), off by a small, bounded amount for long-running NTSC
    drop-frame (29.97/59.94) timelines. Good enough to locate "near the
    playhead"; revisit if marker placement proves visibly wrong in practice.
    """
    hours, minutes, seconds, frames = (int(p) for p in timecode.replace(";", ":").split(":"))
    return ((hours * 3600) + (minutes * 60) + seconds) * round(fps) + frames


def cmd_add_marker(resolve, args: dict) -> dict:
    if resolve is None:
        return {"ok": False, "message": "DaVinci Resolve isn't running."}

    color = args.get("color") or "Blue"
    if color not in VALID_MARKER_COLORS:
        return {"ok": False, "message": f"'{color}' isn't a valid Resolve marker color."}
    name = args.get("name") or "Dum-E Marker"
    note = args.get("note") or ""

    project = _current_project(resolve)
    if project is None:
        return {"ok": False, "message": "No project is currently open in Resolve."}

    try:
        timeline = project.GetCurrentTimeline()
        if timeline is None:
            return {"ok": False, "message": "No timeline is currently open in Resolve."}

        current_tc = timeline.GetCurrentTimecode()
        if not current_tc:
            return {"ok": False, "message": "Couldn't read the playhead position (switch to the Edit or Color page and try again)."}

        fps = float(project.GetSetting("timelineFrameRate"))
        frame_id = timeline.GetStartFrame() + (
            _timecode_to_frames(current_tc, fps) - _timecode_to_frames(timeline.GetStartTimecode(), fps)
        )

        if timeline.AddMarker(frame_id, color, name, note, 1, ""):
            return {"ok": True, "message": f"Added a {color} marker at {current_tc}."}
        return {"ok": False, "message": "Resolve rejected the marker (a marker may already exist at that frame)."}
    except Exception as exc:
        return {"ok": False, "message": f"Failed to add marker: {exc}"}


def cmd_list_timelines(resolve) -> dict:
    project = _current_project(resolve)
    if project is None:
        return {"timelines": []}
    try:
        count = project.GetTimelineCount()
        timelines = []
        for i in range(1, count + 1):
            timeline = project.GetTimelineByIndex(i)
            if timeline is not None:
                timelines.append({"name": timeline.GetName(), "index": i})
        return {"timelines": timelines}
    except Exception as exc:
        return {"error": f"Resolve timeline list failed: {exc}"}


def _walk_folder_clips(folder, folder_path: str, clips: list, depth: int, max_depth: int = 20) -> None:
    """Appends clip dicts for `folder` and (depth-capped) every subfolder,
    recursively. The depth cap is a defensive guard against any pathological
    folder structure -- Resolve media pools aren't expected to nest anywhere
    near this deep in practice."""
    for clip in folder.GetClipList() or []:
        # Clip property keys are loosely documented -- fetch defensively
        # rather than assuming any key beyond GetName() always exists.
        props = clip.GetClipProperty() or {}
        clips.append({
            "name": clip.GetName(),
            "file_name": props.get("File Name") or None,
            "frames": str(props["Frames"]) if props.get("Frames") not in (None, "") else None,
            "folder_path": folder_path,
        })

    if depth >= max_depth:
        return
    for subfolder in folder.GetSubFolderList() or []:
        sub_path = f"{folder_path}/{subfolder.GetName()}" if folder_path else subfolder.GetName()
        _walk_folder_clips(subfolder, sub_path, clips, depth + 1, max_depth)


def cmd_list_media_pool_clips(resolve) -> dict:
    project = _current_project(resolve)
    if project is None:
        return {"clips": []}
    try:
        clips: list = []
        _walk_folder_clips(project.GetMediaPool().GetRootFolder(), "", clips, 0)
        return {"clips": clips}
    except Exception as exc:
        return {"error": f"Resolve media pool query failed: {exc}"}


def cmd_import_media(resolve, args: dict) -> dict:
    empty_result = {"ok": False, "message": "", "imported_count": 0, "timeline_created": False}
    if resolve is None:
        return {**empty_result, "message": "DaVinci Resolve isn't running."}

    paths = [p for p in (args.get("paths") or []) if p]
    if not paths:
        return {**empty_result, "message": "No paths were provided to import."}

    existing_paths = [p for p in paths if os.path.exists(p)]
    if not existing_paths:
        return {**empty_result, "message": "None of the given paths exist on disk."}

    project = _current_project(resolve)
    if project is None:
        return {**empty_result, "message": "No project is currently open in Resolve."}

    try:
        media_storage = resolve.GetMediaStorage()
        imported = media_storage.AddItemListToMediaPool(existing_paths) or []
        if not imported:
            return {**empty_result, "message": "Resolve didn't import any of the given paths -- check they're valid media files."}

        timeline_name = (args.get("timeline_name") or "").strip()
        timeline_created = False
        if timeline_name:
            media_pool = project.GetMediaPool()
            new_timeline = media_pool.CreateTimelineFromClips(timeline_name, imported)
            if new_timeline is None:
                return {
                    "ok": True,
                    "message": f"Imported {len(imported)} clip(s), but couldn't create timeline "
                               f"'{timeline_name}' (name may already be in use).",
                    "imported_count": len(imported),
                    "timeline_created": False,
                }
            project.SetCurrentTimeline(new_timeline)
            timeline_created = True

        message = f"Imported {len(imported)} clip(s)."
        if timeline_created:
            message += f" Created timeline '{timeline_name}'."
        return {"ok": True, "message": message, "imported_count": len(imported), "timeline_created": timeline_created}
    except Exception as exc:
        return {**empty_result, "message": f"Import failed: {exc}"}


TRACK_TYPES = ("video", "audio", "subtitle")


def cmd_list_timeline_clips(resolve) -> dict:
    project = _current_project(resolve)
    if project is None:
        return {"clips": []}
    try:
        timeline = project.GetCurrentTimeline()
        if timeline is None:
            return {"clips": []}

        clips = []
        for track_type in TRACK_TYPES:
            track_count = timeline.GetTrackCount(track_type)
            for track_index in range(1, track_count + 1):
                items = timeline.GetItemListInTrack(track_type, track_index) or []
                for item_index, item in enumerate(items):
                    clips.append({
                        "name": item.GetName(),
                        "track_type": track_type,
                        "track_index": track_index,
                        "item_index": item_index,
                        "start": item.GetStart(),
                        "end": item.GetEnd(),
                        "enabled": item.GetClipEnabled(),
                    })
        return {"clips": clips}
    except Exception as exc:
        return {"error": f"Resolve timeline clip list failed: {exc}"}


def _find_timeline_item(timeline, args: dict):
    """Re-fetches the item at (track_type, track_index, item_index) fresh --
    never trust an item reference across calls -- and verifies its name
    still matches `expected_name` (the name the UI last displayed) before
    the caller mutates anything, since the list may have changed between
    when the user looked and when they acted. Returns (item, None) on
    success, or (None, error_message) on any mismatch/failure."""
    track_type = args.get("track_type")
    track_index = args.get("track_index")
    item_index = args.get("item_index")
    expected_name = args.get("expected_name")

    if track_type not in TRACK_TYPES or not isinstance(track_index, int) or not isinstance(item_index, int):
        return None, "Invalid track/item reference."

    items = timeline.GetItemListInTrack(track_type, track_index) or []
    if item_index < 0 or item_index >= len(items):
        return None, "That clip is no longer at the expected position -- refresh and try again."

    item = items[item_index]
    if expected_name is not None and item.GetName() != expected_name:
        return None, f"That clip has changed (expected '{expected_name}', found '{item.GetName()}') -- refresh and try again."
    return item, None


def cmd_set_timeline_clip_enabled(resolve, args: dict) -> dict:
    project = _current_project(resolve)
    if project is None:
        return {"ok": False, "message": "No project is currently open in Resolve."}
    timeline = project.GetCurrentTimeline()
    if timeline is None:
        return {"ok": False, "message": "No timeline is currently open in Resolve."}

    item, error = _find_timeline_item(timeline, args)
    if error:
        return {"ok": False, "message": error}

    enabled = bool(args.get("enabled"))
    try:
        if item.SetClipEnabled(enabled):
            return {"ok": True, "message": f"{'Enabled' if enabled else 'Disabled'} '{item.GetName()}'."}
        return {"ok": False, "message": "Resolve rejected that change."}
    except Exception as exc:
        return {"ok": False, "message": f"Failed to change clip state: {exc}"}


def cmd_delete_timeline_clip(resolve, args: dict) -> dict:
    project = _current_project(resolve)
    if project is None:
        return {"ok": False, "message": "No project is currently open in Resolve."}
    timeline = project.GetCurrentTimeline()
    if timeline is None:
        return {"ok": False, "message": "No timeline is currently open in Resolve."}

    item, error = _find_timeline_item(timeline, args)
    if error:
        return {"ok": False, "message": error}

    name = item.GetName()
    ripple = bool(args.get("ripple"))
    try:
        if timeline.DeleteClips([item], ripple):
            return {"ok": True, "message": f"Deleted '{name}'."}
        return {"ok": False, "message": "Resolve rejected the delete."}
    except Exception as exc:
        return {"ok": False, "message": f"Failed to delete clip: {exc}"}


def cmd_list_render_presets(resolve) -> dict:
    project = _current_project(resolve)
    if project is None:
        return {"presets": []}
    try:
        return {"presets": list(project.GetRenderPresetList() or [])}
    except Exception as exc:
        return {"error": f"Resolve render preset list failed: {exc}"}


def cmd_stop_render(resolve) -> dict:
    project = _current_project(resolve)
    if project is None:
        return {"ok": False, "message": "No project is currently open in Resolve."}
    try:
        project.StopRendering()
        return {"ok": True, "message": "Stopped rendering."}
    except Exception as exc:
        return {"ok": False, "message": f"Failed to stop rendering: {exc}"}


def cmd_render(resolve, args: dict) -> None:
    """Long-running: streams one JSON progress object per line (unlike every
    other command here, which prints exactly one object total) -- mirrors
    ingestion/ingest.py's protocol, since a render can take minutes and
    CLAUDE.md requires progress feedback for long-running media exports."""
    def progress(stage: str, **extra) -> None:
        print(json.dumps({"stage": stage, **extra}))
        sys.stdout.flush()

    if resolve is None:
        progress("error", message="DaVinci Resolve isn't running.")
        return

    project = _current_project(resolve)
    if project is None:
        progress("error", message="No project is currently open in Resolve.")
        return

    preset_name = args.get("preset_name")
    target_dir = args.get("target_dir")
    custom_name = args.get("custom_name") or None
    if not preset_name or not target_dir:
        progress("error", message="A render preset and target directory are required.")
        return

    try:
        if not project.LoadRenderPreset(preset_name):
            progress("error", message=f"Couldn't load render preset '{preset_name}'.")
            return

        settings = {"TargetDir": target_dir}
        if custom_name:
            settings["CustomName"] = custom_name
        project.SetRenderSettings(settings)

        job_id = project.AddRenderJob()
        if not job_id:
            progress("error", message="Resolve didn't accept the render job (check the preset and settings).")
            return

        if not project.StartRendering([job_id], isInteractiveMode=False):
            progress("error", message="Resolve refused to start rendering.")
            return

        progress("started", job_id=job_id)
        while project.IsRenderingInProgress():
            status = project.GetRenderJobStatus(job_id) or {}
            progress(
                "rendering",
                job_status=status.get("JobStatus"),
                completion_percentage=status.get("CompletionPercentage"),
                raw=status,
            )
            time.sleep(1)

        final_status = project.GetRenderJobStatus(job_id) or {}
        job_status = final_status.get("JobStatus")
        if job_status and str(job_status).lower() in ("complete", "completed", "finished"):
            progress("done", job_status=job_status, raw=final_status)
        else:
            progress("error", message=f"Render did not complete successfully (status: {job_status}).", raw=final_status)
    except Exception as exc:
        progress("error", message=f"Render failed: {exc}")


def main() -> None:
    if len(sys.argv) < 2:
        emit({"error": "Usage: resolve_bridge.py <status|project|add_marker> [json-args]"})

    command = sys.argv[1]
    args = {}
    if len(sys.argv) > 2:
        try:
            args = json.loads(sys.argv[2])
        except json.JSONDecodeError as exc:
            emit({"error": f"Invalid JSON args: {exc}"})

    resolve = get_resolve()

    if command == "render":
        # Long-running -- streams its own lines, never goes through emit().
        cmd_render(resolve, args)
        return

    if command == "status":
        emit(cmd_status(resolve))
    elif command == "project":
        emit(cmd_project(resolve))
    elif command == "add_marker":
        emit(cmd_add_marker(resolve, args))
    elif command == "list_timelines":
        emit(cmd_list_timelines(resolve))
    elif command == "list_media_pool_clips":
        emit(cmd_list_media_pool_clips(resolve))
    elif command == "import_media":
        emit(cmd_import_media(resolve, args))
    elif command == "list_timeline_clips":
        emit(cmd_list_timeline_clips(resolve))
    elif command == "set_timeline_clip_enabled":
        emit(cmd_set_timeline_clip_enabled(resolve, args))
    elif command == "delete_timeline_clip":
        emit(cmd_delete_timeline_clip(resolve, args))
    elif command == "list_render_presets":
        emit(cmd_list_render_presets(resolve))
    elif command == "stop_render":
        emit(cmd_stop_render(resolve))
    else:
        emit({"error": f"Unknown command: {command}"})


if __name__ == "__main__":
    main()
