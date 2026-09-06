/**
 * Dum-E Bridge -- lets Dum-E (an external desktop app) read live status from
 * Premiere Pro. No network access: communicates purely via polling files in
 * a shared local directory, since UXP's network permission model has real,
 * documented problems with localhost/IP connections (see PLAN.md).
 *
 * Protocol: Dum-E writes request.json ({id, command, args}) into BRIDGE_DIR.
 * This plugin polls for it, deletes it immediately (never processed twice),
 * executes the command, and writes response-<id>.json ({id, ok, data}).
 */

const uxp = require("uxp");
const fs = uxp.storage.localFileSystem;
const { types } = uxp.storage;

// Fixed, literal path -- deliberately NOT under the user's home directory,
// since resolving "~" from UXP's sandboxed JS environment isn't reliably
// available. Both sides (this plugin and src-tauri/src/commands/premiere.rs)
// hardcode this exact same path.
const BRIDGE_DIR = "/tmp/dume_premiere_bridge";
const POLL_INTERVAL_MS = 500;

const statusEl = document.getElementById("status");
const logEl = document.getElementById("log");

function log(message) {
  const line = `${new Date().toLocaleTimeString()}  ${message}`;
  console.log("[Dum-E Bridge]", message);
  logEl.textContent = `${line}\n${logEl.textContent}`.split("\n").slice(0, 20).join("\n");
}

async function getBridgeFolder() {
  const url = `file:${BRIDGE_DIR}`;
  try {
    return await fs.getEntryWithUrl(url);
  } catch (e) {
    return await fs.createEntryWithUrl(url, { type: types.folder, overwrite: false });
  }
}

async function handleCommand(command, args) {
  const ppro = require("premierepro");
  if (command === "get_status") {
    const project = await ppro.Project.getActiveProject();
    return {
      // ppro.Application has no static factory (unlike Project's
      // getActiveProject()) and its instance can't be constructed directly
      // (native binding rejects `new Application()` -- confirmed the same
      // rejection happens for `new Project()`, so this is a generic guard,
      // not something specific to Application being broken). There is no
      // reachable path to a live Application instance through this module,
      // so app_version is always null for now -- confirmed via
      // debug_app diagnostics 2026-09-06, see PLAN.md.
      app_version: null,
      active_project_name: project ? project.name : null,
    };
  }
  if (command === "add_marker") {
    const project = await ppro.Project.getActiveProject();
    if (!project) throw new Error("No active project open");
    const sequence = await project.getActiveSequence();
    if (!sequence) throw new Error("No active sequence open");

    const playerPosition = await sequence.getPlayerPosition();
    const markers = await ppro.Markers.getMarkers(sequence);
    const name = (args && args.name) || "Dum-E Marker";
    const note = (args && args.note) || "";
    const colorIndex = args && typeof args.color_index === "number" ? args.color_index : null;

    // executeTransaction() throws "Requires locked access" if called
    // directly -- confirmed live 2026-09-06 -- so the actual mutation has
    // to happen inside project.lockedAccess()'s (synchronous) callback.
    // The async reads above (getPlayerPosition/getMarkers) already
    // resolved before we get here, so lockedAccess only wraps synchronous
    // calls, matching its `() => void` signature.
    let success = false;
    project.lockedAccess(() => {
      const addAction = markers.createAddMarkerAction(
        name,
        ppro.Marker.MARKER_TYPE_COMMENT,
        playerPosition,
        ppro.TickTime.TIME_ZERO,
        note
      );
      success = project.executeTransaction((compoundAction) => {
        compoundAction.addAction(addAction);
      }, "Dum-E: Add Marker");
    });
    if (!success) throw new Error("Premiere rejected the add-marker transaction");

    // Markers.createAddMarkerAction() has no color parameter (unlike
    // Resolve's add_marker) -- there's no handle to the just-created Marker
    // either (the Action object isn't it), so color is set as a second,
    // separate transaction against whichever marker now matches this one's
    // name+start time. getMarkers()/getName()/getStart() are synchronous,
    // unlike most of this API.
    if (colorIndex !== null) {
      const updatedMarkers = markers.getMarkers();
      const newMarker = updatedMarkers.find(
        (m) => m.getName() === name && m.getStart().equals(playerPosition)
      );
      if (newMarker) {
        project.lockedAccess(() => {
          const colorAction = newMarker.createSetColorByIndexAction(colorIndex);
          project.executeTransaction((compoundAction) => {
            compoundAction.addAction(colorAction);
          }, "Dum-E: Set Marker Color");
        });
      }
    }
    return true;
  }
  if (command === "import_media") {
    const project = await ppro.Project.getActiveProject();
    if (!project) throw new Error("No active project open");
    const filePaths = (args && args.paths) || [];
    if (filePaths.length === 0) throw new Error("No file paths given");
    const sequenceName = args && args.sequence_name;

    // importFiles() only returns a success boolean, not the created
    // ProjectItems -- and (same lesson as Resolve step 2) a ProjectItem
    // handed back from one plugin invocation can't be reused in another,
    // so import and sequence creation happen together in this one call.
    // The created clips are found by diffing the root bin's contents
    // before/after against the basenames of the requested paths, since
    // there's no cross-call clip ID to look them up by otherwise.
    const rootItem = await project.getRootItem();
    const beforeNames = new Set((await rootItem.getItems()).map((item) => item.name));
    const requestedBasenames = new Set(filePaths.map((p) => p.split("/").pop()));

    const ok = await project.importFiles(filePaths, true, rootItem, false);
    if (!ok) throw new Error("Premiere reported the import failed");

    const afterItems = await rootItem.getItems();
    const newClipItems = afterItems.filter(
      (item) =>
        item.type === ppro.ProjectItem.TYPE_CLIP &&
        requestedBasenames.has(item.name) &&
        !beforeNames.has(item.name)
    );

    let sequenceCreated = false;
    if (sequenceName && newClipItems.length > 0) {
      const sequence = await project.createSequenceFromMedia(sequenceName, newClipItems);
      sequenceCreated = !!sequence;
    }

    return { imported_count: newClipItems.length, sequence_created: sequenceCreated };
  }
  if (command === "list_timeline_clips") {
    const project = await ppro.Project.getActiveProject();
    if (!project) throw new Error("No active project open");
    const sequence = await project.getActiveSequence();
    if (!sequence) throw new Error("No active sequence open");

    const clips = [];
    const videoTrackCount = await sequence.getVideoTrackCount();
    for (let t = 0; t < videoTrackCount; t++) {
      const track = await sequence.getVideoTrack(t);
      const items = track.getTrackItems(ppro.Constants.TrackItemType.CLIP, false);
      for (let i = 0; i < items.length; i++) {
        clips.push({
          track_type: "video",
          track_index: t,
          item_index: i,
          name: await items[i].getName(),
          enabled: !(await items[i].isDisabled()),
        });
      }
    }
    const audioTrackCount = await sequence.getAudioTrackCount();
    for (let t = 0; t < audioTrackCount; t++) {
      const track = await sequence.getAudioTrack(t);
      const items = track.getTrackItems(ppro.Constants.TrackItemType.CLIP, false);
      for (let i = 0; i < items.length; i++) {
        clips.push({
          track_type: "audio",
          track_index: t,
          item_index: i,
          name: await items[i].getName(),
          enabled: !(await items[i].isDisabled()),
        });
      }
    }
    return { clips };
  }
  if (command === "set_timeline_clip_enabled") {
    const project = await ppro.Project.getActiveProject();
    if (!project) throw new Error("No active project open");
    const sequence = await project.getActiveSequence();
    if (!sequence) throw new Error("No active sequence open");

    const { track_type, track_index, item_index, expected_name, enabled } = args || {};
    const track =
      track_type === "video"
        ? await sequence.getVideoTrack(track_index)
        : await sequence.getAudioTrack(track_index);
    if (!track) throw new Error(`No ${track_type} track at index ${track_index}`);

    const items = track.getTrackItems(ppro.Constants.TrackItemType.CLIP, false);
    const item = items[item_index];
    if (!item) throw new Error(`No clip at ${track_type} track ${track_index}, position ${item_index}`);

    // Same discipline as Resolve's timeline-clip commands: no stable
    // cross-call clip ID exists here either, so re-verify the name at this
    // position before acting rather than trusting a stale position.
    const actualName = await item.getName();
    if (actualName !== expected_name) {
      throw new Error(`Clip name mismatch: expected "${expected_name}", found "${actualName}"`);
    }

    let success = false;
    project.lockedAccess(() => {
      const action = item.createSetDisabledAction(!enabled);
      success = project.executeTransaction((compoundAction) => {
        compoundAction.addAction(action);
      }, "Dum-E: Set Clip Enabled");
    });
    if (!success) throw new Error("Premiere rejected the transaction");
    return true;
  }
  if (command === "render") {
    const project = await ppro.Project.getActiveProject();
    if (!project) throw new Error("No active project open");
    const sequence = await project.getActiveSequence();
    if (!sequence) throw new Error("No active sequence open");

    const presetFile = args && args.preset_file;
    const targetDir = args && args.target_dir;
    if (!presetFile) throw new Error("No render preset (.epr) file given");
    if (!targetDir) throw new Error("No target directory given");

    const manager = ppro.EncoderManager.getManager();
    const extension = await ppro.EncoderManager.getExportFileExtension(sequence, presetFile);
    const baseName = (args && args.custom_name) || sequence.name;
    const outputFile = `${targetDir}/${baseName}.${extension}`;

    // EXPORT_IMMEDIATELY blocks this call until the render finishes rather
    // than queuing to Adobe Media Encoder -- simpler than Resolve's
    // render command, which streams progress once a second over a
    // long-running subprocess, but there's no equivalent event channel
    // from this stateless request/response bridge back to Dum-E to stream
    // through. Premiere shows its own native progress UI for the export
    // in the meantime, satisfying CLAUDE.md's progress-feedback mandate
    // without building a second streaming subsystem. See PLAN.md.
    const ok = await manager.exportSequence(
      sequence,
      ppro.Constants.ExportType.IMMEDIATELY,
      outputFile,
      presetFile,
      true
    );
    if (!ok) throw new Error("Premiere reported the export failed");
    return { output_file: outputFile };
  }
  throw new Error(`Unknown command: ${command}`);
}

async function pollOnce() {
  let folder;
  try {
    folder = await getBridgeFolder();
  } catch (e) {
    statusEl.textContent = `Can't access bridge folder: ${e.message}`;
    return;
  }

  let entries;
  try {
    entries = await folder.getEntries();
  } catch (e) {
    statusEl.textContent = `Can't list bridge folder: ${e.message}`;
    return;
  }

  const requestEntry = entries.find((entry) => entry.name === "request.json");
  if (!requestEntry) {
    return;
  }

  let request;
  try {
    const text = await requestEntry.read();
    await requestEntry.delete();
    request = JSON.parse(text);
  } catch (e) {
    log(`Failed to read/parse request: ${e.message}`);
    return;
  }

  statusEl.textContent = `Handling "${request.command}"…`;
  let payload;
  try {
    const data = await handleCommand(request.command, request.args);
    payload = { id: request.id, ok: true, data };
  } catch (e) {
    payload = { id: request.id, ok: false, error: String(e.message || e) };
  }

  try {
    const responseFile = await folder.createFile(`response-${request.id}.json`, { overwrite: true });
    await responseFile.write(JSON.stringify(payload));
    statusEl.textContent = `Connected -- last handled "${request.command}"`;
    log(`Handled "${request.command}" -> ${JSON.stringify(payload.data ?? payload.error)}`);
  } catch (e) {
    log(`Failed to write response: ${e.message}`);
  }
}

statusEl.textContent = "Waiting for Dum-E…";
log(`Polling ${BRIDGE_DIR} every ${POLL_INTERVAL_MS}ms`);

// A plain setInterval would fire again before a slow pollOnce() resolves,
// stacking overlapping filesystem calls against the same shared folder.
// This self-rescheduling loop guarantees at most one poll in flight.
let polling = false;
setInterval(() => {
  if (polling) return;
  polling = true;
  pollOnce()
    .catch((e) => log(`Poll loop error: ${e.message}`))
    .finally(() => {
      polling = false;
    });
}, POLL_INTERVAL_MS);
