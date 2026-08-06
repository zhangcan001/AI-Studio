use crate::application::asset_library_service::AssetLibraryService;
use crate::application::asset_query_service::AssetQueryService;
use crate::application::comfy_service::ComfyService;
use crate::application::generation_catalog_service::GenerationCatalogService;
use crate::application::generation_service::GenerationService;
use crate::application::project_service::ProjectService;
use crate::application::source_asset_import_service::SourceAssetImportService;
use crate::application::task_cancellation_service::TaskCancellationService;
use crate::application::task_history_service::TaskHistoryService;
use crate::application::task_query_service::TaskQueryService;
use crate::application::task_recovery_service::TaskRecoveryService;
use crate::application::workflow_library_service::WorkflowLibraryService;
use crate::infrastructure::filesystem::AppDataDirs;
use std::sync::Arc;

pub struct AppState {
    pub data_dirs: AppDataDirs,
    pub comfy_service: Arc<ComfyService>,
    pub generation_service: Arc<GenerationService>,
    pub workflow_library_service: Arc<WorkflowLibraryService>,
    pub generation_catalog_service: Arc<GenerationCatalogService>,
    pub task_query_service: Arc<TaskQueryService>,
    pub asset_query_service: Arc<AssetQueryService>,
    pub asset_library_service: Arc<AssetLibraryService>,
    pub task_history_service: Arc<TaskHistoryService>,
    pub source_asset_import_service: Arc<SourceAssetImportService>,
    pub task_cancellation_service: Arc<TaskCancellationService>,
    pub task_recovery_service: Arc<TaskRecoveryService>,
    pub project_service: Arc<ProjectService>,
}

impl AppState {
    pub fn new(
        data_dirs: AppDataDirs,
        comfy_service: Arc<ComfyService>,
        generation_service: Arc<GenerationService>,
        workflow_library_service: Arc<WorkflowLibraryService>,
        generation_catalog_service: Arc<GenerationCatalogService>,
        task_query_service: Arc<TaskQueryService>,
        asset_query_service: Arc<AssetQueryService>,
        asset_library_service: Arc<AssetLibraryService>,
        task_history_service: Arc<TaskHistoryService>,
        source_asset_import_service: Arc<SourceAssetImportService>,
        task_cancellation_service: Arc<TaskCancellationService>,
        task_recovery_service: Arc<TaskRecoveryService>,
        project_service: Arc<ProjectService>,
    ) -> Self {
        Self {
            data_dirs,
            comfy_service,
            generation_service,
            workflow_library_service,
            generation_catalog_service,
            task_query_service,
            asset_query_service,
            asset_library_service,
            task_history_service,
            source_asset_import_service,
            task_cancellation_service,
            task_recovery_service,
            project_service,
        }
    }
}
