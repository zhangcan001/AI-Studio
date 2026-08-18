export type ProductionAuditHealth = "HEALTHY" | "WARNING" | "BLOCKED";

export type ProductionAuditSeverity = "INFO" | "WARNING" | "ERROR";

export type ProductionAuditActivityKind =
  | "RUN_CREATED"
  | "RUN_COMPLETED"
  | "RUN_FAILED"
  | "BATCH_CREATED"
  | "BATCH_PAUSED"
  | "BATCH_COMPLETED"
  | "ITEM_FAILED"
  | "ITEM_RETRIED"
  | "TASK_SUCCEEDED"
  | "TASK_FAILED"
  | "ASSET_CREATED"
  | "SHOT_IMAGE_SELECTED"
  | "SHOT_VIDEO_SELECTED"
  | string;

export interface ProductionAuditIssue {
  severity: ProductionAuditSeverity;
  code: string;
  message: string;
  entityType: string;
  entityId: string;
  relatedIds: string[];
}

export interface ProductionAuditSummary {
  projectId: string;
  health: ProductionAuditHealth;
  activeRuns: number;
  completedRuns: number;
  failedRuns: number;
  activeBatches: number;
  pausedBatches: number;
  failedBatches: number;
  logicalItems: number;
  attempts: number;
  succeededItems: number;
  failedItems: number;
  reviewRequiredItems: number;
  tasks: number;
  succeededTasks: number;
  failedTasks: number;
  assets: number;
  unassignedShots: number;
  checkedAt: string;
  issues: ProductionAuditIssue[];
}

export interface ProductionAuditActivity {
  id: string;
  kind: ProductionAuditActivityKind;
  timestamp: string;
  severity: ProductionAuditSeverity;
  title: string;
  detail: string;
  runId?: string;
  batchId?: string;
  itemId?: string;
  taskId?: string;
  shotId?: string;
  shotName?: string;
  assetId?: string;
  errorCode?: string;
  status?: string;
  retryOfItemId?: string;
}

export type ProductionAuditRootType = "RUN" | "BATCH" | "SHOT" | "TASK";

export type ProductionAuditNodeType =
  | "RUN"
  | "STAGE"
  | "BATCH"
  | "LOGICAL_ITEM"
  | "ATTEMPT"
  | "TASK"
  | "SNAPSHOT"
  | "ASSET"
  | "SHOT";

export interface ProductionAuditLineageNode {
  entityType: ProductionAuditNodeType | string;
  id: string;
  label: string;
  status?: string;
  parentId?: string;
  runId?: string;
  batchId?: string;
  itemId?: string;
  taskId?: string;
  shotId?: string;
  shotName?: string;
  assetId?: string;
  snapshotId?: string;
}

export interface ProductionAuditLineage {
  projectId: string;
  rootType: ProductionAuditRootType;
  rootId: string;
  nodes: ProductionAuditLineageNode[];
}

export interface ProductionAuditIntegrity {
  projectId: string;
  health: ProductionAuditHealth;
  issues: ProductionAuditIssue[];
  checkedAt: string;
}

export interface ProductionAuditRecentActivityRequest {
  projectId: string;
  limit?: number;
}

export interface ProductionAuditLineageRequest {
  projectId: string;
  rootType: ProductionAuditRootType;
  rootId: string;
}
