import type { ShotStage, ShotView } from "../../types/shot";
import { deriveShotStatus, deriveStageStatus, type ShotStatus } from "./shotDomain";

export type ShotProductionStepId =
  | "prompt"
  | "references"
  | "image"
  | "image-review"
  | "video"
  | "video-review"
  | "complete";

export type ShotProductionStepStatus = "COMPLETE" | "ACTIVE" | "READY" | "BLOCKED" | "PENDING" | "FAILED";

export type ShotProductionVideoInputMode = "TEXT_ONLY" | "SINGLE_IMAGE" | "REFERENCE_IMAGES" | "UNSUPPORTED";

export interface ShotProductionStageContext {
  available?: boolean;
  configured?: boolean;
  referenceRequired?: boolean;
  referenceMinimum?: number;
  referenceCount?: number;
  videoInputMode?: ShotProductionVideoInputMode;
}

export interface ShotProductionContext {
  image?: ShotProductionStageContext;
  video?: ShotProductionStageContext;
}

export interface ShotProductionStep {
  id: ShotProductionStepId;
  label: string;
  status: ShotProductionStepStatus;
  detail?: string;
  actionable: boolean;
}

export interface ShotProductionNextAction {
  stepId: ShotProductionStepId;
  label: string;
  detail?: string;
  actionable: boolean;
}

export interface ShotProductionReadModel {
  steps: ShotProductionStep[];
  nextAction: ShotProductionNextAction;
}

const stepLabels: Record<ShotProductionStepId, string> = {
  prompt: "提示词",
  references: "参考",
  image: "图片",
  "image-review": "图片确认",
  video: "视频",
  "video-review": "视频审核",
  complete: "完成",
};

const statusLabels: Record<ShotProductionStepStatus, string> = {
  COMPLETE: "已完成",
  ACTIVE: "进行中",
  READY: "待处理",
  BLOCKED: "受阻",
  PENDING: "等待",
  FAILED: "失败",
};

const nextActionLabels: Partial<Record<ShotProductionStepId, string>> = {
  prompt: "填写提示词",
  references: "补充参考素材",
  image: "生成关键帧",
  "image-review": "选择关键帧",
  video: "生成视频",
  "video-review": "审核视频",
};

const interactiveStepIds = new Set<ShotProductionStepId>([
  "prompt",
  "references",
  "image",
  "image-review",
  "video",
  "video-review",
]);

function contextFor(context: ShotProductionContext | undefined, stage: ShotStage): ShotProductionStageContext {
  const stageContext = context?.[stage];
  return {
    available: stageContext?.available ?? Boolean(stageContext?.configured),
    configured: stageContext?.configured ?? false,
    referenceRequired: stageContext?.referenceRequired ?? false,
    referenceMinimum: stageContext?.referenceMinimum,
    referenceCount: stageContext?.referenceCount,
    videoInputMode: stageContext?.videoInputMode,
  };
}

function stageStatus(
  shot: ShotView,
  stage: ShotStage,
  context: ShotProductionStageContext,
): { status: ShotProductionStepStatus; detail?: string } {
  if (!context.available) {
    return {
      status: "BLOCKED",
      detail: `尚未配置${stage === "image" ? "图片" : "视频"}工作流`,
    };
  }

  const status = deriveStageStatus(shot, stage);
  if (status === "FAILED") return { status: "FAILED", detail: "最近一次生成失败，需要处理" };
  if (status === "GENERATING_IMAGE" || status === "GENERATING_VIDEO") {
    return { status: "ACTIVE", detail: "生成任务进行中" };
  }
  if (status === "IMAGE_REVIEW" || status === "VIDEO_REVIEW") {
    return { status: "COMPLETE", detail: "候选已生成，等待人工确认" };
  }
  if (status === "IMAGE_SELECTED" || status === "COMPLETED") {
    return { status: "COMPLETE" };
  }
  return {
    status: "READY",
    detail: status === "DRAFT" ? "可以开始生成" : statusLabels[status],
  };
}

function reviewStatus(
  shot: ShotView,
  stage: ShotStage,
  context: ShotProductionStageContext,
): { status: ShotProductionStepStatus; detail?: string } {
  const selectedAssetId = stage === "image" ? shot.selectedImageAssetId : shot.selectedVideoAssetId;
  if (selectedAssetId) return { status: "COMPLETE", detail: "已确认当前候选" };
  if (!context.available) return { status: "PENDING", detail: "等待阶段配置" };

  const status = deriveStageStatus(shot, stage);
  if (status === "FAILED") return { status: "FAILED", detail: "生成失败，暂无可确认候选" };
  if (status === "IMAGE_REVIEW" || status === "VIDEO_REVIEW") return { status: "ACTIVE", detail: "候选等待人工确认" };
  if (status === "GENERATING_IMAGE" || status === "GENERATING_VIDEO") return { status: "PENDING", detail: "等待生成结果" };
  return { status: "PENDING", detail: "生成候选后再确认" };
}

function referenceStatus(
  shot: ShotView,
  stage: ShotStage,
  context: ShotProductionStageContext,
): { status: ShotProductionStepStatus; detail?: string } {
  if (!context.referenceRequired) return { status: "COMPLETE", detail: "当前工作流无需参考素材" };
  const referenceCount = context.referenceCount
    ?? shot.referenceAssets.filter((reference) => reference.stage === stage).length;
  const minimum = Math.max(1, context.referenceMinimum ?? 1);
  if (referenceCount < minimum) {
    return { status: "BLOCKED", detail: `还需要 ${minimum - referenceCount} 张参考素材` };
  }
  return { status: "COMPLETE", detail: `${referenceCount} 张参考素材已准备` };
}

function videoBlockedByInput(shot: ShotView, context: ShotProductionStageContext): string | undefined {
  if (context.videoInputMode === "SINGLE_IMAGE" && !shot.selectedImageAssetId) return "请先选择关键帧图片";
  if (context.videoInputMode === "UNSUPPORTED") return "当前视频工作流输入不受支持";
  return undefined;
}

function makeStep(
  id: ShotProductionStepId,
  result: { status: ShotProductionStepStatus; detail?: string },
): ShotProductionStep {
  return {
    id,
    label: stepLabels[id],
    status: result.status,
    detail: result.detail,
    actionable: interactiveStepIds.has(id),
  };
}

function nextActionFor(steps: ShotProductionStep[]): ShotProductionNextAction {
  const candidate = steps.find((step) => step.id !== "complete" && step.status !== "COMPLETE");
  if (!candidate) return { stepId: "complete", label: "镜头已完成", actionable: false };
  return {
    stepId: candidate.id,
    label: nextActionLabels[candidate.id] ?? candidate.label,
    detail: candidate.detail ?? statusLabels[candidate.status],
    actionable: candidate.actionable,
  };
}

export function buildShotProductionReadModel(
  shot: ShotView,
  context?: ShotProductionContext,
): ShotProductionReadModel {
  const imageContext = contextFor(context, "image");
  const videoContext = contextFor(context, "video");
  const promptReady = shot.promptText.trim().length > 0;
  const imageReferences = referenceStatus(shot, "image", imageContext);
  const videoReferences = referenceStatus(shot, "video", videoContext);
  const imageResult = stageStatus(shot, "image", imageContext);
  const imageReview = reviewStatus(shot, "image", imageContext);
  const videoAvailable = videoContext.available && Boolean(videoContext.configured);
  const videoResult = videoAvailable
    ? stageStatus(shot, "video", videoContext)
    : videoContext.configured
      ? stageStatus(shot, "video", videoContext)
      : { status: "PENDING" as const, detail: "视频阶段尚未配置" };
  const videoInputBlock = videoAvailable ? videoBlockedByInput(shot, videoContext) : undefined;
  const videoStep = videoInputBlock && videoResult.status === "READY"
    ? { status: "BLOCKED" as const, detail: videoInputBlock }
    : videoResult;
  const videoReview = videoAvailable
    ? reviewStatus(shot, "video", videoContext)
    : videoContext.configured
      ? reviewStatus(shot, "video", videoContext)
      : { status: "PENDING" as const, detail: "视频阶段尚未配置" };
  const finalStatus: ShotStatus = deriveShotStatus(shot);
  const videoSkipped = !videoContext.configured && finalStatus === "COMPLETED";

  const steps = [
    makeStep("prompt", promptReady ? { status: "COMPLETE", detail: "提示词已准备" } : { status: "BLOCKED", detail: "需要填写提示词" }),
    makeStep("references", videoAvailable && videoContext.referenceRequired ? videoReferences : imageReferences),
    makeStep("image", imageResult),
    makeStep("image-review", imageReview),
    makeStep("video", videoSkipped ? { status: "COMPLETE", detail: "当前镜头无需视频阶段" } : videoStep),
    makeStep("video-review", videoSkipped ? { status: "COMPLETE", detail: "当前镜头无需视频审核" } : videoReview),
    makeStep("complete", finalStatus === "COMPLETED" ? { status: "COMPLETE" } : { status: "PENDING", detail: "完成前仍有步骤需要处理" }),
  ];

  return { steps, nextAction: nextActionFor(steps) };
}
