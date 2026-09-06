# Dum-E B-Roll bridge

Lets Dum-E's chat trigger a b-roll quality scoring pass by shelling out to a
**separate, unmodified checkout of the [B-Roll Analyzer](https://github.com)
project** you already have on this machine (part of Blair's Rough Cut Studio
Suite). This directory contains only a thin bridge script -- it never copies,
vendors, or modifies anything in that project.

`broll_bridge.py` imports B-Roll Analyzer's own `analyzer.py` directly
(`sys.path` insert, read-only) -- that module is explicitly built as a
UI-free core module (its Tkinter UI lives only in `app.py`), so this works
without needing that app's GUI at all. It intentionally does not enable the
optional local-CLIP "high energy" scoring for this first slice -- technical
quality only (sharpness/exposure/stability), matching `analyzer.py`'s base
dependencies rather than pulling in torch/open_clip.

## Setup

This runs in its **own** venv, separate from B-Roll Analyzer's own
environment (if it even has one set up) -- Dum-E never assumes or depends on
that project's own venv existing:

```bash
cd broll_bridge
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
```

Then in Dum-E's Settings, set **B-Roll Analyzer path** to wherever you have
that project checked out (e.g. `~/Developer/Blair/B-Roll Analyzer`) -- the
folder containing its `analyzer.py`.

## Protocol

Invoked as `<this venv's python> broll_bridge.py '<json args>'`, with args
`{"analyzer_dir": str, "folder": str}`. Prints one JSON object per line to
stdout, mirroring `resolve_bridge.py`'s own convention:

- `{"type": "progress", "current": int, "total": int, "file": str}` -- once
  per clip, before it's analyzed (analysis itself isn't sub-progress-reported
  here; a whole clip is the smallest unit of feedback for this first slice).
- `{"type": "result", "clips": [...]}` -- once, at the end. Each clip:
  `path`, `filename`, `duration`, `fps`, `width`, `height`, `overall_score`
  (0-100), `best_window_start`/`best_window_end` (seconds, the recommended
  segment), `error` (null unless that clip failed to analyze).
- `{"type": "error", "message": str}` -- a fatal error (bad args, folder not
  found, B-Roll Analyzer path not found/importable), exits nonzero.
