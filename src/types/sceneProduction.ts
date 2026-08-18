import type { DraftValue, GenerationValues } from "./generation";
import type { PromptEntryView } from "./prompt";
import type { ReferenceAnchorView } from "./referenceAnchor";
import type { ProductionBatchDetail } from "./productionQueue";
import type { ShotStage, ShotView } from "./shot";

export type SceneProductionStage = ShotStage;
export type SceneProductionClassification = "DONE" | "PREPARED" | "ELIGIBLE" | "BLOCKED";

export interface BatchWorkflowStagePreset {
  workflowVersionId: string;
  recipeId: string;
  values: GenerationValues;
}

export interface BatchWorkflowPreset {
  id: string;
  name: string;
  description: string;
  image?: BatchWorkflowStagePreset;
  video?: BatchWorkflowStagePreset;
  createdAt: string;
  updatedAt: string;
  available: boolean;
  reason?: string;
  unavailableReason?: string;
  imageAvailable?: boolean;
  videoAvailable?: boolean;
}

export interface BatchWorkflowPresetCreateRequest {
  name: string;
  description?: string;
  image?: BatchWorkflowStagePreset;
  video?: BatchWorkflowStagePreset;
}

export interface BatchWorkflowPresetUpdateRequest extends BatchWorkflowPresetCreateRequest {
  presetId: string;
}

export interface SceneProductionPlanRow {
  shotId: string;
  name: string;
  globalOrdinal: number;
  classification: SceneProductionClassification;
  blockingReasons: string[];
  existingBatchId?: string | null;
}

export interface SceneProductionPlan {
  projectId: string;
  sceneId: string;
  sceneName: string;
  stage: SceneProductionStage;
  total: number;
  done: number;
  prepared: number;
  eligible: number;
  blocked: number;
  canPrepare: boolean;
  maxBatchItems: number;
  rows: SceneProductionPlanRow[];
}

export interface SceneProductionPlanRequest {
  projectId: string;
  sceneId: string;
  stage: SceneProductionStage;
}

export interface SceneProductionPrepareRequest extends SceneProductionPlanRequest {
  allowPartial: boolean;
}

export interface SceneProductionPrepareResult {
  projectId: string;
  sceneId: string;
  stage: SceneProductionStage;
  created: boolean;
  alreadyPrepared: boolean;
  batchId?: string | null;
  existingBatchIds: string[];
  createdCount: number;
  skippedCount: number;
  message?: string;
  detail?: ProductionBatchDetail | null;
}

export interface SceneProductionSceneOption {
  value: string;
  label: string;
}

/**
 * The current shot shape is deliberately narrow. It is only used to snapshot
 * scalar stage configuration into a reusable preset; references and selected
 * media never cross this boundary.
 */
export type SceneProductionCurrentShot = Pick<ShotView, "id" | "name" | "stageConfigs">;

export interface SceneProductionPanelProps {
  projectId: string;
  sceneOptions: SceneProductionSceneOption[];
  currentSceneId?: string;
  currentShot?: SceneProductionCurrentShot;
  promptEntries?: PromptEntryView[];
  referenceAnchors?: ReferenceAnchorView[];
  initialPresets?: BatchWorkflowPreset[];
  initialPlan?: SceneProductionPlan;
  onRefresh?: () => Promise<void>;
  onNotice?: (message: string) => void;
  onNavigateToReview?: (stage: SceneProductionStage) => void;
}

const MEDIA_VALUE_TYPES = new Set<DraftValue["type"]>([
  "image_asset",
  "image_assets",
  "video_asset",
  "video_assets",
  "audio_asset",
  "audio_assets",
]);

/** Keep only values safe to reuse across projects/shots. */
export function sanitizeReusableGenerationValues(values: GenerationValues): GenerationValues {
  return Object.fromEntries(
    Object.entries(values).filter(([, value]) => !MEDIA_VALUE_TYPES.has(value.type)),
  );
}

export function sceneProductionStageLabel(stage: SceneProductionStage): string {
  return stage === "image" ? "图片" : "视频";
}

export function sceneProductionClassificationLabel(classification: SceneProductionClassification): string {
  return {
    DONE: "已完成",
    PREPARED: "已准备",
    ELIGIBLE: "可生产",
    BLOCKED: "被阻塞",
  }[classification];
}

export function sceneProductionStagePreset(
  preset: BatchWorkflowPreset | undefined,
  stage: SceneProductionStage,
): BatchWorkflowStagePreset | undefined {
  return stage === "image" ? preset?.image : preset?.video;
}
