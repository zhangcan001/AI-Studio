import type { ProductionBatchDetail, ProductionBatchSummary, SequentialBatchStartState } from "../../types/productionQueue";

export const emptySequentialBatchStartState: SequentialBatchStartState = {
  status: "IDLE",
  queuedBatchIds: [],
};

export type SequentialBatchStartAction =
  | { type: "RESET" }
  | { type: "REPLACE"; state: SequentialBatchStartState }
  | { type: "QUEUE_BATCH"; batchId: string; currentBatchId?: string }
  | { type: "START_ACTIVE"; batchId: string }
  | { type: "CLEAR_CURRENT" }
  | { type: "PAUSE"; reason: string; canResume: boolean }
  | { type: "REMOVE_QUEUED"; batchId: string }
  | { type: "CANCEL_FUTURE" }
  | { type: "RESUME" };

export function sequentialBatchStartReducer(
  state: SequentialBatchStartState,
  action: SequentialBatchStartAction,
): SequentialBatchStartState {
  switch (action.type) {
    case "RESET":
      return { ...emptySequentialBatchStartState };
    case "REPLACE":
      return action.state;
    case "QUEUE_BATCH":
      return addSequentialBatchToQueue(state, action.batchId, action.currentBatchId);
    case "START_ACTIVE":
      return {
        ...state,
        status: "ACTIVE",
        currentBatchId: action.batchId,
        queuedBatchIds: state.queuedBatchIds.filter((batchId) => batchId !== action.batchId),
        pauseReason: undefined,
        canResume: undefined,
      };
    case "CLEAR_CURRENT":
      return { ...state, currentBatchId: undefined };
    case "PAUSE":
      return { ...state, status: "PAUSED", pauseReason: action.reason, canResume: action.canResume };
    case "REMOVE_QUEUED":
      return {
        ...state,
        queuedBatchIds: state.queuedBatchIds.filter((batchId) => batchId !== action.batchId),
      };
    case "CANCEL_FUTURE":
      return {
        ...state,
        status: state.currentBatchId ? state.status : "IDLE",
        queuedBatchIds: [],
      };
    case "RESUME":
      return {
        ...state,
        status: "ACTIVE",
        currentBatchId: undefined,
        pauseReason: undefined,
        canResume: undefined,
      };
  }
}

export function addSequentialBatchToQueue(
  state: SequentialBatchStartState,
  batchId: string,
  currentBatchId?: string,
): SequentialBatchStartState {
  if (state.currentBatchId === batchId || state.queuedBatchIds.includes(batchId)) return state;
  const paused = state.status === "PAUSED";
  return {
    ...state,
    status: paused ? "PAUSED" : "ACTIVE",
    currentBatchId: state.currentBatchId ?? currentBatchId,
    queuedBatchIds: [...state.queuedBatchIds, batchId],
    pauseReason: paused ? state.pauseReason : undefined,
    canResume: paused ? state.canResume : undefined,
  };
}

export function retainSequentialBatchFirst(state: SequentialBatchStartState, batchId: string): SequentialBatchStartState {
  return state.queuedBatchIds.includes(batchId)
    ? state
    : { ...state, queuedBatchIds: [batchId, ...state.queuedBatchIds] };
}

export function isCleanSequentialCompletion(batch: ProductionBatchDetail): boolean {
  return batch.status === "COMPLETED"
    && batch.running === 0
    && batch.pending === 0
    && batch.failed === 0
    && batch.cancelled === 0
    && batch.skipped === 0
    && batch.succeeded === batch.total;
}

export function isTerminalProductionBatch(batch?: ProductionBatchDetail | null): boolean {
  if (!batch) return false;
  const terminalItemCount = batch.succeeded + batch.failed + batch.cancelled + batch.skipped;
  return Boolean(
    batch.archivedAt
      || batch.status === "COMPLETED"
      || ["FAILED", "CANCELLED", "CANCELED"].includes(batch.status as string)
      || (batch.total > 0 && terminalItemCount >= batch.total),
  );
}

export function hasTerminalSequentialFailure(batch: ProductionBatchDetail): boolean {
  return isTerminalProductionBatch(batch) && !isCleanSequentialCompletion(batch);
}

export function isStructuredProductionQueueBusy(error: unknown): boolean {
  return Boolean(
    error
      && typeof error === "object"
      && "code" in error
      && (error as { code?: unknown }).code === "PRODUCTION_QUEUE_BUSY",
  );
}

function recentFirst<T extends { updatedAt?: string; createdAt?: string }>(left: T, right: T): number {
  const leftTime = Date.parse(left.updatedAt ?? left.createdAt ?? "") || 0;
  const rightTime = Date.parse(right.updatedAt ?? right.createdAt ?? "") || 0;
  return rightTime - leftTime;
}

export function selectDefaultProductionBatchId(
  queues: readonly ProductionBatchSummary[],
  focusedBatchId?: string,
): string | undefined {
  const available = queues.filter((queue) => !queue.archivedAt);
  const focused = focusedBatchId && available.some((queue) => queue.id === focusedBatchId)
    ? focusedBatchId
    : undefined;
  if (focused) return focused;
  return [...available].filter((queue) => queue.status === "RUNNING").sort(recentFirst)[0]?.id
    ?? [...available].sort(recentFirst)[0]?.id;
}
