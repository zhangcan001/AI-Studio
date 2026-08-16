import type { GenerationValues } from "./generation";

export type ProductionRunStatus =
  | "DRAFT"
  | "READY"
  | "RUNNING"
  | "WAITING_FOR_SELECTION"
  | "SUCCEEDED"
  | "PARTIAL_FAILED"
  | "FAILED"
  | "CANCELLED";

export interface ProductionRunCreateRequest {
  projectId: string;
  name: string;
  krea2WorkflowVersionId: string;
  krea2RecipeId: string;
  krea2PresetId?: string;
  krea2Values: GenerationValues;
  imageCount: number;
  h3WorkflowVersionId?: string;
  h3RecipeId?: string;
  h3Profile?: string;
  h3Values: GenerationValues;
  templateId?: string;
}

export interface ProductionRunStageItem {
  id: string;
  stageId: string;
  ordinal: number;
  status: string;
  productionBatchItemId?: string;
  taskId?: string;
  taskStatus?: string;
  assetId?: string;
  sourceAssetId?: string;
  referenceIndex?: number;
  attempt: number;
  submissionIdempotencyKey?: string;
  parentStageItemId?: string;
  frozenValues: GenerationValues;
  errorCode?: string;
  errorMessage?: string;
}

export interface ProductionRunStage {
  id: string;
  ordinal: number;
  stageType: "KREA2_IMAGE_GENERATION" | "ASSET_SELECTION" | "H3_VIDEO_GENERATION" | string;
  status: string;
  workflowVersionId?: string;
  recipeId?: string;
  productionBatchId?: string;
  frozenConfig: Record<string, unknown>;
  prompt?: string;
  items: ProductionRunStageItem[];
}

export interface ProductionRun {
  id: string;
  projectId: string;
  name: string;
  status: ProductionRunStatus | string;
  currentStageOrdinal: number;
  templateId?: string;
  createdAt: string;
  updatedAt: string;
  startedAt?: string;
  finishedAt?: string;
  stages: ProductionRunStage[];
}

export interface ProductionRunListItem {
  id: string;
  projectId: string;
  name: string;
  status: ProductionRunStatus | string;
  currentStageOrdinal: number;
  templateId?: string;
  createdAt: string;
  updatedAt: string;
}

export interface ProductionRunTemplate {
  id: string;
  projectId: string;
  name: string;
  krea2WorkflowVersionId?: string;
  krea2RecipeId?: string;
  krea2PresetId?: string;
  defaultImageCount: number;
  h3WorkflowVersionId?: string;
  h3RecipeId?: string;
  h3Profile?: string;
  defaultDurationSeconds?: number;
  defaultWidth?: number;
  defaultHeight?: number;
  createdAt: string;
  updatedAt: string;
}
