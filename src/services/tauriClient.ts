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
  AssetSourceImportBatch,
  AssetView,
} from "../types/asset";
import type { AssetVideoPromptView } from "../types/assetVideoPrompt";
import type {
  H3LocalImportInspection,
  H3LocalImportMode,
  H3LocalImportResult,
  H3ProjectSegmentDraft,
} from "../types/h3LocalImport";
import type { AssetTag, ProjectTemplate, TemplateProjectResult } from "../types/organization";
import type { GenerationValues, RecipeViewModel } from "../types/generation";
import type {
  WorkflowBenchmarkCreateRequest,
  WorkflowBenchmarkCandidatePreview,
  WorkflowBenchmarkQuality,
  WorkflowBenchmarkSummary,
  WorkflowBenchmarkView,
} from "../types/benchmark";
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
import type { ProjectManifestExportView } from "../types/projectManifest";
import type {
  PromptTemplateAnalysis,
  PromptTemplateApplyRequest,
  PromptTemplateApplyResult,
  PromptTemplateBulkPreview,
  PromptTemplateBulkPreviewRequest,
  PromptTemplatePreview,
  PromptTemplatePreviewRequest,
} from "../types/promptTemplate";
import type {
  ProductionAssignShotsRequest,
  ProductionEpisode,
  ProductionEpisodeRequest,
  ProductionReorderRequest,
  ProductionScene,
  ProductionSceneRequest,
  ProductionSceneShotReorderRequest,
  ProductionSeries,
  ProductionSeriesRequest,
  ProductionStructureTree,
  ProductionUnassignShotsRequest,
} from "../types/productionStructure";
import type {
  ComfyEndpointTest,
  ComfyEnvironmentProfile,
  ComfyPreflightReport,
  ComfySettingsView,
  RuntimeParameterProfile,
} from "../types/settings";
import type { WorkspaceResume } from "../types/workspaceResume";
import type { ProjectCommandCenterAggregate } from "../types/projectCommandCenter";
import type { ReferenceAnchorRequest, ReferenceAnchorUpdateRequest, ReferenceAnchorView } from "../types/referenceAnchor";
import type {
  AssetUsageSummary,
  CharacterProfileRequest,
  ConsistencyProfileView,
  CostumeVariantRequest,
  CostumeVariantUpdateRequest,
  CostumeVariantView,
  ProfileType,
  ProfileUsageSummary,
  PropProfileRequest,
  PropProfileUpdateRequest,
  ReferenceSetDetailView,
  ReferenceSetPurpose,
  ReferenceSetRequest,
  ReferenceSetSummary,
  ReferenceSetUpdateRequest,
  ReferenceSetView,
  ReferenceSetUsageSummary,
  SceneProfileRequest,
  SceneProfileUpdateRequest,
  StyleProfileRequest,
  StyleProfileUpdateRequest,
} from "../types/consistency";
import type { PresetView } from "../types/preset";
import type {
  BatchWorkflowPreset,
  BatchWorkflowPresetCreateRequest,
  BatchWorkflowPresetUpdateRequest,
  SceneProductionPlan,
  SceneProductionPlanRequest,
  SceneProductionPrepareRequest,
  SceneProductionPrepareResult,
} from "../types/sceneProduction";
import type {
  EpisodeProductionPlan,
  EpisodeProductionPlanRequest,
  EpisodeProductionPrepareRequest,
  EpisodeProductionPrepareResult,
} from "../types/episodeProduction";
import type {
  SeriesProductionPlan,
  SeriesProductionPlanRequest,
  SeriesProductionPrepareRequest,
  SeriesProductionPrepareResult,
} from "../types/seriesProduction";
import type {
  ProductionBatchRunbookRequest,
  ProductionBatchRunbookView,
} from "../types/productionBatchRunbook";
import type {
  ProductionRun,
  ProductionRunCreateRequest,
  ProductionRunListItem,
  ProductionRunTemplate,
} from "../types/productionRun";
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
  ProductionPartialResumePlan,
  ProductionPartialResumeResult,
  ProductionQueueOverview,
} from "../types/productionQueue";
import type {
  ProductionBatchReview,
  ProductionReviewRegenerateResult,
  ProductionReviewStatus,
} from "../types/productionItemReview";
import type {
  ProductionAuditIntegrity,
  ProductionAuditLineage,
  ProductionAuditLineageRequest,
  ProductionAuditActivity,
  ProductionAuditSummary,
} from "../types/productionAudit";
import type {
  CapabilityCheckView,
  WorkflowAutoOnboardingPlanView,
  WorkflowCapabilityBatchView,
  WorkflowDeletionInspection,
  WorkflowDeletionResult,
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

export function analyzePromptTemplate(text: string): Promise<PromptTemplateAnalysis> {
  return invoke<PromptTemplateAnalysis>("prompt_template_analyze", { text });
}

export function previewPromptTemplate(request: PromptTemplatePreviewRequest): Promise<PromptTemplatePreview> {
  return invoke<PromptTemplatePreview>("prompt_template_preview", { request });
}

export function previewPromptTemplateBulk(request: PromptTemplateBulkPreviewRequest): Promise<PromptTemplateBulkPreview> {
  return invoke<PromptTemplateBulkPreview>("prompt_template_bulk_preview", { request });
}

export function applyPromptTemplate(request: PromptTemplateApplyRequest): Promise<PromptTemplateApplyResult> {
  return invoke<PromptTemplateApplyResult>("prompt_template_apply", { request });
}

export function listBatchWorkflowPresets(): Promise<BatchWorkflowPreset[]> {
  return invoke<BatchWorkflowPreset[]>("batch_workflow_presets_list");
}

export function createBatchWorkflowPreset(request: BatchWorkflowPresetCreateRequest): Promise<BatchWorkflowPreset> {
  return invoke<BatchWorkflowPreset>("batch_workflow_preset_create", { input: request });
}

export function updateBatchWorkflowPreset(request: BatchWorkflowPresetUpdateRequest): Promise<BatchWorkflowPreset> {
  const { presetId, ...input } = request;
  return invoke<BatchWorkflowPreset>("batch_workflow_preset_update", { presetId, input });
}

export function deleteBatchWorkflowPreset(presetId: string): Promise<void> {
  return invoke<void>("batch_workflow_preset_delete", { presetId });
}

export function getSceneProductionPlan(request: SceneProductionPlanRequest): Promise<SceneProductionPlan> {
  return invoke<SceneProductionPlan>("scene_production_plan", {
    projectId: request.projectId,
    sceneId: request.sceneId,
    stage: request.stage,
  });
}

export function prepareSceneProduction(request: SceneProductionPrepareRequest): Promise<SceneProductionPrepareResult> {
  return invoke<{
    projectId: string;
    sceneId: string;
    stage: string;
    created: boolean;
    createdCount: number;
    alreadyPrepared: boolean;
    existingBatchIds: string[];
    detail?: ProductionBatchDetail | null;
  }>("scene_production_prepare", { request }).then((result) => ({
    ...result,
    stage: result.stage as SceneProductionPrepareResult["stage"],
    batchId: result.detail?.id ?? null,
    skippedCount: 0,
  }));
}

export function getEpisodeProductionPlan(request: EpisodeProductionPlanRequest): Promise<EpisodeProductionPlan> {
  return invoke<EpisodeProductionPlan>("episode_production_plan", { request });
}

export function prepareEpisodeProduction(request: EpisodeProductionPrepareRequest): Promise<EpisodeProductionPrepareResult> {
  return invoke<EpisodeProductionPrepareResult>("episode_production_prepare", { request });
}

export function getSeriesProductionPlan(request: SeriesProductionPlanRequest): Promise<SeriesProductionPlan> {
  return invoke<SeriesProductionPlan>("series_production_plan", { request });
}

export function prepareSeriesProduction(request: SeriesProductionPrepareRequest): Promise<SeriesProductionPrepareResult> {
  return invoke<SeriesProductionPrepareResult>("series_production_prepare", { request });
}

export function getProductionBatchRunbook(request: ProductionBatchRunbookRequest): Promise<ProductionBatchRunbookView> {
  return invoke<ProductionBatchRunbookView>("production_batch_runbook", { request });
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

export function listComfyEnvironmentProfiles(): Promise<ComfyEnvironmentProfile[]> {
  return invoke<ComfyEnvironmentProfile[]>("comfy_environment_profiles_list");
}

export function saveComfyEnvironmentProfile(profile: ComfyEnvironmentProfile): Promise<ComfyEnvironmentProfile> {
  return invoke<ComfyEnvironmentProfile>("comfy_environment_profile_save", { profile });
}

export function deleteComfyEnvironmentProfile(profileId: string): Promise<void> {
  return invoke<void>("comfy_environment_profile_delete", { profileId });
}

export function applyComfyEnvironmentProfile(profileId: string): Promise<ComfySettingsView> {
  return invoke<ComfySettingsView>("comfy_environment_profile_apply", { profileId });
}

export function getWorkspaceResume(): Promise<WorkspaceResume> {
  return invoke<WorkspaceResume>("workspace_resume_get");
}

export function saveWorkspaceResume(workspaceResume: WorkspaceResume): Promise<WorkspaceResume> {
  return invoke<WorkspaceResume>("workspace_resume_save", { workspaceResume });
}

export function getComfyPreflight(): Promise<ComfyPreflightReport> {
  return invoke<ComfyPreflightReport>("comfy_preflight_current");
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

export function autoOnboardWorkflow(existingWorkflowId?: string): Promise<WorkflowAutoOnboardingPlanView | null> {
  return invoke<WorkflowAutoOnboardingPlanView | null>("workflow_onboarding_auto_import_api_workflow", {
    existingWorkflowId,
  });
}

export function autoConfirmOnboarding(draftId: string): Promise<WorkflowAutoOnboardingPlanView> {
  return invoke<WorkflowAutoOnboardingPlanView>("workflow_onboarding_auto_confirm", { draftId });
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

export function refreshWorkflowProductionWorkspace(): Promise<WorkflowProductionWorkspaceResponse> {
  return invoke<WorkflowProductionWorkspaceResponse>("workflow_runtime_workspace_refresh");
}

export function repairBuiltinWorkflowPackage(packageName: string): Promise<WorkflowProductionWorkspaceResponse> {
  return invoke<WorkflowProductionWorkspaceResponse>("workflow_repair_builtin_package", { packageName });
}

export function setWorkflowEnabled(workflowVersionId: string, enabled: boolean): Promise<void> {
  return invoke<void>("workflow_set_enabled", { workflowVersionId, enabled });
}

export function recheckWorkflowCapability(workflowVersionId: string): Promise<CapabilityCheckView> {
  return invoke<CapabilityCheckView>("workflow_recheck_capability", { workflowVersionId });
}

export function recheckAllWorkflowCapabilities(): Promise<WorkflowCapabilityBatchView[]> {
  return invoke<WorkflowCapabilityBatchView[]>("workflow_recheck_all_capabilities");
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

export function inspectWorkflowDeletion(workflowVersionId: string): Promise<WorkflowDeletionInspection> {
  return invoke<WorkflowDeletionInspection>("workflow_inspect_deletion", { workflowVersionId });
}

export function deleteWorkflowVersion(workflowVersionId: string): Promise<WorkflowDeletionResult> {
  return invoke<WorkflowDeletionResult>("workflow_delete_version", { workflowVersionId });
}

export function deleteWorkflow(workflowId: string): Promise<WorkflowDeletionResult[]> {
  return invoke<WorkflowDeletionResult[]>("workflow_delete_workflow", { workflowId });
}

export function restoreWorkflowVersion(workflowVersionId: string): Promise<void> {
  return invoke<void>("workflow_restore_version", { workflowVersionId });
}

export function listGenerationCatalog(): Promise<RecipeViewModel[]> {
  return invoke<RecipeViewModel[]>("generation_catalog_list");
}

export function createGeneration(request: {
  projectId: string;
  workflowVersionId: string;
  recipeId: string;
  values: GenerationValues;
  submissionIdempotencyKey?: string;
}): Promise<TaskView> {
  return invoke<TaskView>("generation_create", { request });
}

export function previewWorkflowBenchmark(
  request: WorkflowBenchmarkCreateRequest,
): Promise<{ candidates: WorkflowBenchmarkCandidatePreview[] }> {
  return invoke<{ candidates: WorkflowBenchmarkCandidatePreview[] }>("workflow_benchmark_preview", { request });
}

export function createWorkflowBenchmark(request: WorkflowBenchmarkCreateRequest): Promise<WorkflowBenchmarkView> {
  return invoke<WorkflowBenchmarkView>("workflow_benchmark_create", { request });
}

export function setWorkflowBenchmarkRecommendation(
  projectId: string,
  experimentId: string,
  recommendationType?: string,
): Promise<WorkflowBenchmarkView> {
  return invoke<WorkflowBenchmarkView>("workflow_benchmark_set_recommendation", {
    request: { projectId, experimentId, recommendationType },
  });
}

export function saveWorkflowBenchmarkQuality(
  projectId: string,
  experimentId: string,
  candidateId: string,
  quality: WorkflowBenchmarkQuality,
): Promise<WorkflowBenchmarkView> {
  return invoke<WorkflowBenchmarkView>("workflow_benchmark_save_quality", {
    request: { projectId, experimentId, candidateId, ...quality },
  });
}

export function listWorkflowBenchmarks(projectId: string, limit = 20): Promise<WorkflowBenchmarkSummary[]> {
  return invoke<WorkflowBenchmarkSummary[]>("workflow_benchmark_list", { request: { projectId, limit } });
}

export function getWorkflowBenchmark(projectId: string, experimentId: string): Promise<WorkflowBenchmarkView> {
  return invoke<WorkflowBenchmarkView>("workflow_benchmark_get", { projectId, experimentId });
}

export function setWorkflowBenchmarkWinner(
  projectId: string,
  experimentId: string,
  candidateId?: string,
): Promise<WorkflowBenchmarkView> {
  return invoke<WorkflowBenchmarkView>("workflow_benchmark_set_winner", {
    request: { projectId, experimentId, candidateId },
  });
}

export function cloneWorkflowBenchmark(
  projectId: string,
  experimentId: string,
  name?: string,
): Promise<WorkflowBenchmarkView> {
  return invoke<WorkflowBenchmarkView>("workflow_benchmark_clone", {
    request: { projectId, experimentId, name },
  });
}

export function queueWorkflowBenchmark(
  projectId: string,
  experimentId: string,
  autoStart: boolean,
): Promise<WorkflowBenchmarkView> {
  return invoke<WorkflowBenchmarkView>("workflow_benchmark_queue_existing", {
    request: { projectId, experimentId, autoStart },
  });
}

export function deleteWorkflowBenchmark(projectId: string, experimentId: string): Promise<{ deleted: boolean; experimentId: string }> {
  return invoke<{ deleted: boolean; experimentId: string }>("workflow_benchmark_delete", { projectId, experimentId });
}

export function createProductionRun(request: ProductionRunCreateRequest): Promise<ProductionRun> {
  return invoke<ProductionRun>("production_run_create", { request });
}

export function listProductionRuns(projectId: string, limit = 20): Promise<ProductionRunListItem[]> {
  return invoke<ProductionRunListItem[]>("production_run_list", { request: { projectId, limit } });
}

export function getProductionRun(projectId: string, runId: string): Promise<ProductionRun> {
  return invoke<ProductionRun>("production_run_get", { projectId, runId });
}

export function runProductionImages(projectId: string, runId: string): Promise<ProductionRun> {
  return invoke<ProductionRun>("production_run_run_images", { projectId, runId });
}

export function selectProductionRunAssets(projectId: string, runId: string, assetIds: string[]): Promise<ProductionRun> {
  return invoke<ProductionRun>("production_run_select_assets", { request: { projectId, runId, assetIds } });
}

export function runProductionVideo(projectId: string, runId: string): Promise<ProductionRun> {
  return invoke<ProductionRun>("production_run_run_video", { projectId, runId });
}

export function retryProductionVideo(projectId: string, runId: string): Promise<ProductionRun> {
  return invoke<ProductionRun>("production_run_retry_video", { projectId, runId });
}

export function refreshProductionRun(projectId: string, runId: string): Promise<ProductionRun> {
  return invoke<ProductionRun>("production_run_refresh", { projectId, runId });
}

export function cancelProductionRun(projectId: string, runId: string): Promise<ProductionRun> {
  return invoke<ProductionRun>("production_run_cancel", { projectId, runId });
}

export function saveProductionRunTemplate(request: Omit<ProductionRunTemplate, "id" | "createdAt" | "updatedAt" | "projectId"> & { projectId: string }): Promise<ProductionRunTemplate> {
  return invoke<ProductionRunTemplate>("production_run_template_save", { request });
}

export function listProductionRunTemplates(projectId: string): Promise<ProductionRunTemplate[]> {
  return invoke<ProductionRunTemplate[]>("production_run_template_list", { projectId });
}

export function listShots(projectId: string): Promise<ShotView[]> {
  return invoke<ShotView[]>("shot_list", { projectId });
}

export function listProductionStructure(projectId: string): Promise<ProductionStructureTree> {
  return invoke<ProductionStructureTree>("production_structure_tree", { projectId });
}

export function createProductionSeries(request: ProductionSeriesRequest): Promise<ProductionSeries> {
  return invoke<ProductionSeries>("production_series_create", { request });
}

export function updateProductionSeries(request: Required<Pick<ProductionSeriesRequest, "projectId" | "seriesId" | "name">> & Pick<ProductionSeriesRequest, "description">): Promise<ProductionSeries> {
  return invoke<ProductionSeries>("production_series_update", { request });
}

export function deleteProductionSeries(projectId: string, seriesId: string): Promise<void> {
  return invoke<void>("production_series_delete", { projectId, seriesId });
}

export function reorderProductionSeries(projectId: string, orderedIds: string[]): Promise<ProductionSeries[]> {
  return invoke<ProductionSeries[]>("production_series_reorder", { request: { projectId, orderedIds } });
}

export function createProductionEpisode(request: ProductionEpisodeRequest): Promise<ProductionEpisode> {
  return invoke<ProductionEpisode>("production_episode_create", { request });
}

export function updateProductionEpisode(request: Required<Pick<ProductionEpisodeRequest, "projectId" | "episodeId" | "name">> & Pick<ProductionEpisodeRequest, "description">): Promise<ProductionEpisode> {
  return invoke<ProductionEpisode>("production_episode_update", { request });
}

export function deleteProductionEpisode(projectId: string, episodeId: string): Promise<void> {
  return invoke<void>("production_episode_delete", { projectId, episodeId });
}

export function reorderProductionEpisodes(projectId: string, seriesId: string, orderedIds: string[]): Promise<ProductionEpisode[]> {
  const request: ProductionReorderRequest = { projectId, parentId: seriesId, orderedIds };
  return invoke<ProductionEpisode[]>("production_episode_reorder", { request });
}

export function createProductionScene(request: ProductionSceneRequest): Promise<ProductionScene> {
  return invoke<ProductionScene>("production_scene_create", { request });
}

export function updateProductionScene(request: Required<Pick<ProductionSceneRequest, "projectId" | "sceneId" | "name">> & Pick<ProductionSceneRequest, "description">): Promise<ProductionScene> {
  return invoke<ProductionScene>("production_scene_update", { request });
}

export function deleteProductionScene(projectId: string, sceneId: string): Promise<void> {
  return invoke<void>("production_scene_delete", { projectId, sceneId });
}

export function reorderProductionScenes(projectId: string, episodeId: string, orderedIds: string[]): Promise<ProductionScene[]> {
  const request: ProductionReorderRequest = { projectId, parentId: episodeId, orderedIds };
  return invoke<ProductionScene[]>("production_scene_reorder", { request });
}

export function assignProductionSceneShots(request: ProductionAssignShotsRequest): Promise<void> {
  return invoke<void>("production_scene_assign_shots", { request });
}

export function unassignProductionSceneShots(request: ProductionUnassignShotsRequest): Promise<void> {
  return invoke<void>("production_scene_unassign_shots", { request });
}

export function reorderProductionSceneShots(request: ProductionSceneShotReorderRequest): Promise<void> {
  return invoke<void>("production_scene_reorder_shots", { request });
}

export function exportProjectManifest(projectId: string, destination?: string): Promise<ProjectManifestExportView | null> {
  return invoke<ProjectManifestExportView | null>("project_manifest_export", { projectId, destination });
}

export function listReferenceAnchors(projectId: string): Promise<ReferenceAnchorView[]> {
  return invoke<ReferenceAnchorView[]>("reference_anchors_list", { projectId });
}

export function listConsistencyProfiles(
  projectId: string,
  profileType?: ProfileType,
): Promise<ConsistencyProfileView[]> {
  const args = profileType ? { projectId, profileType } : { projectId };
  return invoke<ConsistencyProfileView[]>("consistency_profile_list", args);
}

export function getConsistencyProfile(
  projectId: string,
  profileType: ProfileType,
  profileId: string,
): Promise<ConsistencyProfileView> {
  return invoke<ConsistencyProfileView>("consistency_profile_get", { projectId, profileType, profileId });
}

export function createCharacterProfile(request: CharacterProfileRequest): Promise<ConsistencyProfileView> {
  return invoke<ConsistencyProfileView>("character_profile_create", { request });
}

export function updateCharacterProfile(request: CharacterProfileRequest & { profileId: string }): Promise<ConsistencyProfileView> {
  return invoke<ConsistencyProfileView>("character_profile_update", { request });
}

export function createSceneProfile(request: SceneProfileRequest): Promise<ConsistencyProfileView> {
  return invoke<ConsistencyProfileView>("scene_profile_create", { request });
}

export function updateSceneProfile(request: SceneProfileUpdateRequest): Promise<ConsistencyProfileView> {
  return invoke<ConsistencyProfileView>("scene_profile_update", { request });
}

export function createPropProfile(request: PropProfileRequest): Promise<ConsistencyProfileView> {
  return invoke<ConsistencyProfileView>("prop_profile_create", { request });
}

export function updatePropProfile(request: PropProfileUpdateRequest): Promise<ConsistencyProfileView> {
  return invoke<ConsistencyProfileView>("prop_profile_update", { request });
}

export function createStyleProfile(request: StyleProfileRequest): Promise<ConsistencyProfileView> {
  return invoke<ConsistencyProfileView>("style_profile_create", { request });
}

export function updateStyleProfile(request: StyleProfileUpdateRequest): Promise<ConsistencyProfileView> {
  return invoke<ConsistencyProfileView>("style_profile_update", { request });
}

export function deleteConsistencyProfile(
  projectId: string,
  profileType: ProfileType,
  profileId: string,
): Promise<void> {
  return invoke<void>("consistency_profile_delete", { projectId, profileType, profileId });
}

export function listCostumeVariants(projectId: string, characterProfileId: string): Promise<CostumeVariantView[]> {
  return invoke<CostumeVariantView[]>("costume_variant_list", { projectId, characterProfileId });
}

export function getCostumeVariant(projectId: string, costumeVariantId: string): Promise<CostumeVariantView> {
  return invoke<CostumeVariantView>("costume_variant_get", { projectId, costumeVariantId });
}

export function createCostumeVariant(request: CostumeVariantRequest): Promise<CostumeVariantView> {
  return invoke<CostumeVariantView>("costume_variant_create", { request });
}

export function updateCostumeVariant(request: CostumeVariantUpdateRequest): Promise<CostumeVariantView> {
  return invoke<CostumeVariantView>("costume_variant_update", { request });
}

export function deleteCostumeVariant(projectId: string, costumeVariantId: string): Promise<void> {
  return invoke<void>("costume_variant_delete", { projectId, costumeVariantId });
}

export function listReferenceSets(
  projectId: string,
  purpose?: ReferenceSetPurpose,
): Promise<ReferenceSetSummary[]> {
  const args = purpose ? { projectId, purpose } : { projectId };
  return invoke<ReferenceSetSummary[]>("reference_set_list", args);
}

export function getReferenceSetDetail(projectId: string, referenceSetId: string): Promise<ReferenceSetDetailView> {
  return invoke<ReferenceSetDetailView>("reference_set_detail_get", { projectId, referenceSetId });
}

export function createReferenceSet(request: ReferenceSetRequest): Promise<ReferenceSetView> {
  return invoke<ReferenceSetView>("reference_set_create", { request });
}

export function updateReferenceSet(request: ReferenceSetUpdateRequest): Promise<ReferenceSetView> {
  return invoke<ReferenceSetView>("reference_set_update", { request });
}

export function deleteReferenceSet(projectId: string, referenceSetId: string): Promise<void> {
  return invoke<void>("reference_set_delete", { projectId, referenceSetId });
}

export function createReferenceSetFromAnchor(
  projectId: string,
  anchorId: string,
  newName?: string,
): Promise<ReferenceSetView> {
  return invoke<ReferenceSetView>("reference_set_create_from_anchor", {
    request: { projectId, anchorId, newName },
  });
}

export function getAssetUsage(projectId: string, assetId: string): Promise<AssetUsageSummary> {
  return invoke<AssetUsageSummary>("asset_usage_get", { projectId, assetId });
}

export function getProfileUsage(
  projectId: string,
  profileType: ProfileType,
  profileId: string,
): Promise<ProfileUsageSummary> {
  return invoke<ProfileUsageSummary>("profile_usage_get", { projectId, profileType, profileId });
}

export function getReferenceSetUsage(projectId: string, referenceSetId: string): Promise<ReferenceSetUsageSummary> {
  return invoke<ReferenceSetUsageSummary>("reference_set_usage_get", { projectId, referenceSetId });
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
  retryTaskId?: string;
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

export interface ShotBulkImportRowPreview {
  rowNumber: number;
  name: string;
  description: string;
  imagePrompt?: string;
  videoPrompt?: string;
  errors: Array<{ code: string; message: string; rowNumber?: number; shotId?: string }>;
  warnings: Array<{ code: string; message: string; rowNumber?: number; shotId?: string }>;
}

export interface ShotBulkImportPreview {
  total: number;
  valid: number;
  invalid: number;
  warnings: number;
  rows: ShotBulkImportRowPreview[];
}

export interface ShotBulkImportRequest {
  projectId: string;
  format: "tsv" | "json";
  content: string;
}

export function previewShotBulkImport(request: ShotBulkImportRequest): Promise<ShotBulkImportPreview> {
  return invoke<ShotBulkImportPreview>("preview_shot_bulk_import", { request });
}

export function commitShotBulkImport(request: ShotBulkImportRequest): Promise<{ projectId: string; created: Array<{ shotId: string; ordinal: number; name: string }> }> {
  return invoke("commit_shot_bulk_import", { request });
}

export type BulkPromptSource =
  | { type: "text"; text: string }
  | { type: "promptLibraryVersion"; promptEntryId: string; promptVersionId: string }
  | { type: "clearProvenance" };

export function bulkAssignShotPrompt(request: {
  projectId: string;
  stage: ShotStage;
  shotIds: string[];
  source: BulkPromptSource;
}): Promise<{ projectId: string; stage: ShotStage; updatedShotIds: string[] }> {
  return invoke("bulk_assign_shot_prompt", { request });
}

export function bulkSetShotStageConfig(request: {
  projectId: string;
  stage: ShotStage;
  shotIds: string[];
  workflowVersionId: string;
  recipeId: string;
  values: ShotInputValues;
  prompt?: BulkPromptSource;
}): Promise<{ projectId: string; stage: ShotStage; configuredShotIds: string[]; promptUpdatedShotIds: string[] }> {
  return invoke("bulk_set_shot_stage_config", { request });
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

export function getProductionPartialResumePlan(
  projectId: string,
  batchId: string,
): Promise<ProductionPartialResumePlan> {
  return invoke<ProductionPartialResumePlan>("production_queue_partial_resume_plan", { projectId, batchId });
}

export function partialResumeProductionQueue(
  projectId: string,
  batchId: string,
  selectedLeafItemIds: string[],
): Promise<ProductionPartialResumeResult> {
  return invoke<ProductionPartialResumeResult>("production_queue_partial_resume", {
    projectId,
    batchId,
    selectedLeafItemIds,
  });
}

export function getProductionBatchReview(projectId: string, batchId: string): Promise<ProductionBatchReview> {
  return invoke<ProductionBatchReview>("production_item_review_get", { projectId, batchId });
}

export function setProductionReviewStatus(request: {
  projectId: string;
  batchId: string;
  itemId: string;
  status: Exclude<ProductionReviewStatus, "FAILED" | "IN_PROGRESS">;
}): Promise<ProductionBatchReview> {
  return invoke<ProductionBatchReview>("production_item_review_set_status", { request });
}

export function setProductionReviewNote(request: {
  projectId: string;
  batchId: string;
  itemId: string;
  note: string;
}): Promise<ProductionBatchReview> {
  return invoke<ProductionBatchReview>("production_item_review_set_note", { request });
}

export function regenerateProductionItem(request: {
  projectId: string;
  batchId: string;
  itemId: string;
  promptOverride?: string;
  durationSeconds?: number;
  width?: number;
  height?: number;
  useOriginalSeed: boolean;
  autoStart: boolean;
}): Promise<ProductionReviewRegenerateResult> {
  return invoke<ProductionReviewRegenerateResult>("production_item_review_regenerate", { request });
}

export function regenerateMarkedProductionItems(request: {
  projectId: string;
  batchId: string;
  autoStart: boolean;
}): Promise<ProductionReviewRegenerateResult> {
  return invoke<ProductionReviewRegenerateResult>("production_item_review_regenerate_marked", { request });
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

export function importSourceAssets(projectId: string): Promise<AssetSourceImportBatch> {
  return invoke<AssetSourceImportBatch>("asset_pick_and_import_source_assets", { projectId });
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
  generationMode?: string;
  fl2vaWorkflowVersionId?: string;
  fl2vaRecipeId?: string;
  ref2vaWorkflowVersionId?: string;
  ref2vaRecipeId?: string;
  qualityProfile?: "QUALITY" | "FAST";
  qualityRecipes?: Array<{
    mode: string;
    workflowVersionId: string;
    recipeId: string;
  }>;
}): Promise<H3LocalImportResult> {
  return invoke<H3LocalImportResult>("h3_local_import_commit", { request });
}

export function updateH3ProjectSegmentDraft(
  request: H3ProjectSegmentDraft,
): Promise<H3LocalImportInspection> {
  return invoke<H3LocalImportInspection>("h3_local_import_update_project_segment_draft", { request });
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

export function getProductionAuditSummary(projectId: string): Promise<ProductionAuditSummary> {
  return invoke<ProductionAuditSummary>("production_audit_summary", { request: { projectId } });
}

export function getProductionAuditRecentActivity(projectId: string, limit = 50): Promise<ProductionAuditActivity[]> {
  return invoke<ProductionAuditActivity[]>("production_audit_recent_activity", { request: { projectId, limit } });
}

export function getProductionAuditLineage(request: ProductionAuditLineageRequest): Promise<ProductionAuditLineage> {
  return invoke<ProductionAuditLineage>("production_audit_lineage", { request });
}

export function getProductionAuditIntegrity(projectId: string): Promise<ProductionAuditIntegrity> {
  return invoke<ProductionAuditIntegrity>("production_audit_integrity", { request: { projectId } });
}

export function getProjectCommandCenter(projectId: string): Promise<ProjectCommandCenterAggregate> {
  return invoke<ProjectCommandCenterAggregate>("project_command_center_get", { projectId });
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

export function getReferenceAnchor(projectId: string, anchorId: string): Promise<ReferenceAnchorView> {
  return invoke<ReferenceAnchorView>("reference_anchor_get", { projectId, anchorId });
}

export function createReferenceAnchor(request: ReferenceAnchorRequest): Promise<ReferenceAnchorView> {
  return invoke<ReferenceAnchorView>("reference_anchor_create", { request });
}

export function updateReferenceAnchor(request: ReferenceAnchorUpdateRequest): Promise<ReferenceAnchorView> {
  return invoke<ReferenceAnchorView>("reference_anchor_update", { request });
}

export function deleteReferenceAnchor(projectId: string, anchorId: string): Promise<void> {
  return invoke<void>("reference_anchor_delete", { projectId, anchorId });
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
