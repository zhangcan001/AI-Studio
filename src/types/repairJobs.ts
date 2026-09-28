export interface RepairItem {
  workflowId: string;
  workflowVersionId: string;
  recipeId: string;
  reason: string;
  workflowVersion?: string;
  recipeVersion?: string;
  packageName?: string;
}

export interface RepairFailure {
  recipeId: string;
  message: string;
}

export interface RepairSummary {
  planned: number;
  repaired: number;
  skipped: number;
  needsReview: RepairItem[];
  failed: RepairFailure[];
  publishedRecipeIds: string[];
}

export type RepairJobStatus = "RUNNING" | "COMPLETED" | "FAILED" | "SKIPPED";

export interface RepairJobStatusView {
  jobId: string;
  status: RepairJobStatus;
  startedAt: string;
  completedAt: string | null;
  summary: RepairSummary | null;
  summaryRaw: string;
}
