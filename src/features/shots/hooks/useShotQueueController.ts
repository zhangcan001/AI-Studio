import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import {
  getProductionAdmissionStatus,
  getProductionQueue,
  getProductionQueueOverview,
  listProductionQueues,
  pauseProductionQueue,
  requeueProductionQueueItem,
  startProductionQueue,
  type ProductionPackageCreateBatchesResult,
} from "../../../services/tauriClient";
import type {
  ProductionAdmissionStatus,
  ProductionBatchDetail,
  ProductionBatchSummary,
  ProductionQueueOverview,
  SequentialBatchStartState,
} from "../../../types/productionQueue";
import { toUserMessage } from "../../../i18n/errorMessages";
import {
  emptySequentialBatchStartState,
  hasTerminalSequentialFailure,
  isCleanSequentialCompletion,
  isStructuredProductionQueueBusy,
  retainSequentialBatchFirst,
  sequentialBatchStartReducer,
  type SequentialBatchStartAction,
} from "../shotQueueState";
import { selectDefaultProductionBatchId } from "../shotQueueState";

export interface ProductionQueueSnapshot {
  queues: ProductionBatchSummary[];
  overview: ProductionQueueOverview;
}

export interface UseShotQueueControllerOptions {
  projectId: string;
  enabled: boolean;
  productionMonitorBatch?: ProductionBatchDetail;
  reloadWorkspace: () => Promise<void>;
  refreshProductionMonitor: (batchId: string) => Promise<void>;
  onError: (message: string) => void;
  onNotice: (message: string) => void;
  onFocusBatch?: (batchId: string) => void;
  onOpenProductionQueue?: () => void;
}

export function useShotQueueController({
  projectId,
  enabled,
  productionMonitorBatch,
  reloadWorkspace,
  refreshProductionMonitor,
  onError,
  onNotice,
  onFocusBatch,
  onOpenProductionQueue,
}: UseShotQueueControllerOptions) {
  const [queues, setQueues] = useState<ProductionBatchSummary[]>([]);
  const [overview, setOverview] = useState<ProductionQueueOverview>();
  const [expanded, setExpanded] = useState(false);
  const [focusedBatchId, setFocusedBatchId] = useState<string>();
  const [createdBatchIds, setCreatedBatchIds] = useState<string[]>([]);
  const [sequential, dispatch] = useReducer(sequentialBatchStartReducer, undefined, () => ({ ...emptySequentialBatchStartState }));
  const sequentialRef = useRef<SequentialBatchStartState>(sequential);
  const sessionRef = useRef(0);
  const mountedRef = useRef(true);
  const startInFlightRef = useRef<string | undefined>(undefined);
  const advanceInFlightRef = useRef(false);

  const applySequentialAction = useCallback((action: SequentialBatchStartAction) => {
    const next = sequentialBatchStartReducer(sequentialRef.current, action);
    sequentialRef.current = next;
    dispatch({ type: "REPLACE", state: next });
    return next;
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      sessionRef.current += 1;
      startInFlightRef.current = undefined;
      advanceInFlightRef.current = false;
    };
  }, []);

  useEffect(() => {
    sessionRef.current += 1;
    startInFlightRef.current = undefined;
    advanceInFlightRef.current = false;
    applySequentialAction({ type: "RESET" });
  }, [applySequentialAction, projectId]);

  const reloadProductionQueues = useCallback(async (throwOnError = false): Promise<ProductionQueueSnapshot | undefined> => {
    try {
      const [nextQueues, nextOverview] = await Promise.all([
        listProductionQueues(projectId),
        getProductionQueueOverview(projectId),
      ]);
      setQueues(nextQueues);
      setOverview(nextOverview);
      return { queues: nextQueues, overview: nextOverview };
    } catch (queueError: unknown) {
      if (throwOnError) throw queueError;
      onError(toUserMessage(queueError));
      return undefined;
    }
  }, [onError, projectId]);

  const focusBatch = useCallback((batchId: string) => {
    onFocusBatch?.(batchId);
    setFocusedBatchId(batchId);
    setExpanded(true);
  }, [onFocusBatch]);

  const openQueue = useCallback(async (result?: ProductionPackageCreateBatchesResult) => {
    const nextCreatedBatchIds = result?.batches.map((batch) => batch.batchId) ?? [];
    const firstBatchId = nextCreatedBatchIds[0];
    if (result) setCreatedBatchIds(nextCreatedBatchIds);
    if (firstBatchId) focusBatch(firstBatchId);
    const snapshot = await reloadProductionQueues(true);
    if (firstBatchId && !snapshot?.queues.some((queue) => queue.id === firstBatchId)) {
      throw new Error("Created production batch is not visible in queue projection");
    }
    setExpanded(true);
    onOpenProductionQueue?.();
  }, [focusBatch, onOpenProductionQueue, reloadProductionQueues]);

  const maybeAdvanceSequentialBatchStart = useCallback(async () => {
    if (!mountedRef.current || advanceInFlightRef.current) return;
    const sessionId = sessionRef.current;
    const isCurrentSession = () => mountedRef.current && sessionRef.current === sessionId;
    const initialState = sequentialRef.current;
    if (initialState.status === "PAUSED") return;
    if (!initialState.currentBatchId && initialState.queuedBatchIds.length === 0) {
      if (initialState.status !== "IDLE") applySequentialAction({ type: "RESET" });
      return;
    }

    advanceInFlightRef.current = true;
    try {
      let admission: ProductionAdmissionStatus;
      try {
        admission = await getProductionAdmissionStatus();
      } catch (admissionError: unknown) {
        if (!isCurrentSession()) return;
        const message = toUserMessage(admissionError);
        applySequentialAction({ type: "PAUSE", reason: message, canResume: true });
        onError(message);
        return;
      }
      if (!isCurrentSession() || admission.busy) return;

      const currentBatchId = sequentialRef.current.currentBatchId;
      if (currentBatchId) {
        let currentBatch: ProductionBatchDetail;
        try {
          currentBatch = await getProductionQueue(projectId, currentBatchId);
        } catch (batchError: unknown) {
          if (!isCurrentSession()) return;
          const message = toUserMessage(batchError);
          applySequentialAction({ type: "PAUSE", reason: message, canResume: true });
          onError(message);
          return;
        }
        if (!isCurrentSession()) return;
        if (currentBatch.status === "PAUSED") {
          applySequentialAction({ type: "PAUSE", reason: "当前批次已暂停，请先处理当前批次。", canResume: false });
          return;
        }
        if (!isCleanSequentialCompletion(currentBatch)) {
          if (hasTerminalSequentialFailure(currentBatch)) {
            if (sequentialRef.current.queuedBatchIds.length === 0) {
              applySequentialAction({ type: "RESET" });
            } else {
              applySequentialAction({ type: "PAUSE", reason: "上一批存在失败、取消或跳过项。", canResume: true });
            }
          }
          return;
        }
        if (sequentialRef.current.queuedBatchIds.length === 0) {
          applySequentialAction({ type: "RESET" });
          return;
        }
        applySequentialAction({ type: "CLEAR_CURRENT" });
      }

      const nextBatchId = sequentialRef.current.queuedBatchIds[0];
      if (!nextBatchId) {
        applySequentialAction({ type: "RESET" });
        return;
      }

      startInFlightRef.current = nextBatchId;
      try {
        await startProductionQueue(projectId, nextBatchId);
      } catch (startError: unknown) {
        if (!isCurrentSession()) return;
        if (isStructuredProductionQueueBusy(startError)) {
          applySequentialAction({ type: "START_ACTIVE", batchId: nextBatchId });
        } else {
          const message = toUserMessage(startError);
          applySequentialAction({ type: "PAUSE", reason: message, canResume: true });
          onError(message);
        }
        return;
      } finally {
        if (startInFlightRef.current === nextBatchId) startInFlightRef.current = undefined;
      }

      if (!isCurrentSession()) return;
      applySequentialAction({ type: "START_ACTIVE", batchId: nextBatchId });
      focusBatch(nextBatchId);
      await reloadProductionQueues();
      if (!isCurrentSession()) return;
      await reloadWorkspace();
      if (!isCurrentSession()) return;
      await refreshProductionMonitor(nextBatchId);
    } finally {
      advanceInFlightRef.current = false;
    }
  }, [applySequentialAction, focusBatch, onError, projectId, refreshProductionMonitor, reloadProductionQueues, reloadWorkspace]);

  useEffect(() => {
    if (!enabled) return;
    void maybeAdvanceSequentialBatchStart();
  }, [enabled, maybeAdvanceSequentialBatchStart, queues, productionMonitorBatch]);

  useEffect(() => {
    if (!enabled) return;
    setExpanded(true);
    void reloadProductionQueues();
  }, [enabled, reloadProductionQueues]);

  const startBatch = useCallback(async (batchId: string) => {
    if (!batchId || !mountedRef.current) return;
    const sessionId = sessionRef.current;
    const isCurrentSession = () => mountedRef.current && sessionRef.current === sessionId;
    const currentState = sequentialRef.current;
    if (currentState.queuedBatchIds.includes(batchId)) return;
    if (currentState.currentBatchId === batchId && currentState.status === "ACTIVE" && !startInFlightRef.current) return;
    if (currentState.currentBatchId && currentState.currentBatchId !== batchId) {
      applySequentialAction({ type: "QUEUE_BATCH", batchId, currentBatchId: currentState.currentBatchId });
      return;
    }
    if (startInFlightRef.current || advanceInFlightRef.current) {
      applySequentialAction({ type: "QUEUE_BATCH", batchId, currentBatchId: startInFlightRef.current ?? currentState.currentBatchId });
      return;
    }

    startInFlightRef.current = batchId;
    try {
      let admission: ProductionAdmissionStatus;
      try {
        admission = await getProductionAdmissionStatus();
      } catch (admissionError: unknown) {
        if (!isCurrentSession()) return;
        const message = toUserMessage(admissionError);
        const base = sequentialBatchStartReducer(sequentialRef.current, { type: "CLEAR_CURRENT" });
        applySequentialAction({ type: "REPLACE", state: {
          ...retainSequentialBatchFirst(base, batchId),
          status: "PAUSED",
          pauseReason: message,
          canResume: true,
        } });
        onError(message);
        return;
      }
      if (!isCurrentSession()) return;

      if (admission.busy) {
        if (admission.batchId === batchId) {
          applySequentialAction({ type: "START_ACTIVE", batchId });
        } else {
          applySequentialAction({ type: "QUEUE_BATCH", batchId, currentBatchId: admission.batchId });
        }
        return;
      }

      applySequentialAction({ type: "START_ACTIVE", batchId });
      try {
        await startProductionQueue(projectId, batchId);
      } catch (startError: unknown) {
        if (!isCurrentSession()) return;
        const base = sequentialBatchStartReducer(sequentialRef.current, { type: "CLEAR_CURRENT" });
        if (isStructuredProductionQueueBusy(startError)) {
          applySequentialAction({ type: "REPLACE", state: {
            ...retainSequentialBatchFirst(base, batchId),
            status: "ACTIVE",
          } });
        } else {
          const message = toUserMessage(startError);
          applySequentialAction({ type: "REPLACE", state: {
            ...retainSequentialBatchFirst(base, batchId),
            status: "PAUSED",
            pauseReason: message,
            canResume: true,
          } });
          onError(message);
        }
        return;
      }

      if (!isCurrentSession()) return;
      applySequentialAction({ type: "START_ACTIVE", batchId });
      focusBatch(batchId);
      await reloadProductionQueues();
      if (!isCurrentSession()) return;
      await reloadWorkspace();
      if (!isCurrentSession()) return;
      await refreshProductionMonitor(batchId);
    } finally {
      if (startInFlightRef.current === batchId) startInFlightRef.current = undefined;
    }
  }, [applySequentialAction, focusBatch, onError, projectId, refreshProductionMonitor, reloadProductionQueues, reloadWorkspace]);

  const cancelQueuedStart = useCallback((batchId: string) => {
    applySequentialAction({ type: "REMOVE_QUEUED", batchId });
  }, [applySequentialAction]);

  const cancelSequentialStart = useCallback(() => {
    applySequentialAction({ type: "CANCEL_FUTURE" });
    onNotice("已取消后续连续运行；当前任务会继续完成。");
  }, [applySequentialAction, onNotice]);

  const resumeSequentialStart = useCallback(() => {
    if (sequentialRef.current.status !== "PAUSED") return;
    applySequentialAction({ type: "RESUME" });
    void maybeAdvanceSequentialBatchStart();
  }, [applySequentialAction, maybeAdvanceSequentialBatchStart]);

  const pauseBatch = useCallback(async (batchId: string) => {
    await pauseProductionQueue(projectId, batchId);
    await reloadProductionQueues();
    await reloadWorkspace();
  }, [projectId, reloadProductionQueues, reloadWorkspace]);

  const selectedBatchId = useMemo(
    () => selectDefaultProductionBatchId(queues, focusedBatchId),
    [focusedBatchId, queues],
  );

  const requeueItem = useCallback(async (itemId: string) => {
    const batchId = selectedBatchId;
    if (!batchId) return;
    await requeueProductionQueueItem(projectId, batchId, itemId);
    await reloadProductionQueues();
    await refreshProductionMonitor(batchId);
    onNotice("已重新加入当前批次等待队列；不会自动开始。 ");
  }, [onNotice, projectId, refreshProductionMonitor, reloadProductionQueues, selectedBatchId]);

  return {
    queues,
    overview,
    expanded,
    setExpanded,
    sequential,
    focusedBatchId,
    createdBatchIds,
    selectedBatchId,
    reloadProductionQueues,
    focusBatch,
    openQueue,
    startBatch,
    cancelQueuedStart,
    cancelSequentialStart,
    resumeSequentialStart,
    pauseBatch,
    requeueItem,
  };
}
