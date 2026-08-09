import type { GenerationValues } from "./generation";

export type ProductionBatchStatus = "READY" | "RUNNING" | "PAUSED" | "COMPLETED";
export type ProductionBatchItemStatus =
  | "PENDING"
  | "DISPATCHING"
  | "DISPATCHED"
  | "SUCCEEDED"
  | "FAILED"
  | "CANCELLED"
  | "SKIPPED";

export interface ProductionBatchSummary {
  id: string;
  projectId: string;
  name: string;
  status: ProductionBatchStatus;
  continueOnFailure: boolean;
  archivedAt?: string;
  createdAt: string;
  updatedAt: string;
}

export interface ProductionBatchItemView {
  id: string;
  ordinal: number;
  workflowVersionId: string;
  recipeId: string;
  status: ProductionBatchItemStatus;
  taskId?: string;
  retryOfItemId?: string;
  errorCode?: string;
  errorMessage?: string;
}

export interface ProductionBatchDetail extends ProductionBatchSummary {
  total: number;
  pending: number;
  running: number;
  succeeded: number;
  failed: number;
  cancelled: number;
  skipped: number;
  items: ProductionBatchItemView[];
}

export interface ProductionQueueOverview {
  totalQueues: number;
  runningQueues: number;
  pausedQueues: number;
  completedQueues: number;
  archivedQueues: number;
  totalItems: number;
  pendingItems: number;
  activeItems: number;
  succeededItems: number;
  failedItems: number;
  cancelledItems: number;
  skippedItems: number;
}

export interface ProductionAdmissionStatus {
  busy: boolean;
  batchId?: string;
  projectId?: string;
  batchName?: string;
  activeTaskId?: string;
}

export interface ProductionBatchCreateItem {
  workflowVersionId: string;
  recipeId: string;
  values: GenerationValues;
}
