"""
Dum-E Bridge -- lets Dum-E (an external desktop app) read live status from,
and make one safe change to, a running Blender session. Same file-based
protocol as premiere_plugin/index.js, for the same reason: bpy is only
callable from *inside* Blender's own embedded Python (there's no externally
attachable scripting module like DaVinci Resolve ships), so this has to run
as code loaded into Blender itself rather than a subprocess Dum-E spawns.

Protocol: Dum-E writes request.json ({id, command, args}) into BRIDGE_DIR.
This add-on polls for it (via bpy.app.timers, which runs safely on
Blender's main thread -- a raw background thread can't touch bpy state),
deletes it immediately (never processed twice), executes the command, and
writes response-<id>.json ({ok, data, error}).

Install: Blender > Edit > Preferences > Add-ons > Install from Disk...,
select this file, then enable "Dum-E Bridge" in the list. Stays enabled
across restarts (a real advantage over Premiere's UXP plugin, which needs
re-loading via UXP Developer Tools every session) -- see blender_bridge/README.md.
"""

bl_info = {
    "name": "Dum-E Bridge",
    "author": "Dum-E",
    "version": (1, 0, 0),
    "blender": (4, 0, 0),
    "location": "N/A -- background bridge, no UI panel",
    "description": "Local file-based bridge letting the Dum-E desktop app query/control this Blender session",
    "category": "System",
}

import json
import os

import bpy

# Fixed, literal path -- deliberately NOT under the user's home directory,
# for the same reason as the Premiere bridge: not reliably resolvable from
# every embedded-scripting context. Both sides (this add-on and
# src-tauri/src/commands/blender.rs) hardcode this exact same path.
BRIDGE_DIR = "/tmp/dume_blender_bridge"
POLL_INTERVAL_SECONDS = 0.5


def handle_command(command, args):
    args = args or {}

    if command == "status":
        return {
            "version": bpy.app.version_string,
            "file_path": bpy.data.filepath or None,
            "scene_name": bpy.context.scene.name,
        }

    if command == "scene_info":
        scene = bpy.context.scene
        return {
            "scene_name": scene.name,
            "frame_start": scene.frame_start,
            "frame_end": scene.frame_end,
            "frame_current": scene.frame_current,
            "fps": scene.render.fps,
            "render_engine": scene.render.engine,
            "object_count": len(scene.objects),
        }

    if command == "add_marker":
        scene = bpy.context.scene
        name = args.get("name") or "Dum-E Marker"
        frame = scene.frame_current
        # Blender's timeline markers have no color property (unlike Resolve's
        # 16 or Premiere's 7) -- name + frame is the whole of what's settable.
        marker = scene.timeline_markers.new(name, frame=frame)
        return {"name": marker.name, "frame": frame}

    if command == "import_media":
        paths = args.get("paths") or []
        if not paths:
            raise ValueError("No file paths given")
        scene_name = args.get("scene_name")

        # Mirrors Resolve/Premiere's "import + optionally build a
        # timeline/sequence from exactly what was imported" in one call --
        # a new Scene here is the closest equivalent to a new Resolve
        # timeline or Premiere sequence, since Blender's VSE lives on a
        # Scene, not as an independently addressable object.
        scene_created = bool(scene_name)
        scene = bpy.data.scenes.new(scene_name) if scene_created else bpy.context.scene
        seq_editor = scene.sequence_editor_create()

        # Video-only for this first slice -- new_movie() doesn't also pull
        # in the clip's audio (unlike dragging a file into the VSE via the
        # UI, which creates a paired sound strip); a separate new_sound()
        # call would be needed for that, deferred until it's actually needed.
        #
        # `seq_editor.strips` (not `.sequences` -- confirmed live against a
        # running Blender 5.2: `.sequences` raises AttributeError, renamed
        # at some point after this API was originally documented).
        #
        # Each clip goes on its own channel, all starting at frame 1 --
        # NOT placed sequentially on one track. Confirmed live, reproduced
        # deterministically: reading a strip's `frame_final_end` (or
        # `frame_start`/`frame_final_duration` -- all three carry the same
        # "expected to be removed in Blender 6.0" DeprecationWarning, with
        # no working replacement yet in 5.2) between two `new_movie()`
        # calls corrupts Blender's RNA state, making the *next* RNA object
        # creation fail with "MemoryError: couldn't create BPy_rna object"
        # -- including, once corrupted, the unrelated
        # `bpy.context.window.scene` assignment below. That's what a real
        # two-clip import hit: the first clip silently vanished and the
        # final scene-switch line then raised the same error. Never
        # reading these properties between strip creations avoided it
        # completely in a clean repro. Sequential single-track placement
        # (matching Resolve/Premiere's layout) is deferred until either a
        # non-deprecated timing accessor exists, or clip duration can be
        # probed some other way before the strip is created.
        imported_count = 0
        for i, path in enumerate(paths):
            name = os.path.basename(path)
            seq_editor.strips.new_movie(name, path, i + 1, 1)
            imported_count += 1

        if scene_created:
            # Same reasoning as Resolve's SetCurrentTimeline/Premiere's new
            # sequence becoming the active tab -- the user should see the
            # result of the import immediately, not have to go find it.
            bpy.context.window.scene = scene

        return {
            "imported_count": imported_count,
            "scene_created": scene_created,
            "scene_name": scene.name,
        }

    raise ValueError(f"Unknown command: {command}")


def poll_bridge():
    request_path = os.path.join(BRIDGE_DIR, "request.json")
    if not os.path.exists(request_path):
        return POLL_INTERVAL_SECONDS

    try:
        with open(request_path, "r") as f:
            request = json.load(f)
    except (OSError, json.JSONDecodeError):
        # Rust writes this file with a single non-atomic write() -- a poll
        # landing mid-write would see a partial/invalid file. Leave it for
        # the next tick rather than deleting a request we couldn't parse.
        return POLL_INTERVAL_SECONDS

    os.remove(request_path)
    req_id = request["id"]

    try:
        data = handle_command(request.get("command"), request.get("args"))
        response = {"ok": True, "data": data, "error": None}
    except Exception as e:
        response = {"ok": False, "data": None, "error": str(e)}

    response_path = os.path.join(BRIDGE_DIR, f"response-{req_id}.json")
    with open(response_path, "w") as f:
        json.dump(response, f)

    return POLL_INTERVAL_SECONDS


def register():
    os.makedirs(BRIDGE_DIR, exist_ok=True)
    if not bpy.app.timers.is_registered(poll_bridge):
        bpy.app.timers.register(poll_bridge, persistent=True)


def unregister():
    if bpy.app.timers.is_registered(poll_bridge):
        bpy.app.timers.unregister(poll_bridge)


if __name__ == "__main__":
    register()
