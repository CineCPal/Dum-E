# Dum-E — Project Plan & Progress Log

## Vision

Dum-E is a local-first chatbot with a personality that trains video
producers on their equipment and editing software. Long-term, it should
also drive Premiere Pro, DaVinci Resolve, After Effects, and Blender
directly, and ship as a standalone iOS companion app. Given the scope, the
project is being built in phases.

## Phases

- [x] **Phase 1 — MVP (this build):** desktop chatbot, RAG over `books/`,
      Dum-E personality, self-hosted SearXNG fallback, About/Settings UI, docs.
- [~] **Phase 2 — App automation:** drive Premiere Pro, DaVinci Resolve,
      After Effects, and Blender from Dum-E (edit videos, run project
      tasks). **In progress** — see "Phase 2 step 1" below for what's done.
      Confirmed approach for the rest: per-app scripting bridges (DaVinci
      Resolve's official Python/Lua API — in use now; Blender's `bpy`;
      Premiere Pro/After Effects via ExtendScript or UXP panels acting as a
      local command server Dum-E's desktop app can talk to).
- [ ] **Phase 3 — iOS companion:** a standalone iOS app with its own
      on-device model and its own copy of the manual knowledge base (per
      user decision: NOT a remote client to the desktop app, and not able to
      drive desktop apps — phones can't control desktop software). Not
      started.

## Phase 1 decisions log

- **Tauri v2** over Electron: lighter footprint (per project mandate), and
  Tauri v2 + `sqlite-vec` + `rusqlite` + `reqwest`/`tokio` were all already
  cached locally — offline-buildable, low risk.
- **Self-hosted SearXNG over Brave Search API** (switched mid-build): Brave
  discontinued its free API tier in February 2026 (now requires a credit
  card, $5 prepaid credit ≈ 1,000-1,600 queries, then metered billing) — a
  real departure from the project's free/open-source/local-API mandate.
  SearXNG (open-source metasearch, self-hosted via Docker, `searxng/`) has
  no API key, no billing, and no third-party vendor at all. This removed
  the need for Keychain-based secret storage entirely (no more `keyring`
  crate/`secrets` module) since there's no API key to protect — `config.rs`
  just stores a `searxng_url` (default `http://127.0.0.1:8888`) alongside
  the other non-secret settings.
- **`command-r7b` as default chat model** over `llama3.1:latest`: Cohere's
  Command-R family is purpose-trained for retrieval-augmented generation
  (answer strictly from supplied documents, cite them, decline when they
  don't cover the question) — exactly Dum-E's core behavior. Near-identical
  RAM footprint (5.1GB vs 4.9GB), both already pulled locally.
- **sqlite-vec over LanceDB**: already cached/offline-buildable; keeps
  everything in one SQLite file per the project's "local SQLite" mandate;
  plenty fast at this corpus's scale (tens of thousands of chunks,
  brute-force KNN). Trade-off: sqlite-vec is pre-1.0 (alpha) — revisit if
  the manual corpus grows an order of magnitude.
- **PyMuPDF for PDF parsing**, run as a one-time Python script rather than
  in Rust: no Rust PDF-parsing crate was cached locally, and PyMuPDF handles
  large multi-column manuals (187MB reference manual included) with low
  memory overhead via lazy page access. AGPL-licensed — fine for internal
  use, flag if ever redistributed.
- **Internet fallback sends only the raw question text**, never filenames,
  paths, or manual content — and is off by default (opt-in) so nothing
  leaves the machine until the user explicitly enables it. The SearXNG
  container's port is bound to `127.0.0.1` only (not exposed on the LAN).
- **Homebrew Python (not the python.org installer) for the ingestion venv**:
  the python.org macOS build disables `sqlite3.enable_load_extension` for
  security reasons, which breaks loading sqlite-vec. Documented in
  `ingestion/README.md`.
- **Embedding model is not user-swappable in the Settings UI**: `vec_chunks`
  is a fixed `float[768]` column; swapping to a model with a different
  embedding dimension would break retrieval without a full re-index. Kept
  fixed to `nomic-embed-text` for MVP; config field exists for future use.
- **Single ongoing chat thread** (not full multi-chat history management):
  the schema supports multiple `chats`, but the UI only ever uses chat id 1
  for this MVP, keeping the command surface small.

## Verification status

Ran during this build:
- [x] Ingestion smoke-tested end-to-end against one manual (Portkeys LH5P,
      22 pages → 4 chunks), including a manual KNN query matching exactly
      what the Rust `top_k_chunks` function runs.
- [x] Full ingestion run kicked off against all 22 manuals in `books/`
      (~815MB) — see the "Full ingestion run" note below for the outcome.
- [x] Rust backend (`cargo check`) compiles clean, no warnings.
- [x] Frontend (`tsc --noEmit` + `vite build`) compiles/builds clean.
- [x] **Product-gating fix for cross-tool bleed-through**: added
      `rag/products.rs` (question → `manuals.product` slug matching, whole-word
      for single-token aliases so "resolve" doesn't match inside "unresolved")
      plus `WIDE_RETRIEVAL_K=40` in `commands/chat.rs` so a thin manual (e.g.
      Sony FS5, ~120 chunks) isn't crowded out of top-k by a much larger one
      (DaVinci Resolve, ~4,700 chunks across 6 manuals) before product
      filtering runs. A question naming an uncovered tool (Premiere Pro, After
      Effects, Blender, Final Cut, Avid, Photoshop, CapCut) now skips manual
      retrieval entirely and goes straight to fallback/decline. Verified via
      `cargo test` (2 unit tests) and `ingestion/stress_test_routing.py` (a
      throwaway script reimplementing the exact retrieval+routing logic in
      Python against the real `dume.db` + live Ollama embeddings) — **18/18
      test cases passed**, confirmed 2026-09-06.
- [x] SearXNG stood up via Docker Compose (`searxng/`), verified reachable
      and returning real results from `/search?format=json` via `curl`.
- [x] Uncovered-topic questions confirmed to correctly discloses the
      fallback/decline switch (covered by the stress test above).

- [x] Launched `npm run tauri dev` and drove the real native window
      end-to-end (screenshot + accessibility-driven clicks/typing, not just
      the stress-test script) — confirmed 2026-09-06:
      - Native window (title bar, traffic lights, no browser chrome) + dark
        mode by default.
      - About modal shows version, Ollama/model/status, manuals/chunks
        indexed, last-indexed timestamp, fallback status, privacy notice.
      - A manual-covered question (Sony FS5 white balance) renders 6 manual
        citation chips correctly.
      - Fallback ON: an uncovered-product question (After Effects masks)
        correctly triggers a real SearXNG search, and renders 5 clickable
        web citation chips.
      - Fallback OFF: **found and fixed a real bug** — see below — then
        reverified the same uncovered-product question (Blender keyframes)
        now gets an honest, instant decline.
      - Toggled the fallback setting back on afterward, restoring the app to
        its prior state.

Still to do:
- [ ] Empirically tune `similarity_threshold_primary`/`_secondary` (current
      defaults: 0.55 / 0.45) against a mix of clearly-covered and
      clearly-uncovered real questions; record final values here. (Current
      defaults held up across all 18 stress-test cases, but that's a script,
      not a broad real-usage sample.)

## Bug found + fixed: Decline path didn't actually decline

**Found 2026-09-06** while manually driving the UI: with the internet
fallback toggled off, asking an uncovered-product question (e.g. "How do I
add a keyframe in Blender?") did NOT produce an honest "I don't know" — Dum-E
instead answered fluently and confidently from `command-r7b`'s own training
data (a full multi-section Blender tutorial), even though the system prompt
handed to the model explicitly said "MANUAL EXCERPTS: (none found for this
question)" and instructed it to decline rather than guess (`rag/mod.rs`
`PERSONALITY` rule 5). The model just didn't reliably follow that instruction.

**Fix**: stopped relying on the chat model to self-censor. The Decline path
(`src-tauri/src/rag/mod.rs::decline_answer()`) now returns a fixed, canned
honest-decline string directly — `commands/chat.rs` skips calling
`ollama::chat_stream` entirely for this path (see the new `AnswerPlan` enum:
`Generate(Vec<ChatTurn>)` vs `Fixed(String)`). An honest "I don't know" can
never again depend on whether the model chooses to cooperate. Re-verified in
the running app: the same Blender question now gets an instant, correct
decline. `cargo test` still 2/2 passing after the change.

**Known gap this leaves**: the stress-test script only checks *routing*
(which path was chosen), not *content* (what the model actually said) — it
would not have caught this bug on its own. Worth adding a Rust unit test
asserting `decide_path` → `Decline` never reaches `ollama::chat_stream`, if
this class of regression is a concern going forward.

## Known content gap: Fusion manual thinness

`DaVinci-Resolve-20-Fusion-Visual-Effects.pdf` is tagged product
`davinci-resolve` (it's a Resolve-suite guide), not `fusion` — so a question
naming only "Fusion" gets filtered to just `Fusion21_Manual.pdf`'s chunks
and never sees that richer guide, even though it covers Fusion tools in
depth. Same situation for `fairlight-live` (only `FairlightLiveUserManual.pdf`,
thin) vs. the fuller `DaVinci-Resolve-20-Fairlight-Audio-Post.pdf` (tagged
`davinci-resolve`). Both still pass the stress test today (similarity is
high enough off the thin manual alone), but if that stops being true,
consider tagging cross-cutting manuals with multiple products rather than
one.

## Full ingestion run

Completed successfully against the real `books/` corpus: all 22 manuals
indexed, 0 errors. Result: **10,899 chunks**, `dume.db` at **78.7MB**. The
187MB `DaVinci_Resolve_21_Reference_Manual.pdf` (4,444 pages) alone produced
3,523 chunks and ingested without incident — PyMuPDF's lazy page access
kept memory manageable, and the batched `/api/embed` calls (32 chunks/batch)
completed the whole corpus in one background run with no manual
intervention.

## Known gaps

- No Premiere Pro, After Effects, or Blender manuals in `books/` yet —
  Dum-E relies on the web fallback for those tools until PDFs are added.
- `ingestion/` and `books/` are located via a compile-time path
  (`CARGO_MANIFEST_DIR`), correct for `npm run tauri dev` on this machine
  but not yet wired up as bundled resources for a distributable build.

## Phase 2 step 1: DaVinci Resolve — live status + one write action

**Done 2026-09-06.** First slice of Phase 2, deliberately scoped small to
prove out the automation architecture rather than build broad Resolve
control in one pass: read-only project/timeline status (surfaced in Settings
and the About modal) plus one safe, easily-undoable write action (add a
marker at the playhead).

**Architecture**: `resolve_bridge/resolve_bridge.py` — a stdlib-only Python
script (no pip deps; uses the `DaVinciResolveScript` module that ships
inside the Resolve app itself) spawned as a short-lived subprocess per call
from `src-tauri/src/commands/resolve.rs`, mirroring `commands/reindex.rs`'s
existing subprocess pattern but simpler: one JSON object on stdout per call,
not a streamed multi-line job (these are instant point queries/actions, not
a long-running task). Commands: `status`, `project`, `add_marker`. Always
degrades cleanly to `{"running": false, ...}` etc. when Resolve isn't
running — verified explicitly, including in the live UI (Settings correctly
shows "Not running" with no error).

**Verified on this machine**: DaVinci Resolve 21.0.4 (Studio), scripting API
bundled at `/Library/Application Support/Blackmagic Design/DaVinci
Resolve/Developer/Scripting/`. Tested end-to-end against a disposable test
project/timeline (created and deleted via the same scripting API, never
touching real projects): `status`/`project` report correctly; `add_marker`'s
timecode-to-frame math was independently checked against Resolve's own
`GetMarkers()` and lands exactly on the expected frame for non-drop-frame
rates. Also drove the actual running app (Settings → Refresh → live project
info; Settings → Add marker → confirmed via `GetMarkers()` the marker landed
correctly with the right color/name/note) and confirmed the About modal's
new "DaVinci Resolve" status row.

**Bug found + fixed during verification**: `Project.GetSetting("timelineFrameRate")`
returns a raw Python float (e.g. `24.0`) in practice, despite the official
API docs promising a string — this broke deserialization into the Rust
`ResolveTimelineInfo.fps: String` field the first time a real project was
open. Fixed by `str()`-ing it explicitly in the bridge script rather than
loosening the Rust type, so the contract Rust relies on stays a guaranteed
string regardless of what Resolve's API actually returns.

**Deferred to later steps**: timeline editing, rendering, media import,
scoping automation to more than one product at a time, and any equivalent
integration for Premiere Pro / After Effects / Blender.

## Phase 2 step 2: media import + timeline creation + more queries

**Done 2026-09-06.** Deepened the Resolve bridge with a coherent workflow
slice — get footage into Resolve and onto an editable timeline — while still
deferring rendering and destructive timeline editing (trim/delete/reorder
existing clips) to a later step, same discipline as step 1.

**New bridge commands** (`resolve_bridge/resolve_bridge.py`): `list_timelines`
(all timelines in the project), `list_media_pool_clips` (root-folder clips
only — not recursive into subfolders yet), and `import_media` (combined
import + optional timeline creation in **one** subprocess call — a
`MediaPoolItem` from one script invocation can't be reused in another, since
each call reconnects to Resolve fresh, so import and
`CreateTimelineFromClips` must happen together using the exact freshly
returned clip objects rather than a fragile name-based re-lookup across
calls).

**New native file/folder picker**: added `tauri-plugin-dialog` (official
first-party Tauri plugin, already cached locally in both the Cargo registry
and reachable via npm — same low-risk bar as Phase 1's dependency choices).
Settings' new "Import Footage" block uses it for "Add Files…"/"Add Folder…"
rather than a raw text path input, matching the project's UI-quality mandate
for a media-facing tool.

**Verified**: generated a real tiny test clip with `ffmpeg` (not a fabricated
stand-in), tested all three new bridge commands standalone against a
disposable test project (created/deleted via the scripting API, same
approach as step 1) — including the error path (all paths missing) and both
import modes (with/without a timeline name). Then drove the actual running
app end-to-end: typed a timeline name, clicked "Add Files…", the real native
macOS Open panel appeared (confirming the Tauri dialog plugin wiring),
selected the test clip, and confirmed in Resolve's own UI (Media Pool +
Timeline) that the clip was imported and the timeline was created and made
current — the Settings status line auto-refreshed to "1 timeline · 2 clips"
(the timeline itself is also a media-pool "clip" entry in Resolve's own data
model — not a bug, matches `GetRootFolder().GetClipList()`'s real behavior).

**Deferred to a later step**: rendering (needs an async progress-streaming
pattern like `reindex.rs`'s, since renders run long), destructive timeline
editing (trim/delete/reorder existing clips), recursing into media pool
subfolders, and any equivalent integration for Premiere Pro / After Effects /
Blender.

## Phase 2 step 3: rendering + destructive timeline editing + subfolder recursion

**Done 2026-09-06.** Closed out the three pieces deferred from steps 1-2.

**Rendering** (`resolve_bridge/resolve_bridge.py`'s `render` command) is the
first Resolve bridge command that isn't a single JSON object — it streams a
JSON progress line per second (`LoadRenderPreset` → `SetRenderSettings` →
`AddRenderJob` → `StartRendering` → poll `GetRenderJobStatus` while
`IsRenderingInProgress()`), mirroring `ingestion/ingest.py`'s protocol,
because CLAUDE.md requires progress feedback for long-running media exports
and a render can take minutes. `resolve_start_render` in
`commands/resolve.rs` mirrors `reindex.rs::run_reindex`'s subprocess-streaming
structure exactly (`render://progress`/`done`/`error` events). Also added
one-shot `list_render_presets` and `stop_render`. Empirically confirmed
`GetRenderJobStatus`'s actual keys (not fully documented): `JobStatus`,
`CompletionPercentage`, plus bonus `EstimatedTimeRemainingInMs`/
`TimeTakenToRenderInMs` — matched the planned defensive extraction exactly.

**Destructive timeline editing**: confirmed via the API docs that
`TimelineItem` has no trim/move method at all (no `SetStart`/`SetEnd`) — so
this is bounded to `list_timeline_clips` (read), `set_timeline_clip_enabled`
(safe/reversible, the default UI action), and `delete_timeline_clip`
(irreversible via the API — the frontend calls `@tauri-apps/plugin-dialog`'s
native `confirm()` before ever invoking it, verified in the running app:
Cancel leaves the clip untouched, OK deletes it). Every mutating call
addresses the clip by position (`track_type`/`track_index`/`item_index`) and
re-verifies `expected_name` against a fresh list before acting, since Resolve
exposes no stable cross-call clip ID — verified the mismatch path errors
cleanly with a deliberately-wrong name.

**Subfolder recursion**: `list_media_pool_clips` now walks
`Folder.GetSubFolderList()` recursively (depth-capped at 20), tagging each
clip with a `folder_path` (empty for root). Verified with a real nested
"Interviews" subfolder + clip.

**Verified**: all six new/changed bridge commands tested standalone against
disposable test projects first, then the full flow driven through the actual
running app (screenshots + accessibility clicks) — enable/disable, the
native delete-confirmation dialog (both Cancel and OK paths), and a real
render producing a real output file, confirmed via Resolve's own Media
Pool/Timeline/Deliver state throughout, never a real project.

**A debugging note worth keeping**: mid-verification, `StartRendering`
started reliably failing (`AddRenderJob` returning falsy, or succeeding but
`StartRendering` returning `False`) after several rapid render attempts
against the same reused, repeatedly deleted-and-recreated test project name.
Restarting Resolve entirely did *not* fix it. Isolating the test onto a
**freshly, uniquely named** project resolved it immediately — rendering
worked first try. Root cause wasn't pinned down further (not worth the time
for a test-only artifact), but the takeaway: if `StartRendering` ever
misbehaves mysteriously again, try a clean project name before assuming the
bridge code is at fault — rapid create/delete/reuse churn of the same
project name during interactive testing appears to leave Resolve's render
pipeline in a bad state that a plain app restart doesn't clear.

**Deferred**: trim/reorder of existing clips (not exposed by the scripting
API at all — see README's Known limitations), and any equivalent integration
for Premiere Pro / After Effects / Blender.

## Phase 2 step 4: Premiere Pro bridge — first slice, freeze investigated

**In progress, 2026-09-06.** Unlike Resolve, Premiere has no locally callable
scripting module — automation has to run as a UXP panel loaded *inside*
Premiere. `premiere_plugin/` (manifest + `index.js`) polls a shared directory
(`/tmp/dume_premiere_bridge`) for `request.json`, executes the command via
`require("premierepro")`, writes `response-<id>.json`.
`src-tauri/src/commands/premiere.rs::premiere_status` writes the request and
polls for the matching response (5s timeout, treated as "not
running/loaded" rather than an error — same philosophy as the Resolve
bridge). A WebSocket bridge was ruled out: UXP's network permission model has
documented problems with localhost/IP connections.

**Premiere froze mid-testing session** (crash report:
`~/Library/Logs/DiagnosticReports/Adobe Premiere Pro 2026-2026-09-06-164209.ips`,
`OnForceQuit`/`SIGABRT` — a forced quit of a hung app, not an internal
exception). Investigated via the crash report + the matching
`UXPLogs_2026-09-06_16-26-01_483114.log`: **no trace of the Dum-E Bridge
plugin anywhere** (not in the UXP log, no `/tmp/dume_premiere_bridge` on
disk afterward), and — critically — Settings has no UI wired up to call
`premiere_status` at all yet, so the Rust-side poller had never been
invoked either. The last logged activity before the ~11-minute silence
leading to the freeze was Premiere's own Home Screen "Discover Panel"/NPS
survey telemetry throwing internal errors. **Working conclusion: the freeze
was very likely an Adobe Home Screen/CCX Start issue, not caused by Dum-E**
— but this isn't proven, just the best evidence available (nothing else in
Premiere's own logs pointed anywhere near our code).

**Fixed anyway** (cheap insurance regardless of root cause):
`index.js`'s poll loop used `setInterval` calling an async `pollOnce()`
without waiting for it to resolve — a slow poll (disk contention with the
Rust side hitting the same shared folder) could stack overlapping
filesystem calls. Now guarded with an in-flight flag so at most one poll
runs at a time.

**Verified end-to-end after the fix**: relaunched Premiere, opened a real
project (not the Home Screen) instead of trusting the Home Screen surface,
loaded the plugin via UXP Developer Tools with its live console attached,
and drove the file-based bridge directly from the terminal (bypassing the
not-yet-built Settings UI) — 4+ clean `get_status` round trips, Premiere
CPU usage normal (~8%) throughout, no hang.

**Found and diagnosed a real (minor) bug**: `get_status`'s `app_version`
field always came back `undefined` (silently dropped by `JSON.stringify`,
not sent as `null`). Root-caused via a temporary `debug_app` diagnostic
command run through the same live bridge: `ppro.Application` is a
constructor function, not a singleton instance — it has no static factory
method (unlike `Project.getActiveProject()`), and direct construction
(`new ppro.Application()`) is rejected by the native binding with
"Connection to object lost". Confirmed via a control test that
`new ppro.Project()` throws the identical error even though `Project` is
known-good (obtained correctly elsewhere via its factory) — so that error
is a generic guard against direct construction of any of these native-bound
classes, not evidence of a broken connection. **Conclusion: there is no
reachable path to a live `Application` instance through
`require("premierepro")`** — looks like a real gap in Adobe's
`@adobe/premierepro` type declarations (which document `Application.version`
as if `Application` were already an instance) rather than a bug in our
code. `app_version` now explicitly returns `null` with a comment explaining
why, instead of silently vanishing from the JSON. Also independently
verified with a throwaway Rust test that `serde_json` already defaults a
missing `Option<T>` field to `None` without needing `#[serde(default)]` —
so the prior `undefined`-dropped-key behavior was never actually at risk of
breaking deserialization on the Rust side.

**Settings/About UI wired up** (2026-09-06): added `premiere_reachable` to
`AboutInfo` (About modal shows a "Premiere Pro" row exactly like DaVinci
Resolve's) and a "Premiere Pro" section in Settings (status text + manual
Refresh button — no write actions yet, since `get_status` is still the only
bridge command). New `usePremiere` hook mirrors `useResolve`'s shape but
much simpler (read-only). One deliberate asymmetry from Resolve: the
combined `get_about_info` call would otherwise block up to the bridge's
full 5s timeout every time the plugin isn't loaded (the common case),
stalling the whole About dialog since all its checks run before returning —
added a separate `premiere_reachable()` helper in `commands/premiere.rs`
with a short 750ms `REACHABILITY_TIMEOUT` for just this passive check,
while the real `premiere_status` command (used by Settings' explicit
Refresh) keeps the full 5s `TIMEOUT` for patience during real interactive
use.

**Verified end-to-end in the actual running app** (screenshots +
`cliclick`-driven native-window interaction, not just `cargo check`/`tsc`):
About modal shows "Premiere Pro — reachable" (green, styled identically to
Resolve's row) while DaVinci Resolve correctly shows "not running" alongside
it (confirms the two integrations don't cross-contaminate). Settings shows
"Premiere Pro / Connected — Dum-E Test.prproj"; clicked Refresh, state held
correct with no error. Both `cargo check` and `tsc --noEmit` pass clean.

**First write action added** (2026-09-06): `add_marker` — a marker at the
playhead on the active sequence, deliberately the same small, safe,
easily-undoable first slice Resolve started with in step 1. New bridge
protocol field: `request.json` now carries an optional `args` object
(`{name, note}` for this command) alongside `id`/`command`; `call_plugin()`
in `commands/premiere.rs` takes an `args: Option<serde_json::Value>`
parameter that's serialized straight through. New `PremiereActionResult
{ok, message}` type mirrors `ResolveActionResult`. Settings' Premiere Pro
section grew a note field + "Add marker" button, shown only when
`premiere.status.connected`; wired through a new `addMarker` action on
`usePremiere` mirroring `useResolve`'s.

**No color parameter**, unlike Resolve's marker action:
`Markers.createAddMarkerAction()` in the `premierepro` UXP API doesn't
accept one -- setting a marker's color needs a second
`createSetColorByIndexAction` call against the *created* `Marker` object,
which `add_marker` would need to return from the transaction; deferred
since it's not needed for a first slice.

**Bug found + fixed during first live test**: calling
`project.executeTransaction()` directly threw `"Requires locked access"`
(confirmed live, not documented in the `@adobe/premierepro` type
declarations checked while building this). Root cause: Premiere's API
requires mutating calls to run inside `project.lockedAccess()`'s
callback. Fixed by doing the async reads (`sequence.getPlayerPosition()`,
`ppro.Markers.getMarkers(sequence)`) *before* entering `lockedAccess`
(whose callback is synchronous, matching its `() => void` signature), then
calling `createAddMarkerAction`/`executeTransaction` inside it. The failed
first attempt correctly created no marker (verified in Premiere's own
Markers panel -- exactly one marker existed after the fix, not a stray
one from the broken attempt), confirming the error path doesn't leave
partial state.

**Verified end-to-end, twice** -- once via a raw bridge round trip from the
terminal, once through the actual Settings UI (`cliclick`-driven: typed a
note, clicked "Add marker", read back the success message) -- and both
confirmed authoritatively in Premiere's own Markers panel, not just via the
bridge's `ok: true` response: two distinct markers ended up on the
sequence, one per test, each with the exact name/note sent. `cargo check`
and `tsc --noEmit` both clean throughout.

**Second write action added** (2026-09-06): `import_media` — import footage
into the project panel, optionally building a new sequence from exactly
what was imported, mirroring Resolve step 2's `import_media` exactly
(including its native file/folder picker UX via `tauri-plugin-dialog`).
New `PremiereImportResult {ok, message, imported_count, sequence_created}`
type mirrors `ResolveImportResult`. Bridge `args` now also carries
`{paths, sequence_name}` for this command.

**Same lesson as Resolve step 2, re-learned here**: `project.importFiles()`
only returns a success boolean, not the created `ProjectItem`s, and a
`ProjectItem` handed back from one plugin invocation can't be reused in
another (fresh `require("premierepro")` connection each call) -- so
`import_media` does the import *and* the optional
`project.createSequenceFromMedia()` in one bridge call, finding the newly
created clips by diffing the root bin's contents (`getRootItem().getItems()`)
before/after against the requested paths' basenames, since there's no
cross-call clip ID to look them up by otherwise.

**No `lockedAccess` needed here** (unlike `add_marker`): `importFiles()` and
`createSequenceFromMedia()` are high-level async project-mutation methods,
not the low-level synchronous `Action`/`CompoundAction` transaction API --
worked correctly on the first live attempt, no "Requires locked access"
error this time. Confirms that constraint is specific to the
transaction/Action API, not a blanket rule for every mutating call.

**Verified end-to-end, twice**, same rigor as `add_marker`: a real disposable
test clip generated with `ffmpeg` (SMPTE color bars + tone, not a fabricated
stand-in) imported first via a raw bridge call (`ok:true,
{"imported_count":1,"sequence_created":true}`), then through the actual
Settings UI end-to-end including the real native macOS Open panel
(`cliclick`-driven, copied the test file to Desktop first since blind-typing
a path into the sandboxed picker via "Go to folder" proved unreliable) --
both confirmed authoritatively in Premiere itself: Project panel showed the
new sequence + imported clip, the new sequence became the active tab with
the clip already placed on V1/A1, and the video preview showed the actual
test pattern. Settings correctly rendered `"Imported 1 clip and created
sequence \"Dum-E UI Import Test\""`. `cargo check` and `tsc --noEmit` both
clean throughout.

**Deferred / still to do**: targetBin selection (always imports to the
project root for now, matching Resolve's root-only `import_media` from its
own step 2). No Premiere manuals in `books/` yet (web fallback covers
Premiere questions today).

## Phase 2 step 4 continued: closing the gap with Resolve (marker color, timeline editing, render)

**Done 2026-09-06.** Closed out the three remaining gaps between the
Premiere and Resolve bridges: marker color, timeline clip listing/enable,
and rendering.

**Marker color**: `Markers.createAddMarkerAction()` still has no color
parameter, so `add_marker` sets it as a *second* transaction after the
first succeeds, against whichever marker in `markers.getMarkers()` now
matches this one's name + start time (`getName()`/`getStart()`/`TickTime
.equals()` are all synchronous, unlike most of this API) --
`createSetColorByIndexAction(colorIndex)` against `Constants.MarkerColor`
(only 7 colors: green/red/magenta/orange/yellow/blue/cyan, vs. Resolve's
16 -- new `PREMIERE_MARKER_COLORS` constant in `lib/types.ts`, ordered to
match the enum exactly since the index itself is what's sent over the
bridge). Verified live: added a marker with `color_index: 1` (red), zoomed
into a screenshot of Premiere's own Markers panel to confirm the color
swatch rendered red, not just that the call returned `ok: true`.

**Timeline clip listing + enable/disable** (`list_timeline_clips`,
`set_timeline_clip_enabled`): mirrors Resolve's own timeline-clip commands
exactly, including the addressing scheme (`track_type`/`track_index`
/`item_index` plus `expected_name`, re-verified against a fresh lookup
before acting -- Premiere exposes no stable cross-call clip ID either).
**Real API ceiling found, not a scope cut**: grepped the full
`@adobe/premierepro` type declarations for any `createRemove*Action` on
`VideoTrack`/`AudioTrack`/`VideoClipTrackItem` -- there is none. Unlike
Resolve (which supports `delete_timeline_clip`, just not trim/move),
Premiere's scripting API has no way to delete a clip from a timeline at
all. Enable/disable (`createSetDisabledAction`, needs the same
`lockedAccess` wrapping as `add_marker`) is the ceiling here, documented as
such in both `commands/premiere.rs` and the Settings UI copy rather than
silently omitting delete.

**Rendering** (`render` / `premiere_start_render`): uses
`EncoderManager.getManager().exportSequence(sequence,
Constants.ExportType.IMMEDIATELY, outputFile, presetFile, true)`, with
`EncoderManager.getExportFileExtension()` used to build the right output
extension for whatever preset is picked. **Deliberately simpler than
Resolve's render**, which streams `render://progress` events once a second
over a long-running subprocess: this file-based bridge has no equivalent
channel for the plugin to push updates back between polls, so
`premiere_start_render` just blocks on the plugin's response for up to a
new `RENDER_TIMEOUT` (1 hour) while `EXPORT_IMMEDIATELY` makes Premiere
show its own native export progress UI in the meantime -- that's where
CLAUDE.md's progress-feedback mandate actually gets satisfied here, not
through Dum-E's own UI. Also a real, adjacent difference from Resolve:
Premiere's scripting API has no way to enumerate export presets (Resolve's
`GetRenderPresetList()` has no equivalent), so the Settings UI has the user
pick a `.epr` file directly via the native picker rather than choosing from
a dropdown.

**Verified end-to-end for all three**, live in the actual running app, not
just `cargo check`/`tsc`: marker color confirmed via a zoomed screenshot of
Premiere's Markers panel; clip disable confirmed by watching the Program
monitor go black (the disabled clip stopped rendering) and clip enable
verified by re-listing via `list_timeline_clips`; render verified twice
(raw bridge call, then the full Settings UI including the real native
pickers for both the `.epr` preset and the output folder) using a real
`Match Source - Apple ProRes 422 LT.epr` preset already installed with
Premiere, producing a real, correctly-sized 96MB `.mov` `ffprobe`-verified
at both the codec and container level. One real bug caught by this
process, not by inspection: a hand-typed `expected_name` in a manual bridge
test didn't match the real clip name because macOS screenshot filenames
contain a Unicode narrow no-break space (U+202F) between the time and
AM/PM, not a regular space -- `set_timeline_clip_enabled` correctly refused
to act on the mismatch rather than guessing, exactly the safety behavior
the name-reverification design exists for.

## Chat-triggered external tool integration: B-Roll Analyzer (first slice)

**Done 2026-09-06.** Until now, chat and app automation were two entirely
separate systems -- `commands::chat::ask_question` never called into
`commands::resolve`/`commands::premiere` at all, so nothing typed in chat
could trigger a Resolve/Premiere action. This is the first slice closing
that gap, and it does so by reaching *outside* Dum-E entirely: a separate,
much more mature project the user already has
(`~/Developer/Blair/Rough Cut Studio Suite/`, specifically its **B-Roll
Analyzer** app) already solves local b-roll quality scoring -- CLIP-based
vision scoring, sharpness/exposure/stability, best-segment detection --
far better than anything worth rebuilding from scratch here. Rather than
duplicate that work, `broll_bridge/` imports its `analyzer.py` module
directly (read-only, via `sys.path` insert) and shells out to it, the exact
integration trick B-Roll Analyzer's own sibling project, Rough Cut Studio
Suite, already uses on itself via `broll_worker.py` -- and structurally the
same "subprocess + own venv + JSON stdout" pattern `resolve_bridge.py` and
`ingestion/ingest.py` already established in this codebase, just applied to
a project outside Dum-E instead of one inside it.

**Zero modifications to B-Roll Analyzer or its suite** -- confirmed
deliberately, not just assumed: `broll_bridge/` lives entirely inside
Dum-E, only *imports* `analyzer.py` (a read operation), and runs in **its
own separate venv** (`broll_bridge/.venv`, opencv-python + numpy only --
deliberately skips the optional CLIP "high energy" scoring for this first
slice, which would additionally need torch/open_clip) rather than assuming
or touching B-Roll Analyzer's own environment. New `broll_analyzer_path`
config field (Settings UI, text input + native folder-picker Browse
button) points at wherever the user has that project checked out --
nothing is bundled, vendored, or copied.

**Deterministic chat trigger, not LLM tool-calling** -- a deliberate choice
given this project's own Decline-path lesson (chat models don't reliably
follow instructions; see the "Bug found + fixed: Decline path" section
above). `useChat.ts`'s `send()` checks the typed message against a simple
regex (mentions "b-roll" AND a scoring verb) *before* the message ever
reaches `ask_question`/the RAG pipeline -- if matched, it opens a native
folder picker directly instead, then calls the new `broll_score_folder`
command with the chosen path. Real Ollama tool-calling (`command-r7b` is
genuinely good at this) is a reasonable v2 once this pipeline itself is
proven trustworthy; a first slice deliberately removed that variable.

**`broll_score_folder`** (`commands/broll.rs`) mirrors `ask_question`'s own
persistence exactly -- inserts the triggering text as a `user` message,
runs the analysis, inserts the formatted result as an `assistant` message,
and returns that `ChatMessage` -- so a b-roll request looks like a normal
chat turn in history (survives reload, no special frontend rendering
needed). Deliberately simpler than the Resolve/Premiere render commands:
blocks on the whole analysis rather than streaming progress events, since
proving the "chat text -> external tool -> chat text" pipeline was this
slice's actual goal, not a polished progress UI -- and even failures
(missing config, bad folder, bridge error) become a normal assistant reply
rather than a thrown error, so the frontend needed no new error-handling
path either.

**Verified end-to-end, twice**: first the bridge script directly against
two real ffmpeg-generated test clips (one sharp, one deliberately
`gblur`-heavy) -- scored 73.6 vs. 60.0, correctly differentiated on
sharpness. Then the full live pipeline in the running app: set the B-Roll
Analyzer path in Settings, typed "Score my b-roll for me" into chat,
confirmed the trigger fired (optimistic user bubble appeared, native
folder picker opened), picked a real folder, and got back a correctly
formatted, persisted assistant reply -- `"Scored 2 clips in
\`/Users/cj/Desktop/dume_broll_test\` ... sharp_test.mp4 — 74/100 ...
blurry_test.mp4 — 60/100"` -- matching the standalone bridge test exactly.
`cargo check` and `tsc --noEmit` both clean throughout.

**Deferred**: real LLM tool-calling (v2, once this path is trusted); the
optional CLIP "high energy" scoring (needs torch/open_clip in the bridge's
venv); any write action back into the analysis (e.g. "send top clips to
Resolve/Premiere" -- B-Roll Analyzer already has its own "Send to Resolve"
using the same live-scripting-API trick `resolve_bridge.py` uses, worth
studying before reimplementing); richer structured result rendering in
chat (currently plain formatted text, not clip cards).
