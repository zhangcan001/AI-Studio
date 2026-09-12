import type { ComfyStatus } from "./comfy";
import type { ComfyPreflightReport } from "./settings";
import type { ProductionAuditActivity, ProductionAuditSummary } from "./productionAudit";

export interface ProjectCommandCenterSceneSummary {
  id: string;
  name: string;
  path: string;
  total: number;
  completed: number;
}

export interface ProjectCommandCenterConsistencyView {
  characterProfiles: number;
  sceneProfiles: number;
  propProfiles: number;
  styleProfiles: number;
  referenceSets: number;
  shotProfileBindings: number;
  shotReferenceSetBindings: number;
  scopeProfileBindings: number;
  scopeReferenceSetBindings: number;
  consistencyInUse: boolean;
}

export interface ProjectCommandCenterPreparationView {
  snapshotCount: number;
  preparedImageItems: number;
  preparedVideoItems: number;
  activePreparedItems: number;
  latestPreparedAt?: string | null;
}

export interface ProjectCommandCenterDailyProductionItem {
  id: string;
  label: string;
  reasonCode: string;
  reason: string;
  severity: string;
  destination: string;
  stage?: string | null;
  shotId?: string | null;
  batchId?: string | null;
  taskId?: string | null;
  assetId?: string | null;
  workflowVersionId?: string | null;
  recipeId?: string | null;
}

export interface ProjectCommandCenterDailyProductionBucket {
  totalCount: number;
  items: ProjectCommandCenterDailyProductionItem[];
  hasMore: boolean;
}

export interface ProjectCommandCenterDailyProductionView {
  needsAttention: ProjectCommandCenterDailyProductionBucket;
  ready: ProjectCommandCenterDailyProductionBucket;
  running: ProjectCommandCenterDailyProductionBucket;
  review: ProjectCommandCenterDailyProductionBucket;
  completed: ProjectCommandCenterDailyProductionBucket;
  topAction?: ProjectCommandCenterDailyProductionItem | null;
}

export interface ProjectCommandCenterAggregate {
  project: { id: string; name: string; description?: string | null; createdAt: string; updatedAt: string };
  structure: {
    seriesCount: number;
    episodeCount: number;
    sceneCount: number;
    assignedShotCount: number;
    unassignedShotCount: number;
    firstUnassignedShotId?: string | null;
    blocked: boolean;
    scenes: ProjectCommandCenterSceneSummary[];
  };
  shots: {
    total: number;
    draft: number;
    ready: number;
    generating: number;
    imageReview: number;
    imageSelected: number;
    videoReview: number;
    completed: number;
    failed: number;
    configured: number;
    missingConfig: number;
    firstGeneratingShotId?: string | null;
    firstGeneratingTaskId?: string | null;
    firstFailedShotId?: string | null;
    firstFailedTaskId?: string | null;
    firstImageReviewShotId?: string | null;
    firstVideoReviewShotId?: string | null;
    firstMissingConfigShotId?: string | null;
    firstReadyShotId?: string | null;
    firstCompletedShotId?: string | null;
    firstCompletedAssetId?: string | null;
  };
  queue: {
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
    autoResumableItems: number;
    reviewRequiredItems: number;
    firstActiveBatchId?: string | null;
    firstActiveShotId?: string | null;
    firstActiveTaskId?: string | null;
    firstAutoResumableBatchId?: string | null;
    firstAutoResumableShotId?: string | null;
    firstAutoResumableTaskId?: string | null;
    firstReviewRequiredBatchId?: string | null;
    firstReviewRequiredShotId?: string | null;
    firstReviewRequiredTaskId?: string | null;
  };
  tasksAssets: {
    taskCount: number;
    activeTaskCount: number;
    succeededTaskCount: number;
    failedTaskCount: number;
    assetCount: number;
    imageAssetCount: number;
    videoAssetCount: number;
    audioAssetCount: number;
    otherAssetCount: number;
  };
  referenceAnchors: { total: number; usable: number; character: number; scene: number; prop: number; style: number };
  promptLibrary: { total: number; versions: number; items: Array<{ id: string; name: string; versionCount: number; updatedAt: string }> };
  comfy: { status?: ComfyStatus | null; preflight?: ComfyPreflightReport | null };
  readiness: { status?: string | null; connection?: string | null; workflowReady: number; workflowTotal: number; runtimeBusy: boolean; activeTaskCount: number; productionBusy: boolean };
  content: { shots: number; prompts: number; assets: number; scenes: number; configuredShots: number };
  production: { active: number; completed: number; failed: number; reviewRequired: number };
  dailyProduction?: ProjectCommandCenterDailyProductionView;
  consistency?: ProjectCommandCenterConsistencyView;
  preparation?: ProjectCommandCenterPreparationView;
  issues: Array<{ id: string; severity: string; title: string; detail: string; source: string }>;
  audit: ProductionAuditSummary;
  recentActivity: ProductionAuditActivity[];
  recommendedAction: { kind: string; priority: number; reasonCode: string; reason: string; shotId?: string | null; batchId?: string | null; taskId?: string | null; assetId?: string | null };
  quickActions: Array<{ id: string; label: string; destination: string }>;
  checkedAt: string;
}
