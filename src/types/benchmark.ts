import type { GenerationValues } from "./generation";

export type BenchmarkMediaType = "IMAGE" | "VIDEO";
export type BenchmarkSeedMode = "FIXED" | "EXPLORATION";
export type BenchmarkCandidateCompatibility = "COMPATIBLE" | "PARTIAL" | "INCOMPATIBLE";

export interface WorkflowBenchmarkCandidateRequest {
  workflowVersionId: string;
  recipeId: string;
  presetId?: string;
  label?: string;
}

export interface WorkflowBenchmarkCreateRequest {
  projectId: string;
  name: string;
  mediaType: BenchmarkMediaType;
  baseValues: GenerationValues;
  candidates: WorkflowBenchmarkCandidateRequest[];
  seedMode?: BenchmarkSeedMode;
  fixedSeed?: string;
  autoStart?: boolean;
}

export interface WorkflowBenchmarkCandidatePreview {
  id: string;
  position: number;
  workflowVersionId: string;
  recipeId: string;
  presetId?: string;
  presetName?: string;
  label: string;
  compatibility: BenchmarkCandidateCompatibility;
  compatibilityReasons: string[];
  frozenValues: GenerationValues;
  assetIds: string[];
}

export interface WorkflowBenchmarkCandidateView extends WorkflowBenchmarkCandidatePreview {
  productionBatchItemId?: string;
  taskId?: string;
  taskStatus?: string;
  taskCreatedAt?: string;
  taskStartedAt?: string;
  taskFinishedAt?: string;
  executionDurationMs?: number;
  outputAssetIds: string[];
  reviewStatus?: string;
  reviewNote?: string;
}

export interface WorkflowBenchmarkSummary {
  id: string;
  projectId: string;
  name: string;
  mediaType: BenchmarkMediaType;
  status: "DRAFT" | "QUEUED" | "RUNNING" | "COMPLETED" | "PARTIAL" | "CANCELLED" | "FAILED_TO_QUEUE";
  winnerCandidateId?: string;
  productionBatchId?: string;
  candidateCount: number;
  succeededCount: number;
  failedCount: number;
  fastestCandidateId?: string;
  fastestDurationMs?: number;
  createdAt: string;
  updatedAt: string;
}

export interface WorkflowBenchmarkView extends WorkflowBenchmarkSummary {
  baseValues: GenerationValues;
  assetIds: string[];
  candidates: WorkflowBenchmarkCandidateView[];
  summary: WorkflowBenchmarkSummary;
}
