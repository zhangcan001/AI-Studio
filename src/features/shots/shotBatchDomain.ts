import type { ShotStage, ShotView } from "../../types/shot";
import { deriveStageStatus } from "./shotDomain";

export interface ShotProgressSummary {
  total: number;
  pendingKeyframes: number;
  keyframesSelected: number;
  videoGenerating: number;
  pendingVideoReview: number;
  completed: number;
  needsAttention: number;
}

export function shotProgressSummary(shots: ShotView[]): ShotProgressSummary {
  return shots.reduce<ShotProgressSummary>((summary, shot) => {
    const image = deriveStageStatus(shot, "image");
    const video = deriveStageStatus(shot, "video");
    const hasVideoStage = shot.stageConfigs.some((config) => config.stage === "video");
    summary.total += 1;
    if (["DRAFT", "READY"].includes(image) && !shot.selectedImageAssetId) summary.pendingKeyframes += 1;
    if (Boolean(shot.selectedImageAssetId)) summary.keyframesSelected += 1;
    if (video === "GENERATING_VIDEO") summary.videoGenerating += 1;
    if (video === "VIDEO_REVIEW") summary.pendingVideoReview += 1;
    if ((!hasVideoStage && Boolean(shot.selectedImageAssetId)) || video === "COMPLETED") summary.completed += 1;
    if (image === "FAILED" || video === "FAILED") summary.needsAttention += 1;
    return summary;
  }, {
    total: 0,
    pendingKeyframes: 0,
    keyframesSelected: 0,
    videoGenerating: 0,
    pendingVideoReview: 0,
    completed: 0,
    needsAttention: 0,
  });
}

export function stageLabel(stage: ShotStage): string {
  return stage === "image" ? "关键帧" : "最终视频";
}
