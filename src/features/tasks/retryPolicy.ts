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

export function taskRetrySubmissionKey(taskId: string): string {
  return `task-retry:${taskId}`;
}

export function taskRetryDecision(detail: TaskDetail, comfyConnected: boolean): RetryDecision {
  if (detail.status !== "FAILED") {
    return { allowed: false, reason: "只有失败的任务可以重试。" };
  }
  if (!detail.errorCode) {
    return { allowed: false, reason: "该失败没有可用的重试分类。" };
  }
  if (detail.errorCode === "EXECUTION_ERROR") {
    return {
      allowed: false,
      reason: "执行失败需要人工检查。GPU 显存不足不会自动重试。",
    };
  }
  if (!RETRYABLE_ERROR_CODES.has(detail.errorCode)) {
    return { allowed: false, reason: "该失败未被归类为临时错误。" };
  }
  if (!detail.reusableDraft.available) {
    return { allowed: false, reason: detail.reusableDraft.reason ?? "保存的输入不可用。" };
  }
  if (detail.reusableDraft.missingAssetIds.length > 0) {
    return { allowed: false, reason: "缺少引用的媒体素材。" };
  }
  if (!comfyConnected) {
    return { allowed: false, reason: "请重新连接 ComfyUI 后再重试。" };
  }
  return { allowed: true };
}
