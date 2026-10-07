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
  fields: import("../types/generation").RecipeField[];
  resolutionPresets?: { id: string; label: string; width: number; height: number }[];
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
  generatorBindings?: GeneratorBindingSummary[];
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
  detail?: { sources: RunShotContext[]; inputs: RunInput[] };
}
export interface RunRetryRequest { ref: RunRef; selectedItemIds: string[] }

export interface CreationShotSummary { id: string; name: string; ordinal: number }
export interface CreationShot {
  summary: CreationShotSummary;
  prompt: string;
  selectionRef: string | null;
  values: Record<string, import("../types/generation").DraftValue>;
  referenceAssetIds: string[];
  selectedResultId: string | null;
  recentRun: RunRef | null;
}
export interface CreationAsset { id: string; name: string; mediaKind: "image" | "video" | "audio"; selected: boolean; thumbnailBytes?: number[] | null }
export interface CreationContext {
  projectId: string;
  projectName: string;
  stage: "image" | "video";
  shots: CreationShotSummary[];
  selectedShot: CreationShot | null;
  candidates: CreationAsset[];
  mediaInputs: CreationAsset[];
  promptChoices: CreationPromptChoice[];
}
export interface CreationPromptChoice { promptId: string; promptVersionId: string; name: string; version: number; text: string }
export interface CreationPromptProvenance { promptId: string; promptVersionId: string }
export interface CreationShotUpdate { shotId: string; name: string }
export interface CreationSubmission {
  promptId?: string;
  promptVersionId?: string;
  projectId: string;
  shotId: string;
  stage: "image" | "video";
  selectionRef: string;
  values: Record<string, import("../types/generation").DraftValue>;
  submissionIdempotencyKey: string;
}
export interface CreationReadiness {
  ready: boolean;
  issues: { code: import("./errors").ProductErrorCode; message: string; details: import("./errors").ProductErrorDetails }[];
  fieldErrors: CreationReadiness["issues"];
  actions: string[];
}
export interface CreationAccepted {
  accepted: boolean;
  runRef: RunRef;
  startOutcome: "STARTED" | "FAILED_TO_START" | "ALREADY_ACCEPTED";
  startIssue: CreationReadiness["issues"][number] | null;
}

export type RunListFilter = "all" | "active" | "failed" | "completed";
export interface RunList { items: ProductRun[]; nextCursor: string | null; coverage: string }
export interface RunShotContext { id: string; name: string; stage: string }
export interface RunInput { taskId: string | null; itemId?: string | null; generatorName: string; selectionRef: string | null; values: Record<string, import("../types/generation").DraftValue>; reuseUnavailableReason: string | null; errorMessage: string | null }
export interface RunResult { assetId: string; name: string; mediaKind: string | null; assetExists: boolean; availability: string | null; reviewState: string | null; reviewRevision: number | null; selectedShotIds: string[]; thumbnailBytes: number[] | null }
export interface RunResultReviewRequest { runRef: RunRef; assetId: string; decision: "APPROVED" | "REJECTED"; comment: string; expectedRevision: number }
