use crate::app_state::AppState;
use crate::application::prompt_template_bulk_service::{
    PromptTemplateApplyInput, PromptTemplateApplyResult, PromptTemplateBulkError,
    PromptTemplateBulkPreview, PromptTemplateBulkPreviewInput, PromptTemplatePreview,
    PromptTemplatePreviewInput,
};
use crate::application::prompt_template_service::{PromptTemplateError, PromptTemplateService};
use crate::domain::ShotStage;
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplatePreviewRequest {
    pub project_id: String,
    pub prompt_entry_id: String,
    pub prompt_version_id: String,
    pub shot_id: String,
    #[serde(default)]
    pub context_anchor_ids: Vec<String>,
    #[serde(default)]
    pub custom_values: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplateBulkPreviewRequest {
    pub project_id: String,
    pub prompt_entry_id: String,
    pub prompt_version_id: String,
    pub shot_ids: Vec<String>,
    #[serde(default)]
    pub context_anchor_ids: Vec<String>,
    #[serde(default)]
    pub custom_values: BTreeMap<String, String>,
    pub preview_limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplateApplyRequest {
    pub project_id: String,
    pub prompt_entry_id: String,
    pub prompt_version_id: String,
    pub stage: String,
    pub shot_ids: Vec<String>,
    #[serde(default)]
    pub context_anchor_ids: Vec<String>,
    #[serde(default)]
    pub custom_values: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplateAnalysisView {
    pub is_template: bool,
    pub variables: Vec<String>,
    pub builtin_variables: Vec<String>,
    pub custom_variables: Vec<String>,
    pub requires_structure: bool,
}

#[tauri::command(rename_all = "camelCase")]
pub fn prompt_template_analyze(
    state: State<'_, AppState>,
    text: String,
) -> Result<PromptTemplateAnalysisView, AppError> {
    let analysis = state
        .prompt_template_service
        .analyze(&text)
        .map_err(map_template_error)?;
    Ok(PromptTemplateAnalysisView {
        is_template: analysis.is_template,
        variables: analysis.variables,
        builtin_variables: analysis.builtin_variables,
        custom_variables: analysis.custom_variables,
        requires_structure: analysis.requires_structure,
    })
}

#[tauri::command(rename_all = "camelCase")]
pub async fn prompt_template_preview(
    state: State<'_, AppState>,
    request: PromptTemplatePreviewRequest,
) -> Result<PromptTemplatePreview, AppError> {
    state
        .prompt_template_bulk_service
        .preview(PromptTemplatePreviewInput {
            project_id: request.project_id,
            prompt_entry_id: request.prompt_entry_id,
            prompt_version_id: request.prompt_version_id,
            shot_id: request.shot_id,
            context_anchor_ids: request.context_anchor_ids,
            custom_values: request.custom_values,
        })
        .await
        .map_err(map_bulk_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn prompt_template_bulk_preview(
    state: State<'_, AppState>,
    request: PromptTemplateBulkPreviewRequest,
) -> Result<PromptTemplateBulkPreview, AppError> {
    state
        .prompt_template_bulk_service
        .preview_bulk(PromptTemplateBulkPreviewInput {
            project_id: request.project_id,
            prompt_entry_id: request.prompt_entry_id,
            prompt_version_id: request.prompt_version_id,
            shot_ids: request.shot_ids,
            context_anchor_ids: request.context_anchor_ids,
            custom_values: request.custom_values,
            preview_limit: request.preview_limit,
        })
        .await
        .map_err(map_bulk_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn prompt_template_apply(
    state: State<'_, AppState>,
    request: PromptTemplateApplyRequest,
) -> Result<PromptTemplateApplyResult, AppError> {
    let stage = ShotStage::try_from_str(request.stage.trim()).map_err(|error| {
        AppError::invalid_input(format!("PROMPT_TEMPLATE_STAGE_INVALID: {error}"))
    })?;
    state
        .prompt_template_bulk_service
        .apply(PromptTemplateApplyInput {
            project_id: request.project_id,
            prompt_entry_id: request.prompt_entry_id,
            prompt_version_id: request.prompt_version_id,
            stage,
            shot_ids: request.shot_ids,
            context_anchor_ids: request.context_anchor_ids,
            custom_values: request.custom_values,
        })
        .await
        .map_err(map_bulk_error)
}

fn map_template_error(error: PromptTemplateError) -> AppError {
    AppError::invalid_input(error.to_string())
}

fn map_bulk_error(error: PromptTemplateBulkError) -> AppError {
    match error {
        PromptTemplateBulkError::Repository(repository) => super::map_repository_error(&repository),
        other => AppError::invalid_input(other.to_string()),
    }
}

#[allow(dead_code)]
fn _service_type_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<PromptTemplateService>();
}
