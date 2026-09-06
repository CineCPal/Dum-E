# Dum-E

Dum-E is a local-first desktop chatbot that trains video producers on their
gear and editing software. It answers questions and teaches skills, always
checking its library of reference manuals first and only falling back to a
live web search when the manuals don't cover a topic.

Named after Tony Stark's eager, endearing robot assistant — Dum-E is warm,
a little goofy, always honest about what it does and doesn't know, and
always tells you where an answer came from.

**Phase 1** (done): a desktop chat app with retrieval-augmented generation
(RAG) over the manuals in `books/`, running entirely on your Mac.

**Phase 2** (in progress): driving creative apps directly from Dum-E.
Step 1 covered DaVinci Resolve's live project/timeline status (Settings +
About) plus one write action (adding a playhead marker). Step 2 added media
import (native file/folder picker) and building a new timeline from exactly
what you import, plus more read queries (timelines, media pool clips). Step
3 added rendering (with progress feedback), disabling/deleting clips on the
current timeline, and recursive media-pool subfolder listing. Trimming/
reordering existing clips (not exposed by Resolve's scripting API — see
Known limitations), Premiere Pro, After Effects, and Blender are not built
yet. A standalone iOS companion app is a separate future phase.

## Architecture

Two independent pieces, connected by one SQLite file:

- **`ingestion/`** — a standalone Python script that scans `books/*.pdf`,
  extracts and chunks their text (page-aware, so citations can say
  "p. 42"), embeds each chunk with a local Ollama model, and writes
  everything into `dume.db` (SQLite + the [sqlite-vec](https://github.com/asg017/sqlite-vec)
  extension for vector search). It's incremental: unchanged PDFs (by
  SHA-256) are skipped, so adding one new manual and re-running only
  processes that file. Runs as a subprocess of the app (via the Settings
  "Re-index" button) or standalone from the command line.
- **`src-tauri/`** (Rust) + **`src/`** (React) — the desktop app. It never
  parses PDFs itself; it only reads the SQLite index ingestion produced. At
  chat time: embed the question → cosine-similarity search over the manual
  chunks → if confident, answer from those chunks and cite `(Manual, p. X)`
  → if not confident and you've enabled the web fallback, search a
  self-hosted SearXNG instance and answer from those results instead,
  disclosing the switch → if neither is available, say so honestly rather
  than guess.
- **`searxng/`** — a self-hosted [SearXNG](https://docs.searxng.org/)
  metasearch engine (via Docker) that powers the optional web fallback. No
  API key, no billing, no third-party vendor.
- **`resolve_bridge/`** — a small stdlib-only Python script
  (`resolve_bridge.py`) that talks to DaVinci Resolve's local scripting API
  (bundled with Resolve itself, no separate install). Spawned as a
  short-lived subprocess per call from `src-tauri/src/commands/resolve.rs`
  (same pattern as `ingestion/`'s reindex subprocess, one JSON object per
  call) — never a long-running daemon. The one exception is `render`: like
  `ingestion/ingest.py`, it streams a JSON progress line per second while a
  render is in flight (`render://progress` events), since renders can take
  minutes and CLAUDE.md requires progress feedback for long-running media
  exports.

```text
question --> embed --> sqlite-vec KNN search --> confident? --yes--> Ollama chat (manual excerpts + citations)
                                                 \--no--> fallback enabled? --yes--> SearXNG --> Ollama chat (web results + citations)
                                                                            \--no--> honest "I don't know" (no model call)
```

The personality and grounding rules live in one system prompt
(`src-tauri/src/rag/mod.rs`) shared by all three paths. The "honest I don't
know" path never calls the chat model at all — it returns a fixed string
(`rag::decline_answer()`), since in testing the model would otherwise answer
fluently from its own training data instead of declining, despite being told
not to.

## Prerequisites

- macOS (this build targets a local dev machine; see **Known limitations**)
- [Ollama](https://ollama.com) running locally, with a chat model and an
  embedding model pulled:

  ```bash
  ollama pull command-r7b
  ollama pull nomic-embed-text
  ```
- Node.js 18+ and Rust (stable, via `rustup`) for the desktop app
- Python 3.11+ **built with SQLite loadable-extension support** for
  ingestion — see the note in `ingestion/README.md` if `enable_load_extension`
  raises `AttributeError` (the python.org macOS installer disables it;
  Homebrew's Python doesn't)
- (Optional) [Docker](https://www.docker.com/), only needed if you want the
  internet fallback — it runs a local, self-hosted SearXNG instance (see
  `searxng/README.md`); no account or API key required
- (Optional) [DaVinci Resolve](https://www.blackmagicdesign.com/products/davinciresolve)
  running, for the Phase 2 Resolve integration (Settings → DaVinci Resolve).
  Its scripting API ships inside the app — no separate install, and the
  default local scripting permission is already sufficient (only *network*
  scripting requires a preference change, which Dum-E doesn't need). Without
  Resolve running, that section of Settings just shows "Not running."

## Setup

See [QUICKSTART.md](QUICKSTART.md) for the condensed version. In short:

```bash
# 1. Ingest the manuals (one-time, re-run any time books/ changes)
cd ingestion
python3 -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt
python3 ingest.py
cd ..

# 2. (Optional) Start the internet fallback's search engine
cd searxng
export SEARXNG_SECRET=$(openssl rand -hex 32)
docker compose up -d
cd ..

# 3. Run the app
npm install
npm run tauri dev
```

## Directory structure

```text
Dum-E/
├── books/              # reference manual PDFs (read-only input)
├── ingestion/           # Python: PDF -> chunks -> embeddings -> dume.db
├── searxng/             # self-hosted search fallback (Docker Compose + config)
├── resolve_bridge/       # Python: talks to DaVinci Resolve's local scripting API
├── src-tauri/           # Rust backend (Tauri commands, db, RAG, Ollama/SearXNG clients)
├── src/                 # React frontend (chat UI, settings, about)
└── PLAN.md              # living progress/decision log
```

## Configuration reference

Non-secret settings live in `config.json` in the app's data directory
(`~/Library/Application Support/com.dume.app/`), editable from the Settings
dialog:

| Field | Default | Notes |
|---|---|---|
| `chat_model` | `command-r7b:latest` | Cohere's small model, trained for grounded/cited RAG answers. Any locally-pulled Ollama model works except `llama3.3` (42GB, excluded — too large for a 16GB Mac). |
| `embedding_model` | `nomic-embed-text:latest` | Fixed at 768 dimensions to match the `vec_chunks` table; changing it requires re-indexing from scratch, so it isn't exposed in the Settings UI yet. |
| `fallback_enabled` | `false` | Opt-in. No query ever leaves the machine until you turn this on. |
| `searxng_url` | `http://127.0.0.1:8888` | Where Dum-E sends fallback queries. Point it at a different SearXNG instance if you run one elsewhere. |
| `top_k` | `6` | How many manual chunks are retrieved per question. |
| `similarity_threshold_primary` / `_secondary` | `0.55` / `0.45` | Confidence gate: the top hit must clear the primary threshold and at least one more hit must clear the secondary one, or Dum-E treats the manuals as not covering the question. Tune these empirically (see `PLAN.md`). |

The "Re-index" button spawns `python3` from `PATH` (override with the
`DUME_PYTHON` environment variable if needed) — make sure it resolves to an
interpreter with `ingestion/requirements.txt` installed, e.g. by activating
`ingestion/.venv` in the same shell before running `npm run tauri dev`.

## Adding new manuals

Drop a PDF into `books/`, then click **Re-index** in Settings (or run
`python3 ingestion/ingest.py`). No code changes needed — the friendly title
shown in citations comes from `ingestion/manifest.json`; if a new filename
isn't listed there, Dum-E derives a readable title from the filename itself.

## Known limitations

- **No Premiere Pro, After Effects, or Blender manuals yet.** `books/`
  currently only has camera/gear manuals and DaVinci Resolve documentation.
  Dum-E will lean on the web fallback for the other tools until matching
  PDFs are added.
- **Dev-workflow only.** `ingestion/` and `books/` are located via a
  compile-time path relative to `src-tauri/` (`CARGO_MANIFEST_DIR`), which
  is correct for `npm run tauri dev` on this machine but hasn't been wired
  up as bundled app resources for a distributable build.
- **App automation is Resolve-only.** Phase 2 covers live Resolve
  project/timeline status, playhead markers, media import + timeline
  creation, rendering (via a saved render preset), and disabling/deleting
  clips on the current timeline — no Premiere Pro / After Effects / Blender
  automation at all. **No iOS app** either. See `PLAN.md` for what's planned
  next.
- **No timeline trim/reorder.** Resolve's scripting API exposes no
  `SetStart`/`SetEnd`/move method for an existing `TimelineItem` — only
  `DeleteClips` and `SetClipEnabled`. Reconstructing a trim via
  delete-and-re-append would risk losing grades/effects on that clip, so
  it's not implemented; `TimelineItem`'s real capabilities set the ceiling
  here, not an arbitrary scope cut.
- **Clip deletion has no undo.** `Timeline.DeleteClips` is irreversible via
  the scripting API — the UI requires a native confirmation dialog before
  ever calling it. Disabling a clip (`SetClipEnabled`) is the reversible
  alternative and is the default action in the UI.
- **Rendering needs an existing Resolve render preset.** The dropdown lists
  `Project.GetRenderPresetList()`; Resolve ships several built-in ones
  (H.264 Master, YouTube presets, etc.) even in a brand-new project, so this
  is normally populated with no setup, but a project that's had all its
  presets deleted would show an empty list.
- **Resolve marker placement assumes non-drop-frame timecodes.** The
  playhead-to-frame conversion in `resolve_bridge/resolve_bridge.py`
  (`_timecode_to_frames`) doesn't apply drop-frame correction, so it's exact
  for the common rates (23.976/24/25/30/50/60) but can drift slightly on
  long-running NTSC drop-frame (29.97/59.94) timelines.
- **sqlite-vec is pre-1.0** (alpha). Fine at this corpus's scale (tens of
  thousands of chunks, brute-force KNN); revisit if the manual library grows
  by an order of magnitude.
- **PyMuPDF is AGPL-licensed.** Fine for personal/internal use; flag it if
  this app is ever distributed outside that.

## Privacy & security

- Everything — PDF parsing, embeddings, chat inference, the SQLite index —
  runs locally. Nothing about your files, manuals, or chat history is sent
  anywhere by default.
- The optional internet fallback queries a **self-hosted SearXNG instance**
  you run yourself (see `searxng/`) — not a third-party API. It sends only
  your typed question text, never filenames, file paths, or manual content,
  and only after you've explicitly enabled it. The SearXNG container's port
  is bound to `127.0.0.1` only, not exposed on your LAN.
- Error messages and logs (via `tauri-plugin-log`) avoid printing file paths
  or filenames; ingestion progress events report manual **titles** only.

## License

Internal/personal tool. See individual dependency licenses for redistribution
constraints (notably PyMuPDF's AGPL license, and each Ollama model's own
license).
