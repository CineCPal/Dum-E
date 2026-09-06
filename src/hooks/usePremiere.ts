import { useCallback, useState } from "react";
import { api } from "@/lib/api";
import type { PremiereStatus, PremiereTimelineClip } from "@/lib/types";

export function usePremiere() {
  const [status, setStatus] = useState<PremiereStatus | null>(null);
  const [timelineClips, setTimelineClips] = useState<PremiereTimelineClip[]>([]);
  const [loading, setLoading] = useState(false);
  const [rendering, setRendering] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [actionMessage, setActionMessage] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const s = await api.getPremiereStatus();
      setStatus(s);
      setTimelineClips(s.connected ? await api.getPremiereTimelineClips() : []);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  const addMarker = useCallback(async (name: string, note: string, colorIndex: number | null) => {
    setActionMessage(null);
    setError(null);
    try {
      const result = await api.addPremiereMarker(name, note, colorIndex);
      setActionMessage(result.message);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const importMedia = useCallback(async (paths: string[], sequenceName: string) => {
    setActionMessage(null);
    setError(null);
    try {
      const result = await api.importPremiereMedia(paths, sequenceName.trim() || null);
      setActionMessage(result.message);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const setClipEnabled = useCallback(
    async (clip: PremiereTimelineClip, enabled: boolean) => {
      setActionMessage(null);
      setError(null);
      try {
        const result = await api.setPremiereTimelineClipEnabled(
          clip.track_type,
          clip.track_index,
          clip.item_index,
          clip.name,
          enabled,
        );
        setActionMessage(result.message);
        if (result.ok) await refresh();
      } catch (e) {
        setError(String(e));
      }
    },
    [refresh],
  );

  // Unlike Resolve's render (streamed render://progress events over a
  // long-running subprocess), Premiere's EXPORT_IMMEDIATELY blocks the
  // whole call until done -- this just tracks that wait, while Premiere
  // itself shows its own native export progress UI in the meantime.
  const startRender = useCallback(async (presetFile: string, targetDir: string, customName: string) => {
    setActionMessage(null);
    setError(null);
    setRendering(true);
    try {
      const result = await api.startPremiereRender(presetFile, targetDir, customName.trim() || null);
      setActionMessage(result.message);
    } catch (e) {
      setError(String(e));
    } finally {
      setRendering(false);
    }
  }, []);

  return {
    status,
    timelineClips,
    loading,
    rendering,
    error,
    actionMessage,
    refresh,
    addMarker,
    importMedia,
    setClipEnabled,
    startRender,
  };
}
