import type { ShotStage } from "./shot";

export type ProductionPreparationStage = ShotStage;
export type SceneProductionStage = ProductionPreparationStage;
export type ProductionPreparationStatus = "READY" | "INCOMPLETE" | "BLOCKED";
export type ReadinessGateState = "PASS" | "WARNING" | "INCOMPLETE" | "BLOCKER";

export const PREPARATION_GATE_KEYS = [
  "CHARACTER",
  "SCENE",
  "REFERENCE",
  "PROMPT",
  "WORKFLOW",
  "OUTPUT",
  "COMFY_CAPABILITY",
] as const;

export type ReadinessGateKey = (typeof PREPARATION_GATE_KEYS)[number] | string;

export interface ReadinessCheckView {
  key?: string;
  state?: ReadinessGateState | string;
  code?: string;
  message?: string;
  source?: string;
  entityIds?: string[];
  fixAction?: string | null;
}

export interface ReadinessGateView {
  key: ReadinessGateKey;
  state: ReadinessGateState | string;
  checks?: ReadinessCheckView[];
}

export interface ShotReadinessView {
  projectId: string;
  shotId: string;
  stage: ProductionPreparationStage | string;
  status: ProductionPreparationStatus | string;
  score: number;
  gates: ReadinessGateView[];
  contextHash: string;
  evaluatedAt?: string;
  comfyCheckedAt?: string | null;
  cached?: boolean;
}

export interface SourceTraceView {
  scope?: string;
  scopeId?: string;
  bindingId?: string | null;
  entityId?: string;
  inheritanceMode?: string;
}

export interface ResolvedProfileView {
  profileId?: string;
  id?: string;
  profileType?: string;
  name?: string;
  ordinal?: number;
  revisionId?: string | null;
  contentHash?: string;
  source?: SourceTraceView | string;
  costumeVariantId?: string | null;
}

export interface ResolvedReferenceAssetView {
  assetId: string;
  sha256?: string;
  role?: string;
  ordinal?: number;
  sourceReferenceSetId?: string;
  sourceProfileId?: string | null;
  sourceScope?: string;
  name?: string;
  thumbnailUrl?: string;
}

export interface ResolvedReferenceSetView {
  referenceSetId: string;
  name?: string;
  role?: string;
  ordinal?: number;
  required?: boolean;
  contentHash?: string;
  source?: SourceTraceView | string;
  assets?: ResolvedReferenceAssetView[];
}

export interface ResolvedStructureNodeView {
  id: string;
  ordinal?: number;
  name: string;
}

export interface PromptSegmentView {
  kind?: string;
  text: string;
  sourceScope?: string;
  sourceEntityId?: string;
  revisionId?: string | null;
  omittedReason?: string | null;
}

export interface ResolvedContextView {
  stage?: string;
  structure?: {
    series?: ResolvedStructureNodeView | null;
    episode?: ResolvedStructureNodeView | null;
    scene?: ResolvedStructureNodeView | null;
    shot?: ResolvedStructureNodeView | null;
  };
  profiles?: {
    characters?: ResolvedProfileView[];
    scene?: ResolvedProfileView | null;
    props?: ResolvedProfileView[];
    style?: ResolvedProfileView | null;
  };
  referencePack?: {
    referenceSets?: ResolvedReferenceSetView[];
    referenceAssets?: ResolvedReferenceAssetView[];
    promptContext?: {
      renderedText?: string;
      negativePrompt?: string;
      segments?: PromptSegmentView[];
    };
  };
  referenceAssets?: ResolvedReferenceAssetView[];
  promptContext?: {
    renderedText?: string;
    negativePrompt?: string;
    segments?: PromptSegmentView[];
  };
  workflow?: {
    workflowVersionId?: string | null;
    recipeId?: string | null;
    scalarValues?: unknown;
  };
  output?: {
    width?: number | null;
    height?: number | null;
    count?: number | null;
    durationSeconds?: number | null;
  };
  stageInput?: {
    selectedImageAssetId?: string | null;
    selectedImageSha256?: string | null;
  };
  legacy?: {
    hasReferencePack?: boolean;
    usesLegacyShotReferences?: boolean;
    prompt?: string | null;
  };
  diagnostics?: Array<{ severity?: string; code?: string; message?: string }>;
}

export interface ShotProductionPlanSummary {
  shotId: string;
  ordinal: number;
  name: string;
  status: ProductionPreparationStatus | string;
  score: number;
  warningCount: number;
  incompleteCount: number;
  blockerCount: number;
  contextHash: string;
  characterNames?: string[];
  characterCount?: number;
  sceneProfileName?: string | null;
  referenceCount?: number;
  workflowVersionId?: string | null;
  recipeId?: string | null;
  currentStageStatus?: string | null;
  alreadyPrepared?: boolean;
  existingBatchIds?: string[];
  matchingPreparedBatchIds?: string[];
  stalePreparedBatchIds?: string[];
  blockers?: string[];
  warnings?: string[];
  thumbnailUrl?: string | null;
  thumbnailAssetId?: string | null;
  legacy?: boolean;
}

export interface ScenePreparationView {
  projectId: string;
  sceneId: string;
  sceneName: string;
  stage: ProductionPreparationStage;
  total: number;
  readyCount: number;
  incompleteCount: number;
  blockedCount: number;
  preparedCount: number;
  warningCount: number;
  items: ShotProductionPlanSummary[];
  evaluatedAt?: string;
}

export interface SceneProductionPreflightRequest {
  projectId: string;
  sceneId: string;
  stage: ProductionPreparationStage;
}

export interface ShotProductionPlanDetail {
  projectId: string;
  shotId: string;
  ordinal: number;
  name: string;
  sceneId?: string | null;
  stage: ProductionPreparationStage;
  contextHash: string;
  resolvedContext: ResolvedContextView;
  readiness: ShotReadinessView;
  currentStageStatus?: string | null;
  existingBatchIds: string[];
  matchingPreparedBatchIds: string[];
  stalePreparedBatchIds: string[];
  alreadyPrepared: boolean;
  blockers: string[];
  warnings: string[];
  snapshotIdentity?: {
    snapshotId: string;
    productionBatchId: string;
    productionBatchItemId: string;
    contextHash: string;
  } | null;
}

export type ShotProductionPlan = ShotProductionPlanDetail;

export interface ShotProductionPlanDetailRequest {
  projectId: string;
  shotId: string;
  stage: ProductionPreparationStage;
}

export interface SceneProductionAdmissionRequest extends SceneProductionPreflightRequest {
  shotIds: string[];
  allowPartial: boolean;
}

export interface SceneProductionAdmissionResult {
  projectId: string;
  sceneId?: string;
  stage: ProductionPreparationStage;
  createdBatchIds?: string[];
  batchId?: string | null;
  createdCount: number;
  skippedIncomplete: number;
  skippedBlocked: number;
  alreadyPreparedCount: number;
  matchingPreparedBatchIds?: string[];
  message?: string;
}

export type ProductionPreparationAdmission = SceneProductionAdmissionResult;

export const MAX_PREPARATION_BATCH_ITEMS = 100;

export function preparationStatusLabel(status: string): string {
  return {
    READY: "READY",
    INCOMPLETE: "INCOMPLETE",
    BLOCKED: "BLOCKED",
  }[status] ?? status;
}

export function preparationGateLabel(key: string): string {
  return {
    CHARACTER: "角色",
    SCENE: "场景",
    REFERENCE: "参考",
    PROMPT: "提示词",
    WORKFLOW: "工作流",
    OUTPUT: "输出",
    COMFY_CAPABILITY: "ComfyUI",
  }[key] ?? key;
}

export function readinessStateLabel(state: string): string {
  return {
    PASS: "通过",
    WARNING: "警告",
    INCOMPLETE: "未完成",
    BLOCKER: "阻塞",
  }[state] ?? state;
}

export function summaryStatus(value: ShotProductionPlanSummary): ProductionPreparationStatus | string {
  return value.status;
}

export function preparationCanSelect(item: Pick<ShotProductionPlanSummary, "status" | "alreadyPrepared">): boolean {
  return item.status === "READY" && !item.alreadyPrepared;
}
