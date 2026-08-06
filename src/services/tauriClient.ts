import { invoke } from "@tauri-apps/api/core";
import type { AppStatus } from "../types/app";
import type { CapabilitySummary, ComfyStatus } from "../types/comfy";
import type { AssetCategoryFilter, AssetLibraryPage, AssetView, PageCursor } from "../types/asset";
import type { GenerationValues, RecipeViewModel } from "../types/generation";
import type {
  ReusableGenerationDraft,
  TaskDetail,
  TaskHistoryFilter,
  TaskHistoryPage,
} from "../types/history";
import type { TaskView } from "../types/task";
import type { ProjectView } from "../types/project";
import type { PresetView } from "../types/preset";
import type {
  CapabilityCheckView,
  WorkflowOnboardingDraftView,
  WorkflowOnboardingInputMappingRequest,
  WorkflowOnboardingMetadataRequest,
  WorkflowOnboardingOutputMappingRequest,
  WorkflowOnboardingPublishView,
  WorkflowOnboardingRemoveInputMappingRequest,
  WorkflowOnboardingValidationView,
  WorkflowWorkspaceView,
} from "../types/workflowOnboarding";
import { buildAssetMediaUrl } from "./mediaUrl";

export function ping(): Promise<string> {
  return invoke<string>("ping");
}

export function getAppStatus(): Promise<AppStatus> {
  return invoke<AppStatus>("get_app_status");
}

export function getComfyStatus(): Promise<ComfyStatus> {
  return invoke<ComfyStatus>("comfy_get_status");
}

export function refreshComfyCapabilities(): Promise<CapabilitySummary> {
  return invoke<CapabilitySummary>("comfy_refresh_capabilities");
}

export interface WorkflowSyncReport {
  packagesFound: number;
  valid: number;
  invalid: number;
  inserted: number;
  reused: number;
  errors: Array<{ package: string; code: string; message: string }>;
}

export function refreshWorkflowLibrary(): Promise<WorkflowSyncReport> {
  return invoke<WorkflowSyncReport>("workflow_library_refresh");
}

export function pickApiWorkflow(existingWorkflowId?: string): Promise<WorkflowOnboardingDraftView | null> {
  return invoke<WorkflowOnboardingDraftView | null>("workflow_onboarding_pick_api_workflow", {
    existingWorkflowId,
  });
}

export function getOnboardingDraft(draftId: string): Promise<WorkflowOnboardingDraftView> {
  return invoke<WorkflowOnboardingDraftView>("workflow_onboarding_get", { draftId });
}

export function checkOnboardingCapability(draftId: string): Promise<CapabilityCheckView> {
  return invoke<CapabilityCheckView>("workflow_onboarding_check_capability", { draftId });
}

export function setOnboardingMetadata(
  draftId: string,
  request: WorkflowOnboardingMetadataRequest,
): Promise<WorkflowOnboardingDraftView> {
  return invoke<WorkflowOnboardingDraftView>("workflow_onboarding_set_metadata", { draftId, request });
}

export function setOnboardingInputMapping(
  draftId: string,
  request: WorkflowOnboardingInputMappingRequest,
): Promise<WorkflowOnboardingDraftView> {
  return invoke<WorkflowOnboardingDraftView>("workflow_onboarding_set_input_mapping", { draftId, request });
}

export function removeOnboardingInputMapping(
  draftId: string,
  request: WorkflowOnboardingRemoveInputMappingRequest,
): Promise<WorkflowOnboardingDraftView> {
  return invoke<WorkflowOnboardingDraftView>("workflow_onboarding_remove_input_mapping", { draftId, request });
}

export function setOnboardingOutputMapping(
  draftId: string,
  request: WorkflowOnboardingOutputMappingRequest,
): Promise<WorkflowOnboardingDraftView> {
  return invoke<WorkflowOnboardingDraftView>("workflow_onboarding_set_output_mapping", { draftId, request });
}

export function validateOnboarding(draftId: string): Promise<WorkflowOnboardingValidationView> {
  return invoke<WorkflowOnboardingValidationView>("workflow_onboarding_validate", { draftId });
}

export function publishOnboarding(draftId: string): Promise<WorkflowOnboardingPublishView> {
  return invoke<WorkflowOnboardingPublishView>("workflow_onboarding_publish", { draftId });
}

export function discardOnboarding(draftId: string): Promise<void> {
  return invoke<void>("workflow_onboarding_discard", { draftId });
}

export function listWorkflowWorkspace(): Promise<WorkflowWorkspaceView[]> {
  return invoke<WorkflowWorkspaceView[]>("workflow_workspace_list");
}

export function listGenerationCatalog(): Promise<RecipeViewModel[]> {
  return invoke<RecipeViewModel[]>("generation_catalog_list");
}

export function createGeneration(request: {
  projectId: string;
  workflowVersionId: string;
  recipeId: string;
  values: GenerationValues;
}): Promise<TaskView> {
  return invoke<TaskView>("generation_create", { request });
}

export function listProjects(): Promise<ProjectView[]> {
  return invoke<ProjectView[]>("project_list");
}

export function createProject(name: string, description?: string): Promise<ProjectView> {
  return invoke<ProjectView>("project_create", { name, description });
}

export function updateProject(
  projectId: string,
  name: string,
  description?: string,
): Promise<ProjectView> {
  return invoke<ProjectView>("project_update", { projectId, name, description });
}

export function getTask(projectId: string, taskId: string): Promise<TaskView> {
  return invoke<TaskView>("task_get", { projectId, taskId });
}

export function listRecentTasks(projectId: string, limit = 10): Promise<TaskView[]> {
  return invoke<TaskView[]>("task_list_recent", { projectId, limit });
}

export interface RecoveryReport {
  examined: number;
  succeeded: number;
  failed: number;
  deferred: number;
  unresolved: number;
}

export function cancelTask(projectId: string, taskId: string): Promise<TaskView> {
  return invoke<TaskView>("task_cancel", { projectId, taskId });
}

export function reconcileActiveTasks(): Promise<RecoveryReport> {
  return invoke<RecoveryReport>("task_reconcile_active");
}

export function listAssetsByTask(projectId: string, taskId: string): Promise<AssetView[]> {
  return invoke<AssetView[]>("asset_list_by_task", { projectId, taskId });
}

export function listRecentAssets(projectId: string, limit = 100): Promise<AssetView[]> {
  return invoke<AssetView[]>("asset_list_recent", { projectId, limit });
}

export function pickAndImportImage(projectId: string): Promise<AssetView | null> {
  return invoke<AssetView | null>("asset_pick_and_import_image", { projectId });
}

export function pickAndImportVideo(projectId: string): Promise<AssetView | null> {
  return invoke<AssetView | null>("asset_pick_and_import_video", { projectId });
}

export function pickAndImportAudio(projectId: string): Promise<AssetView | null> {
  return invoke<AssetView | null>("asset_pick_and_import_audio", { projectId });
}

export function readAssetImage(projectId: string, assetId: string): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("asset_read_image", { projectId, assetId });
}

export function readAssetThumbnail(projectId: string, assetId: string): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("asset_read_thumbnail", { projectId, assetId });
}

export function getAssetMediaUrl(
  projectId: string,
  assetId: string,
  mediaKind: "video" | "audio" = "video",
): string {
  return buildAssetMediaUrl(projectId, assetId, mediaKind);
}

export function getAsset(projectId: string, assetId: string): Promise<AssetView> {
  return invoke<AssetView>("asset_get", { projectId, assetId });
}

export function taskHistoryPage(
  projectId: string,
  filter: TaskHistoryFilter,
  cursor?: PageCursor,
  limit = 30,
): Promise<TaskHistoryPage> {
  return invoke<TaskHistoryPage>("task_history_page", { projectId, filter, cursor, limit });
}

export function getTaskDetail(projectId: string, taskId: string): Promise<TaskDetail> {
  return invoke<TaskDetail>("task_get_detail", { projectId, taskId });
}

export function getReusableDraft(projectId: string, taskId: string): Promise<ReusableGenerationDraft> {
  return invoke<ReusableGenerationDraft>("task_get_reusable_draft", { projectId, taskId });
}

export function assetLibraryPage(
  projectId: string,
  category: AssetCategoryFilter,
  cursor?: PageCursor,
  limit = 30,
): Promise<AssetLibraryPage> {
  return invoke<AssetLibraryPage>("asset_library_page", { projectId, category, cursor, limit });
}

export function listPresets(
  projectId: string,
  workflowVersionId: string,
  recipeId: string,
): Promise<PresetView[]> {
  return invoke<PresetView[]>("preset_list", { projectId, workflowVersionId, recipeId });
}

export function createPreset(request: {
  projectId: string;
  workflowVersionId: string;
  recipeId: string;
  name: string;
  values: GenerationValues;
}): Promise<PresetView> {
  return invoke<PresetView>("preset_create", { request });
}

export function updatePreset(request: {
  projectId: string;
  presetId: string;
  name: string;
  values: GenerationValues;
}): Promise<PresetView> {
  return invoke<PresetView>("preset_update", { request });
}

export function deletePreset(projectId: string, presetId: string): Promise<void> {
  return invoke<void>("preset_delete", { projectId, presetId });
}
