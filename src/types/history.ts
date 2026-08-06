import type { AssetView, PageCursor } from "./asset";
import type { GenerationValues } from "./generation";
import type { TaskStatus } from "./task";

export type TaskHistoryFilter = "ALL" | "ACTIVE" | "SUCCEEDED" | "FAILED" | "CANCELLED";

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
  values: GenerationValues;
  missingAssetIds: string[];
}
