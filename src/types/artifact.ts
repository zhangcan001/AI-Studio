export type ArtifactMediaType = "image" | "video" | "audio" | "other";
export type ArtifactAvailability = "available" | "missing" | "unavailable";
export type ArtifactReviewDecision = "PENDING" | "APPROVED" | "REJECTED";
export type ArtifactReviewQueueFilter = "pending" | "completed";

export interface ArtifactDto {
  id: string;
  taskId: string;
  outputId: string;
  ordinal: number;
  mediaType: ArtifactMediaType;
  name: string;
  mimeType: string;
  width?: number;
  height?: number;
  durationMs?: number;
  sizeBytes: number;
  version: number;
  createdAt: string;
  thumbnailAvailable: boolean;
  availability: ArtifactAvailability;
  reviewStatus?: ArtifactReviewDecision;
  reviewRevision?: number;
}

export interface ProductionTaskContextDto {
  id: string;
  status: string;
  workflowVersionId: string;
  recipeId: string;
  createdAt: string;
  finishedAt?: string;
}

export interface ProductionTaskDto {
  productionItemId: string;
  ordinal: number;
  productionItemStatus: string;
  errorCode?: string;
  errorMessage?: string;
  task?: ProductionTaskContextDto;
  artifacts: ArtifactDto[];
}

export interface ProductionBatchArtifactsDto {
  batchId: string;
  batchName: string;
  status: string;
  total: number;
  pending: number;
  running: number;
  succeeded: number;
  failed: number;
  cancelled: number;
  skipped: number;
  items: ProductionTaskDto[];
}

export interface ArtifactReviewDto {
  id: string;
  artifactId: string;
  decision: ArtifactReviewDecision;
  comment: string;
  revision: number;
  createdAt: string;
  updatedAt: string;
}

export interface ArtifactReviewQueueItemDto {
  artifact: ArtifactDto;
  task?: ProductionTaskContextDto;
  review: ArtifactReviewDto;
}

export interface ArtifactReviewQueuePageDto {
  items: ArtifactReviewQueueItemDto[];
  total: number;
  limit: number;
  offset: number;
}

export interface ArtifactReviewSubmitRequest {
  projectId: string;
  artifactId: string;
  decision: Exclude<ArtifactReviewDecision, "PENDING">;
  comment: string;
  expectedRevision: number;
}

export interface ArtifactReviewResetRequest {
  projectId: string;
  artifactId: string;
  expectedRevision: number;
}
