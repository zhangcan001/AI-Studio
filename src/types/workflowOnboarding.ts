import type { WorkflowSyncReport } from "../services/tauriClient";

export type CapabilityState =
  | "NOT_CHECKED"
  | "READY"
  | "MISSING_NODES"
  | "INCOMPATIBLE_INPUT_VALUES"
  | "COMFY_OFFLINE";

export type WorkflowFieldType =
  | "textarea"
  | "integer"
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
  numericMin?: string;
  numericMax?: string;
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
