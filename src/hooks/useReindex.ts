import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api";
import type { ReindexProgress } from "@/lib/types";

export function useReindex() {
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<ReindexProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const unlistenProgress = listen<ReindexProgress>("reindex://progress", (event) => {
      setProgress(event.payload);
    });
    const unlistenDone = listen("reindex://done", () => {
      setRunning(false);
      setProgress(null);
    });
    const unlistenError = listen<string>("reindex://error", (event) => {
      setRunning(false);
      setError(event.payload);
    });

    return () => {
      unlistenProgress.then((f) => f());
      unlistenDone.then((f) => f());
      unlistenError.then((f) => f());
    };
  }, []);

  const start = useCallback(async () => {
    setError(null);
    setRunning(true);
    try {
      await api.runReindex();
    } catch (e) {
      setError(String(e));
      setRunning(false);
    }
  }, []);

  return { running, progress, error, start };
}
