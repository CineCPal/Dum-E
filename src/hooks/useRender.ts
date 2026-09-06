import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api";
import type { RenderProgress } from "@/lib/types";

export function useRender() {
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<RenderProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const unlistenProgress = listen<RenderProgress>("render://progress", (event) => {
      setProgress(event.payload);
    });
    const unlistenDone = listen("render://done", () => {
      setRunning(false);
      setProgress(null);
    });
    const unlistenError = listen<string>("render://error", (event) => {
      setRunning(false);
      setError(event.payload);
    });

    return () => {
      unlistenProgress.then((f) => f());
      unlistenDone.then((f) => f());
      unlistenError.then((f) => f());
    };
  }, []);

  const start = useCallback(async (presetName: string, targetDir: string, customName: string) => {
    setError(null);
    setProgress(null);
    setRunning(true);
    try {
      await api.startResolveRender(presetName, targetDir, customName.trim() || null);
    } catch (e) {
      setError(String(e));
      setRunning(false);
    }
  }, []);

  const stop = useCallback(async () => {
    try {
      await api.stopResolveRender();
    } catch (e) {
      setError(String(e));
    }
  }, []);

  return { running, progress, error, start, stop };
}
