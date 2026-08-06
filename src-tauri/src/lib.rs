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
};
pub use infrastructure::database::{
    SqliteAssetRepository, SqliteGenerationDefinitionRepository,
    SqliteGenerationSnapshotRepository, SqliteProjectRepository, SqliteTaskRepository,
};

use app_state::AppState;
use application::{
    comfy_service::ComfyService,
    generation_service::GenerationService,
    ports::{ComfyAdapter, ComfyConnectionConfig},
};
use error::AppError;
use infrastructure::{
    comfy::ComfyHttpAdapter,
    database,
    filesystem::{AppDataDirs, FileSystemAssetStore},
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

            let comfy_config = ComfyConnectionConfig::default();
            let comfy_adapter = ComfyHttpAdapter::new(comfy_config.clone()).map_err(|error| {
                AppError::initialization(format!("failed to create ComfyUI HTTP client: {error}"))
            })?;
            let comfy_adapter: Arc<dyn ComfyAdapter> = Arc::new(comfy_adapter);
            let comfy_service = Arc::new(ComfyService::new(comfy_adapter.clone(), &comfy_config));
            let task_repository: Arc<dyn TaskRepository> =
                Arc::new(SqliteTaskRepository::new(database_pool.clone()));
            let snapshot_repository: Arc<dyn GenerationSnapshotRepository> = Arc::new(
                SqliteGenerationSnapshotRepository::new(database_pool.clone()),
            );
            let definition_repository = Arc::new(
                infrastructure::database::SqliteGenerationDefinitionRepository::new(
                    database_pool.clone(),
                ),
            );
            let project_repository: Arc<dyn ProjectRepository> =
                Arc::new(SqliteProjectRepository::new(database_pool.clone()));
            let asset_repository: Arc<dyn AssetRepository> =
                Arc::new(SqliteAssetRepository::new(database_pool));
            let asset_store: Arc<dyn AssetStore> = Arc::new(FileSystemAssetStore::new());
            let clock: Arc<dyn Clock> = Arc::new(SystemClock);
            let generation_service = Arc::new(GenerationService::new(
                task_repository,
                snapshot_repository,
                definition_repository,
                comfy_adapter,
                project_repository,
                asset_store,
                asset_repository,
                clock,
            ));
            app.manage(AppState::new(data_dirs, comfy_service, generation_service));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::ping,
            commands::get_app_status,
            commands::comfy::comfy_get_status,
            commands::comfy::comfy_refresh_capabilities
        ])
        .run(tauri::generate_context!())
        .map_err(|error| AppError::initialization(format!("Tauri runtime failed: {error}")))
}
