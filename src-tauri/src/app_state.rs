use crate::application::asset_query_service::AssetQueryService;
use crate::application::comfy_service::ComfyService;
use crate::application::generation_catalog_service::GenerationCatalogService;
use crate::application::generation_service::GenerationService;
use crate::application::source_asset_import_service::SourceAssetImportService;
use crate::application::task_query_service::TaskQueryService;
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
    pub source_asset_import_service: Arc<SourceAssetImportService>,
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
        source_asset_import_service: Arc<SourceAssetImportService>,
    ) -> Self {
        Self {
            data_dirs,
            comfy_service,
            generation_service,
            workflow_library_service,
            generation_catalog_service,
            task_query_service,
            asset_query_service,
            source_asset_import_service,
        }
    }
}
