import { useCallback, useEffect, useRef, useState } from "react";
import {
  getProductionBatchArtifacts,
  getProductionQueue,
} from "../../../services/tauriClient";
import type { ProductionBatchArtifactsDto } from "../../../types/artifact";
import type { ProductionBatchDetail } from "../../../types/productionQueue";
import { toUserMessage } from "../../../i18n/errorMessages";
import { isTerminalProductionBatch } from "../shotQueueState";

export interface UseShotProductionMonitorOptions {
  projectId: string;
  enabled: boolean;
  selectedBatchId?: string;
}

export function useShotProductionMonitor({ projectId, enabled, selectedBatchId }: UseShotProductionMonitorOptions) {
  const [batch, setBatch] = useState<ProductionBatchDetail>();
  const [artifacts, setArtifacts] = useState<ProductionBatchArtifactsDto>();
  const [loading, setLoading] = useState(false);
  const [error, setErrorState] = useState<string>();
  const requestInFlightRef = useRef(false);
  const pendingBatchRef = useRef<string | undefined>(undefined);
  const mountedRef = useRef(true);
  const currentBatchRef = useRef<string | undefined>(undefined);

  const clearError = useCallback(() => setErrorState(undefined), []);
  const setError = useCallback((message: string) => setErrorState(message), []);
  const getCurrentBatchId = useCallback(() => currentBatchRef.current, []);
  const focusBatch = useCallback((batchId: string) => {
    currentBatchRef.current = batchId;
    pendingBatchRef.current = undefined;
  }, []);

  const refresh = useCallback(async (batchId: string) => {
    if (!enabled || !batchId || !mountedRef.current) return;
    if (requestInFlightRef.current) {
      pendingBatchRef.current = batchId;
      return;
    }
    requestInFlightRef.current = true;
    setLoading(true);
    setErrorState(undefined);
    try {
      const [nextBatch, nextArtifacts] = await Promise.all([
        getProductionQueue(projectId, batchId),
        getProductionBatchArtifacts(projectId, batchId),
      ]);
      if (!mountedRef.current || currentBatchRef.current !== batchId) return;
      setBatch(nextBatch);
      setArtifacts(nextArtifacts);
    } catch (monitorError: unknown) {
      if (mountedRef.current && currentBatchRef.current === batchId) setErrorState(toUserMessage(monitorError));
    } finally {
      requestInFlightRef.current = false;
      if (mountedRef.current) setLoading(false);
      const pendingBatchId = pendingBatchRef.current;
      pendingBatchRef.current = undefined;
      if (mountedRef.current && pendingBatchId && currentBatchRef.current === pendingBatchId) {
        void refresh(pendingBatchId);
      }
    }
  }, [enabled, projectId]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      requestInFlightRef.current = false;
      pendingBatchRef.current = undefined;
      currentBatchRef.current = undefined;
    };
  }, []);

  useEffect(() => {
    currentBatchRef.current = selectedBatchId;
    pendingBatchRef.current = undefined;
    setBatch(undefined);
    setArtifacts(undefined);
    setErrorState(undefined);
    if (!enabled || !selectedBatchId) return;
    if (typeof document !== "undefined" && document.visibilityState === "hidden") return;
    void refresh(selectedBatchId);
  }, [enabled, refresh, selectedBatchId]);

  useEffect(() => {
    if (!enabled || !selectedBatchId || isTerminalProductionBatch(batch)) return;
    const refreshIfVisible = () => {
      if (typeof document !== "undefined" && document.visibilityState === "hidden") return;
      void refresh(selectedBatchId);
    };
    const onVisibilityChange = () => {
      if (document.visibilityState === "visible") refreshIfVisible();
    };
    const intervalId = window.setInterval(refreshIfVisible, 3000);
    document.addEventListener("visibilitychange", onVisibilityChange);
    return () => {
      window.clearInterval(intervalId);
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  }, [batch, enabled, refresh, selectedBatchId]);

  return {
    batch,
    artifacts,
    loading,
    error,
    refresh,
    clearError,
    setError,
    getCurrentBatchId,
    focusBatch,
  };
}
