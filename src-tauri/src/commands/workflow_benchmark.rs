use crate::app_state::AppState;
use crate::application::generation_input_preparer::GenerationInputValue;
use crate::application::workflow_benchmark_service::{
    WorkflowBenchmarkCandidatePreviewView, WorkflowBenchmarkCandidateRequest,
    WorkflowBenchmarkCreateRequest, WorkflowBenchmarkDeleteView, WorkflowBenchmarkError,
    WorkflowBenchmarkSummaryView, WorkflowBenchmarkView,
};
use crate::commands::generation::InputValueDto;
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowBenchmarkCandidateRequestDto {
    pub workflow_version_id: String,
    pub recipe_id: String,
    #[serde(default)]
    pub preset_id: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowBenchmarkCreateRequestDto {
    pub project_id: String,
    pub name: String,
    pub media_type: String,
    pub base_values: BTreeMap<String, InputValueDto>,
    pub candidates: Vec<WorkflowBenchmarkCandidateRequestDto>,
    #[serde(default = "default_seed_mode")]
    pub seed_mode: String,
    #[serde(default)]
    pub fixed_seed: Option<String>,
    #[serde(default)]
    pub auto_start: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowBenchmarkListRequest {
    pub project_id: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowBenchmarkWinnerRequest {
    pub project_id: String,
    pub experiment_id: String,
    #[serde(default)]
    pub candidate_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowBenchmarkCloneRequest {
    pub project_id: String,
    pub experiment_id: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowBenchmarkQueueExistingRequest {
    pub project_id: String,
    pub experiment_id: String,
    #[serde(default)]
    pub auto_start: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowBenchmarkPreviewResponse {
    pub candidates: Vec<WorkflowBenchmarkCandidatePreviewView>,
}

fn default_seed_mode() -> String {
    "FIXED".to_owned()
}

fn default_limit() -> u32 {
    20
}

impl WorkflowBenchmarkCreateRequestDto {
    fn into_application(self) -> Result<WorkflowBenchmarkCreateRequest, AppError> {
        super::validate_project_id(&self.project_id)?;
        let base_values = self
            .base_values
            .into_iter()
            .map(|(key, value)| value.into_application(&key).map(|value| (key, value)))
            .collect::<Result<BTreeMap<String, GenerationInputValue>, AppError>>()?;
        let fixed_seed = self
            .fixed_seed
            .as_deref()
            .map(|value| {
                value
                    .parse::<u64>()
                    .map_err(|_| AppError::invalid_input("固定 Seed 必须是十进制字符串。"))
            })
            .transpose()?;
        Ok(WorkflowBenchmarkCreateRequest {
            project_id: self.project_id,
            name: self.name,
            media_type: self.media_type.to_ascii_uppercase(),
            base_values,
            candidates: self
                .candidates
                .into_iter()
                .map(|candidate| WorkflowBenchmarkCandidateRequest {
                    workflow_version_id: candidate.workflow_version_id,
                    recipe_id: candidate.recipe_id,
                    preset_id: candidate.preset_id,
                    label: candidate.label,
                })
                .collect(),
            seed_mode: self.seed_mode.to_ascii_uppercase(),
            fixed_seed,
            auto_start: self.auto_start,
        })
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_benchmark_preview(
    state: State<'_, AppState>,
    request: WorkflowBenchmarkCreateRequestDto,
) -> Result<WorkflowBenchmarkPreviewResponse, AppError> {
    let request = request.into_application()?;
    state
        .workflow_benchmark_service
        .preview(&request)
        .await
        .map(|candidates| WorkflowBenchmarkPreviewResponse { candidates })
        .map_err(map_benchmark_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_benchmark_create(
    state: State<'_, AppState>,
    request: WorkflowBenchmarkCreateRequestDto,
) -> Result<WorkflowBenchmarkView, AppError> {
    let request = request.into_application()?;
    state
        .workflow_benchmark_service
        .create(request)
        .await
        .map_err(map_benchmark_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_benchmark_list(
    state: State<'_, AppState>,
    request: WorkflowBenchmarkListRequest,
) -> Result<Vec<WorkflowBenchmarkSummaryView>, AppError> {
    super::validate_project_id(&request.project_id)?;
    state
        .workflow_benchmark_service
        .list(&request.project_id, request.limit)
        .await
        .map_err(map_benchmark_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_benchmark_get(
    state: State<'_, AppState>,
    project_id: String,
    experiment_id: String,
) -> Result<WorkflowBenchmarkView, AppError> {
    super::validate_project_id(&project_id)?;
    state
        .workflow_benchmark_service
        .get(&project_id, &experiment_id)
        .await
        .map_err(map_benchmark_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_benchmark_set_winner(
    state: State<'_, AppState>,
    request: WorkflowBenchmarkWinnerRequest,
) -> Result<WorkflowBenchmarkView, AppError> {
    super::validate_project_id(&request.project_id)?;
    state
        .workflow_benchmark_service
        .set_winner(
            &request.project_id,
            &request.experiment_id,
            request.candidate_id.as_deref(),
        )
        .await
        .map_err(map_benchmark_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_benchmark_clone(
    state: State<'_, AppState>,
    request: WorkflowBenchmarkCloneRequest,
) -> Result<WorkflowBenchmarkView, AppError> {
    super::validate_project_id(&request.project_id)?;
    state
        .workflow_benchmark_service
        .clone_experiment(&request.project_id, &request.experiment_id, request.name)
        .await
        .map_err(map_benchmark_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_benchmark_queue_existing(
    state: State<'_, AppState>,
    request: WorkflowBenchmarkQueueExistingRequest,
) -> Result<WorkflowBenchmarkView, AppError> {
    super::validate_project_id(&request.project_id)?;
    state
        .workflow_benchmark_service
        .queue_existing(
            &request.project_id,
            &request.experiment_id,
            request.auto_start,
        )
        .await
        .map_err(map_benchmark_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_benchmark_delete(
    state: State<'_, AppState>,
    project_id: String,
    experiment_id: String,
) -> Result<WorkflowBenchmarkDeleteView, AppError> {
    super::validate_project_id(&project_id)?;
    state
        .workflow_benchmark_service
        .delete(&project_id, &experiment_id)
        .await
        .map_err(map_benchmark_error)
}

fn map_benchmark_error(error: WorkflowBenchmarkError) -> AppError {
    match error {
        WorkflowBenchmarkError::InvalidInput(message)
        | WorkflowBenchmarkError::NotFound(message)
        | WorkflowBenchmarkError::InvalidRecipe(message)
        | WorkflowBenchmarkError::Serialization(message)
        | WorkflowBenchmarkError::Queue(message) => AppError::invalid_input(message),
        WorkflowBenchmarkError::Repository(error) => super::map_repository_error(&error),
    }
}
