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
    firstImageReviewShotId?: string | null;
    firstVideoReviewShotId?: string | null;
    firstMissingConfigShotId?: string | null;
    firstReadyShotId?: string | null;
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
    firstAutoResumableBatchId?: string | null;
    firstReviewRequiredBatchId?: string | null;
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
  promptTemplates: { total: number; versions: number; items: Array<{ id: string; name: string; versionCount: number; updatedAt: string }> };
  comfy: { status?: ComfyStatus | null; preflight?: ComfyPreflightReport | null };
  readiness: { status?: string | null; connection?: string | null; workflowReady: number; workflowTotal: number; runtimeBusy: boolean; activeTaskCount: number; productionBusy: boolean };
  content: { shots: number; prompts: number; assets: number; scenes: number; configuredShots: number };
  production: { active: number; completed: number; failed: number; reviewRequired: number };
  consistency?: ProjectCommandCenterConsistencyView;
  preparation?: ProjectCommandCenterPreparationView;
  issues: Array<{ id: string; severity: string; title: string; detail: string; source: string }>;
  audit: ProductionAuditSummary;
  recentActivity: ProductionAuditActivity[];
  recommendedAction: { kind: string; priority: number; reasonCode: string; reason: string; shotId?: string | null; batchId?: string | null };
  quickActions: Array<{ id: string; label: string; destination: string }>;
  checkedAt: string;
}
