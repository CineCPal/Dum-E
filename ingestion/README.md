# Dum-E ingestion

Standalone script: scans `books/*.pdf`, extracts and chunks text
(page-aware, for citations), embeds each chunk via a local Ollama model, and
writes everything into the SQLite (+ sqlite-vec) database the desktop app
queries at chat time. Decoupled from the Tauri app on purpose, so it can be
run standalone, from a script, or eventually reused by a larger Python
video-production tool suite.

## Usage

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
python3 ingest.py                     # uses ../books and the default app-data db path
python3 ingest.py --books-dir /path/to/pdfs --db-path /path/to/dume.db
```

Safe to re-run any time: each PDF's SHA-256 hash is compared against what's
already indexed, so unchanged manuals are skipped. Progress is printed as
one JSON object per line (`{"stage": ..., "manual": ..., ...}`), which is
what the app's "Re-index" button parses to drive its progress bar.

## Important macOS gotcha: `enable_load_extension`

sqlite-vec is a loadable SQLite extension, so ingestion needs
`sqlite3.Connection.enable_load_extension()`. **The official python.org
macOS installer disables this for security reasons** — you'll see:

```text
AttributeError: 'sqlite3.Connection' object has no attribute 'enable_load_extension'
```

Fix: create the virtual environment with a Python build that *does* support
it, e.g. Homebrew's:

```bash
/opt/homebrew/bin/python3 -m venv .venv
```

(Apple Silicon path shown; Intel Homebrew installs to `/usr/local/bin/python3`.)
Verify with:

```bash
python3 -c "import sqlite3; c = sqlite3.connect(':memory:'); c.enable_load_extension(True); print('ok')"
```

## Files

- `ingest.py` — entrypoint: scan, hash, parse, chunk, embed, write.
- `chunker.py` — page-aware sliding-window chunking with paragraph/sentence
  boundary snapping.
- `manifest.json` — filename → friendly citation title. A PDF not listed
  here gets a title derived from its filename instead — no code changes
  needed to add a new manual.
