# Dum-E — Quickstart

## 1. Check prerequisites

```bash
node --version      # 18+
cargo --version      # any recent stable
python3 --version    # 3.11+
ollama --version
```

If `python3 -c "import sqlite3; sqlite3.connect(':memory:').enable_load_extension(True)"`
raises an `AttributeError`, use Homebrew's Python for the venv below instead
of the python.org installer build — see `ingestion/README.md`.

## 2. Pull the local models (skip if already pulled)

```bash
ollama pull command-r7b
ollama pull nomic-embed-text
```

## 3. Ingest the manuals

```bash
cd ingestion
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
python3 ingest.py
cd ..
```

This reads every PDF in `books/`, chunks it, embeds it via Ollama, and
writes `dume.db` into the app's data directory. Re-run any time you add a
manual — unchanged files are skipped automatically.

## 4. (Optional) Start the internet fallback's search engine

```bash
cd searxng
export SEARXNG_SECRET=$(openssl rand -hex 32)
docker compose up -d
cd ..
```

Requires Docker Desktop running. See `searxng/README.md` if this is your
first time setting it up.

## 5. Run the app

```bash
npm install
npm run tauri dev
```

A native window titled **Dum-E** should open in dark mode (not a browser
tab).

## 6. First-run checklist

- Open the **ℹ️ About** button: Ollama should show reachable (green), and
  the chat/embedding models should show "available."
- Book index stats should show 22 manuals and a nonzero chunk count (from
  step 3).
- Ask something a manual covers, e.g. *"How do I set dual native ISO on the
  PYXIS 6K?"* — the answer should cite `(Blackmagic PYXIS 6K Manual, p. X)`.
- Internet fallback is off by default. To enable it: make sure SearXNG is
  running (step 4), open **⚙️ Settings**, and flip the toggle. Ask something
  the manuals don't cover (e.g. a Premiere Pro question) and confirm Dum-E
  discloses it switched to a web search.
- (Optional) With DaVinci Resolve running, open **⚙️ Settings** and check the
  **DaVinci Resolve** section — it should show "Connected" plus your current
  project/timeline and timeline/clip counts, and clicking **Refresh** should
  update it live. With a timeline open, **Add marker** should drop a marker
  at the playhead (check Resolve's own Edit page to confirm). With Resolve
  not running, this section should just say "Not running," not error.
- (Optional) Still in **⚙️ Settings** → **DaVinci Resolve**, try **Add
  Files…** with a project open — a native macOS file picker should appear;
  pick a video/audio file and it should show up in Resolve's Media Pool.
  Type a name first and it'll also build a new timeline from exactly what
  you picked and switch to it.
- (Optional) With a timeline open, the **Timeline clips** list should show
  its clips — toggle one's switch off and confirm it goes struck-through
  (and check Resolve's Edit page: the clip should show disabled, still
  reversible). Click the trash icon and confirm a native "Delete clip"
  dialog appears before anything happens; Cancel should leave the clip
  untouched.
- (Optional) In the **Render** block, pick a preset, click **Browse…** to
  choose an output folder, and click **Render** — a progress line should
  update, and the rendered file should appear in that folder when done
  (check Resolve's own Deliver page too). **Cancel** should stop it early.
