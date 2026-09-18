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
        .organization
        .organization
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
        .organization
        .organization
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
        .organization
        .organization
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
        .organization
        .organization
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
        .organization
        .organization
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
        .organization
        .organization
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
        .organization
        .organization
        .set_favorite(&project_id, &asset_id, favorite)
        .await
        .map_err(map_organization_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn asset_bulk_set_favorite(
    state: State<'_, AppState>,
    project_id: String,
    asset_ids: Vec<String>,
    favorite: bool,
) -> Result<(), AppError> {
    state
        .organization
        .organization
        .bulk_set_favorite(&project_id, &asset_ids, favorite)
        .await
        .map_err(map_organization_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn asset_bulk_add_tag(
    state: State<'_, AppState>,
    project_id: String,
    asset_ids: Vec<String>,
    tag_id: String,
) -> Result<(), AppError> {
    state
        .organization
        .organization
        .bulk_add_tag(&project_id, &asset_ids, &tag_id)
        .await
        .map_err(map_organization_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn asset_bulk_remove_tag(
    state: State<'_, AppState>,
    project_id: String,
    asset_ids: Vec<String>,
    tag_id: String,
) -> Result<(), AppError> {
    state
        .organization
        .organization
        .bulk_remove_tag(&project_id, &asset_ids, &tag_id)
        .await
        .map_err(map_organization_error)
}

#[tauri::command]
pub async fn project_template_list(
    state: State<'_, AppState>,
) -> Result<Vec<crate::application::ports::ProjectTemplate>, AppError> {
    state
        .organization
        .project_template
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
        .organization
        .project_template
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
        .organization
        .project_template
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
        .organization
        .project_template
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
        .organization
        .project_template
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
