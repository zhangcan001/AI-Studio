mod app_state;
mod application;
mod commands;
pub mod compiler;
pub mod domain;
mod error;
mod infrastructure;

pub use application::ports::{GenerationSnapshotRepository, RepositoryError, TaskRepository};
pub use infrastructure::database::{SqliteGenerationSnapshotRepository, SqliteTaskRepository};

use app_state::AppState;
use application::{comfy_service::ComfyService, ports::ComfyConnectionConfig};
use error::AppError;
use infrastructure::{comfy::ComfyHttpAdapter, database, filesystem::AppDataDirs};
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
            app.manage(database_pool);

            let comfy_config = ComfyConnectionConfig::default();
            let comfy_adapter = ComfyHttpAdapter::new(comfy_config.clone()).map_err(|error| {
                AppError::initialization(format!("failed to create ComfyUI HTTP client: {error}"))
            })?;
            let comfy_service = Arc::new(ComfyService::new(Arc::new(comfy_adapter), &comfy_config));
            app.manage(AppState::new(data_dirs, comfy_service));

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
