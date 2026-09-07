import { useCallback, useState } from "react";
import { api } from "@/lib/api";
import type { BlenderSceneInfo, BlenderStatus } from "@/lib/types";

export function useBlender() {
  const [status, setStatus] = useState<BlenderStatus | null>(null);
  const [sceneInfo, setSceneInfo] = useState<BlenderSceneInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [actionMessage, setActionMessage] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const s = await api.getBlenderStatus();
      setStatus(s);
      setSceneInfo(s.running ? await api.getBlenderSceneInfo() : null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  const addMarker = useCallback(
    async (name: string) => {
      setActionMessage(null);
      setError(null);
      try {
        const result = await api.addBlenderMarker(name);
        setActionMessage(result.message);
        if (result.ok) await refresh();
      } catch (e) {
        setError(String(e));
      }
    },
    [refresh],
  );

  const importMedia = useCallback(
    async (paths: string[], sceneName: string) => {
      setActionMessage(null);
      setError(null);
      try {
        const result = await api.importBlenderMedia(paths, sceneName.trim() || null);
        setActionMessage(result.message);
        if (result.ok) await refresh();
      } catch (e) {
        setError(String(e));
      }
    },
    [refresh],
  );

  return { status, sceneInfo, loading, error, actionMessage, refresh, addMarker, importMedia };
}
