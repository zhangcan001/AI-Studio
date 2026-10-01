export interface RunRef { source: "production-run" | "queue-batch" | "task"; id: string }
export interface GeneratorOption {
  selectionRef: string;
  name: string;
  version: string;
  mode: string;
  mediaKind: "image" | "video";
  availability: boolean;
  availabilityReason: string | null;
  recommended: boolean;
}
export interface GeneratorBindingSummary {
  stage: string;
  mode: string;
  selectionRef: string;
  revision: number;
  bindingInstanceId: string;
}
export interface GeneratorBindingSetRequest {
  stage: string;
  mode: string;
  selectionRef: string;
  expectedRevision: number | null;
  expectedBindingInstanceId: string | null;
}
export interface ProjectOverview {
  blockingState: "HEALTHY" | "WARNING" | "BLOCKED";
  project: { id: string; name: string; description: string | null; createdAt: string; updatedAt: string };
  progress: { total: number; completed: number; failed: number };
  nextAction: { kind: string; priority: number; reasonCode: string; reason: string; shotId: string | null; batchId: string | null; taskId: string | null; assetId: string | null };
  blockingIssues: { severity: string; title: string }[];
  activeRuns: { runningBatches: number; pausedBatches: number; activeTasks: number };
  recentResults: { total: number; images: number; videos: number };
  runtimeReadiness: { status: "READY" | "WARNING" | "BLOCKED" | null; connection: string | null; workflowReady: number; workflowTotal: number; runtimeBusy: boolean; activeTaskCount: number; productionBusy: boolean };
}
export interface ProductRun {
  ref: RunRef;
  projectId: string;
  title: string;
  status: "QUEUED" | "RUNNING" | "PAUSED" | "FAILED" | "SUCCEEDED" | "PARTIAL" | "CANCELLED";
  phase: string;
  createdAt: string;
  updatedAt: string;
  progress: { total: number; succeeded: number; failed: number; cancelled: number };
  recoverability: { retryItemIds: string[]; reviewRequired: number };
  resultsSummary: string[];
  errorSummary: string | null;
  preferredParent: RunRef | null;
  availableActions: string[];
}
export interface RunRetryRequest { ref: RunRef; selectedItemIds: string[] }
