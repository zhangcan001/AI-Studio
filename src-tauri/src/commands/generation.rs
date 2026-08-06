use crate::{
    app_state::AppState,
    application::{
        generation_service::{CreateGenerationRequest, GenerationServiceError},
        task_query_service::TaskView,
    },
    domain::{InputValue, SeedValue},
    error::AppError,
};
use serde::Deserialize;
use std::collections::BTreeMap;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationCreateRequest {
    pub project_id: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub values: BTreeMap<String, InputValueDto>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum InputValueDto {
    #[serde(rename = "string")]
    String { value: String },
    #[serde(rename = "integer")]
    Integer { value: i64 },
    #[serde(rename = "seed_random")]
    SeedRandom,
    #[serde(rename = "seed_fixed")]
    SeedFixed { value: String },
}

impl GenerationCreateRequest {
    fn into_application(self) -> Result<CreateGenerationRequest, AppError> {
        let values = self
            .values
            .into_iter()
            .map(|(key, value)| {
                let value = match value {
                    InputValueDto::String { value } => InputValue::String(value),
                    InputValueDto::Integer { value } => InputValue::Integer(value),
                    InputValueDto::SeedRandom => InputValue::Seed(SeedValue::Random),
                    InputValueDto::SeedFixed { value } => {
                        let seed = value.parse::<u64>().map_err(|_| {
                            AppError::invalid_input(format!(
                                "seed value for {key} must be a decimal u64 string"
                            ))
                        })?;
                        InputValue::Seed(SeedValue::Fixed(seed))
                    }
                };
                Ok((key, value))
            })
            .collect::<Result<BTreeMap<_, _>, AppError>>()?;

        Ok(CreateGenerationRequest {
            project_id: self.project_id,
            workflow_version_id: self.workflow_version_id,
            recipe_id: self.recipe_id,
            values,
        })
    }
}

#[tauri::command]
pub async fn generation_create(
    state: State<'_, AppState>,
    request: GenerationCreateRequest,
) -> Result<TaskView, AppError> {
    let request = request.into_application()?;
    let task = state
        .generation_service
        .start_generation(request)
        .await
        .map_err(map_generation_error)?;
    state
        .task_query_service
        .view(task)
        .await
        .map_err(|error| AppError::internal(error.to_string()))
}

fn map_generation_error(error: GenerationServiceError) -> AppError {
    match &error {
        GenerationServiceError::DefinitionNotFound { .. } => {
            AppError::generation_definition_not_found(error.to_string())
        }
        GenerationServiceError::Repository(repository_error) => {
            super::map_repository_error(repository_error)
        }
        GenerationServiceError::Compile(_) => AppError::invalid_input(error.to_string()),
        GenerationServiceError::Domain(_)
        | GenerationServiceError::Snapshot(_)
        | GenerationServiceError::Comfy(_)
        | GenerationServiceError::StreamDisconnected(_)
        | GenerationServiceError::OutputCollection(_)
        | GenerationServiceError::AssetImport(_)
        | GenerationServiceError::ExecutionFailed { .. } => AppError::internal(error.to_string()),
    }
}
