pub mod comfy;

use crate::{app_state::AppState, error::AppError};
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct AppStatus {
    pub backend: &'static str,
    pub database: &'static str,
    pub data_root: String,
    pub version: &'static str,
}

#[tauri::command]
pub fn ping() -> Result<&'static str, AppError> {
    Ok("pong")
}

#[tauri::command]
pub fn get_app_status(state: State<'_, AppState>) -> Result<AppStatus, AppError> {
    if !state.data_dirs.root.is_dir() || !state.data_dirs.database.is_file() {
        return Err(AppError::internal(
            "application data directory or database is not ready",
        ));
    }

    Ok(AppStatus {
        backend: "ready",
        database: "ready",
        data_root: state.data_dirs.root.display().to_string(),
        version: env!("CARGO_PKG_VERSION"),
    })
}
