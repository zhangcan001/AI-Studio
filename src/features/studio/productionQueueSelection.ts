import type { ProductionBatchSummary } from "../../types/productionQueue";

const ACTIVE_PRODUCTION_QUEUE_STORAGE_PREFIX = "aistudio.activeProductionBatch.";

function storageKey(projectId: string): string {
  return `${ACTIVE_PRODUCTION_QUEUE_STORAGE_PREFIX}${projectId}`;
}

export function readStoredProductionQueueId(projectId: string): string | undefined {
  try {
    return typeof localStorage === "undefined"
      ? undefined
      : localStorage.getItem(storageKey(projectId)) ?? undefined;
  } catch {
    return undefined;
  }
}

export function rememberProductionQueue(projectId: string, batchId?: string): void {
  try {
    if (typeof localStorage === "undefined") return;
    if (batchId) localStorage.setItem(storageKey(projectId), batchId);
    else localStorage.removeItem(storageKey(projectId));
  } catch {
    // Queue persistence is an enhancement; storage failures must not block the queue.
  }
}

export function selectProductionQueueId(
  queues: ProductionBatchSummary[],
  preferredIds: Array<string | undefined>,
  autoFocusLatest: boolean,
): string | undefined {
  const preferredQueue = preferredIds
    .filter((id): id is string => Boolean(id))
    .map((id) => queues.find((queue) => queue.id === id))
    .find((queue): queue is ProductionBatchSummary => Boolean(queue));
  if (preferredQueue) return preferredQueue.id;
  if (!autoFocusLatest) return undefined;

  return (
    queues.find((queue) => !queue.archivedAt && queue.status === "RUNNING")
      ?? queues.find((queue) => !queue.archivedAt && queue.status === "PAUSED")
      ?? queues.find((queue) => !queue.archivedAt && queue.status === "READY")
      ?? queues.find((queue) => !queue.archivedAt)
  )?.id;
}
