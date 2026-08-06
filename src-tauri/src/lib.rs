mod app_state;
mod application;
mod commands;
pub mod compiler;
pub mod domain;
mod error;
mod infrastructure;

pub use application::ports::{
    AssetRepository, AssetStore, Clock, GenerationDefinitionRepository,
    GenerationSnapshotRepository, ProjectRepository, RepositoryError, TaskRepository,
    WorkflowLibraryRepository,
};
pub use infrastructure::database::{
    SqliteAssetRepository, SqliteGenerationDefinitionRepository,
    SqliteGenerationSnapshotRepository, SqliteProjectRepository, SqliteTaskRepository,
    SqliteWorkflowLibraryRepository,
};

use app_state::AppState;
use application::{
    asset_query_service::AssetQueryService,
    comfy_service::ComfyService,
    generation_catalog_service::GenerationCatalogService,
    generation_service::GenerationService,
    ports::{ComfyAdapter, ComfyConnectionConfig, WorkflowLibrarySource},
    project_bootstrap::DefaultProjectBootstrap,
    task_query_service::TaskQueryService,
    workflow_library_service::WorkflowLibraryService,
};
use error::AppError;
use infrastructure::{
    comfy::ComfyHttpAdapter,
    database,
    filesystem::{AppDataDirs, FileSystemAssetStore, FileSystemWorkflowLibrarySource},
    tauri::TauriTaskUpdateSink,
    time::SystemClock,
};
use std::sync::Arc;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    initialize_logging();
    tracing::info!("application starting");

    if let Err(error) = run_application() {
        tracing::error!(code = error.code(), message = %error.message, "application failed to start");
        eprintln!("AI Studio failed to start: {error}");
        std::process::exit(1);
    }
}

fn initialize_logging() {
    // TODO(DEV-M0): add persistent file logging under AppDataDirs::logs.
    let _ = tracing_subscriber::fmt().with_target(false).try_init();
}

fn run_application() -> Result<(), AppError> {
    tauri::Builder::default()
        .setup(|app| {
            let data_root = app
                .path()
                .local_data_dir()
                .map_err(|error| {
                    AppError::initialization(format!(
                        "failed to resolve local data directory: {error}"
                    ))
                })?
                .join("AIStudio")
                .join("AIStudioData");

            let data_dirs = AppDataDirs::initialize(data_root)?;
            tracing::info!(path = %data_dirs.root.display(), "data directory initialized");

            let database_pool =
                tauri::async_runtime::block_on(database::initialize(&data_dirs.database))?;
            app.manage(database_pool.clone());

            let clock: Arc<dyn Clock> = Arc::new(SystemClock);
            let task_repository: Arc<dyn TaskRepository> =
                Arc::new(SqliteTaskRepository::new(database_pool.clone()));
            let snapshot_repository: Arc<dyn GenerationSnapshotRepository> = Arc::new(
                SqliteGenerationSnapshotRepository::new(database_pool.clone()),
            );
            let definition_repository: Arc<dyn GenerationDefinitionRepository> = Arc::new(
                infrastructure::database::SqliteGenerationDefinitionRepository::new(
                    database_pool.clone(),
                ),
            );
            let project_repository: Arc<dyn ProjectRepository> =
                Arc::new(SqliteProjectRepository::new(database_pool.clone()));
            let asset_repository: Arc<dyn AssetRepository> =
                Arc::new(SqliteAssetRepository::new(database_pool.clone()));
            let asset_store: Arc<dyn AssetStore> = Arc::new(FileSystemAssetStore::new());

            let project_bootstrap =
                DefaultProjectBootstrap::new(project_repository.clone(), clock.clone());
            tauri::async_runtime::block_on(
                project_bootstrap.ensure_default_project(&data_dirs.projects),
            )
            .map_err(|error| AppError::initialization(error.to_string()))?;

            let workflow_library_repository: Arc<dyn WorkflowLibraryRepository> = Arc::new(
                infrastructure::database::SqliteWorkflowLibraryRepository::new(
                    database_pool.clone(),
                ),
            );
            let workflow_library_source: Arc<dyn WorkflowLibrarySource> = Arc::new(
                FileSystemWorkflowLibrarySource::new(data_dirs.workflow_library.clone()),
            );
            let workflow_library_service = Arc::new(WorkflowLibraryService::new(
                workflow_library_source,
                workflow_library_repository,
                clock.clone(),
            ));
            match tauri::async_runtime::block_on(workflow_library_service.sync()) {
                Ok(report) => tracing::info!(
                    packages = report.packages_found,
                    valid = report.valid,
                    invalid = report.invalid,
                    "runtime workflow library synchronized"
                ),
                Err(error) => tracing::warn!(
                    error = %error,
                    "runtime workflow library synchronization skipped"
                ),
            }

            let comfy_config = ComfyConnectionConfig::default();
            let comfy_adapter = ComfyHttpAdapter::new(comfy_config.clone()).map_err(|error| {
                AppError::initialization(format!("failed to create ComfyUI HTTP client: {error}"))
            })?;
            let comfy_adapter: Arc<dyn ComfyAdapter> = Arc::new(comfy_adapter);
            let comfy_service = Arc::new(ComfyService::new(comfy_adapter.clone(), &comfy_config));
            let task_update_sink: Arc<dyn application::ports::TaskUpdateSink> =
                Arc::new(TauriTaskUpdateSink::new(app.handle().clone()));
            let generation_service = Arc::new(
                GenerationService::new(
                    task_repository,
                    snapshot_repository,
                    definition_repository.clone(),
                    comfy_adapter,
                    project_repository,
                    asset_store.clone(),
                    asset_repository.clone(),
                    clock,
                )
                .with_task_update_sink(task_update_sink),
            );
            let generation_catalog_service =
                Arc::new(GenerationCatalogService::new(definition_repository));
            let task_query_service = Arc::new(TaskQueryService::new(
                Arc::new(SqliteTaskRepository::new(database_pool.clone())),
                asset_repository.clone(),
            ));
            let asset_query_service =
                Arc::new(AssetQueryService::new(asset_repository, asset_store));
            app.manage(AppState::new(
                data_dirs,
                comfy_service,
                generation_service,
                workflow_library_service,
                generation_catalog_service,
                task_query_service,
                asset_query_service,
            ));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::ping,
            commands::get_app_status,
            commands::comfy::comfy_get_status,
            commands::comfy::comfy_refresh_capabilities,
            commands::workflow_library::workflow_library_refresh,
            commands::catalog::generation_catalog_list,
            commands::generation::generation_create,
            commands::task::task_get,
            commands::task::task_list_recent,
            commands::asset::asset_list_by_task,
            commands::asset::asset_read_image
        ])
        .run(tauri::generate_context!())
        .map_err(|error| AppError::initialization(format!("Tauri runtime failed: {error}")))
}
