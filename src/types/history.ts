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

export interface TaskDetail {
  id: string;
  projectId: string;
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
  errorMessage?: string;
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
