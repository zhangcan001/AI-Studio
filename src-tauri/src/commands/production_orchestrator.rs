use crate::app_state::AppState;
use crate::application::generation_input_preparer::GenerationInputValue;
use crate::application::production_orchestrator_service::{
    ProductionOrchestratorError, ProductionOrchestratorService, ProductionRunCreateRequest,
    ProductionRunListItem, ProductionRunTemplateRequest, ProductionRunTemplateView,
    ProductionRunView,
};
use crate::commands::generation::InputValueDto;
use crate::error::AppError;
use serde::Deserialize;
use std::collections::BTreeMap;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionRunCreateRequestDto {
    pub project_id: String,
    pub name: String,
    pub krea2_workflow_version_id: String,
    pub krea2_recipe_id: String,
    #[serde(default)]
    pub krea2_preset_id: Option<String>,
    #[serde(default)]
    pub krea2_values: BTreeMap<String, InputValueDto>,
    #[serde(default = "default_image_count")]
    pub image_count: u32,
    #[serde(default)]
    pub h3_workflow_version_id: Option<String>,
    #[serde(default)]
    pub h3_recipe_id: Option<String>,
    #[serde(default)]
    pub h3_profile: Option<String>,
    #[serde(default)]
    pub h3_values: BTreeMap<String, InputValueDto>,
    #[serde(default)]
    pub template_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionRunListRequest {
    pub project_id: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionRunAssetSelectionRequest {
    pub project_id: String,
    pub run_id: String,
    pub asset_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionRunTemplateRequestDto {
    pub project_id: String,
    pub name: String,
    #[serde(default)]
    pub krea2_workflow_version_id: Option<String>,
    #[serde(default)]
    pub krea2_recipe_id: Option<String>,
    #[serde(default)]
    pub krea2_preset_id: Option<String>,
    #[serde(default = "default_image_count")]
    pub default_image_count: u32,
    #[serde(default)]
    pub h3_workflow_version_id: Option<String>,
    #[serde(default)]
    pub h3_recipe_id: Option<String>,
    #[serde(default)]
    pub h3_profile: Option<String>,
    #[serde(default)]
    pub default_duration_seconds: Option<u32>,
    #[serde(default)]
    pub default_width: Option<u32>,
    #[serde(default)]
    pub default_height: Option<u32>,
}

fn default_image_count() -> u32 {
    1
}

fn default_limit() -> u32 {
    20
}

fn convert_values(
    values: BTreeMap<String, InputValueDto>,
) -> Result<BTreeMap<String, GenerationInputValue>, AppError> {
    values
        .into_iter()
        .map(|(key, value)| value.into_application(&key).map(|value| (key, value)))
        .collect()
}

impl ProductionRunCreateRequestDto {
    fn into_application(self) -> Result<ProductionRunCreateRequest, AppError> {
        Ok(ProductionRunCreateRequest {
            project_id: self.project_id,
            name: self.name,
            krea2_workflow_version_id: self.krea2_workflow_version_id,
            krea2_recipe_id: self.krea2_recipe_id,
            krea2_preset_id: self.krea2_preset_id,
            krea2_values: convert_values(self.krea2_values)?,
            image_count: self.image_count,
            h3_workflow_version_id: self.h3_workflow_version_id,
            h3_recipe_id: self.h3_recipe_id,
            h3_profile: self.h3_profile,
            h3_values: convert_values(self.h3_values)?,
            template_id: self.template_id,
        })
    }
}

impl ProductionRunTemplateRequestDto {
    fn into_application(self) -> ProductionRunTemplateRequest {
        ProductionRunTemplateRequest {
            project_id: self.project_id,
            name: self.name,
            krea2_workflow_version_id: self.krea2_workflow_version_id,
            krea2_recipe_id: self.krea2_recipe_id,
            krea2_preset_id: self.krea2_preset_id,
            default_image_count: self.default_image_count,
            h3_workflow_version_id: self.h3_workflow_version_id,
            h3_recipe_id: self.h3_recipe_id,
            h3_profile: self.h3_profile,
            default_duration_seconds: self.default_duration_seconds,
            default_width: self.default_width,
            default_height: self.default_height,
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_run_create(
    state: State<'_, AppState>,
    request: ProductionRunCreateRequestDto,
) -> Result<ProductionRunView, AppError> {
    state
        .production_orchestrator_service
        .create(request.into_application()?)
        .await
        .map_err(map_production_run_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_run_list(
    state: State<'_, AppState>,
    request: ProductionRunListRequest,
) -> Result<Vec<ProductionRunListItem>, AppError> {
    state
        .production_orchestrator_service
        .list(&request.project_id, request.limit)
        .await
        .map_err(map_production_run_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_run_get(
    state: State<'_, AppState>,
    project_id: String,
    run_id: String,
) -> Result<ProductionRunView, AppError> {
    state
        .production_orchestrator_service
        .get(&project_id, &run_id)
        .await
        .map_err(map_production_run_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_run_run_images(
    state: State<'_, AppState>,
    project_id: String,
    run_id: String,
) -> Result<ProductionRunView, AppError> {
    state
        .production_orchestrator_service
        .run_images(&project_id, &run_id)
        .await
        .map_err(map_production_run_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_run_select_assets(
    state: State<'_, AppState>,
    request: ProductionRunAssetSelectionRequest,
) -> Result<ProductionRunView, AppError> {
    state
        .production_orchestrator_service
        .select_assets(&request.project_id, &request.run_id, request.asset_ids)
        .await
        .map_err(map_production_run_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_run_run_video(
    state: State<'_, AppState>,
    project_id: String,
    run_id: String,
) -> Result<ProductionRunView, AppError> {
    state
        .production_orchestrator_service
        .run_video(&project_id, &run_id)
        .await
        .map_err(map_production_run_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_run_retry_video(
    state: State<'_, AppState>,
    project_id: String,
    run_id: String,
) -> Result<ProductionRunView, AppError> {
    state
        .production_orchestrator_service
        .retry_video(&project_id, &run_id)
        .await
        .map_err(map_production_run_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_run_refresh(
    state: State<'_, AppState>,
    project_id: String,
    run_id: String,
) -> Result<ProductionRunView, AppError> {
    state
        .production_orchestrator_service
        .refresh(&project_id, &run_id)
        .await
        .map_err(map_production_run_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_run_cancel(
    state: State<'_, AppState>,
    project_id: String,
    run_id: String,
) -> Result<ProductionRunView, AppError> {
    state
        .production_orchestrator_service
        .cancel(&project_id, &run_id)
        .await
        .map_err(map_production_run_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_run_template_save(
    state: State<'_, AppState>,
    request: ProductionRunTemplateRequestDto,
) -> Result<ProductionRunTemplateView, AppError> {
    state
        .production_orchestrator_service
        .save_template(request.into_application())
        .await
        .map_err(map_production_run_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_run_template_list(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<ProductionRunTemplateView>, AppError> {
    state
        .production_orchestrator_service
        .list_templates(&project_id)
        .await
        .map_err(map_production_run_error)
}

fn map_production_run_error(error: ProductionOrchestratorError) -> AppError {
    match error {
        ProductionOrchestratorError::Repository(message) => AppError::database(message),
        ProductionOrchestratorError::InvalidInput(message)
        | ProductionOrchestratorError::InvalidState(message)
        | ProductionOrchestratorError::NotFound(message)
        | ProductionOrchestratorError::Queue(message) => AppError::invalid_input(message),
    }
}

#[allow(dead_code)]
fn _service_type_is_kept_reachable(_: &ProductionOrchestratorService) {}
