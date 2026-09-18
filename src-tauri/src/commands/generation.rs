use crate::{
    app_state::AppState,
    application::{
        generation_input_preparer::GenerationInputValue,
        production_queue_service::{
            CreateDirectGenerationBatchItem, CreateDirectGenerationBatchRequest,
            CreateDirectGenerationRequest, CreateProductionBatchItem,
        },
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
    #[serde(default)]
    pub model_version_id: Option<String>,
    #[serde(default)]
    pub prompt_version_id: Option<String>,
    #[serde(default)]
    pub tool_instance_id: Option<String>,
    #[serde(default)]
    pub tool_version_id: Option<String>,
    #[serde(default)]
    pub submission_idempotency_key: Option<String>,
    #[serde(default)]
    pub parent_task_id: Option<String>,
}

const MAX_BATCH_ITEMS: usize = 100;

fn validate_batch_size(item_count: usize) -> Result<(), AppError> {
    if item_count == 0 {
        return Err(AppError::invalid_input(
            "batch must contain at least one item",
        ));
    }
    if item_count > MAX_BATCH_ITEMS {
        return Err(AppError::invalid_input(format!(
            "batch contains {item_count} items; maximum is {MAX_BATCH_ITEMS}"
        )));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationBatchCreateRequest {
    pub project_id: String,
    pub items: Vec<GenerationBatchItemRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationBatchItemRequest {
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub values: BTreeMap<String, InputValueDto>,
    #[serde(default)]
    pub model_version_id: Option<String>,
    #[serde(default)]
    pub prompt_version_id: Option<String>,
    #[serde(default)]
    pub tool_instance_id: Option<String>,
    #[serde(default)]
    pub tool_version_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub(crate) enum InputValueDto {
    #[serde(rename = "string")]
    String { value: String },
    #[serde(rename = "integer")]
    Integer { value: i64 },
    #[serde(rename = "number")]
    Number { value: f64 },
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
    #[serde(rename = "video_asset")]
    VideoAsset {
        #[serde(rename = "assetId")]
        asset_id: String,
    },
    #[serde(rename = "audio_asset")]
    AudioAsset {
        #[serde(rename = "assetId")]
        asset_id: String,
    },
    #[serde(rename = "video_assets")]
    VideoAssets {
        #[serde(rename = "assetIds")]
        asset_ids: Vec<String>,
    },
    #[serde(rename = "audio_assets")]
    AudioAssets {
        #[serde(rename = "assetIds")]
        asset_ids: Vec<String>,
    },
}

impl InputValueDto {
    pub(crate) fn into_application(self, key: &str) -> Result<GenerationInputValue, AppError> {
        match self {
            Self::String { value } => Ok(GenerationInputValue::Text(value)),
            Self::Integer { value } => Ok(GenerationInputValue::Integer(value)),
            Self::Number { value } => {
                if !value.is_finite() {
                    return Err(AppError::invalid_input(format!(
                        "number value for {key} must be finite"
                    )));
                }
                Ok(GenerationInputValue::Number(value))
            }
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
            Self::VideoAsset { asset_id } => Ok(GenerationInputValue::VideoAsset(parse_asset_id(
                &asset_id, key, "video",
            )?)),
            Self::AudioAsset { asset_id } => Ok(GenerationInputValue::AudioAsset(parse_asset_id(
                &asset_id, key, "audio",
            )?)),
            Self::VideoAssets { asset_ids } => Ok(GenerationInputValue::VideoAssets(
                asset_ids
                    .into_iter()
                    .map(|asset_id| parse_asset_id(&asset_id, key, "video"))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            Self::AudioAssets { asset_ids } => Ok(GenerationInputValue::AudioAssets(
                asset_ids
                    .into_iter()
                    .map(|asset_id| parse_asset_id(&asset_id, key, "audio"))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
        }
    }
}

pub(crate) fn input_value_into_application(
    value: InputValueDto,
    key: &str,
) -> Result<GenerationInputValue, AppError> {
    value.into_application(key)
}

fn parse_asset_id(value: &str, key: &str, kind: &str) -> Result<crate::domain::AssetId, AppError> {
    crate::domain::AssetId::parse(value.to_owned()).map_err(|error| {
        AppError::invalid_input(format!("{kind} asset id for {key} is invalid: {error}"))
    })
}

impl GenerationCreateRequest {
    pub(crate) fn into_application(self) -> Result<CreateDirectGenerationRequest, AppError> {
        crate::domain::validate_project_id(&self.project_id)
            .map_err(|error| AppError::invalid_input(error.to_string()))?;
        let values = self
            .values
            .into_iter()
            .map(|(key, value)| Ok((key.clone(), value.into_application(&key)?)))
            .collect::<Result<BTreeMap<_, _>, AppError>>()?;

        Ok(CreateDirectGenerationRequest {
            project_id: self.project_id,
            name: "Generation".to_owned(),
            continue_on_failure: true,
            item: CreateProductionBatchItem {
                workflow_version_id: self.workflow_version_id,
                recipe_id: self.recipe_id,
                values,
            },
            shot_id: None,
            stage: None,
            model_version_id: self.model_version_id,
            prompt_version_id: self.prompt_version_id,
            tool_instance_id: self.tool_instance_id,
            tool_version_id: self.tool_version_id,
            submission_idempotency_key: self.submission_idempotency_key,
            parent_task_id: self.parent_task_id,
        })
    }
}

impl GenerationBatchItemRequest {
    fn into_application(self) -> Result<CreateDirectGenerationBatchItem, AppError> {
        let values = self
            .values
            .into_iter()
            .map(|(key, value)| Ok((key.clone(), value.into_application(&key)?)))
            .collect::<Result<BTreeMap<_, _>, AppError>>()?;

        Ok(CreateDirectGenerationBatchItem {
            item: CreateProductionBatchItem {
                workflow_version_id: self.workflow_version_id,
                recipe_id: self.recipe_id,
                values,
            },
            shot_id: None,
            stage: None,
            model_version_id: self.model_version_id,
            prompt_version_id: self.prompt_version_id,
            tool_instance_id: self.tool_instance_id,
            tool_version_id: self.tool_version_id,
            submission_idempotency_key: None,
            parent_task_id: None,
        })
    }
}

#[tauri::command]
pub async fn generation_create(
    state: State<'_, AppState>,
    request: GenerationCreateRequest,
) -> Result<super::production_queue::ProductionBatchDetailView, AppError> {
    let request = request.into_application()?;
    let _admission = state
        .production_queue_service
        .acquire_interactive_admission()
        .await
        .map_err(super::production_queue::map_queue_error)?;
    state
        .production_queue_service
        .create_direct_generation(request)
        .await
        .map(Into::into)
        .map_err(super::production_queue::map_queue_error)
}

#[tauri::command]
pub async fn generation_create_batch(
    state: State<'_, AppState>,
    request: GenerationBatchCreateRequest,
) -> Result<super::production_queue::ProductionBatchDetailView, AppError> {
    crate::domain::validate_project_id(&request.project_id)
        .map_err(|error| AppError::invalid_input(error.to_string()))?;
    validate_batch_size(request.items.len())?;
    let _admission = state
        .production_queue_service
        .acquire_interactive_admission()
        .await
        .map_err(super::production_queue::map_queue_error)?;

    let items = request
        .items
        .into_iter()
        .map(GenerationBatchItemRequest::into_application)
        .collect::<Result<Vec<_>, _>>()?;
    state
        .production_queue_service
        .create_direct_generation_batch(CreateDirectGenerationBatchRequest {
            project_id: request.project_id,
            name: "Generation batch".to_owned(),
            continue_on_failure: true,
            items,
        })
        .await
        .map(Into::into)
        .map_err(super::production_queue::map_queue_error)
}

#[cfg(test)]
mod tests {
    use super::{validate_batch_size, MAX_BATCH_ITEMS};

    #[test]
    fn batch_size_rejects_empty_batch() {
        let error = validate_batch_size(0).unwrap_err();
        assert_eq!(error.code(), "INVALID_INPUT");
        assert!(error.message.contains("at least one"));
    }

    #[test]
    fn batch_size_accepts_limit_and_rejects_over_limit() {
        validate_batch_size(MAX_BATCH_ITEMS).unwrap();
        let error = validate_batch_size(MAX_BATCH_ITEMS + 1).unwrap_err();
        assert_eq!(error.code(), "INVALID_INPUT");
        assert!(error.message.contains("maximum"));
    }
}
