import type { WorkflowSyncReport } from "../services/tauriClient";

export type CapabilityState =
  | "NOT_CHECKED"
  | "READY"
  | "MISSING_NODES"
  | "INCOMPATIBLE_INPUT_VALUES"
  | "COMFY_OFFLINE";

export type WorkflowAutoOnboardingState =
  | "AUTO_PUBLISHED"
  | "NEEDS_REVIEW"
  | "WAITING_FOR_COMFY_UI"
  | "ALREADY_EXISTS"
  | "ALREADY_EXISTS_ARCHIVED"
  | "BLOCKED";

export type InferenceConfidence = "CERTAIN" | "HIGH" | "AMBIGUOUS" | "UNKNOWN";

export type WorkflowFieldType =
  | "textarea"
  | "integer"
  | "number"
  | "seed"
  | "image"
  | "images"
  | "video"
  | "videos"
  | "audio"
  | "audios";

export interface WorkflowInputView {
  name: string;
  kind: string;
  currentValueSummary: string;
  isLinked: boolean;
  bindable: boolean;
  suggestedType?: string;
  suggestedSemanticKey?: string;
  numericMin?: string;
  numericMax?: string;
  numericStep?: string;
  allowedOptions: string[];
}

export interface WorkflowNodeView {
  nodeId: string;
  classType: string;
  title: string;
  isOutputNode: boolean;
  inputs: WorkflowInputView[];
}

export interface CapabilityIssueView {
  code: string;
  classType?: string;
  nodeId?: string;
  affectedNodeIds: string[];
  inputName?: string;
  currentValue?: string;
  message: string;
}

export interface CapabilityCheckView {
  state: CapabilityState;
  checkedAt?: string;
  issues: CapabilityIssueView[];
}

export interface WorkflowInputMappingView {
  semanticKey: string;
  fieldType: WorkflowFieldType;
  label: string;
  required: boolean;
  defaultValue?: string;
  minValue?: string;
  maxValue?: string;
  step?: string;
  minItems?: number;
  maxItems?: number;
  targetNode: string;
  targetInput: string;
  itemIndex?: number;
}

export interface WorkflowOutputMappingView {
  outputId: string;
  label: string;
  type: "image" | "video";
  nodeId: string;
  required: boolean;
}

export interface WorkflowManifestView {
  workflowId: string;
  name: string;
  workflowVersion: string;
  recipeVersion: string;
  category: string;
  mode: string;
  recipeId: string;
}

export interface RecipeInputView {
  key: string;
  fieldType: WorkflowFieldType;
  label: string;
  required: boolean;
  defaultValue?: string;
  minValue?: string;
  maxValue?: string;
  step?: string;
  minItems?: number;
  maxItems?: number;
}

export interface RecipeBindingView {
  semanticKey: string;
  targetNode: string;
  targetInput: string;
  itemIndex?: number;
}

export interface RecipeDraftView {
  inputs: RecipeInputView[];
  bindings: RecipeBindingView[];
  outputs: WorkflowOutputMappingView[];
  yaml?: string;
  valid: boolean;
  issues: string[];
}

export interface WorkflowOnboardingValidationView {
  apiFormat: boolean;
  recipe: boolean;
  bindings: boolean;
  outputs: boolean;
  manifest: boolean;
  capability: boolean;
  dryRun: boolean;
  readyToPublish: boolean;
  issues: string[];
}

export interface WorkflowOnboardingDraftView {
  draftId: string;
  workflowSha256: string;
  originalFilename: string;
  nodeCount: number;
  uniqueClassCount: number;
  nodes: WorkflowNodeView[];
  capability: CapabilityCheckView;
  inputMappings: WorkflowInputMappingView[];
  outputMappings: WorkflowOutputMappingView[];
  manifest: WorkflowManifestView;
  recipe: RecipeDraftView;
  validation: WorkflowOnboardingValidationView;
}

export interface WorkflowOnboardingPublishView {
  workflowId: string;
  workflowVersion: string;
  recipeId: string;
  packageName: string;
  workflowSha256: string;
  refreshed: WorkflowSyncReport;
}

export interface WorkflowAutoInferenceView {
  field: string;
  value?: string;
  confidence: InferenceConfidence;
  source: string;
  alternatives: string[];
  nodeId?: string;
  inputName?: string;
}

export interface WorkflowAutoIssueCandidateView {
  label: string;
  nodeId?: string;
  inputName?: string;
  outputId?: string;
  outputType?: "image" | "video" | string;
  fieldType?: WorkflowFieldType | string;
}

export interface WorkflowAutoIssueView {
  code: string;
  message: string;
  field?: string;
  candidates: WorkflowAutoIssueCandidateView[];
}

export interface WorkflowAutoOnboardingPlanView {
  draftId: string;
  state: WorkflowAutoOnboardingState;
  workflowKind: string;
  workflowSha256: string;
  originalFilename: string;
  nodeCount: number;
  uniqueClassCount: number;
  metadata: WorkflowManifestView;
  capability: CapabilityCheckView;
  inputMappings: WorkflowInputMappingView[];
  outputMappings: WorkflowOutputMappingView[];
  validation: WorkflowOnboardingValidationView;
  inferences: WorkflowAutoInferenceView[];
  issues: WorkflowAutoIssueView[];
  autoPublishable: boolean;
  published?: WorkflowOnboardingPublishView;
  existingWorkflowId?: string;
  existingWorkflowVersion?: string;
  existingPackageName?: string;
  message: string;
}

export interface WorkflowWorkspaceView {
  workflowId: string;
  name: string;
  workflowVersion: string;
  mode: string;
  packageName: string;
  packageStatus: string;
  workflowSha256: string;
  nodeCount: number;
  uniqueClassCount: number;
  capability: CapabilityState;
  capabilityIssues: CapabilityIssueView[];
  inputMappings: WorkflowInputMappingView[];
  outputs: WorkflowOutputMappingView[];
  hasSuccessfulRun: boolean;
}

export interface WorkflowRecipeSummaryView {
  recipeId: string;
  version: string;
  inputCount: number;
  outputCount: number;
  presetCount?: number;
}

export interface WorkflowDiagnosticView {
  code: string;
  message: string;
}

export interface WorkflowStagingView {
  stagingId: string;
  status: string;
  inUse: boolean;
}

export interface WorkflowProductionWorkspaceView {
  packageName: string;
  builtin: boolean;
  archived: boolean;
  archivedAt?: string;
  packageStatus: string;
  errorCode?: string;
  errorMessage?: string;
  workflowId?: string;
  workflowVersionId?: string;
  name?: string;
  category?: string;
  mode?: string;
  workflowVersion?: string;
  workflowSha256?: string;
  recipeSha256?: string;
  enabled: boolean;
  capability: string;
  readiness: "READY" | "DEGRADED" | "BLOCKED" | string;
  readinessReasons: string[];
  capabilityIssues: CapabilityIssueView[];
  nodeCount: number;
  recipes: WorkflowRecipeSummaryView[];
  activeTasks: number;
  totalTasks: number;
  hasSuccessfulRun: boolean;
  latestSuccessAt?: string;
  latestFailureAt?: string;
  liveVerifiedAt?: string;
  diagnostics: WorkflowDiagnosticView[];
}

export interface WorkflowProductionWorkspaceResponse {
  items: WorkflowProductionWorkspaceView[];
  staging: WorkflowStagingView[];
}

export interface WorkflowDeletionInspection {
  workflowId: string;
  workflowVersionId: string;
  name: string;
  builtin: boolean;
  enabled: boolean;
  archived: boolean;
  archivedAt?: string;
  activeTaskCount: number;
  activeQueueItemCount: number;
  historicalTaskCount: number;
  productionBatchItemCount: number;
  benchmarkReferenceCount: number;
  canHardDelete: boolean;
  requiresArchive: boolean;
  blockingReasons: string[];
}

export interface WorkflowDeletionResult {
  action: "HARD_DELETE" | "ARCHIVE";
  workflowId: string;
  workflowVersionId: string;
  archived: boolean;
}

export interface WorkflowCapabilityBatchView {
  workflowVersionId: string;
  capability: CapabilityCheckView;
}

export interface WorkflowRestoreView {
  status: string;
  packageName: string;
  workflowId: string;
  workflowVersion: string;
  recipeId?: string;
  enabled: boolean;
  capability: string;
}

export interface WorkflowVersionDiffView {
  workflowId: string;
  versionA: string;
  versionB: string;
  nodeCountA: number;
  nodeCountB: number;
  addedNodes: string[];
  removedNodes: string[];
  changedClassTypes: Array<{ nodeId: string; from: string; to: string }>;
  changedLiteralInputs: Array<{ nodeId: string; input: string; from: string; to: string }>;
  changedLinks: Array<{ nodeId: string; input: string; from: string; to: string }>;
  recipeInputChanges: string[];
  bindingChanges: string[];
  outputChanges: string[];
}

export interface WorkflowOnboardingMetadataRequest {
  workflowId?: string;
  name: string;
  workflowVersion: string;
  recipeVersion: string;
  category: string;
  mode: string;
}

export interface WorkflowOnboardingInputMappingRequest {
  semanticKey: string;
  fieldType: WorkflowFieldType;
  label: string;
  required: boolean;
  defaultValue?: string;
  minValue?: string;
  maxValue?: string;
  step?: string;
  minItems?: number;
  maxItems?: number;
  targetNode: string;
  targetInput: string;
  itemIndex?: number;
}

export interface WorkflowOnboardingOutputMappingRequest {
  outputId: string;
  label: string;
  type: "image" | "video";
  nodeId: string;
  required: boolean;
}

export interface WorkflowOnboardingRemoveInputMappingRequest {
  semanticKey: string;
  itemIndex?: number;
}
