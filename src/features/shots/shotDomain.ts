import type { ShotStage, ShotView } from "../../types/shot";

export type ShotStatus =
  | "DRAFT"
  | "READY"
  | "GENERATING_IMAGE"
  | "IMAGE_REVIEW"
  | "IMAGE_SELECTED"
  | "GENERATING_VIDEO"
  | "VIDEO_REVIEW"
  | "COMPLETED"
  | "FAILED";

const ACTIVE_TASK_STATUSES = new Set([
  "CREATED",
  "VALIDATING",
  "PREPARING",
  "QUEUED",
  "RUNNING",
  "CANCEL_REQUESTED",
  "COLLECTING",
]);

export const shotStatusLabels: Record<ShotStatus, string> = {
  DRAFT: "草稿",
  READY: "待生成",
  GENERATING_IMAGE: "正在生成关键帧",
  IMAGE_REVIEW: "图片候选待选择",
  IMAGE_SELECTED: "已选关键帧",
  GENERATING_VIDEO: "正在生成视频",
  VIDEO_REVIEW: "视频候选待选择",
  COMPLETED: "已完成",
  FAILED: "生成失败",
};

export function deriveStageStatus(
  shot: ShotView,
  stage: ShotStage,
): ShotStatus {
  const selected = stage === "image" ? shot.selectedImageAssetId : shot.selectedVideoAssetId;
  const links = shot.generationLinks.filter((link) => link.stage === stage);
  const latestTask = links.find((link) => link.task)?.task;
  if (latestTask && ACTIVE_TASK_STATUSES.has(latestTask.status)) {
    return stage === "image" ? "GENERATING_IMAGE" : "GENERATING_VIDEO";
  }
  if (selected) return stage === "image" ? "IMAGE_SELECTED" : "COMPLETED";
  if (latestTask?.status === "FAILED") return "FAILED";
  if (latestTask?.status === "SUCCEEDED") {
    return stage === "image" ? "IMAGE_REVIEW" : "VIDEO_REVIEW";
  }
  return shot.stageConfigs.some((config) => config.stage === stage) ? "READY" : "DRAFT";
}

export function recentShotFailure(shot: ShotView, stage: ShotStage) {
  return shot.generationLinks
    .filter((link) => link.stage === stage)
    .map((link) => link.task)
    .find((task) => task?.status === "FAILED");
}

export function deriveShotStatus(shot: ShotView): ShotStatus {
  const image = deriveStageStatus(shot, "image");
  const videoConfigured = shot.stageConfigs.some((config) => config.stage === "video");
  const video = deriveStageStatus(shot, "video");

  if (videoConfigured && video === "COMPLETED") return "COMPLETED";
  if (videoConfigured && ["GENERATING_VIDEO", "VIDEO_REVIEW", "FAILED"].includes(video)) return video;
  if (["GENERATING_IMAGE", "IMAGE_REVIEW", "FAILED"].includes(image)) return image;
  if (!videoConfigured && image === "IMAGE_SELECTED") return "COMPLETED";
  if (image === "IMAGE_SELECTED") return "IMAGE_SELECTED";
  if (image === "READY" || video === "READY") return "READY";
  return "DRAFT";
}

export function isExecutionTruthStatus(status: string): boolean {
  return ACTIVE_TASK_STATUSES.has(status) || status === "FAILED" || status === "SUCCEEDED";
}

export function statusLabel(status: string): string {
  return shotStatusLabels[status as ShotStatus] ?? "未知状态";
}
