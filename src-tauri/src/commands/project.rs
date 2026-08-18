use crate::{
    app_state::AppState,
    application::project_service::{ProjectServiceError, ProjectView},
    error::AppError,
};
use std::path::PathBuf;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command(rename_all = "camelCase")]
pub async fn project_list(state: State<'_, AppState>) -> Result<Vec<ProjectView>, AppError> {
    state
        .project_service
        .list()
        .await
        .map_err(map_project_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn project_create(
    state: State<'_, AppState>,
    name: String,
    description: Option<String>,
) -> Result<ProjectView, AppError> {
    state
        .project_service
        .create(&name, description.as_deref())
        .await
        .map_err(map_project_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn project_update(
    state: State<'_, AppState>,
    project_id: String,
    name: String,
    description: Option<String>,
) -> Result<ProjectView, AppError> {
    super::validate_project_id(&project_id)?;
    state
        .project_service
        .update(&project_id, &name, description.as_deref())
        .await
        .map_err(map_project_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn project_backup_export(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Option<crate::application::project_backup_service::ProjectBackupExportView>, AppError> {
    super::validate_project_id(&project_id)?;
    let Some(file) = app_handle
        .dialog()
        .file()
        .add_filter("AI Studio 项目备份", &["zip"])
        .set_file_name("AI-Studio-Project-Backup.zip")
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let destination = file
        .into_path()
        .map_err(|_| AppError::filesystem("备份保存位置不可用"))?;
    state
        .project_backup_service
        .export(&project_id, destination)
        .await
        .map(Some)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn project_backup_inspect(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<crate::application::project_backup_service::ProjectBackupPreviewView>, AppError>
{
    let Some(file) = app_handle
        .dialog()
        .file()
        .add_filter("AI Studio 项目备份", &["zip"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let source = file
        .into_path()
        .map_err(|_| AppError::filesystem("备份文件位置不可用"))?;
    state.project_backup_service.inspect(source).await.map(Some)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn project_backup_restore(
    state: State<'_, AppState>,
    inspection_id: String,
) -> Result<crate::application::project_backup_service::RestoredProjectView, AppError> {
    state.project_backup_service.restore(&inspection_id).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn project_manifest_export(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    destination: Option<PathBuf>,
) -> Result<Option<crate::application::project_manifest_service::ProjectManifestExportView>, AppError>
{
    super::validate_project_id(&project_id)?;
    let destination = match destination {
        Some(path) => path,
        None => {
            let Some(file) = app_handle
                .dialog()
                .file()
                .add_filter("AI Studio 项目清单", &["json"])
                .set_file_name("AI-Studio-Project-Manifest.json")
                .blocking_save_file()
            else {
                return Ok(None);
            };
            file.into_path()
                .map_err(|_| AppError::filesystem("清单保存位置不可用"))?
        }
    };
    state
        .project_manifest_service
        .export(&project_id, destination)
        .await
        .map(Some)
}

pub(crate) fn map_project_error(error: ProjectServiceError) -> AppError {
    match error {
        ProjectServiceError::InvalidName(message)
        | ProjectServiceError::InvalidDescription(message)
        | ProjectServiceError::InvalidProjectId(message) => AppError::invalid_input(message),
        ProjectServiceError::NotFound(project_id) => {
            AppError::project_not_found(format!("project {project_id} was not found"))
        }
        ProjectServiceError::Repository(error) => super::map_repository_error(&error),
        ProjectServiceError::Directory(error) => AppError::filesystem(error.to_string()),
        ProjectServiceError::Compensation {
            repository,
            cleanup,
        } => AppError::filesystem(format!(
            "project creation failed: {repository}; cleanup failed: {cleanup}"
        )),
    }
}
