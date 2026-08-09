import type { TaskDetail } from "../../types/history";

const RETRYABLE_ERROR_CODES = new Set([
  "COMFY_OFFLINE",
  "COMFY_TIMEOUT",
  "COMFY_STREAM_DISCONNECTED",
  "COMFY_IMAGE_UPLOAD_FAILED",
  "COMFY_INPUT_UPLOAD_FAILED",
  "EXECUTION_INTERRUPTED",
]);

export interface RetryDecision {
  allowed: boolean;
  reason?: string;
}

export function taskRetryDecision(detail: TaskDetail, comfyConnected: boolean): RetryDecision {
  if (detail.status !== "FAILED") {
    return { allowed: false, reason: "Only failed tasks can be retried." };
  }
  if (!detail.errorCode) {
    return { allowed: false, reason: "The failure has no retry classification." };
  }
  if (detail.errorCode === "EXECUTION_ERROR") {
    return {
      allowed: false,
      reason: "Execution failures require review. GPU out-of-memory errors are never retried automatically.",
    };
  }
  if (!RETRYABLE_ERROR_CODES.has(detail.errorCode)) {
    return { allowed: false, reason: "This failure is not classified as transient." };
  }
  if (!detail.reusableDraft.available) {
    return { allowed: false, reason: detail.reusableDraft.reason ?? "Saved inputs are unavailable." };
  }
  if (detail.reusableDraft.missingAssetIds.length > 0) {
    return { allowed: false, reason: "A referenced media asset is missing." };
  }
  if (!comfyConnected) {
    return { allowed: false, reason: "Reconnect ComfyUI before retrying." };
  }
  return { allowed: true };
}
