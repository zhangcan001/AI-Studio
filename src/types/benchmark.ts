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
  repeatCount?: 1 | 3 | 5 | 10;
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
  workflowId?: string;
  workflowVersion?: string;
  workflowSha256?: string;
  recipeVersion?: string;
  recipeSha256?: string;
  runtimePackage?: string;
  runtimeProfile?: string;
}

export interface WorkflowBenchmarkTelemetry {
  compiledWorkflowSha256?: string;
  runtimeProfile?: string;
  queueWaitMs?: number;
  prepareMs?: number;
  comfyExecutionMs?: number;
  collectionMs?: number;
  totalMs?: number;
}

export interface WorkflowBenchmarkRun {
  id: string;
  candidateId: string;
  runNumber: number;
  productionBatchItemId?: string;
  taskId?: string;
  snapshotId?: string;
  outputAssetId?: string;
  generationExecutionId?: string;
  compiledWorkflowSha256?: string;
  runtimeProfile?: string;
  concurrencyClass?: string;
  queueWaitMs?: number;
  prepareMs?: number;
  submitMs?: number;
  comfyExecutionMs?: number;
  collectMs?: number;
  totalMs?: number;
  status?: string;
  errorCode?: string;
  outputFileSize?: number;
}

export interface WorkflowBenchmarkMetricSummary {
  min?: number;
  median?: number;
  mean?: number;
  p95?: number;
  max?: number;
}

export interface WorkflowBenchmarkAggregate {
  runsTotal: number;
  runsSuccess: number;
  runsFailed: number;
  successRate: number;
  totalMs: WorkflowBenchmarkMetricSummary;
  comfyExecutionMs: WorkflowBenchmarkMetricSummary;
  prepareMsMean?: number;
  collectMsMean?: number;
  outputSizeMean?: number;
}

export interface WorkflowBenchmarkQuality {
  promptAdherence?: number;
  visualQuality?: number;
  motionQuality?: number;
  referenceConsistency?: number;
  overall?: number;
  note?: string;
}

export interface WorkflowBenchmarkRecommendation {
  kind: "FASTEST" | "MOST_STABLE" | "BEST_QUALITY" | "BEST_BALANCE" | string;
  candidateId?: string;
  label?: string;
  rationale: string;
}

export interface WorkflowBenchmarkComparison {
  directlyComparable: boolean;
  reason?: string;
  recommendations: WorkflowBenchmarkRecommendation[];
}

export interface WorkflowBenchmarkCandidateView extends WorkflowBenchmarkCandidatePreview {
  productionBatchItemId?: string;
  taskId?: string;
  taskStatus?: string;
  taskCreatedAt?: string;
  taskStartedAt?: string;
  taskFinishedAt?: string;
  executionDurationMs?: number;
  telemetry?: WorkflowBenchmarkTelemetry;
  runs: WorkflowBenchmarkRun[];
  aggregate: WorkflowBenchmarkAggregate;
  quality?: WorkflowBenchmarkQuality;
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
  repeatCount: number;
  seedStrategy: "FIXED_SEED" | "RANDOM_SEED" | string;
  recommendationType?: string;
  createdAt: string;
  updatedAt: string;
}

export interface WorkflowBenchmarkView extends WorkflowBenchmarkSummary {
  baseValues: GenerationValues;
  assetIds: string[];
  candidates: WorkflowBenchmarkCandidateView[];
  summary: WorkflowBenchmarkSummary;
  comparison: WorkflowBenchmarkComparison;
}
