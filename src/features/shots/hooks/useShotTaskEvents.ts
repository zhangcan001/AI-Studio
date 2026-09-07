import { useEffect } from "react";
import { subscribeTaskUpdates } from "../../../services/taskEvents";

export type ShotProductionModeTab = "package" | "project" | "multi-package";

export interface UseShotTaskEventsOptions {
  enabled: boolean;
  projectId: string;
  productionModeTab: ShotProductionModeTab;
  onRefreshQueues: () => void | Promise<void>;
  onRefreshMultiPackage: () => void | Promise<void>;
  getMonitorBatchId?: () => string | undefined;
  onRefreshMonitor: (batchId: string) => void | Promise<void>;
}

export function useShotTaskEvents({
  enabled,
  projectId,
  productionModeTab,
  onRefreshQueues,
  onRefreshMultiPackage,
  getMonitorBatchId,
  onRefreshMonitor,
}: UseShotTaskEventsOptions) {
  useEffect(() => {
    if (!enabled) return undefined;
    let active = true;
    let refreshTimer: number | undefined;
    let unlisten: (() => void) | undefined;

    void subscribeTaskUpdates((task) => {
      if (!active || task.projectId !== projectId || !["SUCCEEDED", "FAILED", "CANCELLED"].includes(task.status)) return;
      if (refreshTimer !== undefined) window.clearTimeout(refreshTimer);
      refreshTimer = window.setTimeout(() => {
        if (!active) return;
        refreshTimer = undefined;
        if (productionModeTab === "multi-package") {
          void onRefreshMultiPackage();
        } else {
          void onRefreshQueues();
        }
        const batchId = getMonitorBatchId?.();
        if (batchId) void onRefreshMonitor(batchId);
      }, 900);
    })
      .then((cleanup) => {
        if (active) unlisten = cleanup;
        else cleanup();
      })
      .catch(() => undefined);

    return () => {
      active = false;
      if (refreshTimer !== undefined) window.clearTimeout(refreshTimer);
      unlisten?.();
    };
  }, [enabled, getMonitorBatchId, onRefreshMonitor, onRefreshMultiPackage, onRefreshQueues, productionModeTab, projectId]);
}
