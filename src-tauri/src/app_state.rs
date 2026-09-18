use crate::application::artifact_service::ArtifactService;
use crate::application::asset_data_service::AssetDataService;
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
use crate::application::external_production_handoff_service::ExternalProductionHandoffService;
use crate::application::generation_catalog_service::GenerationCatalogService;
use crate::application::h3_local_import_service::H3LocalImportService;
use crate::application::model_service::ModelService;
use crate::application::organization_service::OrganizationService;
use crate::application::preset_service::PresetService;
use crate::application::production_audit_service::ProductionAuditService;
use crate::application::production_batch_runbook_service::ProductionBatchRunbookService;
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
use crate::application::provenance_lineage_service::ProvenanceLineageService;
use crate::application::recipe_history_query_service::RecipeHistoryQueryService;
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
use crate::application::tool_service::ToolService;
use crate::application::workflow_benchmark_service::WorkflowBenchmarkService;
use crate::application::workflow_library_service::WorkflowLibraryService;
use crate::application::workflow_lifecycle_coordinator::WorkflowLifecycleCoordinator;
use crate::application::workflow_lifecycle_service::WorkflowLifecycleService;
use crate::application::workflow_onboarding_service::WorkflowOnboardingService;
use crate::application::workflow_registry_service::WorkflowRegistryService;
use crate::application::workflow_workspace_query_service::WorkflowWorkspaceQueryService;
use crate::infrastructure::filesystem::AppDataDirs;
use std::sync::Arc;

pub struct ProductionServices {
    pub orchestrator: Arc<ProductionOrchestratorService>,
    pub package: Arc<ProductionPackageService>,
    pub queue: Arc<ProductionQueueService>,
    pub admission: Arc<ProductionStartAdmissionService>,
    pub audit: Arc<ProductionAuditService>,
    pub preparation: Arc<ProductionPreparationService>,
    pub batch_runbook: Arc<ProductionBatchRunbookService>,
    pub scene: Arc<SceneProductionService>,
    pub episode: Arc<EpisodeProductionService>,
    pub series: Arc<SeriesProductionService>,
    pub external_handoff: Arc<ExternalProductionHandoffService>,
}

pub struct WorkflowServices {
    pub library: Arc<WorkflowLibraryService>,
    pub registry: Arc<WorkflowRegistryService>,
    pub workspace_query: Arc<WorkflowWorkspaceQueryService>,
    pub recipe_history_query: Arc<RecipeHistoryQueryService>,
    pub onboarding: Arc<WorkflowOnboardingService>,
    pub lifecycle: Arc<WorkflowLifecycleService>,
    pub lifecycle_coordinator: Arc<WorkflowLifecycleCoordinator>,
    pub benchmark: Arc<WorkflowBenchmarkService>,
}

pub struct AssetServices {
    pub query: Arc<AssetQueryService>,
    pub library: Arc<AssetLibraryService>,
    pub data: Arc<AssetDataService>,
    pub usage: Arc<AssetUsageService>,
    pub deletion: Arc<AssetDeletionService>,
    pub video_prompt: Arc<AssetVideoPromptService>,
    pub source_import: Arc<SourceAssetImportService>,
    pub artifact: Arc<ArtifactService>,
}

pub struct ProjectServices {
    pub command_center: Arc<ProjectCommandCenterService>,
    pub project: Arc<ProjectService>,
    pub backup: Arc<ProjectBackupService>,
    pub manifest: Arc<ProjectManifestService>,
    pub workflow_binding: Arc<ProjectWorkflowBindingService>,
}

pub struct ShotServices {
    pub shot: Arc<ShotService>,
    pub batch: Arc<ShotBatchService>,
    pub bulk: Arc<ShotBulkService>,
    pub readiness: Arc<ShotReadinessService>,
    pub context_resolver: Arc<ShotContextResolver>,
    pub reference_anchor: Arc<ReferenceAnchorService>,
    pub reference_set: Arc<ReferenceSetService>,
    pub consistency_profile: Arc<ConsistencyProfileService>,
    pub consistency_scope_binding: Arc<ConsistencyScopeBindingService>,
    pub consistency_binding: Arc<ShotConsistencyBindingService>,
}

pub struct CatalogServices {
    pub generation: Arc<GenerationCatalogService>,
    pub preset: Arc<PresetService>,
    pub prompt_library: Arc<PromptLibraryService>,
    pub model: Arc<ModelService>,
    pub tool: Arc<ToolService>,
    pub h3_local_import: Arc<H3LocalImportService>,
    pub batch_workflow_preset: Arc<BatchWorkflowPresetService>,
}

pub struct TaskServices {
    pub query: Arc<TaskQueryService>,
    pub history: Arc<TaskHistoryService>,
    pub cancellation: Arc<TaskCancellationService>,
    pub recovery: Arc<TaskRecoveryService>,
    pub provenance_lineage: Arc<ProvenanceLineageService>,
}

pub struct OrganizationServices {
    pub organization: Arc<OrganizationService>,
    pub project_template: Arc<ProjectTemplateService>,
    pub production_structure: Arc<ProductionStructureService>,
}

pub struct SystemServices {
    pub data_dirs: AppDataDirs,
    pub comfy: Arc<ComfyService>,
    pub comfy_memory: Arc<ComfyMemoryService>,
    pub comfy_preflight: Arc<ComfyPreflightService>,
    pub diagnostics: Arc<DiagnosticsService>,
    pub settings: Arc<SettingsService>,
}

pub struct AppState {
    pub production: ProductionServices,
    pub workflow: WorkflowServices,
    pub assets: AssetServices,
    pub projects: ProjectServices,
    pub shots: ShotServices,
    pub catalog: CatalogServices,
    pub tasks: TaskServices,
    pub organization: OrganizationServices,
    pub system: SystemServices,
}

impl AppState {
    pub fn new(
        production: ProductionServices,
        workflow: WorkflowServices,
        assets: AssetServices,
        projects: ProjectServices,
        shots: ShotServices,
        catalog: CatalogServices,
        tasks: TaskServices,
        organization: OrganizationServices,
        system: SystemServices,
    ) -> Self {
        Self {
            production,
            workflow,
            assets,
            projects,
            shots,
            catalog,
            tasks,
            organization,
            system,
        }
    }
}
