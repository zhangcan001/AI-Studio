import type { DraftValue, RecipeViewModel } from "./generation";
import type { AssetView } from "./asset";
import type { TaskView } from "./task";

export type ShotStage = "image" | "video";

export type ShotScalarValue =
  | { type: "integer"; value: number }
  | { type: "seed_random" }
  | { type: "seed_fixed"; value: string };

export interface ShotStageConfig {
  stage: ShotStage;
  workflowVersionId: string;
  recipeId: string;
  scalarValues: Record<string, ShotScalarValue>;
  updatedAt: string;
}

export interface ShotReferenceAsset {
  stage: ShotStage;
  assetId: string;
  ordinal: number;
}

export interface ShotGenerationLink {
  id: string;
  stage: ShotStage;
  taskId?: string;
  productionBatchItemId?: string;
  createdAt: string;
  task?: TaskView;
}

export interface ShotView {
  id: string;
  projectId: string;
  ordinal: number;
  name: string;
  promptText: string;
  promptEntryId?: string;
  promptVersionId?: string;
  selectedImageAssetId?: string;
  selectedVideoAssetId?: string;
  createdAt: string;
  updatedAt: string;
  status: string;
  imageStatus: string;
  videoStatus: string;
  stageConfigs: ShotStageConfig[];
  referenceAssets: ShotReferenceAsset[];
  generationLinks: ShotGenerationLink[];
}

export interface ShotStudioContext {
  shotId: string;
  stage: ShotStage;
  shot: ShotView;
  recipe: RecipeViewModel;
}

export interface ShotCandidate {
  asset: AssetView;
  taskId?: string;
  stage: ShotStage;
  createdAt: string;
}

export type ShotInputValues = Record<string, DraftValue>;
