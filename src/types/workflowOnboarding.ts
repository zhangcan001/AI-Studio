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

export type WorkflowImportFormat = "API" | "UI" | "UNKNOWN" | "INVALID_JSON";

export type WorkflowRecognitionIdentity = "NEW" | "EXACT_RAW" | "EXACT_SEMANTIC" | "STRUCTURAL_VARIANT";
export type WorkflowRecipeStatus = "CURRENT" | "OUTDATED" | "MISSING";
export type WorkflowRuntimeCapability = "READY" | "MISSING_NODES" | "OFFLINE" | "INCOMPATIBLE" | "NOT_CHECKED";
export type WorkflowSourceKind = "PRODUCT" | "USER";
export type WorkflowLibraryState = "ACTIVE" | "REMOVED";
export type WorkflowImportCommitAction = "NEW_WORKFLOW" | "NEW_VERSION" | "NEW_RECIPE" | "RESTORE_EXISTING";

export interface WorkflowImportCommitRequest {
  draftId: string;
  action: WorkflowImportCommitAction;
  workflowId?: string;
  setCurrent?: boolean;
}

export interface WorkflowAnalysisReportView {
  format: WorkflowImportFormat;
  identity: WorkflowRecognitionIdentity;
  existingWorkflowId?: string;
  existingWorkflowVersionId?: string;
  rawSha?: string;
  semanticSha?: string;
  structuralSha?: string;
  category?: string;
  mode?: string;
  inputs?: WorkflowRecognitionInputView[];
  outputs?: WorkflowRecognitionOutputView[];
  confidence?: string;
  issues?: WorkflowRecognitionIssueView[];
  recipeFreshness?: WorkflowRecipeStatus | string;
  runtimeCapability?: WorkflowRuntimeCapability | string;
  suggestedActions?: string[];
}

export interface WorkflowStructuralChangeView {
  field?: string;
  from?: string;
  to?: string;
  message: string;
}

export interface WorkflowRecognitionInputView {
  semanticKey: string;
  fieldType: string;
  label: string;
  required: boolean;
  nodeId: string;
  inputName: string;
  confidence: "HIGH" | "MEDIUM" | "LOW" | string;
}

export interface WorkflowRecognitionOutputView {
  outputId: string;
  type: "image" | "video" | string;
  nodeId: string;
  required: boolean;
  confidence: "HIGH" | "MEDIUM" | "LOW" | string;
}

export interface WorkflowRecognitionIssueView {
  code: string;
  message: string;
}

export interface WorkflowRecognitionReportView {
  format: WorkflowImportFormat;
  recognized?: boolean;
  importable?: boolean;
  executable?: boolean;
  identity: WorkflowRecognitionIdentity;
  rawSha256?: string;
  semanticSha256?: string;
  structuralSha256?: string;
  existingWorkflowId?: string;
  existingWorkflowVersion?: string;
  existingName?: string;
  category?: string;
  mode?: string;
  confidence?: "HIGH" | "MEDIUM" | "LOW" | string;
  inputs?: WorkflowRecognitionInputView[];
  outputs?: WorkflowRecognitionOutputView[];
  recipeStatus?: WorkflowRecipeStatus;
  runtimeCapability?: WorkflowRuntimeCapability;
  capabilityIssues?: string[];
  issues?: WorkflowRecognitionIssueView[];
  suggestedAction?: string;
  nodeCount?: number;
  uniqueClassCount?: number;
  structuralChanges?: WorkflowStructuralChangeView[];
}

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
  recipeVersion?: string;
  recipeId: string;
  packageName: string;
  workflowSha256: string;
  refreshed: WorkflowSyncReport;
  workflowVersionId?: string;
  sourceKind?: WorkflowSourceKind | string;
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

export interface WorkflowAutoExistingRecipeView {
  recipeId: string;
  recipeVersion: string;
  packageName: string;
}

export interface WorkflowAutoOnboardingPlanView {
  draftId: string;
  analysisId?: string;
  analysis?: WorkflowAnalysisReportView;
  commitRequired?: boolean;
  state: WorkflowAutoOnboardingState;
  /** Optional until the native onboarding response exposes format detection. */
  format?: WorkflowImportFormat;
  inputFormat?: WorkflowImportFormat;
  recognition?: WorkflowRecognitionReportView;
  identity?: WorkflowRecognitionIdentity | string;
  recipeStatus?: WorkflowRecipeStatus | string;
  runtimeCapability?: WorkflowRuntimeCapability | string;
  importability?: "RECOGNIZED" | "IMPORTABLE" | "NOT_IMPORTABLE" | string;
  executable?: boolean;
  structuralChanges?: WorkflowStructuralChangeView[];
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
  existingWorkflowName?: string;
  existingWorkflowSource?: string;
  existingWorkflowSourceKind?: WorkflowSourceKind | string;
  existingWorkflowLibraryState?: WorkflowLibraryState | string;
  existingMatchType?: "RAW_SHA" | "SEMANTIC_SHA" | "STRUCTURAL_SHA" | string;
  existingPackageName?: string;
  existingRecipes?: WorkflowAutoExistingRecipeView[];
  expectedInference?: WorkflowAutoInferenceView[];
  suggestedRecipeVersion?: string;
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
  workflowVersionId?: string;
  version: string;
  inputCount: number;
  outputCount: number;
  presetCount?: number;
}

export interface WorkflowRegistryRecipeView {
  recipeId: string;
  workflowVersionId?: string;
  version?: string;
  recipeVersion?: string;
  packageName?: string;
  packageStatus?: string;
  workflowSha256?: string;
  recipeSha256?: string;
  enabled?: boolean;
  archived?: boolean;
  capability?: string;
  readiness?: string;
  inputCount?: number;
  outputCount?: number;
  presetCount?: number;
}

export interface WorkflowRegistryVersionView {
  workflowVersionId: string;
  workflowId?: string;
  version?: string;
  workflowVersion?: string;
  rawSha256?: string;
  workflowSha256?: string;
  packageName?: string;
  packageSourcePath?: string;
  packageStatus?: string;
  enabled?: boolean;
  archived?: boolean;
  archivedAt?: string;
  capability?: string;
  capabilityIssues?: CapabilityIssueView[];
  readiness?: string;
  readinessReasons?: string[];
  nodeCount?: number;
  activeTasks?: number;
  activeQueueItemCount?: number;
  totalTasks?: number;
  hasSuccessfulRun?: boolean;
  latestSuccessAt?: string;
  latestFailureAt?: string;
  liveVerifiedAt?: string;
  diagnostics?: WorkflowDiagnosticView[];
  recipes?: WorkflowRegistryRecipeView[];
}

export interface WorkflowRegistryView {
  workflowId: string;
  name: string;
  sourceKind: WorkflowSourceKind | string;
  libraryState: WorkflowLibraryState | string;
  currentVersionId?: string | null;
  currentVersion?: WorkflowRegistryVersionView | null;
  currentRecipe?: WorkflowRegistryRecipeView | null;
  versions?: WorkflowRegistryVersionView[];
  recipes?: WorkflowRegistryRecipeView[];
  capability?: string;
  capabilityIssues?: CapabilityIssueView[];
  projectUsageCount?: number;
  historyCount?: number;
  activeTaskCount?: number;
  activeQueueItemCount?: number;
  totalTaskCount?: number;
  hasSuccessfulRun?: boolean;
  latestSuccessAt?: string;
  latestFailureAt?: string;
  liveVerifiedAt?: string;
  removedAt?: string | null;
}

export interface WorkflowRegistryResponse {
  items: WorkflowRegistryView[];
  staging?: WorkflowStagingView[];
}

export interface WorkflowRegistryMutationResult {
  workflowId: string;
  libraryState?: WorkflowLibraryState | string;
  sourceKind?: WorkflowSourceKind | string;
  currentVersionId?: string | null;
  workflowVersionId?: string;
  recipeId?: string;
  enabled?: boolean;
  capability?: string;
  readiness?: string;
  projectBindingCount?: number;
  historyCount?: number;
  purged?: boolean;
}

export interface WorkflowRegistryRestoreResult {
  workflowId: string;
  libraryState: WorkflowLibraryState | string;
  currentVersionId: string | null;
  enabled: boolean;
  readiness: string;
  capability: string;
  projectBindingCount: number;
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
  source?: "PRODUCT" | "USER" | string;
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
  projectBindingCount?: number;
  projectBindingScopes?: string[];
  deleteAction?: "REMOVE" | "HARD_DELETE" | "BLOCKED" | string;
  canHardDelete: boolean;
  requiresArchive: boolean;
  blockingReasons: string[];
  sourceKind?: WorkflowSourceKind | string;
  libraryState?: WorkflowLibraryState | string;
  historyCount?: number;
}

export interface WorkflowPurgeInspection {
  workflowId: string;
  name: string;
  sourceKind: WorkflowSourceKind | string;
  libraryState: WorkflowLibraryState | string;
  taskCount: number;
  batchItemCount: number;
  presetCount: number;
  templateCount: number;
  shotConfigCount: number;
  benchmarkCount: number;
  bindingCount: number;
  stageCount: number;
  runTemplateCount: number;
  packageCount: number;
  canPurge: boolean;
  blockingReasons: string[];
}

export interface WorkflowDeletionResult {
  action: "HARD_DELETE" | "REMOVE" | "ARCHIVE";
  deleteAction?: "REMOVE" | "HARD_DELETE" | "BLOCKED" | string;
  projectBindingCount?: number;
  workflowId: string;
  workflowVersionId: string;
  archived: boolean;
}

export interface WorkflowPurgeResult {
  workflowId: string;
  versionCount: number;
  recipeCount: number;
  committed: boolean;
  cleanupPending: boolean;
  warning?: string;
}

export interface WorkflowRestoreResult {
  workflowVersionId: string;
  archived: boolean;
  enabled: boolean;
  capability: string;
  readiness: string;
  workflowId?: string;
  libraryState?: WorkflowLibraryState | string;
  projectBindingCount?: number;
  needsAttention?: boolean;
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
