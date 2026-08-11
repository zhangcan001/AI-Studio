import type { ProductionBatchDetail, ProductionBatchItemView } from "../../types/productionQueue";

const TRANSIENT_REQUEUE_ERRORS = new Set([
  "COMFY_OFFLINE",
  "COMFY_TIMEOUT",
  "COMFY_STREAM_DISCONNECTED",
  "COMFY_IMAGE_UPLOAD_FAILED",
  "COMFY_INPUT_UPLOAD_FAILED",
  "EXECUTION_INTERRUPTED",
]);

export function isSafeProductionQueueRequeue(item: ProductionBatchItemView): boolean {
  if (item.status === "CANCELLED") return true;
  if (item.status !== "FAILED" && item.status !== "SKIPPED") return false;
  return Boolean(item.errorCode && TRANSIENT_REQUEUE_ERRORS.has(item.errorCode));
}

export function canCancelPendingProductionQueue(
  detail: Pick<ProductionBatchDetail, "status" | "archivedAt" | "pending" | "running">,
): boolean {
  return (
    !detail.archivedAt &&
    (detail.status === "READY" || detail.status === "PAUSED") &&
    detail.pending > 0 &&
    detail.running === 0
  );
}

export interface ProductionInteractionPolicy {
  canSubmitGeneration: boolean;
  canSubmitLocalBatch: boolean;
  canRetryTask: boolean;
  canEditDraft: true;
  canSwitchProject: true;
}

export function productionInteractionPolicy(busy: boolean): ProductionInteractionPolicy {
  return {
    canSubmitGeneration: !busy,
    canSubmitLocalBatch: !busy,
    canRetryTask: !busy,
    canEditDraft: true,
    canSwitchProject: true,
  };
}
