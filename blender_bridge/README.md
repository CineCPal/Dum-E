# Dum-E Bridge for Blender

Lets Dum-E's chat/Settings UI read live status from, and add a timeline
marker to, a running Blender session. Same file-based bridge pattern as
`premiere_plugin/` -- see `dume_bridge.py`'s module docstring for why (bpy
has no externally-attachable scripting module like DaVinci Resolve ships).

## Install

1. Open Blender.
2. Edit > Preferences > Add-ons > Install from Disk...
3. Select `dume_bridge.py` from this folder.
4. Enable "Dum-E Bridge" in the add-on list (search "Dum-E" if it's not
   immediately visible).

That's it -- no restart needed, and unlike Premiere's UXP plugin (which
needs re-loading via UXP Developer Tools every session), this stays
enabled across Blender restarts once installed.

## Verifying it's running

Dum-E's Settings and About dialogs poll for a live Blender session
automatically. If Settings shows "Not running" while Blender is open,
double-check the add-on is enabled (Preferences > Add-ons, search
"Dum-E").

## Known limitations (first slice)

- Read-only `status`/`scene_info` plus one write action (`add_marker`).
  No import/render/timeline-editing yet -- see PLAN.md for what Resolve
  and Premiere ended up with, which is the likely direction here too.
- Blender's timeline markers have no color property (unlike Resolve's 16
  or Premiere's 7) -- only a name and frame.
