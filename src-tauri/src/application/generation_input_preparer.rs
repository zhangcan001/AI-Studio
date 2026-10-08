use crate::application::ports::{
    AssetReadStream, AssetRepository, AssetStore, ComfyAdapter, ComfyAdapterError,
    ComfyImageUpload, ComfyInputStream, ComfyInputUpload, ComfyUploadContext, ComfyUploadedInput,
    ProjectRepository, RepositoryError,
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
    time::Instant,
};

const IMAGE_PREVIEW_REFERENCE: &str = "__aistudio_preflight_image__";
const MAX_COMFY_IMAGE_EDGE: u32 = 2048;
const MAX_COMFY_IMAGE_UPLOAD_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default)]
struct ImagePreprocessTimings {
    fast_path: bool,
    decode_ms: f64,
    resize_ms: f64,
    encode_ms: f64,
    total_ms: f64,
}

struct PreparedComfyImageBytes {
    bytes: Vec<u8>,
    timings: ImagePreprocessTimings,
}

fn elapsed_ms(started_at: Instant) -> f64 {
    started_at.elapsed().as_secs_f64() * 1_000.0
}

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
    H3ReferenceDuration {
        message: String,
    },
    ExternalImageRequired {
        asset_id: String,
    },
    Repository(String),
    Upload {
        input_key: String,
        asset_id: String,
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
            Self::ExternalImageRequired { .. } => "INPUT_EXTERNAL_IMAGE_REQUIRED",
            Self::InvalidAssetMime { .. } => "INPUT_ASSET_MIME_INVALID",
            Self::ReferenceMappingIncomplete { .. } => "REFERENCE_MAPPING_INCOMPLETE",
            Self::DuplicateFirstLastAsset { .. } => "INPUT_ASSET_DUPLICATE",
            Self::H3ReferenceDuration { message } => {
                if message.starts_with("BLOCKED_BY_METADATA") {
                    "BLOCKED_BY_METADATA"
                } else {
                    "INPUT_H3_REFERENCE_DURATION_INVALID"
                }
            }
            Self::Repository(_) => "INPUT_ASSET_REPOSITORY_ERROR",
            Self::Upload { error, .. } => comfy_error_code(error),
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
            Self::H3ReferenceDuration { message } => {
                write!(formatter, "{}: {message}", self.code())
            }
            Self::ExternalImageRequired { asset_id } => write!(
                formatter,
                "{}: asset {asset_id} 需要重新选择素材（受管理的正式外部导入图片）",
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
    /// New-generation policy only. Historical reads/preflights remain separate.
    /// Receipt, ownership, managed file, size, hash and decoded image metadata
    /// must all agree. No inference from a name, category or source tag.
    pub async fn validate_external_image_inputs(
        &self,
        project_id: &str,
        values: &BTreeMap<String, GenerationInputValue>,
    ) -> Result<(), GenerationInputPrepareError> {
        let mut ids = std::collections::HashSet::new();
        for value in values.values() {
            match value {
                GenerationInputValue::ImageAsset(id) => {
                    ids.insert(id.clone());
                }
                GenerationInputValue::ImageAssets(values) => {
                    ids.extend(values.iter().cloned());
                }
                _ => {}
            }
        }
        if ids.is_empty() {
            return Ok(());
        }
        let root = self
            .project_repository
            .get_storage_root(project_id)
            .await
            .map_err(repository_error)?
            .ok_or_else(|| {
                GenerationInputPrepareError::Repository("project storage missing".into())
            })?;
        for id in ids {
            let reject = || GenerationInputPrepareError::ExternalImageRequired {
                asset_id: id.as_str().into(),
            };
            let asset = self.load_image_asset(project_id, &id).await?;
            let receipt = self
                .asset_repository
                .find_external_import(project_id, &id)
                .await
                .map_err(repository_error)?
                .ok_or_else(reject)?;
            if asset.source_task_id.is_some()
                || asset.category != crate::domain::SOURCE_IMAGE_CATEGORY
                || receipt.asset_id != id.as_str()
                || receipt.project_id != project_id
                || receipt.media_type != "image"
                || receipt.sha256 != asset.sha256
                || receipt.mime_type != asset.mime_type
                || receipt.width != asset.width
                || receipt.height != asset.height
                || receipt.file_size != asset.file_size
                || receipt.duration_ms != asset.duration_ms
                || asset.file_size == 0
                || asset.file_size
                    > crate::application::source_asset_import_service::MAX_SOURCE_IMAGE_BYTES
            {
                return Err(reject());
            }
            let mut stream = self
                .asset_store
                .open_read_stream(&root, std::path::Path::new(&asset.storage_path))
                .await
                .map_err(|_| reject())?;
            let mut bytes = Vec::new();
            while let Some(chunk) = stream.next_chunk().await.map_err(|_| reject())? {
                if bytes.len() as u64 + chunk.len() as u64 > asset.file_size {
                    return Err(reject());
                }
                bytes.extend_from_slice(&chunk);
            }
            if bytes.len() as u64 != asset.file_size {
                return Err(reject());
            }
            let inspected = crate::application::image_inspection::inspect_bytes(&bytes)
                .map_err(|_| reject())?;
            if inspected.sha256 != receipt.sha256
                || inspected.mime_type != receipt.mime_type
                || inspected.width != receipt.width
                || inspected.height != receipt.height
            {
                return Err(reject());
            }
            // Header inspection is insufficient: truncated/corrupt pixel data fails closed.
            ImageReader::new(Cursor::new(&bytes))
                .with_guessed_format()
                .map_err(|_| reject())?
                .decode()
                .map_err(|_| reject())?;
        }
        Ok(())
    }

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

    /// The shared read-only media gate, before any new Task/Batch is written.
    pub async fn validate_product_assets(
        &self,
        project_id: &str,
        workflow: serde_json::Value,
        recipe: &crate::domain::Recipe,
        values: &BTreeMap<String, GenerationInputValue>,
    ) -> Result<(), GenerationInputPrepareError> {
        self.validate_asset_references(project_id, values).await?;
        self.validate_external_image_inputs(project_id, values)
            .await?;
        self.validate_local_asset_files(project_id, values).await?;
        self.validate_media_integrity(project_id, values).await?;
        self.validate_h3_reference_durations(project_id, workflow, recipe, values)
            .await
    }

    /// Recheck immutable managed media bytes, not just a still-existing filename.
    async fn validate_media_integrity(
        &self,
        project_id: &str,
        values: &BTreeMap<String, GenerationInputValue>,
    ) -> Result<(), GenerationInputPrepareError> {
        use sha2::{Digest, Sha256};
        let ids = values
            .values()
            .flat_map(|v| match v {
                GenerationInputValue::VideoAsset(id) | GenerationInputValue::AudioAsset(id) => {
                    vec![id]
                }
                GenerationInputValue::VideoAssets(ids) | GenerationInputValue::AudioAssets(ids) => {
                    ids.iter().collect()
                }
                _ => Vec::new(),
            })
            .collect::<std::collections::HashSet<_>>();
        if ids.is_empty() {
            return Ok(());
        }
        let root = self
            .project_repository
            .get_storage_root(project_id)
            .await
            .map_err(repository_error)?
            .ok_or_else(|| {
                GenerationInputPrepareError::Repository("project storage root unavailable".into())
            })?;
        for id in ids {
            let asset = self
                .asset_repository
                .find_by_id(id)
                .await
                .map_err(repository_error)?
                .ok_or_else(|| GenerationInputPrepareError::AssetNotFound {
                    asset_id: id.to_string(),
                })?;
            if asset.project_id != project_id
                || asset.file_size == 0
                || asset.duration_ms.unwrap_or(0) == 0
                || (asset.asset_type == AssetType::Video && (asset.width == 0 || asset.height == 0))
            {
                return Err(GenerationInputPrepareError::AssetRead {
                    asset_id: id.to_string(),
                    message: "managed media metadata is unavailable".into(),
                });
            }
            let mut stream = self
                .asset_store
                .open_read_stream(&root, std::path::Path::new(&asset.storage_path))
                .await
                .map_err(|_| GenerationInputPrepareError::AssetRead {
                    asset_id: id.to_string(),
                    message: "managed media is unavailable".into(),
                })?;
            let mut size = 0u64;
            let mut hash = Sha256::new();
            while let Some(bytes) =
                stream
                    .next_chunk()
                    .await
                    .map_err(|_| GenerationInputPrepareError::AssetRead {
                        asset_id: id.to_string(),
                        message: "managed media read failed".into(),
                    })?
            {
                size = size.saturating_add(bytes.len() as u64);
                if size > asset.file_size {
                    break;
                }
                hash.update(&bytes);
            }
            if size != asset.file_size || format!("{:x}", hash.finalize()) != asset.sha256 {
                return Err(GenerationInputPrepareError::AssetRead {
                    asset_id: id.to_string(),
                    message: "managed media integrity changed".into(),
                });
            }
        }
        Ok(())
    }

    pub async fn validate_h3_reference_durations(
        &self,
        project_id: &str,
        workflow: serde_json::Value,
        recipe: &crate::domain::Recipe,
        values: &BTreeMap<String, GenerationInputValue>,
    ) -> Result<(), GenerationInputPrepareError> {
        use crate::application::product::h3_resolution::{
            active_h3_kind, validate_reference_durations,
        };
        if active_h3_kind(workflow, recipe).as_deref() != Some("MiniMaxH3ReferenceToVideo") {
            return Ok(());
        }
        for key in ["reference_videos", "reference_audios"] {
            let (ids, expectation) = match values.get(key) {
                Some(GenerationInputValue::VideoAssets(ids)) => (ids, MediaExpectation::Video),
                Some(GenerationInputValue::AudioAssets(ids)) => (ids, MediaExpectation::Audio),
                _ => continue,
            };
            let mut durations = Vec::with_capacity(ids.len());
            for id in ids {
                let asset = self.load_media_asset(project_id, id, expectation).await?;
                durations.push(asset.duration_ms);
            }
            validate_reference_durations(key, &durations)
                .map_err(|message| GenerationInputPrepareError::H3ReferenceDuration { message })?;
        }
        Ok(())
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

    /// Read-only local-file check for the saved-version execution preflight.
    /// Keep filesystem paths out of returned diagnostics.
    pub async fn validate_local_asset_files(
        &self,
        project_id: &str,
        values: &BTreeMap<String, GenerationInputValue>,
    ) -> Result<(), GenerationInputPrepareError> {
        if !values.values().any(|value| {
            matches!(
                value,
                GenerationInputValue::ImageAsset(_)
                    | GenerationInputValue::ImageAssets(_)
                    | GenerationInputValue::VideoAsset(_)
                    | GenerationInputValue::VideoAssets(_)
                    | GenerationInputValue::AudioAsset(_)
                    | GenerationInputValue::AudioAssets(_)
            )
        }) {
            return Ok(());
        }
        let root = self
            .project_repository
            .get_storage_root(project_id)
            .await
            .map_err(repository_error)?
            .ok_or_else(|| {
                GenerationInputPrepareError::Repository(
                    "project storage root is unavailable".to_owned(),
                )
            })?;
        for value in values.values() {
            let ids: Vec<&AssetId> = match value {
                GenerationInputValue::ImageAsset(id)
                | GenerationInputValue::VideoAsset(id)
                | GenerationInputValue::AudioAsset(id) => vec![id],
                GenerationInputValue::ImageAssets(ids)
                | GenerationInputValue::VideoAssets(ids)
                | GenerationInputValue::AudioAssets(ids) => ids.iter().collect(),
                _ => Vec::new(),
            };
            for id in ids {
                let asset = self
                    .asset_repository
                    .find_by_id(id)
                    .await
                    .map_err(repository_error)?
                    .ok_or_else(|| GenerationInputPrepareError::AssetNotFound {
                        asset_id: id.as_str().to_owned(),
                    })?;
                if asset.project_id != project_id
                    || crate::application::ports::validate_asset_read_path(
                        &root,
                        std::path::Path::new(&asset.storage_path),
                    )
                    .is_err()
                {
                    return Err(GenerationInputPrepareError::Repository(
                        "referenced local input file is unavailable".to_owned(),
                    ));
                }
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
            let bytes = self
                .asset_store
                .read(project_root, std::path::Path::new(&asset.storage_path))
                .await
                .map_err(|error| GenerationInputPrepareError::AssetRead {
                    asset_id: asset.id.as_str().to_owned(),
                    message: error.to_string(),
                })?;
            let original_bytes = bytes.len();
            let preprocess_started_at = Instant::now();
            let (bytes, mut timings) = if asset.width.max(asset.height) <= MAX_COMFY_IMAGE_EDGE
                && original_bytes <= MAX_COMFY_IMAGE_UPLOAD_BYTES
            {
                // Preserve the zero-copy fast path for already-safe image uploads.
                (
                    bytes,
                    ImagePreprocessTimings {
                        fast_path: true,
                        ..ImagePreprocessTimings::default()
                    },
                )
            } else {
                let preprocess_asset = asset.clone();
                let prepared = tokio::task::spawn_blocking(move || {
                    prepare_image_bytes_for_comfy(bytes, &preprocess_asset)
                })
                .await
                .map_err(|error| GenerationInputPrepareError::AssetRead {
                    asset_id: asset.id.as_str().to_owned(),
                    message: format!("image preparation worker failed: {error}"),
                })?
                .map_err(|message| GenerationInputPrepareError::AssetRead {
                    asset_id: asset.id.as_str().to_owned(),
                    message: format!("image preparation failed: {message}"),
                })?;
                (prepared.bytes, prepared.timings)
            };
            // Include spawn_blocking scheduling/wait in total wall time, but not in the
            // decode/resize/encode stage timings collected inside the CPU worker.
            timings.total_ms = elapsed_ms(preprocess_started_at);
            if bytes.len() != original_bytes {
                tracing::debug!(
                    asset_id = %asset.id,
                    original_bytes,
                    upload_bytes = bytes.len(),
                    preprocess_elapsed_ms = timings.total_ms,
                    "ComfyUI image upload copy downscaled"
                );
            }
            let upload_name = upload_name_at(task_id, asset, position);
            tracing::debug!(
                phase = "image_preprocess",
                task_id = %task_id,
                asset_id = %asset.id,
                filename = %upload_name,
                source_bytes = original_bytes as u64,
                upload_bytes = bytes.len() as u64,
                width = asset.width,
                height = asset.height,
                attempt = 1usize,
                attempt_elapsed_ms = 0u64,
                total_elapsed_ms = 0u64,
                fast_path = timings.fast_path,
                decode_ms = timings.decode_ms,
                resize_ms = timings.resize_ms,
                encode_ms = timings.encode_ms,
                total_ms = timings.total_ms,
                http_status = Option::<u16>::None,
                error_class = "",
                "ComfyUI image preprocessing timings"
            );
            let upload = ComfyImageUpload {
                bytes,
                upload_name,
                content_type: asset.mime_type.clone(),
                context: Some(ComfyUploadContext {
                    task_id: Some(task_id.as_str().to_owned()),
                    asset_id: Some(asset.id.as_str().to_owned()),
                    source_bytes: Some(original_bytes as u64),
                    width: Some(asset.width),
                    height: Some(asset.height),
                }),
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
                context: Some(ComfyUploadContext {
                    task_id: Some(task_id.as_str().to_owned()),
                    asset_id: Some(asset.id.as_str().to_owned()),
                    source_bytes: Some(asset.file_size),
                    width: (asset.width > 0).then_some(asset.width),
                    height: (asset.height > 0).then_some(asset.height),
                }),
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

fn prepare_image_bytes_for_comfy(
    bytes: Vec<u8>,
    asset: &Asset,
) -> Result<PreparedComfyImageBytes, String> {
    let total_started_at = Instant::now();
    if asset.width.max(asset.height) <= MAX_COMFY_IMAGE_EDGE
        && bytes.len() <= MAX_COMFY_IMAGE_UPLOAD_BYTES
    {
        return Ok(PreparedComfyImageBytes {
            bytes,
            timings: ImagePreprocessTimings {
                fast_path: true,
                total_ms: elapsed_ms(total_started_at),
                ..ImagePreprocessTimings::default()
            },
        });
    }

    let format = match asset.mime_type.as_str() {
        "image/png" => ImageFormat::Png,
        "image/jpeg" => ImageFormat::Jpeg,
        "image/webp" => ImageFormat::WebP,
        mime_type => return Err(format!("unsupported image MIME type {mime_type}")),
    };
    let decode_started_at = Instant::now();
    let image = ImageReader::with_format(Cursor::new(bytes.as_slice()), format)
        .decode()
        .map_err(|error| error.to_string())?;
    let decode_ms = elapsed_ms(decode_started_at);
    let (width, height) = (image.width(), image.height());
    let mut target_width = width;
    let mut target_height = height;
    if width.max(height) > MAX_COMFY_IMAGE_EDGE {
        let scale = f64::from(MAX_COMFY_IMAGE_EDGE) / f64::from(width.max(height));
        target_width = (f64::from(width) * scale).round().max(1.0) as u32;
        target_height = (f64::from(height) * scale).round().max(1.0) as u32;
    }

    let mut resize_ms = 0.0;
    let mut resized = if (target_width, target_height) == (width, height) {
        image
    } else {
        let resize_started_at = Instant::now();
        let resized = image.resize_exact(target_width, target_height, FilterType::Lanczos3);
        resize_ms += elapsed_ms(resize_started_at);
        resized
    };
    let mut encode_ms = 0.0;
    let encode_started_at = Instant::now();
    let mut encoded = encode_image_for_comfy(&resized, format)?;
    encode_ms += elapsed_ms(encode_started_at);
    let mut resize_attempt = 0;
    while encoded.len() > MAX_COMFY_IMAGE_UPLOAD_BYTES {
        resize_attempt += 1;
        if resize_attempt > 12 {
            return Err(format!(
                "encoded image remains larger than {} bytes after resizing",
                MAX_COMFY_IMAGE_UPLOAD_BYTES
            ));
        }

        let scale = (MAX_COMFY_IMAGE_UPLOAD_BYTES as f64 / encoded.len() as f64).sqrt() * 0.95;
        let next_width = (f64::from(resized.width()) * scale).floor().max(1.0) as u32;
        let next_height = (f64::from(resized.height()) * scale).floor().max(1.0) as u32;
        if (next_width, next_height) == (resized.width(), resized.height()) {
            return Err(format!(
                "encoded image cannot be reduced below {} bytes",
                MAX_COMFY_IMAGE_UPLOAD_BYTES
            ));
        }
        let resize_started_at = Instant::now();
        resized = resized.resize_exact(next_width, next_height, FilterType::Lanczos3);
        resize_ms += elapsed_ms(resize_started_at);
        let encode_started_at = Instant::now();
        encoded = encode_image_for_comfy(&resized, format)?;
        encode_ms += elapsed_ms(encode_started_at);
    }
    Ok(PreparedComfyImageBytes {
        bytes: encoded,
        timings: ImagePreprocessTimings {
            fast_path: false,
            decode_ms,
            resize_ms,
            encode_ms,
            total_ms: elapsed_ms(total_started_at),
        },
    })
}

fn encode_image_for_comfy(
    image: &image::DynamicImage,
    format: ImageFormat,
) -> Result<Vec<u8>, String> {
    let mut output = Cursor::new(Vec::new());
    image
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
        GenerationInputValue, MAX_COMFY_IMAGE_UPLOAD_BYTES,
    };
    use crate::application::ports::{
        AssetReadStream, AssetRepository, AssetStore, AssetStoreError, ComfyAdapter,
        ComfyAdapterError, ComfyEventSubscription, ComfyHealth, ComfyHistory, ComfyInputUpload,
        ComfyOutputData, ComfyOutputFile, ComfyUploadedInput, ProjectRecord, ProjectRepository,
        PromptSubmission, RepositoryError, StoredAssetFile, SystemStats,
    };
    use crate::domain::{Asset, AssetId, TaskId};
    use async_trait::async_trait;
    use image::{ColorType, ImageFormat, ImageReader};
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
    }

    #[async_trait]
    impl ComfyAdapter for RecordingAdapter {
        async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
            Err(ComfyAdapterError::Timeout(
                "health endpoint intentionally timed out in upload tests".to_owned(),
            ))
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

    #[test]
    fn safe_image_preserves_bytes_and_reports_fast_path_timings() {
        let mut source = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2, 2)
            .write_to(&mut source, ImageFormat::Png)
            .expect("test image should encode");
        let original = source.into_inner();
        let prepared = prepare_image_bytes_for_comfy(original.clone(), &asset())
            .expect("safe image should remain unchanged");

        assert_eq!(prepared.bytes, original);
        assert!(prepared.timings.fast_path);
        assert_eq!(prepared.timings.decode_ms, 0.0);
        assert_eq!(prepared.timings.resize_ms, 0.0);
        assert_eq!(prepared.timings.encode_ms, 0.0);
        assert!(prepared.timings.total_ms >= 0.0);
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

    #[tokio::test]
    async fn h3_reference_duration_admission_reads_real_project_scoped_assets_without_upload() {
        let recipe = crate::compiler::RecipeParser::parse(include_str!(
            "../../runtime_packages/minimax_h3_reference_video_quality_2_2_0/recipe.yaml"
        ))
        .unwrap();
        let workflow: serde_json::Value = serde_json::from_str(include_str!(
            "../../runtime_packages/minimax_h3_reference_video_quality_2_2_0/workflow_api.json"
        ))
        .unwrap();
        for audio in [false, true] {
            let key = if audio {
                "reference_audios"
            } else {
                "reference_videos"
            };
            for (duration, ok) in [
                (Some(1900), false),
                (Some(2000), true),
                (Some(15000), true),
                (Some(15001), false),
                (None, false),
            ] {
                let mut asset = if audio {
                    audio_asset("ast_media", "project-1")
                } else {
                    video_asset("ast_media", "project-1")
                };
                asset.duration_ms = duration;
                let id = asset.id.clone();
                let adapter = RecordingAdapter::default();
                let uploads = adapter.filenames.clone();
                let preparer = preparer(vec![asset], adapter);
                let value = if audio {
                    GenerationInputValue::AudioAssets(vec![id])
                } else {
                    GenerationInputValue::VideoAssets(vec![id])
                };
                let values = [(key.into(), value)].into();
                let result = preparer
                    .validate_h3_reference_durations(
                        "project-1",
                        workflow.clone(),
                        &recipe,
                        &values,
                    )
                    .await;
                assert_eq!(result.is_ok(), ok);
                if duration.is_none() {
                    assert_eq!(result.unwrap_err().code(), "BLOCKED_BY_METADATA");
                }
                assert!(preparer
                    .validate_h3_reference_durations(
                        "other-project",
                        workflow.clone(),
                        &recipe,
                        &values
                    )
                    .await
                    .is_err());
                assert!(uploads.lock().unwrap().is_empty());
            }
            let mut first = if audio {
                audio_asset("ast_first", "project-1")
            } else {
                video_asset("ast_first", "project-1")
            };
            let mut second = if audio {
                audio_asset("ast_second", "project-1")
            } else {
                video_asset("ast_second", "project-1")
            };
            first.duration_ms = Some(8000);
            second.duration_ms = Some(8000);
            let ids = vec![first.id.clone(), second.id.clone()];
            let value = if audio {
                GenerationInputValue::AudioAssets(ids)
            } else {
                GenerationInputValue::VideoAssets(ids)
            };
            let preparer = preparer(vec![first, second], RecordingAdapter::default());
            assert!(preparer
                .validate_h3_reference_durations(
                    "project-1",
                    workflow.clone(),
                    &recipe,
                    &[(key.into(), value)].into()
                )
                .await
                .is_err());
        }
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
    async fn saved_version_preflight_rejects_missing_local_asset_file_without_leaking_path() {
        let mut missing = video_asset("ast_missing_local", "project-1");
        missing.storage_path = "missing-phase2a-input-file.mp4".to_owned();
        let preparer = preparer(vec![missing], RecordingAdapter::default());
        let values = BTreeMap::from([(
            "video".to_owned(),
            GenerationInputValue::VideoAsset(AssetId::parse("ast_missing_local").unwrap()),
        )]);
        let error = preparer
            .validate_local_asset_files("project-1", &values)
            .await
            .unwrap_err();
        assert!(!error.to_string().contains("missing-phase2a-input-file.mp4"));
        assert!(error
            .to_string()
            .contains("referenced local input file is unavailable"));
        preparer
            .validate_local_asset_files("project-1", &BTreeMap::new())
            .await
            .unwrap();
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
    async fn upload_does_not_require_a_timed_out_health_endpoint() {
        let adapter = RecordingAdapter::default();
        let filenames = adapter.filenames.clone();
        let preparer = preparer(vec![asset()], adapter);
        let values = BTreeMap::from([(
            "reference_image".to_owned(),
            GenerationInputValue::ImageAsset(AssetId::parse("ast_reference").unwrap()),
        )]);

        preparer
            .prepare(
                "project-1",
                &TaskId::parse("tsk_upload_without_health").unwrap(),
                &values,
            )
            .await
            .expect("upload should be authoritative when health is unavailable");

        assert_eq!(filenames.lock().unwrap().len(), 1);
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
        let mut source = image::RgbaImage::new(4096, 1024);
        source.put_pixel(0, 0, image::Rgba([255, 0, 0, 0]));
        let source = image::DynamicImage::ImageRgba8(source);
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
            .expect("large PNG should be resized")
            .bytes;
        let resized = ImageReader::with_format(Cursor::new(upload_copy), ImageFormat::Png)
            .decode()
            .expect("resized image should remain valid");

        assert_eq!((resized.width(), resized.height()), (2048, 512));
        assert_eq!(resized.color(), ColorType::Rgba8);
        assert_eq!(original.len(), original_len);
    }

    #[test]
    fn reduces_a_2048px_image_when_encoded_bytes_still_exceed_limit() {
        let mut source = image::RgbaImage::new(2048, 2048);
        let mut state = 0x1234_5678u32;
        for pixel in source.pixels_mut() {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *pixel = image::Rgba([
                (state >> 24) as u8,
                (state >> 16) as u8,
                (state >> 8) as u8,
                state as u8,
            ]);
        }
        let source = image::DynamicImage::ImageRgba8(source);
        let mut encoded = Cursor::new(Vec::new());
        source
            .write_to(&mut encoded, ImageFormat::Png)
            .expect("high-entropy test image should encode");
        let original = encoded.into_inner();
        assert!(original.len() > MAX_COMFY_IMAGE_UPLOAD_BYTES);

        let mut image_asset = asset();
        image_asset.width = 2048;
        image_asset.height = 2048;
        let upload_copy = prepare_image_bytes_for_comfy(original, &image_asset)
            .expect("oversized encoded image should be reduced")
            .bytes;
        assert!(upload_copy.len() <= MAX_COMFY_IMAGE_UPLOAD_BYTES);
        let resized = ImageReader::with_format(Cursor::new(upload_copy), ImageFormat::Png)
            .decode()
            .expect("reduced image should remain valid");
        assert!(resized.width().max(resized.height()) < 2048);
        assert_eq!(resized.color(), ColorType::Rgba8);
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
