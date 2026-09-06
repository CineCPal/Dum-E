#!/usr/bin/env python3
"""Bridge to the (separate, unmodified) B-Roll Analyzer project's core
`analyzer.py` module -- lets Dum-E score a folder of b-roll clips on
technical quality (sharpness/exposure/stability) without touching that
project's own files. `analyzer.py` is explicitly built as a UI-free core
module (Tkinter lives only in its `app.py`), so it's safe to import
directly via `sys.path` rather than needing that app's GUI at all -- the
same reuse trick the Rough Cut Studio Suite's own `broll_worker.py` uses
for the same module.

Deliberately does NOT enable `enable_energy=True` (the optional local-CLIP
"high energy" scoring) for this first slice -- that pulls in torch/
open_clip, a much heavier dependency than the base opencv-python/numpy
this script's own venv installs. Technical-quality scoring only for now.

Protocol (stdout, one JSON object per line, mirrors resolve_bridge.py's
own convention): a "progress" line per clip as it starts, then one final
"result" line with every clip's summary. Args arrive as a JSON string in
argv[1]: {"analyzer_dir": str, "folder": str}.
"""

import json
import os
import sys


def emit(obj):
    print(json.dumps(obj), flush=True)


def main():
    if len(sys.argv) != 2:
        emit({"type": "error", "message": "Expected one JSON argv[1] argument"})
        sys.exit(1)

    try:
        args = json.loads(sys.argv[1])
        analyzer_dir = args["analyzer_dir"]
        folder = args["folder"]
    except (json.JSONDecodeError, KeyError) as e:
        emit({"type": "error", "message": f"Bad arguments: {e}"})
        sys.exit(1)

    if not os.path.isdir(analyzer_dir):
        emit({"type": "error", "message": f"B-Roll Analyzer path not found: {analyzer_dir}"})
        sys.exit(1)
    if not os.path.isdir(folder):
        emit({"type": "error", "message": f"Folder not found: {folder}"})
        sys.exit(1)

    sys.path.insert(0, analyzer_dir)
    try:
        import analyzer  # B-Roll Analyzer's own core module, imported read-only
    except ImportError as e:
        emit({"type": "error", "message": f"Couldn't import analyzer.py from {analyzer_dir}: {e}"})
        sys.exit(1)

    analyzer.limit_opencv_threads()

    files = analyzer.find_video_files(folder)
    if not files:
        emit({"type": "result", "clips": []})
        return

    clips = []
    for i, path in enumerate(files):
        emit({"type": "progress", "current": i, "total": len(files), "file": os.path.basename(path)})
        result = analyzer.analyze_clip(path)
        clips.append({
            "path": result.path,
            "filename": result.filename,
            "duration": result.duration,
            "fps": result.fps,
            "width": result.width,
            "height": result.height,
            "overall_score": result.overall_score,
            "best_window_start": result.best_window_start,
            "best_window_end": result.best_window_end,
            "error": result.error,
        })

    emit({"type": "result", "clips": clips})


if __name__ == "__main__":
    main()
