import { useCallback, useState } from "react";
import { api } from "@/lib/api";
import type { AboutInfo } from "@/lib/types";

export function useAboutInfo() {
  const [info, setInfo] = useState<AboutInfo | null>(null);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      setInfo(await api.getAboutInfo());
    } finally {
      setLoading(false);
    }
  }, []);

  return { info, loading, refresh };
}
