export type ProjectWorkflowStage = "IMAGE" | "VIDEO";

export type ProjectWorkflowMode =
  | "DEFAULT"
  | "FL2VA_TEXT_TO_VIDEO"
  | "FL2VA_IMAGE_TO_VIDEO"
  | "FL2VA_FIRST_LAST"
  | "REF2VA_IMAGE"
  | "REF2VA_AUDIO"
  | "REF2VA_IMAGE_AUDIO"
  | "REF2VA_VIDEO_IMAGE";

export interface ProjectWorkflowBindingInput {
  stage: ProjectWorkflowStage;
  mode: ProjectWorkflowMode;
  workflowVersionId: string;
  recipeId: string;
}

export interface ProjectWorkflowBindingView extends ProjectWorkflowBindingInput {
  /** Present in native responses; optional for legacy adapters and fixtures. */
  bindingInstanceId?: string;
  revision?: number;
  createdAt: string;
  updatedAt: string;
  available: boolean;
  availabilityReasons?: string[];
}

export interface ProjectWorkflowBindingUpsertRequest extends ProjectWorkflowBindingInput {
  expectedBindingInstanceId?: string | null;
  expectedRevision?: number | null;
}

export interface ProjectWorkflowBindingRemoveRequest {
  stage: ProjectWorkflowStage;
  mode: ProjectWorkflowMode;
  expectedBindingInstanceId: string;
  expectedRevision: number;
}

export interface ProjectWorkflowConfigUpdateRequest {
  bindings: ProjectWorkflowBindingInput[];
}

export interface ProjectWorkflowConfigView {
  projectId: string;
  imageDefault?: ProjectWorkflowBindingView | null;
  videoDefault?: ProjectWorkflowBindingView | null;
  videoModeOverrides: ProjectWorkflowBindingView[];
}
