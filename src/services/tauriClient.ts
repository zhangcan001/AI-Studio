import { invoke } from "@tauri-apps/api/core";
import type { AppStatus } from "../types/app";
import type { CapabilitySummary, ComfyMemoryReleaseResult, ComfyStatus } from "../types/comfy";
import type {
  DiagnosticsExport,
  DiagnosticsSummary,
  RuntimeActivityStatus,
} from "../types/diagnostics";
import type {
  AssetDeleteInspection,
  AssetDeleteResult,
  AssetLibraryPage,
  AssetLibraryQuery,
  AssetView,
} from "../types/asset";
import type { AssetVideoPromptView } from "../types/assetVideoPrompt";
import type {
  H3LocalImportInspection,
  H3LocalImportMode,
  H3LocalImportResult,
} from "../types/h3LocalImport";
import type { AssetTag, ProjectTemplate, TemplateProjectResult } from "../types/organization";
import type { GenerationValues, RecipeViewModel } from "../types/generation";
import type { ShotBatchPlan, ShotInputValues, ShotStage, ShotView } from "../types/shot";
import type {
  ReusableGenerationDraft,
  TaskDetail,
  TaskHistoryPage,
  TaskHistoryQuery,
} from "../types/history";
import type { TaskView } from "../types/task";
import type {
  ProjectBackupExportView,
  ProjectBackupPreview,
  ProjectView,
} from "../types/project";
import type {
  ComfyEndpointTest,
  ComfySettingsView,
  RuntimeParameterProfile,
} from "../types/settings";
import type { PresetView } from "../types/preset";
import type {
  PromptEntryView,
  PromptKind,
  PromptLibraryCreateRequest,
  PromptLibraryMetadataRequest,
  PromptLibraryPage,
  PromptVersionView,
} from "../types/prompt";
import type { PageCursor } from "../types/asset";
import type {
  ProductionBatchCreateItem,
  ProductionBatchDetail,
  ProductionBatchSummary,
  ProductionAdmissionStatus,
  ProductionQueueOverview,
} from "../types/productionQueue";
import type {
  CapabilityCheckView,
  WorkflowOnboardingDraftView,
  WorkflowOnboardingInputMappingRequest,
  WorkflowOnboardingMetadataRequest,
  WorkflowOnboardingOutputMappingRequest,
  WorkflowOnboardingPublishView,
  WorkflowOnboardingRemoveInputMappingRequest,
  WorkflowOnboardingValidationView,
  WorkflowProductionWorkspaceResponse,
  WorkflowRestoreView,
  WorkflowVersionDiffView,
  WorkflowWorkspaceView,
} from "../types/workflowOnboarding";
import { buildAssetMediaUrl } from "./mediaUrl";

export function ping(): Promise<string> {
  return invoke<string>("ping");
}

export function getAppStatus(): Promise<AppStatus> {
  return invoke<AppStatus>("get_app_status");
}

export function listPromptLibrary(
  projectId: string,
  filters: { kind?: PromptKind; keyword?: string; tag?: string; cursor?: PageCursor; limit?: number } = {},
): Promise<PromptLibraryPage> {
  return invoke<PromptLibraryPage>("prompt_library_list", {
    projectId,
    kind: filters.kind,
    keyword: filters.keyword,
    tag: filters.tag,
    cursor: filters.cursor,
    limit: filters.limit,
  });
}

export function getPromptLibraryEntry(projectId: string, promptId: string): Promise<PromptEntryView> {
  return invoke<PromptEntryView>("prompt_library_get", { projectId, promptId });
}

export function createPromptLibraryEntry(request: PromptLibraryCreateRequest): Promise<PromptEntryView> {
  return invoke<PromptEntryView>("prompt_library_create", { request });
}

export function addPromptLibraryVersion(
  projectId: string,
  promptId: string,
  text: string,
): Promise<PromptVersionView> {
  return invoke<PromptVersionView>("prompt_library_add_version", {
    request: { projectId, promptId, text },
  });
}

export function updatePromptLibraryMetadata(request: PromptLibraryMetadataRequest): Promise<PromptEntryView> {
  return invoke<PromptEntryView>("prompt_library_update_metadata", { request });
}

export function deletePromptLibraryEntry(projectId: string, promptId: string): Promise<void> {
  return invoke<void>("prompt_library_delete", { projectId, promptId });
}

export function getComfyStatus(): Promise<ComfyStatus> {
  return invoke<ComfyStatus>("comfy_get_status");
}

export function refreshComfyCapabilities(): Promise<CapabilitySummary> {
  return invoke<CapabilitySummary>("comfy_refresh_capabilities");
}

export function getComfySettings(): Promise<ComfySettingsView> {
  return invoke<ComfySettingsView>("comfy_get_settings");
}

export function testComfyConnection(endpoint: string): Promise<ComfyEndpointTest> {
  return invoke<ComfyEndpointTest>("comfy_test_connection", { endpoint });
}

export function saveComfyEndpoint(endpoint: string): Promise<ComfySettingsView> {
  return invoke<ComfySettingsView>("comfy_save_endpoint", { endpoint });
}

export function freeComfyMemory(): Promise<ComfyMemoryReleaseResult> {
  return invoke<ComfyMemoryReleaseResult>("comfy_free_memory");
}

export function getRuntimeActivityStatus(): Promise<RuntimeActivityStatus> {
  return invoke<RuntimeActivityStatus>("runtime_activity_status");
}

export function getDiagnosticsSummary(): Promise<DiagnosticsSummary> {
  return invoke<DiagnosticsSummary>("diagnostics_summary");
}

export function exportDiagnostics(): Promise<DiagnosticsExport | null> {
  return invoke<DiagnosticsExport | null>("diagnostics_export");
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

export function listWorkflowProductionWorkspace(): Promise<WorkflowProductionWorkspaceResponse> {
  return invoke<WorkflowProductionWorkspaceResponse>("workflow_runtime_workspace_list");
}

export function setWorkflowEnabled(workflowVersionId: string, enabled: boolean): Promise<void> {
  return invoke<void>("workflow_set_enabled", { workflowVersionId, enabled });
}

export function recheckWorkflowCapability(workflowVersionId: string): Promise<CapabilityCheckView> {
  return invoke<CapabilityCheckView>("workflow_recheck_capability", { workflowVersionId });
}

export function duplicateWorkflowRecipe(
  workflowVersionId: string,
  recipeId?: string,
  recipeVersion?: string,
): Promise<WorkflowOnboardingDraftView> {
  return invoke<WorkflowOnboardingDraftView>("workflow_duplicate_recipe", {
    workflowVersionId,
    recipeId,
    recipeVersion,
  });
}

export function compareWorkflowVersions(versionAId: string, versionBId: string): Promise<WorkflowVersionDiffView> {
  return invoke<WorkflowVersionDiffView>("workflow_compare_versions", { versionAId, versionBId });
}

export function exportWorkflowPackage(workflowVersionId: string): Promise<{ fileName: string } | null> {
  return invoke<{ fileName: string } | null>("workflow_export_package", { workflowVersionId });
}

export function importWorkflowPackageBackup(): Promise<WorkflowRestoreView | null> {
  return invoke<WorkflowRestoreView | null>("workflow_import_package_backup");
}

export function cleanWorkflowStaging(stagingId: string): Promise<void> {
  return invoke<void>("workflow_clean_staging", { stagingId });
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

export function listShots(projectId: string): Promise<ShotView[]> {
  return invoke<ShotView[]>("shot_list", { projectId });
}

export function getShot(projectId: string, shotId: string): Promise<ShotView> {
  return invoke<ShotView>("shot_get", { projectId, shotId });
}

export function createShot(projectId: string): Promise<ShotView> {
  return invoke<ShotView>("shot_create", { projectId });
}

export function updateShot(request: {
  projectId: string;
  shotId: string;
  name: string;
  promptText: string;
  promptEntryId?: string;
  promptVersionId?: string;
}): Promise<ShotView> {
  return invoke<ShotView>("shot_update", { request });
}

export function deleteShot(projectId: string, shotId: string): Promise<void> {
  return invoke<void>("shot_delete", { projectId, shotId });
}

export function reorderShots(projectId: string, orderedIds: string[]): Promise<ShotView[]> {
  return invoke<ShotView[]>("shot_reorder", { request: { projectId, orderedIds } });
}

export function setShotStageConfig(request: {
  projectId: string;
  shotId: string;
  stage: ShotStage;
  workflowVersionId: string;
  recipeId: string;
  values: ShotInputValues;
}): Promise<ShotView> {
  return invoke<ShotView>("shot_stage_config_set", { request });
}

export function replaceShotReferences(request: {
  projectId: string;
  shotId: string;
  stage: ShotStage;
  assetIds: string[];
}): Promise<ShotView> {
  return invoke<ShotView>("shot_references_replace", { request });
}

export function selectShotResult(request: {
  projectId: string;
  shotId: string;
  stage: ShotStage;
  assetId: string;
  fromLinkedTask?: boolean;
}): Promise<ShotView> {
  return invoke<ShotView>("shot_result_select", { request });
}

export function generateShot(request: {
  projectId: string;
  shotId: string;
  stage: ShotStage;
  values?: ShotInputValues;
  productionBatchItemId?: string;
}): Promise<TaskView> {
  return invoke<TaskView>("shot_generate", { request });
}

export function planShotBatch(projectId: string, stage: ShotStage): Promise<ShotBatchPlan> {
  return invoke<ShotBatchPlan>("shot_batch_plan", { projectId, stage });
}

export function createShotBatch(request: {
  projectId: string;
  stage: ShotStage;
  shotIds: string[];
}): Promise<ProductionBatchDetail> {
  return invoke<ProductionBatchDetail>("shot_batch_create", { request });
}

export interface GenerationBatchItemRequest {
  workflowVersionId: string;
  recipeId: string;
  values: GenerationValues;
}

export interface GenerationBatchCreateResult {
  created: Array<{ index: number; task: TaskView }>;
  failed: Array<{ index: number; code: string; message: string }>;
}

export function createGenerationBatch(request: {
  projectId: string;
  items: GenerationBatchItemRequest[];
}): Promise<GenerationBatchCreateResult> {
  return invoke<GenerationBatchCreateResult>("generation_create_batch", { request });
}

export function createProductionQueue(request: {
  projectId: string;
  name: string;
  continueOnFailure: boolean;
  items: ProductionBatchCreateItem[];
}): Promise<ProductionBatchDetail> {
  return invoke<ProductionBatchDetail>("production_queue_create", { request });
}

export function listProductionQueues(projectId: string): Promise<ProductionBatchSummary[]> {
  return invoke<ProductionBatchSummary[]>("production_queue_list", { projectId });
}

export function getProductionQueueOverview(projectId: string): Promise<ProductionQueueOverview> {
  return invoke<ProductionQueueOverview>("production_queue_overview", { projectId });
}

export function getProductionAdmissionStatus(): Promise<ProductionAdmissionStatus> {
  return invoke<ProductionAdmissionStatus>("production_queue_admission_status");
}

export function getProductionQueue(projectId: string, batchId: string): Promise<ProductionBatchDetail> {
  return invoke<ProductionBatchDetail>("production_queue_get", { projectId, batchId });
}

export function startProductionQueue(projectId: string, batchId: string): Promise<ProductionBatchDetail> {
  return invoke<ProductionBatchDetail>("production_queue_start", { projectId, batchId });
}

export function pauseProductionQueue(projectId: string, batchId: string): Promise<ProductionBatchDetail> {
  return invoke<ProductionBatchDetail>("production_queue_pause", { projectId, batchId });
}

export function cancelPendingProductionQueue(projectId: string, batchId: string): Promise<ProductionBatchDetail> {
  return invoke<ProductionBatchDetail>("production_queue_cancel_pending", { projectId, batchId });
}

export function archiveProductionQueue(projectId: string, batchId: string): Promise<ProductionBatchDetail> {
  return invoke<ProductionBatchDetail>("production_queue_archive", { projectId, batchId });
}

export function restoreProductionQueue(projectId: string, batchId: string): Promise<ProductionBatchDetail> {
  return invoke<ProductionBatchDetail>("production_queue_restore", { projectId, batchId });
}

export function deleteProductionQueue(projectId: string, batchId: string): Promise<void> {
  return invoke<void>("production_queue_delete", { projectId, batchId });
}

export function skipProductionQueueItem(
  projectId: string,
  batchId: string,
  itemId: string,
): Promise<ProductionBatchDetail> {
  return invoke<ProductionBatchDetail>("production_queue_skip_item", { projectId, batchId, itemId });
}

export function requeueProductionQueueItem(
  projectId: string,
  batchId: string,
  itemId: string,
): Promise<ProductionBatchDetail> {
  return invoke<ProductionBatchDetail>("production_queue_requeue_item", { projectId, batchId, itemId });
}

export function requeueProductionQueueItemByItem(projectId: string, itemId: string): Promise<ProductionBatchDetail> {
  return invoke<ProductionBatchDetail>("production_queue_requeue_item_by_item", { projectId, itemId });
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

export function exportProjectBackup(projectId: string): Promise<ProjectBackupExportView | null> {
  return invoke<ProjectBackupExportView | null>("project_backup_export", { projectId });
}

export function inspectProjectBackup(): Promise<ProjectBackupPreview | null> {
  return invoke<ProjectBackupPreview | null>("project_backup_inspect");
}

export function restoreProjectBackup(inspectionId: string): Promise<ProjectView> {
  return invoke<ProjectView>("project_backup_restore", { inspectionId });
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

export function inspectAssetDeletion(projectId: string, assetIds: string[]): Promise<AssetDeleteInspection> {
  return invoke<AssetDeleteInspection>("inspect_asset_deletion", { projectId, assetIds });
}

export function deleteAssets(projectId: string, assetIds: string[]): Promise<AssetDeleteResult> {
  return invoke<AssetDeleteResult>("delete_assets", { projectId, assetIds });
}

export function getAssetVideoPrompt(projectId: string, assetId: string): Promise<AssetVideoPromptView | null> {
  return invoke<AssetVideoPromptView | null>("asset_video_prompt_get", { projectId, assetId });
}

export function listAssetVideoPrompts(projectId: string, assetIds: string[]): Promise<AssetVideoPromptView[]> {
  return invoke<AssetVideoPromptView[]>("asset_video_prompt_list", { projectId, assetIds });
}

export function setAssetVideoPrompt(
  projectId: string,
  assetId: string,
  promptText: string,
): Promise<AssetVideoPromptView> {
  return invoke<AssetVideoPromptView>("asset_video_prompt_set", {
    request: { projectId, assetId, promptText },
  });
}

export function pickH3LocalImportDirectory(
  projectId: string,
  mode: H3LocalImportMode,
): Promise<H3LocalImportInspection | null> {
  return invoke<H3LocalImportInspection | null>("h3_local_import_pick_directory", { projectId, mode });
}

export function rescanH3LocalImport(
  sessionId: string,
  mode: H3LocalImportMode,
): Promise<H3LocalImportInspection> {
  return invoke<H3LocalImportInspection>("h3_local_import_rescan", { sessionId, mode });
}

export function commitH3LocalImport(request: {
  sessionId: string;
  batchName?: string;
  workflowVersionId: string;
  recipeId: string;
  width: number;
  height: number;
  durationSeconds: number;
  seed?: string;
  autoStart: boolean;
}): Promise<H3LocalImportResult> {
  return invoke<H3LocalImportResult>("h3_local_import_commit", { request });
}

export function taskHistoryPage(
  query: TaskHistoryQuery,
): Promise<TaskHistoryPage> {
  return invoke<TaskHistoryPage>("task_history_page", { query });
}

export function getTaskDetail(projectId: string, taskId: string): Promise<TaskDetail> {
  return invoke<TaskDetail>("task_get_detail", { projectId, taskId });
}

export function getReusableDraft(projectId: string, taskId: string): Promise<ReusableGenerationDraft> {
  return invoke<ReusableGenerationDraft>("task_get_reusable_draft", { projectId, taskId });
}

export function assetLibraryPage(query: AssetLibraryQuery): Promise<AssetLibraryPage> {
  return invoke<AssetLibraryPage>("asset_library_page", { query });
}

export function listAssetTags(projectId: string): Promise<AssetTag[]> {
  return invoke<AssetTag[]>("asset_tag_list", { projectId });
}

export function createAssetTag(projectId: string, name: string): Promise<AssetTag> {
  return invoke<AssetTag>("asset_tag_create", { projectId, name });
}

export function renameAssetTag(projectId: string, tagId: string, name: string): Promise<AssetTag> {
  return invoke<AssetTag>("asset_tag_rename", { projectId, tagId, name });
}

export function deleteAssetTag(projectId: string, tagId: string): Promise<void> {
  return invoke<void>("asset_tag_delete", { projectId, tagId });
}

export function assignAssetTag(projectId: string, assetId: string, tagId: string): Promise<void> {
  return invoke<void>("asset_tag_assign", { projectId, assetId, tagId });
}

export function removeAssetTag(projectId: string, assetId: string, tagId: string): Promise<void> {
  return invoke<void>("asset_tag_remove", { projectId, assetId, tagId });
}

export function setAssetFavorite(projectId: string, assetId: string, favorite: boolean): Promise<void> {
  return invoke<void>("asset_set_favorite", { projectId, assetId, favorite });
}

export function bulkSetAssetFavorite(projectId: string, assetIds: string[], favorite: boolean): Promise<void> {
  return invoke<void>("asset_bulk_set_favorite", { projectId, assetIds, favorite });
}

export function bulkAddAssetTag(projectId: string, assetIds: string[], tagId: string): Promise<void> {
  return invoke<void>("asset_bulk_add_tag", { projectId, assetIds, tagId });
}

export function bulkRemoveAssetTag(projectId: string, assetIds: string[], tagId: string): Promise<void> {
  return invoke<void>("asset_bulk_remove_tag", { projectId, assetIds, tagId });
}

export function listProjectTemplates(): Promise<ProjectTemplate[]> {
  return invoke<ProjectTemplate[]>("project_template_list");
}

export function createProjectTemplate(request: {
  name: string; description?: string; workflowVersionId: string; recipeId: string; values: GenerationValues;
}): Promise<ProjectTemplate> {
  return invoke<ProjectTemplate>("project_template_create", { request });
}

export function updateProjectTemplate(templateId: string, name: string, description?: string): Promise<ProjectTemplate> {
  return invoke<ProjectTemplate>("project_template_update", { templateId, name, description });
}

export function deleteProjectTemplate(templateId: string): Promise<void> {
  return invoke<void>("project_template_delete", { templateId });
}

export function createProjectFromTemplate(templateId: string, name: string, description?: string): Promise<TemplateProjectResult> {
  return invoke<TemplateProjectResult>("project_template_create_project", { templateId, name, description });
}

export function listPresets(
  projectId: string,
  workflowVersionId: string,
  recipeId: string,
): Promise<PresetView[]> {
  return invoke<PresetView[]>("preset_list", { projectId, workflowVersionId, recipeId });
}

export function getPreferredPreset(
  projectId: string,
  workflowVersionId: string,
  recipeId: string,
): Promise<string | null> {
  return invoke<string | null>("preset_get_preferred", { projectId, workflowVersionId, recipeId });
}

export function setPreferredPreset(request: {
  projectId: string;
  workflowVersionId: string;
  recipeId: string;
  presetId?: string;
}): Promise<void> {
  return invoke<void>("preset_set_preferred", { request });
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

export function listRuntimeProfiles(): Promise<RuntimeParameterProfile[]> {
  return invoke<RuntimeParameterProfile[]>("runtime_profiles_list");
}

export function saveRuntimeProfile(profile: RuntimeParameterProfile): Promise<RuntimeParameterProfile> {
  return invoke<RuntimeParameterProfile>("runtime_profiles_save", { profile });
}

export function deleteRuntimeProfile(profileId: string): Promise<void> {
  return invoke<void>("runtime_profiles_delete", { profileId });
}

export function listProductionQueueNamePresets(): Promise<string[]> {
  return invoke<string[]>("production_queue_name_presets_list");
}

export function saveProductionQueueNamePreset(name: string): Promise<string[]> {
  return invoke<string[]>("production_queue_name_preset_save", { name });
}

export function deleteProductionQueueNamePreset(name: string): Promise<void> {
  return invoke<void>("production_queue_name_preset_delete", { name });
}
