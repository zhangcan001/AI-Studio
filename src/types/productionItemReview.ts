import type { AssetView } from "./asset";
import type { ProductionBatchDetail } from "./productionQueue";

export type ProductionReviewStatus =
  | "UNREVIEWED"
  | "APPROVED"
  | "STARRED"
  | "REGENERATE"
  | "REJECTED"
  | "FAILED"
  | "IN_PROGRESS";

export interface ProductionReviewCandidateAsset {
  assetId: string;
  assetType: string;
  name: string;
  mimeType: string;
  width?: number;
  height?: number;
  thumbnailAvailable: boolean;
  taskId?: string;
  /** Absolute path projected from the database Asset.storage_path. */
  localPath?: string;
  selected: boolean;
  reviewResult?: string;
}

export interface ProductionReviewItem {
  itemId: string;
  ordinal: number;
  taskId?: string;
  taskStatus: string;
  productionItemStatus: string;
  reviewStatus: ProductionReviewStatus;
  reviewNote: string;
  version?: number;
  lineageKey?: string;
  parentBatchId?: string;
  parentItemId?: string;
  preferred: boolean;
  workflowVersionId: string;
  recipeId: string;
  promptText?: string;
  seed?: string;
  durationSeconds?: number;
  width?: number;
  height?: number;
  qualityProfile: "QUALITY" | "FAST" | string;
  createdAt: string;
  finishedAt?: string;
  outputAssets: AssetView[];
}

export interface ProductionBatchReview {
  batch: ProductionBatchDetail;
  total: number;
  successCount: number;
  failedCount: number;
  unreviewedCount: number;
  approvedCount: number;
  starredCount: number;
  regenerateCount: number;
  rejectedCount: number;
  items: ProductionReviewItem[];
}

export interface ProductionReviewRegenerateResult {
  batch: ProductionBatchDetail;
  sourceItemIds: string[];
  selectedCount: number;
  autoStarted: boolean;
  startWarning?: string;
}
