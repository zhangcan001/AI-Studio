use crate::application::ports::{
    AssetReadStream, AssetRepository, AssetStore, ComfyAdapter, ComfyAdapterError,
    ComfyInputStream, ComfyInputUpload, ComfyUploadedInput, ProjectRepository, RepositoryError,
};
use crate::domain::{
    Asset, AssetId, AssetType, InputValue, SeedValue, TaskId, GENERATED_VIDEO_CATEGORY,
    SOURCE_AUDIO_CATEGORY, SOURCE_VIDEO_CATEGORY,
};
use image::{imageops::FilterType, ImageFormat, ImageReader};
use serde_json::json;
use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
    fmt,
    io::Cursor,
    sync::Arc,
};

const IMAGE_PREVIEW_REFERENCE: &str = "__aistudio_preflight_image__";
const MAX_COMFY_IMAGE_EDGE: u32 = 2048;

#[derive(Clone, Debug, PartialEq)]
pub enum GenerationInputValue {
    Text(String),
    Integer(i64),
    Number(f64),
    Seed(SeedValue),
    ImageAsset(AssetId),
    ImageAssets(Vec<AssetId>),
    VideoAsset(AssetId),
    AudioAsset(AssetId),
    VideoAssets(Vec<AssetId>),
    AudioAssets(Vec<AssetId>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedImageInput {
    pub asset_id: AssetId,
    pub sha256: String,
    pub comfy: ComfyUploadedInput,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedMediaInput {
    pub asset_id: AssetId,
    pub sha256: String,
    pub comfy: ComfyUploadedInput,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedGenerationInputs {
    pub compiler_values: BTreeMap<String, InputValue>,
    pub images: BTreeMap<String, Vec<PreparedImageInput>>,
    pub media: BTreeMap<String, Vec<PreparedMediaInput>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GenerationInputPrepareError {
    AssetNotFound {
        asset_id: String,
    },
    AssetProjectMismatch {
        asset_id: String,
    },
    AssetTypeInvalid {
        asset_id: String,
    },
    AssetRead {
        asset_id: String,
        message: String,
    },
    InvalidAssetMime {
        asset_id: String,
        mime_type: String,
    },
    ReferenceMappingIncomplete {
        input_key: String,
        expected_asset_ids: Vec<String>,
        actual_asset_ids: Vec<String>,
    },
    DuplicateFirstLastAsset {
        asset_id: String,
    },
    Repository(String),
    Upload {
        input_key: String,
        asset_id: String,
        error: ComfyAdapterError,
    },
    ComfyUnavailable {
        error: ComfyAdapterError,
    },
}

impl GenerationInputPrepareError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::AssetNotFound { .. } => "INPUT_ASSET_NOT_FOUND",
            Self::AssetProjectMismatch { .. } => "INPUT_ASSET_PROJECT_MISMATCH",
            Self::AssetTypeInvalid { .. } => "INPUT_ASSET_TYPE_INVALID",
            Self::AssetRead { .. } => "INPUT_ASSET_READ_FAILED",
            Self::InvalidAssetMime { .. } => "INPUT_ASSET_MIME_INVALID",
            Self::ReferenceMappingIncomplete { .. } => "REFERENCE_MAPPING_INCOMPLETE",
            Self::DuplicateFirstLastAsset { .. } => "INPUT_ASSET_DUPLICATE",
            Self::Repository(_) => "INPUT_ASSET_REPOSITORY_ERROR",
            Self::Upload { error, .. } | Self::ComfyUnavailable { error } => {
                comfy_error_code(error)
            }
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
                "{}: asset {asset_id} has an invalid media type or category",
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
            Self::ReferenceMappingIncomplete {
                input_key,
                expected_asset_ids,
                actual_asset_ids,
            } => write!(
                formatter,
                "{}: reference input {input_key} expected {} assets in frozen order, received {}",
                self.code(),
                expected_asset_ids.len(),
                actual_asset_ids.len()
            ),
            Self::DuplicateFirstLastAsset { asset_id } => write!(
                formatter,
                "{}: first_frame and last_frame cannot use the same image asset {asset_id}",
                self.code()
            ),
            Self::Repository(message) => write!(formatter, "{}: {message}", self.code()),
            Self::Upload {
                input_key,
                asset_id,
                error,
            } => write!(
                formatter,
                "{}: input {input_key} asset {asset_id} upload failed: {error}",
                self.code()
            ),
            Self::ComfyUnavailable { error } => write!(
                formatter,
                "{}: ComfyUI 不可用或较忙，已取消上传：{error}",
                self.code()
            ),
        }
    }
}

impl Error for GenerationInputPrepareError {}

pub struct GenerationInputPreparer {
    asset_repository: Arc<dyn AssetRepository>,
    asset_store: Arc<dyn AssetStore>,
    project_repository: Arc<dyn ProjectRepository>,
    comfy_adapter: Arc<dyn ComfyAdapter>,
}

impl GenerationInputPreparer {
    pub fn new(
        asset_repository: Arc<dyn AssetRepository>,
        asset_store: Arc<dyn AssetStore>,
        project_repository: Arc<dyn ProjectRepository>,
        comfy_adapter: Arc<dyn ComfyAdapter>,
    ) -> Self {
        Self {
            asset_repository,
            asset_store,
            project_repository,
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
                    GenerationInputValue::Number(value) => InputValue::Number(*value),
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
                    GenerationInputValue::VideoAsset(_) => {
                        InputValue::Video(IMAGE_PREVIEW_REFERENCE.to_owned())
                    }
                    GenerationInputValue::AudioAsset(_) => {
                        InputValue::Audio(IMAGE_PREVIEW_REFERENCE.to_owned())
                    }
                    GenerationInputValue::VideoAssets(asset_ids) => InputValue::Videos(
                        asset_ids
                            .iter()
                            .map(|_| IMAGE_PREVIEW_REFERENCE.to_owned())
                            .collect(),
                    ),
                    GenerationInputValue::AudioAssets(asset_ids) => InputValue::Audios(
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
                GenerationInputValue::VideoAsset(asset_id) => {
                    self.load_media_asset(project_id, asset_id, MediaExpectation::Video)
                        .await?;
                }
                GenerationInputValue::AudioAsset(asset_id) => {
                    self.load_media_asset(project_id, asset_id, MediaExpectation::Audio)
                        .await?;
                }
                GenerationInputValue::VideoAssets(asset_ids) => {
                    for asset_id in asset_ids {
                        self.load_media_asset(project_id, asset_id, MediaExpectation::Video)
                            .await?;
                    }
                }
                GenerationInputValue::AudioAssets(asset_ids) => {
                    for asset_id in asset_ids {
                        self.load_media_asset(project_id, asset_id, MediaExpectation::Audio)
                            .await?;
                    }
                }
                _ => {}
            }
        }
        if let (
            Some(GenerationInputValue::ImageAsset(first_frame)),
            Some(GenerationInputValue::ImageAsset(last_frame)),
        ) = (values.get("first_frame"), values.get("last_frame"))
        {
            if first_frame == last_frame {
                return Err(GenerationInputPrepareError::DuplicateFirstLastAsset {
                    asset_id: first_frame.as_str().to_owned(),
                });
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
        let project_root = self
            .project_repository
            .get_storage_root(project_id)
            .await
            .map_err(repository_error)?
            .ok_or_else(|| {
                GenerationInputPrepareError::Repository(format!(
                    "storage root is not configured for project {project_id}"
                ))
            })?;
        let mut compiler_values = BTreeMap::new();
        let mut images = BTreeMap::new();
        let mut media = BTreeMap::new();
        let mut upload_cache = HashMap::<AssetId, ComfyUploadedInput>::new();
        let mut upload_health_checked = false;

        for (key, value) in values {
            match value {
                GenerationInputValue::Text(value) => {
                    compiler_values.insert(key.clone(), InputValue::String(value.clone()));
                }
                GenerationInputValue::Integer(value) => {
                    compiler_values.insert(key.clone(), InputValue::Integer(*value));
                }
                GenerationInputValue::Number(value) => {
                    compiler_values.insert(key.clone(), InputValue::Number(*value));
                }
                GenerationInputValue::Seed(value) => {
                    compiler_values.insert(key.clone(), InputValue::Seed(value.clone()));
                }
                GenerationInputValue::ImageAsset(asset_id) => {
                    let asset = self.load_image_asset(project_id, asset_id).await?;
                    let prepared = self
                        .upload_image_asset(
                            task_id,
                            &project_root,
                            key,
                            &asset,
                            None,
                            &mut upload_cache,
                            &mut upload_health_checked,
                        )
                        .await?;
                    compiler_values
                        .insert(key.clone(), InputValue::Image(prepared.comfy.name.clone()));
                    images.insert(key.clone(), vec![prepared]);
                }
                GenerationInputValue::ImageAssets(asset_ids) => {
                    let mut prepared_images = Vec::with_capacity(asset_ids.len());
                    let mut comfy_names = Vec::with_capacity(asset_ids.len());
                    for (index, asset_id) in asset_ids.iter().enumerate() {
                        let asset = self.load_image_asset(project_id, asset_id).await?;
                        let prepared = self
                            .upload_image_asset(
                                task_id,
                                &project_root,
                                key,
                                &asset,
                                Some(index + 1),
                                &mut upload_cache,
                                &mut upload_health_checked,
                            )
                            .await?;
                        comfy_names.push(prepared.comfy.name.clone());
                        prepared_images.push(prepared);
                    }
                    compiler_values.insert(key.clone(), InputValue::Images(comfy_names));
                    images.insert(key.clone(), prepared_images);
                }
                GenerationInputValue::VideoAsset(asset_id) => {
                    let asset = self
                        .load_media_asset(project_id, asset_id, MediaExpectation::Video)
                        .await?;
                    let prepared = self
                        .upload_media_asset(
                            task_id,
                            &project_root,
                            key,
                            &asset,
                            None,
                            &mut upload_cache,
                            &mut upload_health_checked,
                        )
                        .await?;
                    compiler_values
                        .insert(key.clone(), InputValue::Video(prepared.comfy.name.clone()));
                    media.insert(key.clone(), vec![prepared]);
                }
                GenerationInputValue::AudioAsset(asset_id) => {
                    let asset = self
                        .load_media_asset(project_id, asset_id, MediaExpectation::Audio)
                        .await?;
                    let prepared = self
                        .upload_media_asset(
                            task_id,
                            &project_root,
                            key,
                            &asset,
                            None,
                            &mut upload_cache,
                            &mut upload_health_checked,
                        )
                        .await?;
                    compiler_values
                        .insert(key.clone(), InputValue::Audio(prepared.comfy.name.clone()));
                    media.insert(key.clone(), vec![prepared]);
                }
                GenerationInputValue::VideoAssets(asset_ids) => {
                    let mut prepared_media = Vec::with_capacity(asset_ids.len());
                    let mut comfy_names = Vec::with_capacity(asset_ids.len());
                    for (index, asset_id) in asset_ids.iter().enumerate() {
                        let asset = self
                            .load_media_asset(project_id, asset_id, MediaExpectation::Video)
                            .await?;
                        let prepared = self
                            .upload_media_asset(
                                task_id,
                                &project_root,
                                key,
                                &asset,
                                Some(index + 1),
                                &mut upload_cache,
                                &mut upload_health_checked,
                            )
                            .await?;
                        comfy_names.push(prepared.comfy.name.clone());
                        prepared_media.push(prepared);
                    }
                    compiler_values.insert(key.clone(), InputValue::Videos(comfy_names));
                    media.insert(key.clone(), prepared_media);
                }
                GenerationInputValue::AudioAssets(asset_ids) => {
                    let mut prepared_media = Vec::with_capacity(asset_ids.len());
                    let mut comfy_names = Vec::with_capacity(asset_ids.len());
                    for (index, asset_id) in asset_ids.iter().enumerate() {
                        let asset = self
                            .load_media_asset(project_id, asset_id, MediaExpectation::Audio)
                            .await?;
                        let prepared = self
                            .upload_media_asset(
                                task_id,
                                &project_root,
                                key,
                                &asset,
                                Some(index + 1),
                                &mut upload_cache,
                                &mut upload_health_checked,
                            )
                            .await?;
                        comfy_names.push(prepared.comfy.name.clone());
                        prepared_media.push(prepared);
                    }
                    compiler_values.insert(key.clone(), InputValue::Audios(comfy_names));
                    media.insert(key.clone(), prepared_media);
                }
            }
        }

        Ok(PreparedGenerationInputs {
            compiler_values,
            images,
            media,
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

    async fn load_media_asset(
        &self,
        project_id: &str,
        asset_id: &AssetId,
        expectation: MediaExpectation,
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
                asset_id: asset.id.as_str().to_owned(),
            });
        }
        let valid = match expectation {
            MediaExpectation::Video => {
                asset.asset_type == AssetType::Video
                    && matches!(
                        asset.category.as_str(),
                        SOURCE_VIDEO_CATEGORY | GENERATED_VIDEO_CATEGORY
                    )
            }
            MediaExpectation::Audio => {
                asset.asset_type == AssetType::Audio && asset.category == SOURCE_AUDIO_CATEGORY
            }
        };
        if !valid {
            return Err(GenerationInputPrepareError::AssetTypeInvalid {
                asset_id: asset.id.as_str().to_owned(),
            });
        }
        Ok(asset)
    }

    async fn upload_image_asset(
        &self,
        task_id: &TaskId,
        project_root: &std::path::Path,
        input_key: &str,
        asset: &Asset,
        position: Option<usize>,
        upload_cache: &mut HashMap<AssetId, ComfyUploadedInput>,
        upload_health_checked: &mut bool,
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
        let comfy = if let Some(uploaded) = upload_cache.get(&asset.id) {
            uploaded.clone()
        } else {
            self.ensure_comfy_upload_health(upload_health_checked)
                .await?;
            let bytes = self
                .asset_store
                .read(project_root, std::path::Path::new(&asset.storage_path))
                .await
                .map_err(|error| GenerationInputPrepareError::AssetRead {
                    asset_id: asset.id.as_str().to_owned(),
                    message: error.to_string(),
                })?;
            let original_bytes = bytes.len();
            let bytes = prepare_image_bytes_for_comfy(bytes, asset).map_err(|message| {
                GenerationInputPrepareError::AssetRead {
                    asset_id: asset.id.as_str().to_owned(),
                    message: format!("image preparation failed: {message}"),
                }
            })?;
            if bytes.len() != original_bytes {
                tracing::debug!(
                    asset_id = %asset.id,
                    original_bytes,
                    upload_bytes = bytes.len(),
                    "ComfyUI image upload copy downscaled"
                );
            }
            let upload = crate::application::ports::ComfyImageUpload {
                bytes,
                upload_name: upload_name_at(task_id, asset, position),
                content_type: asset.mime_type.clone(),
            };
            let uploaded = self
                .comfy_adapter
                .upload_image(upload)
                .await
                .map_err(|error| GenerationInputPrepareError::Upload {
                    input_key: input_key.to_owned(),
                    asset_id: asset.id.as_str().to_owned(),
                    error,
                })?;
            upload_cache.insert(asset.id.clone(), uploaded.clone());
            uploaded
        };
        Ok(PreparedImageInput {
            asset_id: asset.id.clone(),
            sha256: asset.sha256.clone(),
            comfy,
        })
    }

    async fn upload_media_asset(
        &self,
        task_id: &TaskId,
        project_root: &std::path::Path,
        input_key: &str,
        asset: &Asset,
        position: Option<usize>,
        upload_cache: &mut HashMap<AssetId, ComfyUploadedInput>,
        upload_health_checked: &mut bool,
    ) -> Result<PreparedMediaInput, GenerationInputPrepareError> {
        if !asset.mime_type.starts_with("video/") && !asset.mime_type.starts_with("audio/") {
            return Err(GenerationInputPrepareError::InvalidAssetMime {
                asset_id: asset.id.as_str().to_owned(),
                mime_type: asset.mime_type.clone(),
            });
        }
        let comfy = if let Some(uploaded) = upload_cache.get(&asset.id) {
            uploaded.clone()
        } else {
            self.ensure_comfy_upload_health(upload_health_checked)
                .await?;
            let stream = self
                .asset_store
                .open_read_stream(project_root, std::path::Path::new(&asset.storage_path))
                .await
                .map_err(|error| GenerationInputPrepareError::AssetRead {
                    asset_id: asset.id.as_str().to_owned(),
                    message: error.to_string(),
                })?;
            let upload = ComfyInputUpload {
                filename: upload_name_at(task_id, asset, position),
                content_type: asset.mime_type.clone(),
                content_length: Some(asset.file_size),
                stream: Box::new(AssetToComfyInputStream { inner: stream }),
            };
            let uploaded = self
                .comfy_adapter
                .upload_input_file(upload)
                .await
                .map_err(|error| GenerationInputPrepareError::Upload {
                    input_key: input_key.to_owned(),
                    asset_id: asset.id.as_str().to_owned(),
                    error,
                })?;
            upload_cache.insert(asset.id.clone(), uploaded.clone());
            uploaded
        };
        Ok(PreparedMediaInput {
            asset_id: asset.id.clone(),
            sha256: asset.sha256.clone(),
            comfy,
        })
    }

    async fn ensure_comfy_upload_health(
        &self,
        checked: &mut bool,
    ) -> Result<(), GenerationInputPrepareError> {
        if *checked {
            return Ok(());
        }
        self.comfy_adapter
            .health_check()
            .await
            .map_err(|error| GenerationInputPrepareError::ComfyUnavailable { error })?;
        *checked = true;
        Ok(())
    }
}

fn comfy_error_code(error: &ComfyAdapterError) -> &'static str {
    match error {
        ComfyAdapterError::Offline(_) => "COMFY_OFFLINE",
        ComfyAdapterError::Timeout(_) => "COMFY_TIMEOUT",
        ComfyAdapterError::Protocol(_) | ComfyAdapterError::Incompatible(_) => {
            "COMFY_PROTOCOL_ERROR"
        }
        ComfyAdapterError::ImageUpload(_) => "COMFY_IMAGE_UPLOAD_FAILED",
        ComfyAdapterError::InputUploadTooLarge(_) => "COMFY_INPUT_UPLOAD_TOO_LARGE",
        ComfyAdapterError::InputUpload(_) => "COMFY_INPUT_UPLOAD_FAILED",
        _ => "COMFY_IMAGE_UPLOAD_FAILED",
    }
}

fn prepare_image_bytes_for_comfy(bytes: Vec<u8>, asset: &Asset) -> Result<Vec<u8>, String> {
    if asset.width.max(asset.height) <= MAX_COMFY_IMAGE_EDGE {
        return Ok(bytes);
    }

    let format = match asset.mime_type.as_str() {
        "image/png" => ImageFormat::Png,
        "image/jpeg" => ImageFormat::Jpeg,
        "image/webp" => ImageFormat::WebP,
        mime_type => return Err(format!("unsupported image MIME type {mime_type}")),
    };
    let dimensions = ImageReader::with_format(Cursor::new(bytes.as_slice()), format)
        .into_dimensions()
        .map_err(|error| error.to_string())?;
    let (width, height) = dimensions;
    let longest_edge = width.max(height);
    if longest_edge <= MAX_COMFY_IMAGE_EDGE {
        return Ok(bytes);
    }

    let scale = f64::from(MAX_COMFY_IMAGE_EDGE) / f64::from(longest_edge);
    let target_width = (f64::from(width) * scale).round().max(1.0) as u32;
    let target_height = (f64::from(height) * scale).round().max(1.0) as u32;
    let image = ImageReader::with_format(Cursor::new(bytes.as_slice()), format)
        .decode()
        .map_err(|error| error.to_string())?;
    let resized = image.resize(target_width, target_height, FilterType::Lanczos3);
    let mut output = Cursor::new(Vec::new());
    resized
        .write_to(&mut output, format)
        .map_err(|error| error.to_string())?;
    Ok(output.into_inner())
}

#[derive(Clone, Copy)]
enum MediaExpectation {
    Video,
    Audio,
}

struct AssetToComfyInputStream {
    inner: Box<dyn AssetReadStream>,
}

#[async_trait::async_trait]
impl ComfyInputStream for AssetToComfyInputStream {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, String> {
        self.inner
            .next_chunk()
            .await
            .map_err(|error| error.to_string())
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
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "video/quicktime" => "mov",
        "video/x-matroska" => "mkv",
        "audio/wav" | "audio/x-wav" => "wav",
        "audio/flac" => "flac",
        "audio/mpeg" => "mp3",
        "audio/ogg" => "ogg",
        "audio/opus" => "opus",
        "audio/mp4" => "m4a",
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
    use super::{
        prepare_image_bytes_for_comfy, upload_name, upload_name_at, GenerationInputPreparer,
        GenerationInputValue,
    };
    use crate::application::ports::{
        AssetReadStream, AssetRepository, AssetStore, AssetStoreError, ComfyAdapter,
        ComfyAdapterError, ComfyEventSubscription, ComfyHealth, ComfyHistory, ComfyInputUpload,
        ComfyOutputData, ComfyOutputFile, ComfyUploadedInput, ProjectRecord, ProjectRepository,
        PromptSubmission, RepositoryError, StoredAssetFile, SystemStats,
    };
    use crate::domain::{Asset, AssetId, TaskId};
    use async_trait::async_trait;
    use image::{ImageFormat, ImageReader};
    use serde_json::json;
    use std::collections::{BTreeMap, HashMap, VecDeque};
    use std::io::Cursor;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct FakeAssetRepository {
        assets: Arc<Mutex<HashMap<String, Asset>>>,
    }

    #[async_trait]
    impl AssetRepository for FakeAssetRepository {
        async fn insert_many(&self, assets: &[Asset]) -> Result<(), RepositoryError> {
            let mut stored = self.assets.lock().unwrap();
            for asset in assets {
                stored.insert(asset.id.as_str().to_owned(), asset.clone());
            }
            Ok(())
        }

        async fn find_by_id(&self, id: &AssetId) -> Result<Option<Asset>, RepositoryError> {
            Ok(self.assets.lock().unwrap().get(id.as_str()).cloned())
        }

        async fn list_by_source_task(
            &self,
            _task_id: &TaskId,
        ) -> Result<Vec<Asset>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn list_recent(
            &self,
            _project_id: &str,
            _limit: u32,
        ) -> Result<Vec<Asset>, RepositoryError> {
            Ok(self.assets.lock().unwrap().values().cloned().collect())
        }
    }

    struct ChunkStream {
        chunks: VecDeque<Vec<u8>>,
    }

    #[async_trait]
    impl AssetReadStream for ChunkStream {
        async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, AssetStoreError> {
            Ok(self.chunks.pop_front())
        }
    }

    #[derive(Clone, Copy)]
    struct StreamingAssetStore;

    #[async_trait]
    impl AssetStore for StreamingAssetStore {
        async fn write_image(
            &self,
            _project_root: &Path,
            _asset_id: &AssetId,
            _extension: &str,
            _bytes: &[u8],
        ) -> Result<StoredAssetFile, AssetStoreError> {
            Err(AssetStoreError::Write("not used".to_owned()))
        }

        async fn delete(&self, _path: &Path) -> Result<(), AssetStoreError> {
            Ok(())
        }

        async fn read(
            &self,
            _project_root: &Path,
            _path: &Path,
        ) -> Result<Vec<u8>, AssetStoreError> {
            Ok(vec![1, 2, 3, 4])
        }

        async fn open_read_stream(
            &self,
            _project_root: &Path,
            _path: &Path,
        ) -> Result<Box<dyn AssetReadStream>, AssetStoreError> {
            Ok(Box::new(ChunkStream {
                chunks: VecDeque::from([vec![1, 2], vec![3, 4]]),
            }))
        }
    }

    struct TestProjectRepository;

    #[async_trait]
    impl ProjectRepository for TestProjectRepository {
        async fn list(&self) -> Result<Vec<ProjectRecord>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn find_by_id(
            &self,
            _project_id: &str,
        ) -> Result<Option<ProjectRecord>, RepositoryError> {
            Ok(None)
        }

        async fn insert(&self, _project: &ProjectRecord) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn update_metadata(
            &self,
            _project_id: &str,
            _name: &str,
            _description: Option<&str>,
            _updated_at: chrono::DateTime<chrono::Utc>,
        ) -> Result<Option<ProjectRecord>, RepositoryError> {
            Ok(None)
        }

        async fn get_storage_root(
            &self,
            _project_id: &str,
        ) -> Result<Option<PathBuf>, RepositoryError> {
            Ok(Some(PathBuf::from(".")))
        }

        async fn ensure_default_project(
            &self,
            project_id: &str,
            name: &str,
            root_path: &PathBuf,
            created_at: chrono::DateTime<chrono::Utc>,
        ) -> Result<ProjectRecord, RepositoryError> {
            Ok(ProjectRecord {
                id: project_id.to_owned(),
                name: name.to_owned(),
                description: None,
                root_path: root_path.clone(),
                created_at,
                updated_at: created_at,
            })
        }
    }

    #[derive(Clone, Default)]
    struct RecordingAdapter {
        filenames: Arc<Mutex<Vec<String>>>,
        failure: Arc<Mutex<Option<ComfyAdapterError>>>,
        health_checks: Arc<Mutex<usize>>,
        health_failure: Arc<Mutex<Option<ComfyAdapterError>>>,
    }

    #[async_trait]
    impl ComfyAdapter for RecordingAdapter {
        async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
            *self.health_checks.lock().unwrap() += 1;
            if let Some(error) = self.health_failure.lock().unwrap().clone() {
                return Err(error);
            }
            Ok(ComfyHealth {
                system: SystemStats {
                    comfyui_version: Some("test".to_owned()),
                    python_version: None,
                    os: None,
                    ram_total: None,
                    ram_free: None,
                    devices: Vec::new(),
                },
            })
        }

        async fn get_system_stats(&self) -> Result<SystemStats, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
        }

        async fn get_object_info(&self) -> Result<serde_json::Value, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
        }

        async fn upload_input_file(
            &self,
            mut upload: ComfyInputUpload,
        ) -> Result<ComfyUploadedInput, ComfyAdapterError> {
            if let Some(error) = self.failure.lock().unwrap().clone() {
                return Err(error);
            }
            while upload
                .stream
                .next_chunk()
                .await
                .map_err(ComfyAdapterError::InputUpload)?
                .is_some()
            {}
            self.filenames.lock().unwrap().push(upload.filename.clone());
            Ok(ComfyUploadedInput {
                name: format!("server_{}", upload.filename),
                subfolder: String::new(),
                folder_type: "input".to_owned(),
            })
        }

        async fn get_history(&self, _prompt_id: &str) -> Result<ComfyHistory, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
        }

        async fn download_output(
            &self,
            _file: &ComfyOutputFile,
        ) -> Result<ComfyOutputData, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
        }

        async fn submit_workflow(
            &self,
            _client_id: &str,
            _prompt_id: &str,
            _workflow: serde_json::Value,
        ) -> Result<PromptSubmission, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
        }

        async fn subscribe_events(
            &self,
            _client_id: &str,
        ) -> Result<Box<dyn ComfyEventSubscription>, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
        }
    }

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

    fn video_asset(id: &str, project_id: &str) -> Asset {
        Asset::new_source_video(
            AssetId::parse(id).unwrap(),
            project_id,
            "reference.mp4",
            "reference.mp4",
            "C:/project/reference.mp4",
            "b".repeat(64),
            "video/mp4",
            Some(1280),
            Some(720),
            Some(1000),
            10,
            json!({}),
            chrono::Utc::now(),
        )
        .unwrap()
    }

    fn audio_asset(id: &str, project_id: &str) -> Asset {
        Asset::new_source_audio(
            AssetId::parse(id).unwrap(),
            project_id,
            "reference.wav",
            "reference.wav",
            "C:/project/reference.wav",
            "c".repeat(64),
            "audio/wav",
            Some(1000),
            10,
            json!({}),
            chrono::Utc::now(),
        )
        .unwrap()
    }

    fn preparer(assets: Vec<Asset>, adapter: RecordingAdapter) -> GenerationInputPreparer {
        let repository = FakeAssetRepository::default();
        repository.assets.lock().unwrap().extend(
            assets
                .into_iter()
                .map(|asset| (asset.id.as_str().to_owned(), asset)),
        );
        GenerationInputPreparer::new(
            Arc::new(repository),
            Arc::new(StreamingAssetStore),
            Arc::new(TestProjectRepository),
            Arc::new(adapter),
        )
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

    #[tokio::test]
    async fn prepares_video_audio_and_ordered_media_with_streaming_uploads() {
        let adapter = RecordingAdapter::default();
        let filenames = adapter.filenames.clone();
        let preparer = preparer(
            vec![
                video_asset("ast_video_a", "project-1"),
                video_asset("ast_video_b", "project-1"),
                audio_asset("ast_audio_a", "project-1"),
            ],
            adapter,
        );
        let values = BTreeMap::from([
            (
                "video".to_owned(),
                GenerationInputValue::VideoAsset(AssetId::parse("ast_video_a").unwrap()),
            ),
            (
                "audio".to_owned(),
                GenerationInputValue::AudioAsset(AssetId::parse("ast_audio_a").unwrap()),
            ),
            (
                "videos".to_owned(),
                GenerationInputValue::VideoAssets(vec![
                    AssetId::parse("ast_video_a").unwrap(),
                    AssetId::parse("ast_video_b").unwrap(),
                ]),
            ),
        ]);

        preparer
            .validate_asset_references("project-1", &values)
            .await
            .unwrap();
        let prepared = preparer
            .prepare("project-1", &TaskId::parse("tsk_media").unwrap(), &values)
            .await
            .unwrap();
        assert!(matches!(
            prepared.compiler_values["video"],
            crate::domain::InputValue::Video(ref value) if value.starts_with("server_")
        ));
        assert_eq!(prepared.media["videos"].len(), 2);
        assert_eq!(prepared.media["videos"][0].asset_id.as_str(), "ast_video_a");
        assert_eq!(prepared.media["videos"][1].asset_id.as_str(), "ast_video_b");
        let filenames = filenames.lock().unwrap();
        assert_eq!(filenames.len(), 3);
        assert_eq!(
            filenames
                .iter()
                .filter(|name| name.ends_with(".mp4"))
                .count(),
            2
        );
        assert_eq!(
            filenames
                .iter()
                .filter(|name| name.ends_with(".wav"))
                .count(),
            1
        );
        assert!(filenames.iter().any(|name| name.contains("_02.mp4")));
    }

    #[tokio::test]
    async fn reuses_one_image_upload_for_same_asset_referenced_by_non_duplicate_inputs() {
        let adapter = RecordingAdapter::default();
        let filenames = adapter.filenames.clone();
        let preparer = preparer(vec![asset()], adapter);
        let values = BTreeMap::from([
            (
                "first_frame".to_owned(),
                GenerationInputValue::ImageAsset(AssetId::parse("ast_reference").unwrap()),
            ),
            (
                "reference_image".to_owned(),
                GenerationInputValue::ImageAsset(AssetId::parse("ast_reference").unwrap()),
            ),
        ]);

        let prepared = preparer
            .prepare(
                "project-1",
                &TaskId::parse("tsk_first_last").unwrap(),
                &values,
            )
            .await
            .expect("same image references should reuse one successful upload");

        assert_eq!(filenames.lock().unwrap().len(), 1);
        assert_eq!(
            prepared.images["first_frame"][0].comfy,
            prepared.images["reference_image"][0].comfy
        );
    }

    #[tokio::test]
    async fn probes_comfy_once_before_multiple_reference_uploads() {
        let adapter = RecordingAdapter::default();
        let health_checks = adapter.health_checks.clone();
        let mut second = asset();
        second.id = AssetId::parse("ast_reference_b").unwrap();
        let preparer = preparer(vec![asset(), second], adapter);
        let values = BTreeMap::from([(
            "reference_images".to_owned(),
            GenerationInputValue::ImageAssets(vec![
                AssetId::parse("ast_reference").unwrap(),
                AssetId::parse("ast_reference_b").unwrap(),
            ]),
        )]);

        preparer
            .prepare(
                "project-1",
                &TaskId::parse("tsk_health_probe").unwrap(),
                &values,
            )
            .await
            .expect("healthy ComfyUI should allow image preparation");

        assert_eq!(*health_checks.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn health_probe_failure_prevents_uploading_any_input() {
        let adapter = RecordingAdapter {
            health_failure: Arc::new(Mutex::new(Some(ComfyAdapterError::Offline(
                "ComfyUI is unavailable".to_owned(),
            )))),
            ..RecordingAdapter::default()
        };
        let filenames = adapter.filenames.clone();
        let preparer = preparer(vec![asset()], adapter);
        let values = BTreeMap::from([(
            "reference_image".to_owned(),
            GenerationInputValue::ImageAsset(AssetId::parse("ast_reference").unwrap()),
        )]);

        let error = preparer
            .prepare(
                "project-1",
                &TaskId::parse("tsk_health_failure").unwrap(),
                &values,
            )
            .await
            .expect_err("an unavailable ComfyUI should stop before upload");

        assert_eq!(error.code(), "COMFY_OFFLINE");
        assert!(error.to_string().contains("已取消上传"));
        assert!(filenames.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn rejects_duplicate_first_last_assets_before_upload() {
        let adapter = RecordingAdapter::default();
        let filenames = adapter.filenames.clone();
        let preparer = preparer(vec![asset()], adapter);
        let values = BTreeMap::from([
            (
                "first_frame".to_owned(),
                GenerationInputValue::ImageAsset(AssetId::parse("ast_reference").unwrap()),
            ),
            (
                "last_frame".to_owned(),
                GenerationInputValue::ImageAsset(AssetId::parse("ast_reference").unwrap()),
            ),
        ]);

        let error = preparer
            .validate_asset_references("project-1", &values)
            .await
            .expect_err("duplicate first/last references should be rejected");

        assert_eq!(error.code(), "INPUT_ASSET_DUPLICATE");
        assert!(filenames.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn upload_errors_include_input_and_asset_context() {
        let adapter = RecordingAdapter {
            failure: Arc::new(Mutex::new(Some(ComfyAdapterError::Timeout(
                "POST /upload/image timed out".to_owned(),
            )))),
            ..RecordingAdapter::default()
        };
        let preparer = preparer(vec![asset()], adapter);
        let values = BTreeMap::from([(
            "first_frame".to_owned(),
            GenerationInputValue::ImageAsset(AssetId::parse("ast_reference").unwrap()),
        )]);

        let error = preparer
            .prepare(
                "project-1",
                &TaskId::parse("tsk_upload_error").unwrap(),
                &values,
            )
            .await
            .expect_err("the configured upload failure should propagate");

        assert_eq!(error.code(), "COMFY_TIMEOUT");
        let message = error.to_string();
        assert!(message.contains("input first_frame"));
        assert!(message.contains("asset ast_reference"));
    }

    #[tokio::test]
    async fn rejects_missing_wrong_type_and_cross_project_media_before_upload() {
        let adapter = RecordingAdapter::default();
        let filenames = adapter.filenames.clone();
        let preparer = preparer(
            vec![
                video_asset("ast_video", "project-1"),
                audio_asset("ast_audio", "project-1"),
                video_asset("ast_other", "project-2"),
            ],
            adapter,
        );
        let cases = [
            (
                GenerationInputValue::VideoAsset(AssetId::parse("ast_audio").unwrap()),
                "INPUT_ASSET_TYPE_INVALID",
            ),
            (
                GenerationInputValue::VideoAsset(AssetId::parse("ast_other").unwrap()),
                "INPUT_ASSET_PROJECT_MISMATCH",
            ),
            (
                GenerationInputValue::AudioAsset(AssetId::parse("ast_missing").unwrap()),
                "INPUT_ASSET_NOT_FOUND",
            ),
        ];
        for (value, expected_code) in cases {
            let values = BTreeMap::from([("media".to_owned(), value)]);
            let error = preparer
                .validate_asset_references("project-1", &values)
                .await
                .unwrap_err();
            assert_eq!(error.code(), expected_code);
        }
        assert!(filenames.lock().unwrap().is_empty());
    }

    #[test]
    fn downscales_only_the_comfy_upload_copy() {
        let source = image::DynamicImage::new_rgb8(4096, 1024);
        let mut encoded = Cursor::new(Vec::new());
        source
            .write_to(&mut encoded, ImageFormat::Png)
            .expect("test image should encode");
        let original = encoded.into_inner();
        let original_len = original.len();
        let mut image_asset = asset();
        image_asset.width = 4096;
        image_asset.height = 1024;

        let upload_copy = prepare_image_bytes_for_comfy(original.clone(), &image_asset)
            .expect("large PNG should be resized");
        let resized = ImageReader::with_format(Cursor::new(upload_copy), ImageFormat::Png)
            .decode()
            .expect("resized image should remain valid");

        assert_eq!((resized.width(), resized.height()), (2048, 512));
        assert_eq!(original.len(), original_len);
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

pub fn media_snapshot_value(prepared: &PreparedMediaInput) -> serde_json::Value {
    json!({
        "assetId": prepared.asset_id.as_str(),
        "sha256": prepared.sha256,
        "comfy": {
            "name": prepared.comfy.name,
            "subfolder": prepared.comfy.subfolder,
            "type": prepared.comfy.folder_type
        }
    })
}

pub fn media_list_snapshot_value(prepared: &[PreparedMediaInput]) -> serde_json::Value {
    serde_json::Value::Array(prepared.iter().map(media_snapshot_value).collect())
}

fn repository_error(error: RepositoryError) -> GenerationInputPrepareError {
    GenerationInputPrepareError::Repository(error.to_string())
}
