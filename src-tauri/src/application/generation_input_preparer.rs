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
    pub images: BTreeMap<String, PreparedImageInput>,
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
            if let GenerationInputValue::ImageAsset(asset_id) = value {
                self.load_image_asset(project_id, asset_id).await?;
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
        let mut uploaded_by_asset = BTreeMap::<String, PreparedImageInput>::new();

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
                    let prepared = if let Some(prepared) = uploaded_by_asset.get(asset_id.as_str())
                    {
                        prepared.clone()
                    } else {
                        let prepared = self.upload_asset(task_id, &asset).await?;
                        uploaded_by_asset.insert(asset_id.as_str().to_owned(), prepared.clone());
                        prepared
                    };
                    compiler_values
                        .insert(key.clone(), InputValue::Image(prepared.comfy.name.clone()));
                    images.insert(key.clone(), prepared);
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
            upload_name: upload_name(task_id, asset),
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

pub fn upload_name(task_id: &TaskId, asset: &Asset) -> String {
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
    format!("aistudio_{task}_{asset_id}.{extension}")
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

fn repository_error(error: RepositoryError) -> GenerationInputPrepareError {
    GenerationInputPrepareError::Repository(error.to_string())
}
