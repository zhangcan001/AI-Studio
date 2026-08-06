use crate::{
    app_state::AppState,
    application::{
        generation_input_preparer::GenerationInputValue,
        generation_service::{CreateGenerationRequest, GenerationServiceError},
        task_query_service::TaskView,
    },
    domain::SeedValue,
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
pub(crate) enum InputValueDto {
    #[serde(rename = "string")]
    String { value: String },
    #[serde(rename = "integer")]
    Integer { value: i64 },
    #[serde(rename = "seed_random")]
    SeedRandom,
    #[serde(rename = "seed_fixed")]
    SeedFixed { value: String },
    #[serde(rename = "image_asset")]
    ImageAsset {
        #[serde(rename = "assetId")]
        asset_id: String,
    },
    #[serde(rename = "image_assets")]
    ImageAssets {
        #[serde(rename = "assetIds")]
        asset_ids: Vec<String>,
    },
}

impl InputValueDto {
    pub(crate) fn into_application(self, key: &str) -> Result<GenerationInputValue, AppError> {
        match self {
            Self::String { value } => Ok(GenerationInputValue::Text(value)),
            Self::Integer { value } => Ok(GenerationInputValue::Integer(value)),
            Self::SeedRandom => Ok(GenerationInputValue::Seed(SeedValue::Random)),
            Self::SeedFixed { value } => {
                let seed = value.parse::<u64>().map_err(|_| {
                    AppError::invalid_input(format!(
                        "seed value for {key} must be a decimal u64 string"
                    ))
                })?;
                Ok(GenerationInputValue::Seed(SeedValue::Fixed(seed)))
            }
            Self::ImageAsset { asset_id } => {
                let asset_id = crate::domain::AssetId::parse(asset_id).map_err(|error| {
                    AppError::invalid_input(format!("image asset id is invalid: {error}"))
                })?;
                Ok(GenerationInputValue::ImageAsset(asset_id))
            }
            Self::ImageAssets { asset_ids } => {
                let asset_ids = asset_ids
                    .into_iter()
                    .map(|asset_id| {
                        crate::domain::AssetId::parse(asset_id).map_err(|error| {
                            AppError::invalid_input(format!("image asset id is invalid: {error}"))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(GenerationInputValue::ImageAssets(asset_ids))
            }
        }
    }
}

impl GenerationCreateRequest {
    fn into_application(self) -> Result<CreateGenerationRequest, AppError> {
        crate::domain::validate_project_id(&self.project_id)
            .map_err(|error| AppError::invalid_input(error.to_string()))?;
        let values = self
            .values
            .into_iter()
            .map(|(key, value)| Ok((key.clone(), value.into_application(&key)?)))
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
        GenerationServiceError::InputPrepare(_) => AppError::invalid_input(error.to_string()),
        GenerationServiceError::Domain(_)
        | GenerationServiceError::Snapshot(_)
        | GenerationServiceError::Comfy(_)
        | GenerationServiceError::StreamDisconnected(_)
        | GenerationServiceError::OutputCollection(_)
        | GenerationServiceError::AssetImport(_)
        | GenerationServiceError::ExecutionFailed { .. } => AppError::internal(error.to_string()),
    }
}
