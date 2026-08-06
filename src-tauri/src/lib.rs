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
    asset_library_service::AssetLibraryService,
    asset_query_service::AssetQueryService,
    comfy_service::ComfyService,
    generation_catalog_service::GenerationCatalogService,
    generation_service::GenerationService,
    ports::{ComfyAdapter, ComfyConnectionConfig, WorkflowLibrarySource},
    project_bootstrap::DefaultProjectBootstrap,
    source_asset_import_service::SourceAssetImportService,
    task_cancellation_service::TaskCancellationService,
    task_execution_registry::TaskExecutionRegistry,
    task_history_service::TaskHistoryService,
    task_query_service::TaskQueryService,
    task_recovery_service::TaskRecoveryService,
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
        .plugin(tauri_plugin_dialog::init())
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
            let task_history_repository: Arc<dyn application::ports::TaskHistoryRepository> =
                Arc::new(infrastructure::database::SqliteTaskHistoryRepository::new(
                    database_pool.clone(),
                ));
            let asset_browse_repository: Arc<dyn application::ports::AssetBrowseRepository> =
                Arc::new(infrastructure::database::SqliteAssetBrowseRepository::new(
                    database_pool.clone(),
                ));
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
            let execution_registry = TaskExecutionRegistry::default();
            let generation_service = Arc::new(
                GenerationService::new(
                    task_repository.clone(),
                    snapshot_repository.clone(),
                    definition_repository.clone(),
                    comfy_adapter.clone(),
                    project_repository.clone(),
                    asset_store.clone(),
                    asset_repository.clone(),
                    clock.clone(),
                )
                .with_task_update_sink(task_update_sink.clone())
                .with_execution_registry(execution_registry.clone()),
            );
            let generation_catalog_service =
                Arc::new(GenerationCatalogService::new(definition_repository.clone()));
            let task_query_service = Arc::new(TaskQueryService::new(
                Arc::new(SqliteTaskRepository::new(database_pool.clone())),
                asset_repository.clone(),
            ));
            let asset_query_service = Arc::new(AssetQueryService::new(
                asset_repository.clone(),
                asset_store.clone(),
            ));
            let asset_library_service = Arc::new(AssetLibraryService::new(asset_browse_repository));
            let task_history_service = Arc::new(TaskHistoryService::new(
                task_history_repository,
                snapshot_repository.clone(),
                definition_repository.clone(),
                asset_repository.clone(),
            ));
            let source_asset_import_service = Arc::new(SourceAssetImportService::new(
                project_repository.clone(),
                asset_store.clone(),
                asset_repository.clone(),
                clock.clone(),
            ));
            let task_cancellation_service = Arc::new(TaskCancellationService::new(
                task_repository.clone(),
                execution_registry,
                clock.clone(),
                task_update_sink.clone(),
            ));
            let task_recovery_service = Arc::new(TaskRecoveryService::new(
                task_repository,
                snapshot_repository,
                asset_repository,
                comfy_adapter,
                project_repository,
                asset_store,
                clock,
                task_update_sink,
            ));
            let startup_recovery = task_recovery_service.clone();
            app.manage(AppState::new(
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
            ));

            tauri::async_runtime::spawn(async move {
                match startup_recovery.reconcile_active().await {
                    Ok(report) => tracing::info!(
                        examined = report.examined,
                        succeeded = report.succeeded,
                        failed = report.failed,
                        deferred = report.deferred,
                        unresolved = report.unresolved,
                        "startup task recovery completed"
                    ),
                    Err(error) => tracing::warn!(error = %error, "startup task recovery failed"),
                }
            });

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
            commands::task::task_cancel,
            commands::task::task_reconcile_active,
            commands::task::task_history_page,
            commands::task::task_get_detail,
            commands::task::task_get_reusable_draft,
            commands::asset::asset_list_by_task,
            commands::asset::asset_list_recent,
            commands::asset::asset_pick_and_import_image,
            commands::asset::asset_read_image,
            commands::asset::asset_library_page,
            commands::asset::asset_get
        ])
        .run(tauri::generate_context!())
        .map_err(|error| AppError::initialization(format!("Tauri runtime failed: {error}")))
}
