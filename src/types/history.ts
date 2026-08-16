import type { AssetView, PageCursor } from "./asset";
import type { GenerationValues } from "./generation";
import type { TaskStatus } from "./task";

export type TaskHistoryFilter = "ALL" | "ACTIVE" | "SUCCEEDED" | "FAILED" | "CANCELLED";
export type TaskHistoryTimeFilter = "ALL" | "TODAY" | "LAST_7_DAYS" | "LAST_30_DAYS";

export interface TaskHistoryQuery {
  projectId: string;
  filter: TaskHistoryFilter;
  workflowId?: string;
  keyword?: string;
  timeFilter: TaskHistoryTimeFilter;
  cursor?: PageCursor;
  limit?: number;
}

export interface TaskHistoryWorkflowOption {
  workflowId: string;
  workflowName: string;
}

export interface TaskHistoryItem {
  id: string;
  workflowId: string;
  workflowVersionId: string;
  recipeId: string;
  workflowName: string;
  status: TaskStatus;
  createdAt: string;
  queuedAt?: string;
  startedAt?: string;
  finishedAt?: string;
  errorCode?: string;
  outputCount: number;
}

export interface TaskHistoryPage {
  items: TaskHistoryItem[];
  nextCursor?: PageCursor;
  workflowOptions: TaskHistoryWorkflowOption[];
}

export interface ReusableDraftAvailability {
  available: boolean;
  reason?: string;
  missingAssetIds: string[];
}

export interface TaskNodeError {
  nodeId: string;
  nodeType?: string;
  input?: string;
  errorType?: string;
  message: string;
  details?: string;
  receivedValue?: unknown;
  expectedConfig?: unknown;
}

export interface RuntimeProvenance {
  appVersion: string;
  buildCommit: string;
  workflowId: string;
  workflowVersionId: string;
  workflowVersion: string;
  workflowSha256: string;
  recipeId: string;
  recipeVersion: string;
  recipeSha256: string;
  packageName?: string;
  packageSourcePath?: string;
  dynamicBindingTargets: string[];
}

export interface TaskTelemetry {
  generationExecutionId?: string;
  compiledWorkflowSha256?: string;
  runtimeProfile?: string;
  concurrencyClass?: string;
  prepareStartedAt?: string;
  preparedAt?: string;
  submittedAt?: string;
  executionStartedAt?: string;
  executionFinishedAt?: string;
  collectionFinishedAt?: string;
  queueWaitMs?: number;
  prepareMs?: number;
  submitMs?: number;
  comfyExecutionMs?: number;
  collectionMs?: number;
  totalMs?: number;
}

export interface TaskDetail {
  id: string;
  projectId: string;
  workflowId: string;
  workflowVersionId: string;
  recipeId: string;
  runtimeProvenance?: RuntimeProvenance;
  telemetry?: TaskTelemetry;
  workflowName: string;
  status: TaskStatus;
  createdAt: string;
  queuedAt?: string;
  startedAt?: string;
  finishedAt?: string;
  errorCode?: string;
  errorMessage?: string;
  nodeErrors?: TaskNodeError[];
  rawError?: unknown;
  outputAssets: AssetView[];
  reusableDraft: ReusableDraftAvailability;
}

export interface ReusableGenerationDraft {
  projectId: string;
  workflowVersionId: string;
  recipeId: string;
  workflowName: string;
  createdAt: string;
  values: GenerationValues;
  missingAssetIds: string[];
}
