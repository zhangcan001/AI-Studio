use crate::application::ports::{
    AssetReadStream, AssetRepository, AssetStore, ComfyAdapter, ComfyAdapterError,
    ComfyInputStream, ComfyInputUpload, ComfyUploadedInput, RepositoryError,
};
use crate::domain::{
    Asset, AssetId, AssetType, InputValue, SeedValue, TaskId, GENERATED_VIDEO_CATEGORY,
    SOURCE_AUDIO_CATEGORY, SOURCE_VIDEO_CATEGORY,
};
use serde_json::json;
use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
    fmt,
    sync::Arc,
};

const IMAGE_PREVIEW_REFERENCE: &str = "__aistudio_preflight_image__";

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
                ComfyAdapterError::InputUploadTooLarge(_) => "COMFY_INPUT_UPLOAD_TOO_LARGE",
                ComfyAdapterError::InputUpload(_) => "COMFY_INPUT_UPLOAD_FAILED",
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
                        .upload_image_asset(task_id, &asset, None, &mut upload_cache)
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
                            .upload_image_asset(task_id, &asset, Some(index + 1), &mut upload_cache)
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
                        .upload_media_asset(task_id, &asset, None, &mut upload_cache)
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
                        .upload_media_asset(task_id, &asset, None, &mut upload_cache)
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
                            .upload_media_asset(task_id, &asset, Some(index + 1), &mut upload_cache)
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
                            .upload_media_asset(task_id, &asset, Some(index + 1), &mut upload_cache)
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
        asset: &Asset,
        position: Option<usize>,
        _upload_cache: &mut HashMap<AssetId, ComfyUploadedInput>,
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
        let upload = crate::application::ports::ComfyImageUpload {
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

    async fn upload_media_asset(
        &self,
        task_id: &TaskId,
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
                .open_read_stream(std::path::Path::new(&asset.storage_path))
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
                .map_err(GenerationInputPrepareError::Upload)?;
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
    use super::{upload_name, upload_name_at, GenerationInputPreparer, GenerationInputValue};
    use crate::application::ports::{
        AssetReadStream, AssetRepository, AssetStore, AssetStoreError, ComfyAdapter,
        ComfyAdapterError, ComfyEventSubscription, ComfyHealth, ComfyHistory, ComfyInputUpload,
        ComfyOutputData, ComfyOutputFile, ComfyUploadedInput, PromptSubmission, RepositoryError,
        StoredAssetFile, SystemStats,
    };
    use crate::domain::{Asset, AssetId, TaskId};
    use async_trait::async_trait;
    use serde_json::json;
    use std::collections::{BTreeMap, HashMap, VecDeque};
    use std::path::Path;
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

        async fn read(&self, _path: &Path) -> Result<Vec<u8>, AssetStoreError> {
            Err(AssetStoreError::Read(
                "media must use a bounded read stream".to_owned(),
            ))
        }

        async fn open_read_stream(
            &self,
            _path: &Path,
        ) -> Result<Box<dyn AssetReadStream>, AssetStoreError> {
            Ok(Box::new(ChunkStream {
                chunks: VecDeque::from([vec![1, 2], vec![3, 4]]),
            }))
        }
    }

    #[derive(Clone, Default)]
    struct RecordingAdapter {
        filenames: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl ComfyAdapter for RecordingAdapter {
        async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
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
