import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api";
import type { AppConfig, ConfigUpdate } from "@/lib/types";

export function useConfig() {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [models, setModels] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      setConfig(await api.getConfig());
      try {
        setModels(await api.listOllamaModels());
      } catch {
        setModels([]);
      }
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const update = useCallback(async (patch: ConfigUpdate) => {
    const next = await api.updateConfig(patch);
    setConfig(next);
    return next;
  }, []);

  return { config, models, loading, refresh, update };
}
