export interface WorkflowHistoryCursor {
  createdAt: string;
  id: string;
}

export interface WorkflowRecipeHistoryTask {
  id: string;
  projectId: string;
  projectName: string;
  status: string;
  createdAt: string;
  queuedAt?: string | null;
  startedAt?: string | null;
  finishedAt?: string | null;
}

export interface WorkflowRecipeHistoryQueueItem {
  batchId: string;
  batchName: string;
  projectId: string;
  projectName: string;
  batchStatus: string;
  itemId: string;
  itemStatus: string;
  taskId?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface WorkflowRecipeHistoryPreset {
  id: string;
  name: string;
  projectId: string;
  projectName: string;
  updatedAt: string;
}

export interface WorkflowRecipeHistoryProjectTemplate {
  id: string;
  name: string;
  updatedAt: string;
}

export interface WorkflowRecipeHistoryProductionRunTemplate {
  id: string;
  name: string;
  projectId: string;
  projectName: string;
  recipeRole: string;
  updatedAt: string;
}

export interface WorkflowRecipeHistoryBinding {
  projectId: string;
  projectName: string;
  stage: string;
  mode: string;
  updatedAt: string;
}

export interface WorkflowRecipeHistoryShot {
  shotId: string;
  projectId: string;
  projectName: string;
  stage: string;
  updatedAt: string;
  taskId?: string | null;
  productionBatchItemId?: string | null;
}

export interface WorkflowRecipeHistoryExperiment {
  experimentId: string;
  experimentName: string;
  projectId: string;
  projectName: string;
  candidateId: string;
  runCount: number;
  createdAt: string;
}

export interface WorkflowRecipeHistoryView {
  workflowId: string;
  workflowVersionId: string;
  recipeId: string;
  workflowVersion: string;
  recipeVersion: string;
  schemaVersion: number;
  recipeSha256: string;
  createdAt: string;
  isCurrentVersion: boolean;
  isPromoted: boolean;
  promotedAt?: string | null;
  archived: boolean;
  archivedAt?: string | null;
  workflowLibraryState: string;
  taskCount: number;
  activeTaskCount: number;
  succeededTaskCount: number;
  failedTaskCount: number;
  cancelledTaskCount: number;
  lastFinishedAt?: string | null;
  lastAttemptAt?: string | null;
  executedProjectCount: number;
  referencedProjectCount: number;
  presetCount: number;
  projectTemplateCount: number;
  productionRunTemplateCount: number;
  queueItemCount: number;
  shotStageCount: number;
  experimentCount: number;
  benchmarkRunCount: number;
  projectBindingCount: number;
  taskPage: {
    items: WorkflowRecipeHistoryTask[];
    nextCursor?: WorkflowHistoryCursor | null;
  };
  queueItems: WorkflowRecipeHistoryQueueItem[];
  presets: WorkflowRecipeHistoryPreset[];
  projectTemplates: WorkflowRecipeHistoryProjectTemplate[];
  productionRunTemplates: WorkflowRecipeHistoryProductionRunTemplate[];
  projectBindings: WorkflowRecipeHistoryBinding[];
  shots: WorkflowRecipeHistoryShot[];
  experiments: WorkflowRecipeHistoryExperiment[];
}
