import { useEffect } from "react";
import type { ComfyStatus } from "../types/comfy";

/** Observe the existing backend health authority; own no connection state. */
export function useRuntimeStatusMonitor(enabled: boolean, publish: (status?: ComfyStatus) => void, getStatus: () => Promise<ComfyStatus>) {
  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      try {
        const status = await getStatus();
        if (!cancelled) publish(status);
      } catch {
        // An unavailable typed transport cannot sustain a positive UI precheck.
        if (!cancelled) publish(undefined);
      } finally {
        if (!cancelled) timer = setTimeout(() => void refresh(), 10_000);
      }
    }
    void refresh();
    return () => { cancelled = true; clearTimeout(timer); };
  }, [enabled, publish, getStatus]);
}
