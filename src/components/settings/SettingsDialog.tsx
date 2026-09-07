import { confirm, open as openDialog } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Loader2, Play, RefreshCw, Square, Trash2, Upload } from "lucide-react";
import { useEffect, useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import { useConfig } from "@/hooks/useConfig";
import { useReindex } from "@/hooks/useReindex";
import { useRender } from "@/hooks/useRender";
import { useResolve } from "@/hooks/useResolve";
import { usePremiere } from "@/hooks/usePremiere";
import { useBlender } from "@/hooks/useBlender";
import { RESOLVE_MARKER_COLORS, PREMIERE_MARKER_COLORS } from "@/lib/types";

export function SettingsDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const { config, models, update } = useConfig();
  const { running, progress, error: reindexError, start } = useReindex();
  const resolve = useResolve();
  const premiere = usePremiere();
  const blender = useBlender();
  const render = useRender();
  const [markerColor, setMarkerColor] = useState<string>("Blue");
  const [markerNote, setMarkerNote] = useState("");
  const [premiereMarkerColor, setPremiereMarkerColor] = useState<string>("Blue");
  const [premiereMarkerNote, setPremiereMarkerNote] = useState("");
  const [blenderMarkerName, setBlenderMarkerName] = useState("Dum-E Marker");
  const [importTimelineName, setImportTimelineName] = useState("");
  const [premiereImportSequenceName, setPremiereImportSequenceName] = useState("");
  const [blenderImportSceneName, setBlenderImportSceneName] = useState("");
  const [renderPreset, setRenderPreset] = useState("");
  const [renderTargetDir, setRenderTargetDir] = useState("");
  const [renderCustomName, setRenderCustomName] = useState("");
  const [premiereRenderPresetFile, setPremiereRenderPresetFile] = useState("");
  const [premiereRenderTargetDir, setPremiereRenderTargetDir] = useState("");
  const [premiereRenderCustomName, setPremiereRenderCustomName] = useState("");

  const pickAndImport = async (options: { multiple?: boolean; directory?: boolean }) => {
    const selection = await openDialog(options);
    if (!selection) return;
    const paths = Array.isArray(selection) ? selection : [selection];
    await resolve.importMedia(paths, importTimelineName);
  };

  const pickAndImportPremiere = async (options: { multiple?: boolean; directory?: boolean }) => {
    const selection = await openDialog(options);
    if (!selection) return;
    const paths = Array.isArray(selection) ? selection : [selection];
    await premiere.importMedia(paths, premiereImportSequenceName);
  };

  const pickAndImportBlender = async (options: { multiple?: boolean; directory?: boolean }) => {
    const selection = await openDialog(options);
    if (!selection) return;
    const paths = Array.isArray(selection) ? selection : [selection];
    await blender.importMedia(paths, blenderImportSceneName);
  };

  const pickRenderTargetDir = async () => {
    const selection = await openDialog({ directory: true });
    if (typeof selection === "string") setRenderTargetDir(selection);
  };

  const pickPremiereRenderPresetFile = async () => {
    const selection = await openDialog({ filters: [{ name: "Premiere Export Preset", extensions: ["epr"] }] });
    if (typeof selection === "string") setPremiereRenderPresetFile(selection);
  };

  const pickPremiereRenderTargetDir = async () => {
    const selection = await openDialog({ directory: true });
    if (typeof selection === "string") setPremiereRenderTargetDir(selection);
  };

  const confirmDelete = async (clipName: string) => {
    return confirm(`Delete "${clipName}" from the timeline? DaVinci Resolve doesn't expose an undo for this.`, {
      title: "Delete clip",
      kind: "warning",
    });
  };

  useEffect(() => {
    if (open) {
      resolve.refresh();
      premiere.refresh();
      blender.refresh();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  useEffect(() => {
    if (!renderPreset && resolve.renderPresets.length > 0) setRenderPreset(resolve.renderPresets[0]);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [resolve.renderPresets]);

  if (!config) return null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Settings</DialogTitle>
          <DialogDescription>Everything here stays on this machine unless noted.</DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-6">
          <section className="flex flex-col gap-2">
            <Label htmlFor="chat-model">Chat model</Label>
            <select
              id="chat-model"
              className="h-9 rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
              value={config.chat_model}
              onChange={(e) => update({ chat_model: e.target.value })}
            >
              {!models.includes(config.chat_model) && <option value={config.chat_model}>{config.chat_model}</option>}
              {models.map((m) => (
                <option key={m} value={m}>
                  {m}
                </option>
              ))}
            </select>
            <p className="text-xs text-muted-foreground">
              Command-R7B is recommended for grounded, cited answers. Switch to a smaller model
              (e.g. llama3.2:3b) if you're editing in Resolve or Premiere at the same time.
            </p>
          </section>

          <Separator />

          <section className="flex flex-col gap-3">
            <div className="flex items-center justify-between">
              <div>
                <Label htmlFor="fallback-toggle">Internet search fallback</Label>
                <p className="text-xs text-muted-foreground">
                  Only your question text is sent to your own local SearXNG instance, and only
                  when the manuals don't cover it. No API key, no cloud vendor.
                </p>
              </div>
              <Switch
                id="fallback-toggle"
                checked={config.fallback_enabled}
                onCheckedChange={(checked) => update({ fallback_enabled: checked })}
              />
            </div>

            <div className="flex flex-col gap-2">
              <Label htmlFor="searxng-url">SearXNG URL</Label>
              <Input
                id="searxng-url"
                value={config.searxng_url}
                onChange={(e) => update({ searxng_url: e.target.value })}
                placeholder="http://127.0.0.1:8888"
              />
              <p className="text-xs text-muted-foreground">
                Defaults to a SearXNG instance run locally via Docker — see{" "}
                <code className="rounded bg-muted px-1 py-0.5">searxng/README.md</code>.
              </p>
            </div>

            <div className="flex flex-col gap-2">
              <Label htmlFor="broll-analyzer-path">B-Roll Analyzer path</Label>
              <div className="flex gap-2">
                <Input
                  id="broll-analyzer-path"
                  className="flex-1"
                  value={config.broll_analyzer_path ?? ""}
                  onChange={(e) => update({ broll_analyzer_path: e.target.value })}
                  placeholder="~/Developer/Blair/B-Roll Analyzer"
                />
                <Button
                  variant="secondary"
                  onClick={async () => {
                    const selection = await openDialog({ directory: true });
                    if (typeof selection === "string") await update({ broll_analyzer_path: selection });
                  }}
                >
                  <FolderOpen className="size-4" />
                  Browse…
                </Button>
              </div>
              <p className="text-xs text-muted-foreground">
                A separate, unmodified checkout of the B-Roll Analyzer project (not bundled with
                Dum-E) — the folder containing its <code className="rounded bg-muted px-1 py-0.5">analyzer.py</code>.
                Lets chat score a folder of b-roll on request — see{" "}
                <code className="rounded bg-muted px-1 py-0.5">broll_bridge/README.md</code> for setup.
              </p>
            </div>
          </section>

          <Separator />

          <section className="flex flex-col gap-2">
            <div className="flex items-center justify-between">
              <div>
                <Label>Manual index</Label>
                <p className="text-xs text-muted-foreground">
                  Re-scans books/ and embeds any new or changed manuals. Unchanged files are
                  skipped automatically.
                </p>
              </div>
              <Button variant="secondary" onClick={start} disabled={running}>
                {running ? <Loader2 className="size-4 animate-spin" /> : <RefreshCw className="size-4" />}
                Re-index
              </Button>
            </div>
            {progress && (
              <p className="text-xs text-muted-foreground">
                {progress.stage}
                {progress.manual ? ` — ${progress.manual}` : ""}
                {progress.total_chunks ? ` (${progress.chunk ?? 0}/${progress.total_chunks} chunks)` : ""}
              </p>
            )}
            {reindexError && <p className="text-xs text-destructive">{reindexError}</p>}
          </section>

          <Separator />

          <section className="flex flex-col gap-3">
            <div className="flex items-center justify-between">
              <div>
                <Label>DaVinci Resolve</Label>
                <p className="text-xs text-muted-foreground">
                  {resolve.status?.running
                    ? resolve.projectInfo?.has_project
                      ? `Connected — ${resolve.projectInfo.project_name}${
                          resolve.projectInfo.current_timeline
                            ? ` / ${resolve.projectInfo.current_timeline.name} (${resolve.projectInfo.current_timeline.fps}fps)`
                            : ""
                        } · ${resolve.timelines.length} timeline${resolve.timelines.length === 1 ? "" : "s"} · ${resolve.clips.length} clip${resolve.clips.length === 1 ? "" : "s"}`
                      : "Connected — no project open"
                    : "Not running"}
                </p>
              </div>
              <Button variant="secondary" onClick={resolve.refresh} disabled={resolve.loading}>
                {resolve.loading ? <Loader2 className="size-4 animate-spin" /> : <RefreshCw className="size-4" />}
                Refresh
              </Button>
            </div>

            {resolve.status?.running && resolve.projectInfo?.current_timeline && (
              <div className="flex flex-col gap-2 rounded-md border border-border p-3">
                <p className="text-xs text-muted-foreground">
                  Adds a marker at the current playhead position on the open timeline.
                </p>
                <div className="flex gap-2">
                  <select
                    className="h-9 rounded-md border border-input bg-transparent px-2 text-sm shadow-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    value={markerColor}
                    onChange={(e) => setMarkerColor(e.target.value)}
                  >
                    {RESOLVE_MARKER_COLORS.map((c) => (
                      <option key={c} value={c}>
                        {c}
                      </option>
                    ))}
                  </select>
                  <Input
                    className="flex-1"
                    placeholder="Note (optional)"
                    value={markerNote}
                    onChange={(e) => setMarkerNote(e.target.value)}
                  />
                  <Button
                    variant="secondary"
                    onClick={() => resolve.addMarker(markerColor, "Dum-E Marker", markerNote)}
                  >
                    Add marker
                  </Button>
                </div>
              </div>
            )}

            {resolve.status?.running && resolve.projectInfo?.has_project && (
              <div className="flex flex-col gap-2 rounded-md border border-border p-3">
                <p className="text-xs text-muted-foreground">
                  Imports footage into the media pool, and optionally builds a new timeline from
                  exactly what you pick.
                </p>
                <Input
                  placeholder="New timeline name (optional)"
                  value={importTimelineName}
                  onChange={(e) => setImportTimelineName(e.target.value)}
                />
                <div className="flex gap-2">
                  <Button variant="secondary" onClick={() => pickAndImport({ multiple: true })}>
                    <Upload className="size-4" />
                    Add Files…
                  </Button>
                  <Button variant="secondary" onClick={() => pickAndImport({ directory: true })}>
                    <FolderOpen className="size-4" />
                    Add Folder…
                  </Button>
                </div>
              </div>
            )}

            {resolve.status?.running && resolve.projectInfo?.current_timeline && resolve.timelineClips.length > 0 && (
              <div className="flex flex-col gap-2 rounded-md border border-border p-3">
                <p className="text-xs text-muted-foreground">
                  Clips on the current timeline. Disabling is reversible; deleting is not — Resolve
                  exposes no undo for it via scripting.
                </p>
                <div className="flex flex-col gap-1.5">
                  {resolve.timelineClips.map((clip) => (
                    <div
                      key={`${clip.track_type}-${clip.track_index}-${clip.item_index}`}
                      className="flex items-center justify-between gap-2 text-sm"
                    >
                      <span className={clip.enabled ? "" : "text-muted-foreground line-through"}>
                        {clip.name}{" "}
                        <span className="text-xs text-muted-foreground">
                          ({clip.track_type} {clip.track_index})
                        </span>
                      </span>
                      <div className="flex shrink-0 items-center gap-2">
                        <Switch
                          checked={clip.enabled}
                          onCheckedChange={(checked) => resolve.setClipEnabled(clip, checked)}
                        />
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={async () => {
                            if (await confirmDelete(clip.name)) await resolve.deleteClip(clip, false);
                          }}
                        >
                          <Trash2 className="size-4 text-destructive" />
                        </Button>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {resolve.actionMessage && <p className="text-xs text-muted-foreground">{resolve.actionMessage}</p>}
            {resolve.error && <p className="text-xs text-destructive">{resolve.error}</p>}
          </section>

          <Separator />

          <section className="flex flex-col gap-3">
            <div>
              <Label>Render</Label>
              <p className="text-xs text-muted-foreground">
                Renders the current timeline using a saved Resolve render preset.
              </p>
            </div>

            {resolve.status?.running && resolve.projectInfo?.current_timeline ? (
              <div className="flex flex-col gap-2">
                <select
                  className="h-9 rounded-md border border-input bg-transparent px-2 text-sm shadow-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  value={renderPreset}
                  onChange={(e) => setRenderPreset(e.target.value)}
                  disabled={render.running}
                >
                  {resolve.renderPresets.map((p) => (
                    <option key={p} value={p}>
                      {p}
                    </option>
                  ))}
                </select>
                <div className="flex gap-2">
                  <Input
                    className="flex-1"
                    placeholder="Output folder"
                    value={renderTargetDir}
                    onChange={(e) => setRenderTargetDir(e.target.value)}
                    disabled={render.running}
                  />
                  <Button variant="secondary" onClick={pickRenderTargetDir} disabled={render.running}>
                    <FolderOpen className="size-4" />
                    Browse…
                  </Button>
                </div>
                <Input
                  placeholder="Output filename (optional)"
                  value={renderCustomName}
                  onChange={(e) => setRenderCustomName(e.target.value)}
                  disabled={render.running}
                />
                <div className="flex items-center gap-2">
                  <Button
                    variant="secondary"
                    onClick={() => render.start(renderPreset, renderTargetDir, renderCustomName)}
                    disabled={render.running || !renderPreset || !renderTargetDir}
                  >
                    {render.running ? <Loader2 className="size-4 animate-spin" /> : <Play className="size-4" />}
                    Render
                  </Button>
                  {render.running && (
                    <Button variant="ghost" onClick={render.stop}>
                      <Square className="size-4" />
                      Cancel
                    </Button>
                  )}
                </div>
                {render.progress && (
                  <p className="text-xs text-muted-foreground">
                    {render.progress.stage}
                    {render.progress.completion_percentage != null
                      ? ` — ${render.progress.completion_percentage}%`
                      : ""}
                  </p>
                )}
                {render.error && <p className="text-xs text-destructive">{render.error}</p>}
              </div>
            ) : (
              <p className="text-xs text-muted-foreground">Open a project with a timeline in Resolve to render.</p>
            )}
          </section>

          <Separator />

          <section className="flex flex-col gap-3">
            <div className="flex items-center justify-between">
              <div>
                <Label>Premiere Pro</Label>
                <p className="text-xs text-muted-foreground">
                  {premiere.status?.connected
                    ? `Connected${
                        premiere.status.active_project_name ? ` — ${premiere.status.active_project_name}` : ""
                      }`
                    : "Not connected — load the Dum-E Bridge panel in Premiere"}
                </p>
              </div>
              <Button variant="secondary" onClick={premiere.refresh} disabled={premiere.loading}>
                {premiere.loading ? <Loader2 className="size-4 animate-spin" /> : <RefreshCw className="size-4" />}
                Refresh
              </Button>
            </div>

            {premiere.status?.connected && (
              <div className="flex flex-col gap-2 rounded-md border border-border p-3">
                <p className="text-xs text-muted-foreground">
                  Adds a marker at the current playhead position on the active sequence.
                </p>
                <div className="flex gap-2">
                  <select
                    className="h-9 rounded-md border border-input bg-transparent px-2 text-sm shadow-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    value={premiereMarkerColor}
                    onChange={(e) => setPremiereMarkerColor(e.target.value)}
                  >
                    {PREMIERE_MARKER_COLORS.map((c) => (
                      <option key={c} value={c}>
                        {c}
                      </option>
                    ))}
                  </select>
                  <Input
                    className="flex-1"
                    placeholder="Note (optional)"
                    value={premiereMarkerNote}
                    onChange={(e) => setPremiereMarkerNote(e.target.value)}
                  />
                  <Button
                    variant="secondary"
                    onClick={() =>
                      premiere.addMarker(
                        "Dum-E Marker",
                        premiereMarkerNote,
                        PREMIERE_MARKER_COLORS.indexOf(premiereMarkerColor as (typeof PREMIERE_MARKER_COLORS)[number]),
                      )
                    }
                  >
                    Add marker
                  </Button>
                </div>
              </div>
            )}

            {premiere.status?.connected && (
              <div className="flex flex-col gap-2 rounded-md border border-border p-3">
                <p className="text-xs text-muted-foreground">
                  Imports footage into the project panel, and optionally builds a new sequence from
                  exactly what you pick.
                </p>
                <Input
                  placeholder="New sequence name (optional)"
                  value={premiereImportSequenceName}
                  onChange={(e) => setPremiereImportSequenceName(e.target.value)}
                />
                <div className="flex gap-2">
                  <Button variant="secondary" onClick={() => pickAndImportPremiere({ multiple: true })}>
                    <Upload className="size-4" />
                    Add Files…
                  </Button>
                  <Button variant="secondary" onClick={() => pickAndImportPremiere({ directory: true })}>
                    <FolderOpen className="size-4" />
                    Add Folder…
                  </Button>
                </div>
              </div>
            )}

            {premiere.status?.connected && premiere.timelineClips.length > 0 && (
              <div className="flex flex-col gap-2 rounded-md border border-border p-3">
                <p className="text-xs text-muted-foreground">
                  Clips on the active sequence. Premiere's scripting API only exposes enable/disable
                  here — no way to delete a clip from a timeline via scripting at all.
                </p>
                <div className="flex flex-col gap-1.5">
                  {premiere.timelineClips.map((clip) => (
                    <div
                      key={`${clip.track_type}-${clip.track_index}-${clip.item_index}`}
                      className="flex items-center justify-between gap-2 text-sm"
                    >
                      <span className={clip.enabled ? "" : "text-muted-foreground line-through"}>
                        {clip.name}{" "}
                        <span className="text-xs text-muted-foreground">
                          ({clip.track_type} {clip.track_index})
                        </span>
                      </span>
                      <Switch
                        checked={clip.enabled}
                        onCheckedChange={(checked) => premiere.setClipEnabled(clip, checked)}
                      />
                    </div>
                  ))}
                </div>
              </div>
            )}

            {premiere.status?.connected && (
              <div className="flex flex-col gap-2 rounded-md border border-border p-3">
                <p className="text-xs text-muted-foreground">
                  Exports the active sequence using a saved Premiere export preset (.epr). Premiere
                  shows its own progress window during the export.
                </p>
                <div className="flex gap-2">
                  <Input
                    className="flex-1"
                    placeholder="Export preset (.epr)"
                    value={premiereRenderPresetFile}
                    onChange={(e) => setPremiereRenderPresetFile(e.target.value)}
                    disabled={premiere.rendering}
                  />
                  <Button variant="secondary" onClick={pickPremiereRenderPresetFile} disabled={premiere.rendering}>
                    <FolderOpen className="size-4" />
                    Browse…
                  </Button>
                </div>
                <div className="flex gap-2">
                  <Input
                    className="flex-1"
                    placeholder="Output folder"
                    value={premiereRenderTargetDir}
                    onChange={(e) => setPremiereRenderTargetDir(e.target.value)}
                    disabled={premiere.rendering}
                  />
                  <Button variant="secondary" onClick={pickPremiereRenderTargetDir} disabled={premiere.rendering}>
                    <FolderOpen className="size-4" />
                    Browse…
                  </Button>
                </div>
                <Input
                  placeholder="Output filename (optional)"
                  value={premiereRenderCustomName}
                  onChange={(e) => setPremiereRenderCustomName(e.target.value)}
                  disabled={premiere.rendering}
                />
                <Button
                  variant="secondary"
                  onClick={() =>
                    premiere.startRender(premiereRenderPresetFile, premiereRenderTargetDir, premiereRenderCustomName)
                  }
                  disabled={premiere.rendering || !premiereRenderPresetFile || !premiereRenderTargetDir}
                >
                  {premiere.rendering ? <Loader2 className="size-4 animate-spin" /> : <Play className="size-4" />}
                  {premiere.rendering ? "Rendering…" : "Render"}
                </Button>
              </div>
            )}

            {premiere.actionMessage && <p className="text-xs text-muted-foreground">{premiere.actionMessage}</p>}
            {premiere.error && <p className="text-xs text-destructive">{premiere.error}</p>}
          </section>

          <Separator />

          <section className="flex flex-col gap-3">
            <div className="flex items-center justify-between">
              <div>
                <Label>Blender</Label>
                <p className="text-xs text-muted-foreground">
                  {blender.status?.running
                    ? `Connected — ${blender.sceneInfo?.scene_name ?? blender.status.scene_name ?? "scene"}${
                        blender.status.file_path ? "" : " (unsaved)"
                      }`
                    : "Not running — install and enable the Dum-E Bridge add-on in Blender"}
                </p>
              </div>
              <Button variant="secondary" onClick={blender.refresh} disabled={blender.loading}>
                {blender.loading ? <Loader2 className="size-4 animate-spin" /> : <RefreshCw className="size-4" />}
                Refresh
              </Button>
            </div>

            {blender.status?.running && blender.sceneInfo && (
              <div className="flex flex-col gap-1 rounded-md border border-border p-3 text-xs text-muted-foreground">
                <span>
                  Frame {blender.sceneInfo.frame_current} of {blender.sceneInfo.frame_start}–
                  {blender.sceneInfo.frame_end} · {blender.sceneInfo.fps}fps
                </span>
                <span>
                  {blender.sceneInfo.render_engine} · {blender.sceneInfo.object_count} object
                  {blender.sceneInfo.object_count === 1 ? "" : "s"}
                </span>
              </div>
            )}

            {blender.status?.running && (
              <div className="flex flex-col gap-2 rounded-md border border-border p-3">
                <p className="text-xs text-muted-foreground">
                  Adds a timeline marker at the current frame. Blender's markers have no color —
                  just a name and a frame.
                </p>
                <div className="flex gap-2">
                  <Input
                    className="flex-1"
                    placeholder="Marker name"
                    value={blenderMarkerName}
                    onChange={(e) => setBlenderMarkerName(e.target.value)}
                  />
                  <Button variant="secondary" onClick={() => blender.addMarker(blenderMarkerName || "Dum-E Marker")}>
                    Add marker
                  </Button>
                </div>
              </div>
            )}

            {blender.status?.running && (
              <div className="flex flex-col gap-2 rounded-md border border-border p-3">
                <p className="text-xs text-muted-foreground">
                  Imports footage into the Video Sequencer (one clip per channel, not a sequential
                  timeline yet), and optionally builds a new scene from exactly what you pick.
                </p>
                <Input
                  placeholder="New scene name (optional)"
                  value={blenderImportSceneName}
                  onChange={(e) => setBlenderImportSceneName(e.target.value)}
                />
                <div className="flex gap-2">
                  <Button variant="secondary" onClick={() => pickAndImportBlender({ multiple: true })}>
                    <Upload className="size-4" />
                    Add Files…
                  </Button>
                  <Button variant="secondary" onClick={() => pickAndImportBlender({ directory: true })}>
                    <FolderOpen className="size-4" />
                    Add Folder…
                  </Button>
                </div>
              </div>
            )}

            {blender.actionMessage && <p className="text-xs text-muted-foreground">{blender.actionMessage}</p>}
            {blender.error && <p className="text-xs text-destructive">{blender.error}</p>}
          </section>
        </div>
      </DialogContent>
    </Dialog>
  );
}
