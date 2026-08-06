use crate::application::ports::{
    AssetRepository, AssetStore, ComfyAdapter, ComfyAdapterError, ComfyImageUpload,
    ComfyUploadedImage, RepositoryError,
};
use crate::domain::{Asset, AssetId, AssetType, InputValue, SeedValue, TaskId};
use serde_json::json;
use std::{collections::BTreeMap, error::Error, fmt, sync::Arc};

const IMAGE_PREVIEW_REFERENCE: &str = "__aistudio_preflight_image__";

#[derive(Clone, Debug, PartialEq)]
pub enum GenerationInputValue {
    Text(String),
    Integer(i64),
    Seed(SeedValue),
    ImageAsset(AssetId),
    ImageAssets(Vec<AssetId>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedImageInput {
    pub asset_id: AssetId,
    pub sha256: String,
    pub comfy: ComfyUploadedImage,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedGenerationInputs {
    pub compiler_values: BTreeMap<String, InputValue>,
    pub images: BTreeMap<String, Vec<PreparedImageInput>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GenerationInputPrepareError {
    AssetNotFound { asset_id: String },
    AssetProjectMismatch { asset_id: String },
    AssetTypeInvalid { asset_id: String },
    AssetRead { asset_id: String, message: String },
    InvalidAssetMime { asset_id: String, mime_type: String },
    Repository(String),
    Upload(ComfyAdapterError),
}

impl GenerationInputPrepareError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::AssetNotFound { .. } => "INPUT_ASSET_NOT_FOUND",
            Self::AssetProjectMismatch { .. } => "INPUT_ASSET_PROJECT_MISMATCH",
            Self::AssetTypeInvalid { .. } => "INPUT_ASSET_TYPE_INVALID",
            Self::AssetRead { .. } => "INPUT_ASSET_READ_FAILED",
            Self::InvalidAssetMime { .. } => "INPUT_ASSET_MIME_INVALID",
            Self::Repository(_) => "INPUT_ASSET_REPOSITORY_ERROR",
            Self::Upload(error) => match error {
                ComfyAdapterError::Offline(_) => "COMFY_OFFLINE",
                ComfyAdapterError::Timeout(_) => "COMFY_TIMEOUT",
                ComfyAdapterError::Protocol(_) | ComfyAdapterError::Incompatible(_) => {
                    "COMFY_PROTOCOL_ERROR"
                }
                ComfyAdapterError::ImageUpload(_) => "COMFY_IMAGE_UPLOAD_FAILED",
                _ => "COMFY_IMAGE_UPLOAD_FAILED",
            },
        }
    }
}

impl fmt::Display for GenerationInputPrepareError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AssetNotFound { asset_id } => {
                write!(formatter, "{}: asset {asset_id} was not found", self.code())
            }
            Self::AssetProjectMismatch { asset_id } => write!(
                formatter,
                "{}: asset {asset_id} does not belong to the generation project",
                self.code()
            ),
            Self::AssetTypeInvalid { asset_id } => write!(
                formatter,
                "{}: asset {asset_id} is not an image",
                self.code()
            ),
            Self::AssetRead { asset_id, message } => {
                write!(
                    formatter,
                    "{}: asset {asset_id} could not be read: {message}",
                    self.code()
                )
            }
            Self::InvalidAssetMime {
                asset_id,
                mime_type,
            } => write!(
                formatter,
                "{}: asset {asset_id} has unsupported MIME type {mime_type}",
                self.code()
            ),
            Self::Repository(message) => write!(formatter, "{}: {message}", self.code()),
            Self::Upload(error) => write!(formatter, "{}: {error}", self.code()),
        }
    }
}

impl Error for GenerationInputPrepareError {}

pub struct GenerationInputPreparer {
    asset_repository: Arc<dyn AssetRepository>,
    asset_store: Arc<dyn AssetStore>,
    comfy_adapter: Arc<dyn ComfyAdapter>,
}

impl GenerationInputPreparer {
    pub fn new(
        asset_repository: Arc<dyn AssetRepository>,
        asset_store: Arc<dyn AssetStore>,
        comfy_adapter: Arc<dyn ComfyAdapter>,
    ) -> Self {
        Self {
            asset_repository,
            asset_store,
            comfy_adapter,
        }
    }

    pub fn preflight_values(
        values: &BTreeMap<String, GenerationInputValue>,
    ) -> BTreeMap<String, InputValue> {
        values
            .iter()
            .map(|(key, value)| {
                let value = match value {
                    GenerationInputValue::Text(value) => InputValue::String(value.clone()),
                    GenerationInputValue::Integer(value) => InputValue::Integer(*value),
                    GenerationInputValue::Seed(value) => InputValue::Seed(value.clone()),
                    GenerationInputValue::ImageAsset(_) => {
                        InputValue::Image(IMAGE_PREVIEW_REFERENCE.to_owned())
                    }
                    GenerationInputValue::ImageAssets(asset_ids) => InputValue::Images(
                        asset_ids
                            .iter()
                            .map(|_| IMAGE_PREVIEW_REFERENCE.to_owned())
                            .collect(),
                    ),
                };
                (key.clone(), value)
            })
            .collect()
    }

    pub async fn validate_asset_references(
        &self,
        project_id: &str,
        values: &BTreeMap<String, GenerationInputValue>,
    ) -> Result<(), GenerationInputPrepareError> {
        for value in values.values() {
            match value {
                GenerationInputValue::ImageAsset(asset_id) => {
                    self.load_image_asset(project_id, asset_id).await?;
                }
                GenerationInputValue::ImageAssets(asset_ids) => {
                    for asset_id in asset_ids {
                        self.load_image_asset(project_id, asset_id).await?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub async fn prepare(
        &self,
        project_id: &str,
        task_id: &TaskId,
        values: &BTreeMap<String, GenerationInputValue>,
    ) -> Result<PreparedGenerationInputs, GenerationInputPrepareError> {
        let mut compiler_values = BTreeMap::new();
        let mut images = BTreeMap::new();

        for (key, value) in values {
            match value {
                GenerationInputValue::Text(value) => {
                    compiler_values.insert(key.clone(), InputValue::String(value.clone()));
                }
                GenerationInputValue::Integer(value) => {
                    compiler_values.insert(key.clone(), InputValue::Integer(*value));
                }
                GenerationInputValue::Seed(value) => {
                    compiler_values.insert(key.clone(), InputValue::Seed(value.clone()));
                }
                GenerationInputValue::ImageAsset(asset_id) => {
                    let asset = self.load_image_asset(project_id, asset_id).await?;
                    let prepared = self.upload_asset(task_id, &asset, None).await?;
                    compiler_values
                        .insert(key.clone(), InputValue::Image(prepared.comfy.name.clone()));
                    images.insert(key.clone(), vec![prepared]);
                }
                GenerationInputValue::ImageAssets(asset_ids) => {
                    let mut prepared_images = Vec::with_capacity(asset_ids.len());
                    let mut comfy_names = Vec::with_capacity(asset_ids.len());
                    for (index, asset_id) in asset_ids.iter().enumerate() {
                        let asset = self.load_image_asset(project_id, asset_id).await?;
                        let prepared = self.upload_asset(task_id, &asset, Some(index + 1)).await?;
                        comfy_names.push(prepared.comfy.name.clone());
                        prepared_images.push(prepared);
                    }
                    compiler_values.insert(key.clone(), InputValue::Images(comfy_names));
                    images.insert(key.clone(), prepared_images);
                }
            }
        }

        Ok(PreparedGenerationInputs {
            compiler_values,
            images,
        })
    }

    async fn load_image_asset(
        &self,
        project_id: &str,
        asset_id: &AssetId,
    ) -> Result<Asset, GenerationInputPrepareError> {
        let asset = self
            .asset_repository
            .find_by_id(asset_id)
            .await
            .map_err(repository_error)?
            .ok_or_else(|| GenerationInputPrepareError::AssetNotFound {
                asset_id: asset_id.as_str().to_owned(),
            })?;
        if asset.project_id != project_id {
            return Err(GenerationInputPrepareError::AssetProjectMismatch {
                asset_id: asset_id.as_str().to_owned(),
            });
        }
        if asset.asset_type != AssetType::Image {
            return Err(GenerationInputPrepareError::AssetTypeInvalid {
                asset_id: asset_id.as_str().to_owned(),
            });
        }
        Ok(asset)
    }

    async fn upload_asset(
        &self,
        task_id: &TaskId,
        asset: &Asset,
        position: Option<usize>,
    ) -> Result<PreparedImageInput, GenerationInputPrepareError> {
        if !matches!(
            asset.mime_type.as_str(),
            "image/png" | "image/jpeg" | "image/webp"
        ) {
            return Err(GenerationInputPrepareError::InvalidAssetMime {
                asset_id: asset.id.as_str().to_owned(),
                mime_type: asset.mime_type.clone(),
            });
        }
        let bytes = self
            .asset_store
            .read(std::path::Path::new(&asset.storage_path))
            .await
            .map_err(|error| GenerationInputPrepareError::AssetRead {
                asset_id: asset.id.as_str().to_owned(),
                message: error.to_string(),
            })?;
        let upload = ComfyImageUpload {
            bytes,
            upload_name: upload_name_at(task_id, asset, position),
            content_type: asset.mime_type.clone(),
        };
        let comfy = self
            .comfy_adapter
            .upload_image(upload)
            .await
            .map_err(GenerationInputPrepareError::Upload)?;
        Ok(PreparedImageInput {
            asset_id: asset.id.clone(),
            sha256: asset.sha256.clone(),
            comfy,
        })
    }
}

#[cfg(test)]
pub fn upload_name(task_id: &TaskId, asset: &Asset) -> String {
    upload_name_at(task_id, asset, None)
}

pub fn upload_name_at(task_id: &TaskId, asset: &Asset, position: Option<usize>) -> String {
    let extension = match asset.mime_type.as_str() {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        _ => asset
            .storage_path
            .rsplit_once('.')
            .map(|(_, extension)| extension)
            .filter(|extension| {
                extension
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
            })
            .unwrap_or("img"),
    };
    let task = task_id
        .as_str()
        .strip_prefix("tsk_")
        .unwrap_or(task_id.as_str());
    let asset_id = asset
        .id
        .as_str()
        .strip_prefix("ast_")
        .unwrap_or(asset.id.as_str());
    match position {
        Some(position) => format!("aistudio_{task}_{asset_id}_{position:02}.{extension}"),
        None => format!("aistudio_{task}_{asset_id}.{extension}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{upload_name, upload_name_at};
    use crate::domain::{Asset, AssetId, TaskId};
    use serde_json::json;

    fn asset() -> Asset {
        Asset::new_source_image(
            AssetId::parse("ast_reference").unwrap(),
            "project-1",
            "reference.png",
            "reference.png",
            "C:/project/reference.png",
            "a".repeat(64),
            "image/png",
            2,
            2,
            10,
            json!({}),
            chrono::Utc::now(),
        )
        .unwrap()
    }

    #[test]
    fn ordered_upload_names_are_position_stable() {
        let task = TaskId::parse("tsk_test-task").unwrap();
        let asset = asset();
        assert_eq!(
            upload_name(&task, &asset),
            "aistudio_test-task_reference.png"
        );
        assert_eq!(
            upload_name_at(&task, &asset, Some(2)),
            "aistudio_test-task_reference_02.png"
        );
    }
}

pub fn image_snapshot_value(prepared: &PreparedImageInput) -> serde_json::Value {
    json!({
        "assetId": prepared.asset_id.as_str(),
        "sha256": prepared.sha256,
        "comfy": {
            "name": prepared.comfy.name,
            "subfolder": prepared.comfy.subfolder,
            "type": prepared.comfy.folder_type,
        }
    })
}

pub fn images_snapshot_value(prepared: &[PreparedImageInput]) -> serde_json::Value {
    serde_json::Value::Array(prepared.iter().map(image_snapshot_value).collect())
}

fn repository_error(error: RepositoryError) -> GenerationInputPrepareError {
    GenerationInputPrepareError::Repository(error.to_string())
}
