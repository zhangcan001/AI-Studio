import type { AssetView } from "./asset";
import type { ProductionReviewStatus } from "./productionItemReview";
import type { ShotStage } from "./shot";

export type ReviewCompareSlot = "A" | "B";
export type ReviewCompareMediaKind = "image" | "video";

export interface ReviewCompareReferenceAsset {
  id: string;
  name: string;
  role?: string;
  ordinal?: number;
  sha256?: string;
  asset?: AssetView;
  mediaUrl?: string;
  thumbnailUrl?: string;
  [key: string]: unknown;
}

export interface ReviewCompareReferenceSet {
  id: string;
  name: string;
  assets?: ReviewCompareReferenceAsset[];
  [key: string]: unknown;
}

export interface ReviewCompareOutputSpec {
  mediaKind?: ReviewCompareMediaKind | string;
  width?: number;
  height?: number;
  durationSeconds?: number;
  format?: string;
  [key: string]: unknown;
}

export interface ReviewCompareStageInput {
  stage?: ShotStage | string;
  assetIds?: string[];
  description?: string;
  [key: string]: unknown;
}

export interface ReviewCompareReadiness {
  status?: string;
  reasons?: string[];
  [key: string]: unknown;
}

/**
 * A frozen production context when one exists. This is intentionally a UI
 * contract: hosts can adapt the backend snapshot without making this view
 * know how to load or resolve production data.
 */
export interface ReviewCompareContextSnapshot {
  source?: "snapshot" | "legacy" | string;
  historicalName?: string;
  currentName?: string;
  prompt?: string;
  promptText?: string;
  context?: string;
  workflow?: string;
  workflowName?: string;
  workflowVersionId?: string;
  recipe?: string;
  recipeName?: string;
  recipeId?: string;
  contextHash?: string;
  referenceSets?: ReviewCompareReferenceSet[];
  referenceAssets?: ReviewCompareReferenceAsset[];
  outputSpec?: ReviewCompareOutputSpec | string;
  stageInput?: ReviewCompareStageInput | string;
  readiness?: ReviewCompareReadiness | string;
  negativePrompt?: string;
}

export interface ReviewCompareCandidate {
  id: string;
  itemId?: string;
  label?: string;
  version?: number;
  asset: AssetView;
  mediaKind?: ReviewCompareMediaKind;
  /** A pre-resolved image URL; images are never read as bytes by this view. */
  imageUrl?: string;
  /** A native media URL, normally supplied for video candidates. */
  mediaUrl?: string;
  selected?: boolean;
  preferred?: boolean;
  reviewStatus?: ProductionReviewStatus;
  productionItemStatus?: string;
  taskStatus?: string;
  reviewNote?: string;
  context?: ReviewCompareContextSnapshot;
  historicalContext?: ReviewCompareContextSnapshot;
  contextSnapshot?: ReviewCompareContextSnapshot;
  snapshot?: ReviewCompareContextSnapshot;
}

export interface ReviewCompareItem {
  id: string;
  ordinal: number;
  name?: string;
  shotId?: string;
  shotName?: string;
  stage?: ShotStage | string;
  candidates: ReviewCompareCandidate[];
  selectedCandidateId?: string;
  reviewStatus?: ProductionReviewStatus;
  reviewNote?: string;
  context?: ReviewCompareContextSnapshot;
  historicalContext?: ReviewCompareContextSnapshot;
  contextSnapshot?: ReviewCompareContextSnapshot;
  snapshot?: ReviewCompareContextSnapshot;
}

export type ReviewCompareAction =
  | "confirmAndApprove"
  | "approve"
  | "star"
  | "reject"
  | "regenerate"
  | "createReworkBatch"
  | "saveNote";

export const REVIEW_NOTE_MAX_BYTES = 4096;

export function reviewNoteByteLength(note: string): number {
  return new TextEncoder().encode(note).byteLength;
}

export function validateReviewNote(note: string): string | undefined {
  return reviewNoteByteLength(note) > REVIEW_NOTE_MAX_BYTES ? "备注不能超过 4 KiB。" : undefined;
}
