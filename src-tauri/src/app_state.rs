use crate::application::asset_deletion_service::AssetDeletionService;
use crate::application::asset_library_service::AssetLibraryService;
use crate::application::asset_query_service::AssetQueryService;
use crate::application::asset_video_prompt_service::AssetVideoPromptService;
use crate::application::comfy_memory_service::ComfyMemoryService;
use crate::application::comfy_preflight_service::ComfyPreflightService;
use crate::application::comfy_service::ComfyService;
use crate::application::diagnostics_service::DiagnosticsService;
use crate::application::generation_catalog_service::GenerationCatalogService;
use crate::application::generation_service::GenerationService;
use crate::application::h3_local_import_service::H3LocalImportService;
use crate::application::organization_service::OrganizationService;
use crate::application::preset_service::PresetService;
use crate::application::production_item_review_service::ProductionItemReviewService;
use crate::application::production_orchestrator_service::ProductionOrchestratorService;
use crate::application::production_queue_service::ProductionQueueService;
use crate::application::production_structure_service::ProductionStructureService;
use crate::application::project_backup_service::ProjectBackupService;
use crate::application::project_manifest_service::ProjectManifestService;
use crate::application::project_service::ProjectService;
use crate::application::project_template_service::ProjectTemplateService;
use crate::application::prompt_library_service::PromptLibraryService;
use crate::application::prompt_template_bulk_service::PromptTemplateBulkService;
use crate::application::prompt_template_service::PromptTemplateService;
use crate::application::reference_anchor_service::ReferenceAnchorService;
use crate::application::settings_service::SettingsService;
use crate::application::shot_batch_service::ShotBatchService;
use crate::application::shot_bulk_service::ShotBulkService;
use crate::application::shot_service::ShotService;
use crate::application::source_asset_import_service::SourceAssetImportService;
use crate::application::task_cancellation_service::TaskCancellationService;
use crate::application::task_history_service::TaskHistoryService;
use crate::application::task_query_service::TaskQueryService;
use crate::application::task_recovery_service::TaskRecoveryService;
use crate::application::workflow_benchmark_service::WorkflowBenchmarkService;
use crate::application::workflow_library_service::WorkflowLibraryService;
use crate::application::workflow_lifecycle_service::WorkflowLifecycleService;
use crate::application::workflow_onboarding_service::WorkflowOnboardingService;
use crate::infrastructure::filesystem::AppDataDirs;
use std::sync::Arc;

pub struct AppState {
    pub data_dirs: AppDataDirs,
    pub comfy_service: Arc<ComfyService>,
    pub comfy_memory_service: Arc<ComfyMemoryService>,
    pub generation_service: Arc<GenerationService>,
    pub workflow_library_service: Arc<WorkflowLibraryService>,
    pub workflow_onboarding_service: Arc<WorkflowOnboardingService>,
    pub workflow_lifecycle_service: Arc<WorkflowLifecycleService>,
    pub workflow_benchmark_service: Arc<WorkflowBenchmarkService>,
    pub production_orchestrator_service: Arc<ProductionOrchestratorService>,
    pub generation_catalog_service: Arc<GenerationCatalogService>,
    pub task_query_service: Arc<TaskQueryService>,
    pub asset_query_service: Arc<AssetQueryService>,
    pub asset_library_service: Arc<AssetLibraryService>,
    pub production_structure_service: Arc<ProductionStructureService>,
    pub reference_anchor_service: Arc<ReferenceAnchorService>,
    pub asset_deletion_service: Arc<AssetDeletionService>,
    pub asset_video_prompt_service: Arc<AssetVideoPromptService>,
    pub task_history_service: Arc<TaskHistoryService>,
    pub source_asset_import_service: Arc<SourceAssetImportService>,
    pub h3_local_import_service: Arc<H3LocalImportService>,
    pub task_cancellation_service: Arc<TaskCancellationService>,
    pub task_recovery_service: Arc<TaskRecoveryService>,
    pub project_service: Arc<ProjectService>,
    pub project_backup_service: Arc<ProjectBackupService>,
    pub project_manifest_service: Arc<ProjectManifestService>,
    pub preset_service: Arc<PresetService>,
    pub prompt_library_service: Arc<PromptLibraryService>,
    pub prompt_template_service: Arc<PromptTemplateService>,
    pub prompt_template_bulk_service: Arc<PromptTemplateBulkService>,
    pub shot_service: Arc<ShotService>,
    pub shot_batch_service: Arc<ShotBatchService>,
    pub shot_bulk_service: Arc<ShotBulkService>,
    pub organization_service: Arc<OrganizationService>,
    pub project_template_service: Arc<ProjectTemplateService>,
    pub production_queue_service: Arc<ProductionQueueService>,
    pub production_item_review_service: Arc<ProductionItemReviewService>,
    pub diagnostics_service: Arc<DiagnosticsService>,
    pub comfy_preflight_service: Arc<ComfyPreflightService>,
    pub settings_service: Arc<SettingsService>,
}

impl AppState {
    pub fn new(
        data_dirs: AppDataDirs,
        comfy_service: Arc<ComfyService>,
        comfy_memory_service: Arc<ComfyMemoryService>,
        generation_service: Arc<GenerationService>,
        workflow_library_service: Arc<WorkflowLibraryService>,
        workflow_onboarding_service: Arc<WorkflowOnboardingService>,
        workflow_lifecycle_service: Arc<WorkflowLifecycleService>,
        workflow_benchmark_service: Arc<WorkflowBenchmarkService>,
        production_orchestrator_service: Arc<ProductionOrchestratorService>,
        generation_catalog_service: Arc<GenerationCatalogService>,
        task_query_service: Arc<TaskQueryService>,
        asset_query_service: Arc<AssetQueryService>,
        asset_library_service: Arc<AssetLibraryService>,
        production_structure_service: Arc<ProductionStructureService>,
        reference_anchor_service: Arc<ReferenceAnchorService>,
        asset_deletion_service: Arc<AssetDeletionService>,
        asset_video_prompt_service: Arc<AssetVideoPromptService>,
        task_history_service: Arc<TaskHistoryService>,
        source_asset_import_service: Arc<SourceAssetImportService>,
        h3_local_import_service: Arc<H3LocalImportService>,
        task_cancellation_service: Arc<TaskCancellationService>,
        task_recovery_service: Arc<TaskRecoveryService>,
        project_service: Arc<ProjectService>,
        project_backup_service: Arc<ProjectBackupService>,
        project_manifest_service: Arc<ProjectManifestService>,
        preset_service: Arc<PresetService>,
        prompt_library_service: Arc<PromptLibraryService>,
        prompt_template_service: Arc<PromptTemplateService>,
        prompt_template_bulk_service: Arc<PromptTemplateBulkService>,
        shot_service: Arc<ShotService>,
        shot_batch_service: Arc<ShotBatchService>,
        shot_bulk_service: Arc<ShotBulkService>,
        organization_service: Arc<OrganizationService>,
        project_template_service: Arc<ProjectTemplateService>,
        production_queue_service: Arc<ProductionQueueService>,
        production_item_review_service: Arc<ProductionItemReviewService>,
        diagnostics_service: Arc<DiagnosticsService>,
        comfy_preflight_service: Arc<ComfyPreflightService>,
        settings_service: Arc<SettingsService>,
    ) -> Self {
        Self {
            data_dirs,
            comfy_service,
            comfy_memory_service,
            generation_service,
            workflow_library_service,
            workflow_onboarding_service,
            workflow_lifecycle_service,
            workflow_benchmark_service,
            production_orchestrator_service,
            generation_catalog_service,
            task_query_service,
            asset_query_service,
            asset_library_service,
            production_structure_service,
            reference_anchor_service,
            asset_deletion_service,
            asset_video_prompt_service,
            task_history_service,
            source_asset_import_service,
            h3_local_import_service,
            task_cancellation_service,
            task_recovery_service,
            project_service,
            project_backup_service,
            project_manifest_service,
            preset_service,
            prompt_library_service,
            prompt_template_service,
            prompt_template_bulk_service,
            shot_service,
            shot_batch_service,
            shot_bulk_service,
            organization_service,
            project_template_service,
            production_queue_service,
            production_item_review_service,
            diagnostics_service,
            comfy_preflight_service,
            settings_service,
        }
    }
}
