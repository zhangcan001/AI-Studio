use crate::application::asset_deletion_service::AssetDeletionService;
use crate::application::asset_library_service::AssetLibraryService;
use crate::application::asset_query_service::AssetQueryService;
use crate::application::asset_usage_service::AssetUsageService;
use crate::application::asset_video_prompt_service::AssetVideoPromptService;
use crate::application::batch_workflow_preset_service::BatchWorkflowPresetService;
use crate::application::comfy_memory_service::ComfyMemoryService;
use crate::application::comfy_preflight_service::ComfyPreflightService;
use crate::application::comfy_service::ComfyService;
use crate::application::consistency_profile_service::ConsistencyProfileService;
use crate::application::consistency_scope_binding_service::ConsistencyScopeBindingService;
use crate::application::diagnostics_service::DiagnosticsService;
use crate::application::episode_production_service::EpisodeProductionService;
use crate::application::generation_catalog_service::GenerationCatalogService;
use crate::application::generation_service::GenerationService;
use crate::application::h3_local_import_service::H3LocalImportService;
use crate::application::organization_service::OrganizationService;
use crate::application::preset_service::PresetService;
use crate::application::production_audit_service::ProductionAuditService;
use crate::application::production_batch_runbook_service::ProductionBatchRunbookService;
use crate::application::production_item_review_service::ProductionItemReviewService;
use crate::application::production_orchestrator_service::ProductionOrchestratorService;
use crate::application::production_package_service::ProductionPackageService;
use crate::application::production_preparation_service::ProductionPreparationService;
use crate::application::production_queue_service::ProductionQueueService;
use crate::application::production_start_admission_service::ProductionStartAdmissionService;
use crate::application::production_structure_service::ProductionStructureService;
use crate::application::project_backup_service::ProjectBackupService;
use crate::application::project_command_center_service::ProjectCommandCenterService;
use crate::application::project_manifest_service::ProjectManifestService;
use crate::application::project_service::ProjectService;
use crate::application::project_template_service::ProjectTemplateService;
use crate::application::project_workflow_binding_service::ProjectWorkflowBindingService;
use crate::application::prompt_library_service::PromptLibraryService;
use crate::application::prompt_template_bulk_service::PromptTemplateBulkService;
use crate::application::prompt_template_service::PromptTemplateService;
use crate::application::reference_anchor_service::ReferenceAnchorService;
use crate::application::reference_set_service::ReferenceSetService;
use crate::application::scene_production_service::SceneProductionService;
use crate::application::series_production_service::SeriesProductionService;
use crate::application::settings_service::SettingsService;
use crate::application::shot_batch_service::ShotBatchService;
use crate::application::shot_bulk_service::ShotBulkService;
use crate::application::shot_consistency_binding_service::ShotConsistencyBindingService;
use crate::application::shot_context_resolver::ShotContextResolver;
use crate::application::shot_readiness_service::ShotReadinessService;
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
    pub asset_usage_service: Arc<AssetUsageService>,
    pub production_structure_service: Arc<ProductionStructureService>,
    pub project_command_center_service: Arc<ProjectCommandCenterService>,
    pub reference_anchor_service: Arc<ReferenceAnchorService>,
    pub consistency_profile_service: Arc<ConsistencyProfileService>,
    pub consistency_scope_binding_service: Arc<ConsistencyScopeBindingService>,
    pub shot_consistency_binding_service: Arc<ShotConsistencyBindingService>,
    pub shot_context_resolver: Arc<ShotContextResolver>,
    pub reference_set_service: Arc<ReferenceSetService>,
    pub asset_deletion_service: Arc<AssetDeletionService>,
    pub asset_video_prompt_service: Arc<AssetVideoPromptService>,
    pub task_history_service: Arc<TaskHistoryService>,
    pub source_asset_import_service: Arc<SourceAssetImportService>,
    pub h3_local_import_service: Arc<H3LocalImportService>,
    pub production_package_service: Arc<ProductionPackageService>,
    pub task_cancellation_service: Arc<TaskCancellationService>,
    pub task_recovery_service: Arc<TaskRecoveryService>,
    pub project_service: Arc<ProjectService>,
    pub project_backup_service: Arc<ProjectBackupService>,
    pub project_manifest_service: Arc<ProjectManifestService>,
    pub project_workflow_binding_service: Arc<ProjectWorkflowBindingService>,
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
    pub production_start_admission_service: Arc<ProductionStartAdmissionService>,
    pub production_item_review_service: Arc<ProductionItemReviewService>,
    pub production_audit_service: Arc<ProductionAuditService>,
    pub diagnostics_service: Arc<DiagnosticsService>,
    pub comfy_preflight_service: Arc<ComfyPreflightService>,
    pub shot_readiness_service: Arc<ShotReadinessService>,
    pub production_preparation_service: Arc<ProductionPreparationService>,
    pub settings_service: Arc<SettingsService>,
    pub batch_workflow_preset_service: Arc<BatchWorkflowPresetService>,
    pub scene_production_service: Arc<SceneProductionService>,
    pub episode_production_service: Arc<EpisodeProductionService>,
    pub series_production_service: Arc<SeriesProductionService>,
    pub production_batch_runbook_service: Arc<ProductionBatchRunbookService>,
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
        asset_usage_service: Arc<AssetUsageService>,
        production_structure_service: Arc<ProductionStructureService>,
        project_command_center_service: Arc<ProjectCommandCenterService>,
        reference_anchor_service: Arc<ReferenceAnchorService>,
        consistency_profile_service: Arc<ConsistencyProfileService>,
        consistency_scope_binding_service: Arc<ConsistencyScopeBindingService>,
        shot_consistency_binding_service: Arc<ShotConsistencyBindingService>,
        shot_context_resolver: Arc<ShotContextResolver>,
        reference_set_service: Arc<ReferenceSetService>,
        asset_deletion_service: Arc<AssetDeletionService>,
        asset_video_prompt_service: Arc<AssetVideoPromptService>,
        task_history_service: Arc<TaskHistoryService>,
        source_asset_import_service: Arc<SourceAssetImportService>,
        h3_local_import_service: Arc<H3LocalImportService>,
        production_package_service: Arc<ProductionPackageService>,
        task_cancellation_service: Arc<TaskCancellationService>,
        task_recovery_service: Arc<TaskRecoveryService>,
        project_service: Arc<ProjectService>,
        project_backup_service: Arc<ProjectBackupService>,
        project_manifest_service: Arc<ProjectManifestService>,
        project_workflow_binding_service: Arc<ProjectWorkflowBindingService>,
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
        production_start_admission_service: Arc<ProductionStartAdmissionService>,
        production_item_review_service: Arc<ProductionItemReviewService>,
        production_audit_service: Arc<ProductionAuditService>,
        diagnostics_service: Arc<DiagnosticsService>,
        comfy_preflight_service: Arc<ComfyPreflightService>,
        shot_readiness_service: Arc<ShotReadinessService>,
        production_preparation_service: Arc<ProductionPreparationService>,
        settings_service: Arc<SettingsService>,
        batch_workflow_preset_service: Arc<BatchWorkflowPresetService>,
        scene_production_service: Arc<SceneProductionService>,
        episode_production_service: Arc<EpisodeProductionService>,
        series_production_service: Arc<SeriesProductionService>,
        production_batch_runbook_service: Arc<ProductionBatchRunbookService>,
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
            asset_usage_service,
            production_structure_service,
            project_command_center_service,
            reference_anchor_service,
            consistency_profile_service,
            consistency_scope_binding_service,
            shot_consistency_binding_service,
            shot_context_resolver,
            reference_set_service,
            asset_deletion_service,
            asset_video_prompt_service,
            task_history_service,
            source_asset_import_service,
            h3_local_import_service,
            production_package_service,
            task_cancellation_service,
            task_recovery_service,
            project_service,
            project_backup_service,
            project_manifest_service,
            project_workflow_binding_service,
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
            production_start_admission_service,
            production_item_review_service,
            production_audit_service,
            diagnostics_service,
            comfy_preflight_service,
            shot_readiness_service,
            production_preparation_service,
            settings_service,
            batch_workflow_preset_service,
            scene_production_service,
            episode_production_service,
            series_production_service,
            production_batch_runbook_service,
        }
    }
}
