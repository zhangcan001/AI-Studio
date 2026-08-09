use crate::{
    app_state::AppState,
    application::{
        organization_service::OrganizationError,
        ports::AssetTag,
        project_template_service::{
            CreateProjectTemplate, ProjectTemplateError, TemplateProjectResult,
        },
    },
    error::AppError,
};
use tauri::State;

#[tauri::command(rename_all = "camelCase")]
pub async fn asset_tag_list(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<AssetTag>, AppError> {
    state
        .organization_service
        .list_tags(&project_id)
        .await
        .map_err(map_organization_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn asset_tag_create(
    state: State<'_, AppState>,
    project_id: String,
    name: String,
) -> Result<AssetTag, AppError> {
    state
        .organization_service
        .create_tag(&project_id, &name)
        .await
        .map_err(map_organization_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn asset_tag_rename(
    state: State<'_, AppState>,
    project_id: String,
    tag_id: String,
    name: String,
) -> Result<AssetTag, AppError> {
    state
        .organization_service
        .rename_tag(&project_id, &tag_id, &name)
        .await
        .map_err(map_organization_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn asset_tag_delete(
    state: State<'_, AppState>,
    project_id: String,
    tag_id: String,
) -> Result<(), AppError> {
    state
        .organization_service
        .delete_tag(&project_id, &tag_id)
        .await
        .map_err(map_organization_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn asset_tag_assign(
    state: State<'_, AppState>,
    project_id: String,
    asset_id: String,
    tag_id: String,
) -> Result<(), AppError> {
    state
        .organization_service
        .assign_tag(&project_id, &asset_id, &tag_id)
        .await
        .map_err(map_organization_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn asset_tag_remove(
    state: State<'_, AppState>,
    project_id: String,
    asset_id: String,
    tag_id: String,
) -> Result<(), AppError> {
    state
        .organization_service
        .remove_tag(&project_id, &asset_id, &tag_id)
        .await
        .map_err(map_organization_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn asset_set_favorite(
    state: State<'_, AppState>,
    project_id: String,
    asset_id: String,
    favorite: bool,
) -> Result<(), AppError> {
    state
        .organization_service
        .set_favorite(&project_id, &asset_id, favorite)
        .await
        .map_err(map_organization_error)
}

#[tauri::command]
pub async fn project_template_list(
    state: State<'_, AppState>,
) -> Result<Vec<crate::application::ports::ProjectTemplate>, AppError> {
    state
        .project_template_service
        .list()
        .await
        .map_err(map_template_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn project_template_create(
    state: State<'_, AppState>,
    request: CreateProjectTemplate,
) -> Result<crate::application::ports::ProjectTemplate, AppError> {
    state
        .project_template_service
        .create(request)
        .await
        .map_err(map_template_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn project_template_update(
    state: State<'_, AppState>,
    template_id: String,
    name: String,
    description: Option<String>,
) -> Result<crate::application::ports::ProjectTemplate, AppError> {
    state
        .project_template_service
        .update(&template_id, &name, description.as_deref())
        .await
        .map_err(map_template_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn project_template_delete(
    state: State<'_, AppState>,
    template_id: String,
) -> Result<(), AppError> {
    state
        .project_template_service
        .delete(&template_id)
        .await
        .map_err(map_template_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn project_template_create_project(
    state: State<'_, AppState>,
    template_id: String,
    name: String,
    description: Option<String>,
) -> Result<TemplateProjectResult, AppError> {
    state
        .project_template_service
        .create_project(&template_id, &name, description.as_deref())
        .await
        .map_err(map_template_error)
}

fn map_organization_error(error: OrganizationError) -> AppError {
    match error {
        OrganizationError::InvalidInput(message) => AppError::invalid_input(message),
        OrganizationError::NotFound(message) => AppError::asset_not_found(message),
        OrganizationError::Repository(repository) => super::map_repository_error(&repository),
    }
}

fn map_template_error(error: ProjectTemplateError) -> AppError {
    match error {
        ProjectTemplateError::InvalidInput(message)
        | ProjectTemplateError::Unavailable(message) => AppError::invalid_input(message),
        ProjectTemplateError::NotFound(message) => AppError::project_not_found(message),
        ProjectTemplateError::Organization(error) => map_organization_error(error),
        ProjectTemplateError::Repository(error) => super::map_repository_error(&error),
        ProjectTemplateError::Project(error) => crate::commands::project::map_project_error(error),
    }
}
