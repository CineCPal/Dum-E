import { useCallback, useState } from "react";
import { api } from "@/lib/api";
import type {
  ResolveClipInfo,
  ResolveProjectInfo,
  ResolveStatus,
  ResolveTimelineClip,
  ResolveTimelineSummary,
} from "@/lib/types";

export function useResolve() {
  const [status, setStatus] = useState<ResolveStatus | null>(null);
  const [projectInfo, setProjectInfo] = useState<ResolveProjectInfo | null>(null);
  const [timelines, setTimelines] = useState<ResolveTimelineSummary[]>([]);
  const [clips, setClips] = useState<ResolveClipInfo[]>([]);
  const [timelineClips, setTimelineClips] = useState<ResolveTimelineClip[]>([]);
  const [renderPresets, setRenderPresets] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [actionMessage, setActionMessage] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const s = await api.getResolveStatus();
      setStatus(s);
      if (s.running) {
        const [p, t, c, tc, rp] = await Promise.all([
          api.getResolveProjectInfo(),
          api.getResolveTimelines(),
          api.getResolveMediaPoolClips(),
          api.getResolveTimelineClips(),
          api.getResolveRenderPresets(),
        ]);
        setProjectInfo(p);
        setTimelines(t);
        setClips(c);
        setTimelineClips(tc);
        setRenderPresets(rp);
      } else {
        setProjectInfo(null);
        setTimelines([]);
        setClips([]);
        setTimelineClips([]);
        setRenderPresets([]);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  const addMarker = useCallback(async (color: string, name: string, note: string) => {
    setActionMessage(null);
    setError(null);
    try {
      const result = await api.addResolveMarker(color, name, note);
      setActionMessage(result.message);
      if (result.ok) await refresh();
    } catch (e) {
      setError(String(e));
    }
  }, [refresh]);

  const importMedia = useCallback(async (paths: string[], timelineName: string) => {
    setActionMessage(null);
    setError(null);
    try {
      const result = await api.importResolveMedia(paths, timelineName.trim() || null);
      setActionMessage(result.message);
      if (result.ok) await refresh();
    } catch (e) {
      setError(String(e));
    }
  }, [refresh]);

  const setClipEnabled = useCallback(
    async (clip: ResolveTimelineClip, enabled: boolean) => {
      setActionMessage(null);
      setError(null);
      try {
        const result = await api.setResolveTimelineClipEnabled(
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

  const deleteClip = useCallback(
    async (clip: ResolveTimelineClip, ripple: boolean) => {
      setActionMessage(null);
      setError(null);
      try {
        const result = await api.deleteResolveTimelineClip(
          clip.track_type,
          clip.track_index,
          clip.item_index,
          clip.name,
          ripple,
        );
        setActionMessage(result.message);
        if (result.ok) await refresh();
      } catch (e) {
        setError(String(e));
      }
    },
    [refresh],
  );

  return {
    status,
    projectInfo,
    timelines,
    clips,
    timelineClips,
    renderPresets,
    loading,
    error,
    actionMessage,
    refresh,
    addMarker,
    importMedia,
    setClipEnabled,
    deleteClip,
  };
}
