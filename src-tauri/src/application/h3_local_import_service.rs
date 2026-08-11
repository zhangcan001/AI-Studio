use crate::application::asset_video_prompt_service::AssetVideoPromptService;
use crate::application::generation_input_preparer::GenerationInputValue;
use crate::application::ports::Clock;
use crate::application::production_queue_service::{
    CreateProductionBatchItem, CreateProductionBatchRequest, ProductionQueueService,
};
use crate::application::source_asset_import_service::{
    SourceAssetImportService, MAX_SOURCE_IMAGE_BYTES,
};
use crate::domain::{Asset, SeedValue};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    error::Error,
    fmt, fs,
    path::{Component, Path, PathBuf},
    sync::Arc,
};
use tokio::sync::Mutex;
use uuid::Uuid;

pub const MAX_LOCAL_IMPORT_PAIRS: usize = 100;
pub const MAX_LOCAL_IMPORT_SESSION_MINUTES: i64 = 20;
const MAX_MANIFEST_BYTES: u64 = 10 * 1024 * 1024;
const SESSION_TTL: Duration = Duration::minutes(MAX_LOCAL_IMPORT_SESSION_MINUTES);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum H3LocalImportMode {
    Pairing,
    Manifest,
    Text,
    FirstLast,
    OmniManifest,
}

impl H3LocalImportMode {
    pub fn parse(value: &str) -> Result<Self, H3LocalImportError> {
        match value.trim() {
            "PAIRING" | "pairing" => Ok(Self::Pairing),
            "MANIFEST" | "manifest" => Ok(Self::Manifest),
            "TEXT" | "text" => Ok(Self::Text),
            "FIRST_LAST" | "first_last" => Ok(Self::FirstLast),
            "OMNI_MANIFEST" | "omni_manifest" => Ok(Self::OmniManifest),
            _ => Err(H3LocalImportError::InvalidInput(
                "本地批量导入模式无效".to_owned(),
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pairing => "PAIRING",
            Self::Manifest => "MANIFEST",
            Self::Text => "TEXT",
            Self::FirstLast => "FIRST_LAST",
            Self::OmniManifest => "OMNI_MANIFEST",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum H3LocalPairStatus {
    Ready,
    MissingPrompt,
    MissingImage,
    AmbiguousPrompt,
    AmbiguousImage,
    InvalidPromptEncoding,
    EmptyPrompt,
    PromptTooLarge,
    InvalidImage,
    ImageTooLarge,
    InvalidPath,
    DuplicateImageEntry,
    UnknownImage,
}

impl H3LocalPairStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "READY",
            Self::MissingPrompt => "MISSING_PROMPT",
            Self::MissingImage => "MISSING_IMAGE",
            Self::AmbiguousPrompt => "AMBIGUOUS_PROMPT",
            Self::AmbiguousImage => "AMBIGUOUS_IMAGE",
            Self::InvalidPromptEncoding => "INVALID_PROMPT_ENCODING",
            Self::EmptyPrompt => "EMPTY_PROMPT",
            Self::PromptTooLarge => "PROMPT_TOO_LARGE",
            Self::InvalidImage => "INVALID_IMAGE",
            Self::ImageTooLarge => "IMAGE_TOO_LARGE",
            Self::InvalidPath => "INVALID_PATH",
            Self::DuplicateImageEntry => "DUPLICATE_IMAGE_ENTRY",
            Self::UnknownImage => "UNKNOWN_IMAGE",
        }
    }

    pub fn is_ready(self) -> bool {
        self == Self::Ready
    }
}

#[derive(Clone, Debug)]
pub struct H3LocalImportPair {
    pub ordinal: usize,
    pub image_display_name: String,
    pub prompt_display_name: String,
    pub prompt_preview: Option<String>,
    pub prompt_text: Option<String>,
    pub prompt_bytes: Option<usize>,
    pub status: H3LocalPairStatus,
    pub image_path: Option<PathBuf>,
    pub prompt_path: Option<PathBuf>,
    pub image_sha256: Option<String>,
    pub image_paths: Vec<PathBuf>,
    pub image_sha256s: Vec<String>,
    pub last_image_display_name: Option<String>,
    pub last_image_path: Option<PathBuf>,
    pub last_image_sha256: Option<String>,
    pub video_display_names: Vec<String>,
    pub video_paths: Vec<PathBuf>,
    pub audio_display_names: Vec<String>,
    pub audio_paths: Vec<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct H3LocalImportInspection {
    pub display_root_name: String,
    pub mode: H3LocalImportMode,
    pub detected_manifest: bool,
    pub image_count: usize,
    pub prompt_count: usize,
    pub ready_count: usize,
    pub error_count: usize,
    pub pairs: Vec<H3LocalImportPair>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct H3LocalImportCommitRequest {
    pub batch_name: Option<String>,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub width: i64,
    pub height: i64,
    pub duration_seconds: i64,
    pub seed: Option<SeedValue>,
    pub auto_start: bool,
    pub generation_mode: Option<String>,
}

#[derive(Clone, Debug)]
pub struct H3LocalImportResult {
    pub batch_id: String,
    pub batch_name: String,
    pub item_count: usize,
    pub imported_asset_count: usize,
    pub auto_started: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum H3LocalImportError {
    InvalidInput(String),
    SessionNotFound,
    SessionExpired,
    Filesystem(String),
    FilesystemBoundary(String),
    Inspection(String),
    AssetImport(String),
    Prompt(String),
    Queue(String),
}

impl fmt::Display for H3LocalImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(formatter, "H3_LOCAL_IMPORT_INVALID: {message}"),
            Self::SessionNotFound => {
                formatter.write_str("H3_LOCAL_IMPORT_SESSION_NOT_FOUND: 本地导入会话不存在")
            }
            Self::SessionExpired => formatter
                .write_str("H3_LOCAL_IMPORT_SESSION_EXPIRED: 本地导入会话已过期，请重新选择目录"),
            Self::Filesystem(message) => {
                write!(formatter, "H3_LOCAL_IMPORT_FILESYSTEM_ERROR: {message}")
            }
            Self::FilesystemBoundary(message) => {
                write!(formatter, "FILESYSTEM_BOUNDARY_ERROR: {message}")
            }
            Self::Inspection(message) => {
                write!(formatter, "H3_LOCAL_IMPORT_INSPECTION_ERROR: {message}")
            }
            Self::AssetImport(message) => {
                write!(formatter, "H3_LOCAL_IMPORT_ASSET_ERROR: {message}")
            }
            Self::Prompt(message) => write!(formatter, "H3_LOCAL_IMPORT_PROMPT_ERROR: {message}"),
            Self::Queue(message) => write!(formatter, "H3_LOCAL_IMPORT_QUEUE_ERROR: {message}"),
        }
    }
}

impl Error for H3LocalImportError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum H3CommitGenerationMode {
    LegacyReferenceImage,
    Fl2vaTextToVideo,
    Fl2vaImageToVideo,
    Fl2vaFirstLast,
    Ref2vaImage,
    Ref2vaAudio,
    Ref2vaImageAudio,
    Ref2vaVideoImage,
}

impl H3CommitGenerationMode {
    fn parse(value: Option<&str>) -> Result<Self, H3LocalImportError> {
        match value.map(str::trim).filter(|value| !value.is_empty()) {
            None => Ok(Self::LegacyReferenceImage),
            Some("FL2VA_TEXT_TO_VIDEO") => Ok(Self::Fl2vaTextToVideo),
            Some("FL2VA_IMAGE_TO_VIDEO") => Ok(Self::Fl2vaImageToVideo),
            Some("FL2VA_FIRST_LAST") => Ok(Self::Fl2vaFirstLast),
            Some("REF2VA_IMAGE") => Ok(Self::Ref2vaImage),
            Some("REF2VA_AUDIO") => Ok(Self::Ref2vaAudio),
            Some("REF2VA_IMAGE_AUDIO") => Ok(Self::Ref2vaImageAudio),
            Some("REF2VA_VIDEO_IMAGE") => Ok(Self::Ref2vaVideoImage),
            Some(_) => Err(H3LocalImportError::InvalidInput(
                "H3 生成模式无效".to_owned(),
            )),
        }
    }

    fn imported_asset_label(self) -> &'static str {
        if matches!(self, Self::LegacyReferenceImage) {
            "图片素材"
        } else {
            "素材"
        }
    }

    fn uses_first_frame(self) -> bool {
        matches!(self, Self::Fl2vaImageToVideo | Self::Fl2vaFirstLast)
    }

    fn uses_last_frame(self) -> bool {
        matches!(self, Self::Fl2vaFirstLast)
    }

    fn uses_reference_images(self) -> bool {
        matches!(
            self,
            Self::LegacyReferenceImage
                | Self::Ref2vaImage
                | Self::Ref2vaImageAudio
                | Self::Ref2vaVideoImage
        )
    }

    fn uses_reference_videos(self) -> bool {
        matches!(self, Self::Ref2vaVideoImage)
    }

    fn uses_reference_audios(self) -> bool {
        matches!(self, Self::Ref2vaAudio | Self::Ref2vaImageAudio)
    }
}

struct LocalImportSession {
    project_id: String,
    root_path: PathBuf,
    inspection: H3LocalImportInspection,
    expires_at: DateTime<Utc>,
}

pub struct H3LocalImportService {
    source_asset_import_service: Arc<SourceAssetImportService>,
    asset_video_prompt_service: Arc<AssetVideoPromptService>,
    production_queue_service: Arc<ProductionQueueService>,
    clock: Arc<dyn Clock>,
    sessions: Arc<Mutex<HashMap<String, LocalImportSession>>>,
}

impl H3LocalImportService {
    pub fn new(
        source_asset_import_service: Arc<SourceAssetImportService>,
        asset_video_prompt_service: Arc<AssetVideoPromptService>,
        production_queue_service: Arc<ProductionQueueService>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            source_asset_import_service,
            asset_video_prompt_service,
            production_queue_service,
            clock,
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn cleanup_expired_sessions(&self, keep_session_id: Option<&str>) {
        let now = self.clock.now();
        let mut sessions = self.sessions.lock().await;
        sessions.retain(|session_id, session| {
            keep_session_id.is_some_and(|keep| keep == session_id) || session.expires_at > now
        });
    }

    pub async fn pick(
        &self,
        project_id: &str,
        root_path: PathBuf,
        mode: H3LocalImportMode,
    ) -> Result<(String, H3LocalImportInspection), H3LocalImportError> {
        self.cleanup_expired_sessions(None).await;
        crate::domain::validate_project_id(project_id)
            .map_err(|error| H3LocalImportError::InvalidInput(error.to_string()))?;
        let root_path = canonical_directory(&root_path)?;
        let inspection = inspect_directory(&root_path, mode).await?;
        let session_id = format!("h3_local_{}", Uuid::new_v4());
        let session = LocalImportSession {
            project_id: project_id.to_owned(),
            root_path,
            inspection: inspection.clone(),
            expires_at: self.clock.now() + SESSION_TTL,
        };
        self.sessions
            .lock()
            .await
            .insert(session_id.clone(), session);
        Ok((session_id, inspection))
    }

    pub async fn rescan(
        &self,
        session_id: &str,
        mode: H3LocalImportMode,
    ) -> Result<H3LocalImportInspection, H3LocalImportError> {
        self.cleanup_expired_sessions(Some(session_id)).await;
        let root_path = {
            let sessions = self.sessions.lock().await;
            let session = sessions
                .get(session_id)
                .ok_or(H3LocalImportError::SessionNotFound)?;
            if session.expires_at <= self.clock.now() {
                return Err(H3LocalImportError::SessionExpired);
            }
            session.root_path.clone()
        };
        let inspection = inspect_directory(&root_path, mode).await?;
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or(H3LocalImportError::SessionNotFound)?;
        if session.expires_at <= self.clock.now() {
            sessions.remove(session_id);
            return Err(H3LocalImportError::SessionExpired);
        }
        session.inspection = inspection.clone();
        Ok(inspection)
    }

    pub async fn commit(
        &self,
        session_id: &str,
        request: H3LocalImportCommitRequest,
    ) -> Result<H3LocalImportResult, H3LocalImportError> {
        self.cleanup_expired_sessions(Some(session_id)).await;
        validate_commit_request(&request)?;
        let session = {
            let mut sessions = self.sessions.lock().await;
            let session = sessions
                .remove(session_id)
                .ok_or(H3LocalImportError::SessionNotFound)?;
            if session.expires_at <= self.clock.now() {
                return Err(H3LocalImportError::SessionExpired);
            }
            session
        };

        // The inspection is deliberately repeated at commit time. The selected folder is
        // never handed to the frontend and all file paths remain inside this short-lived
        // backend session.
        let inspection = inspect_directory(&session.root_path, session.inspection.mode).await?;
        if inspection.error_count > 0 || inspection.ready_count == 0 {
            return Err(H3LocalImportError::Inspection(format!(
                "目录检查未通过：{}",
                if inspection.errors.is_empty() {
                    "没有可生成的有效配对".to_owned()
                } else {
                    inspection.errors.join("；")
                }
            )));
        }
        if inspection.ready_count > MAX_LOCAL_IMPORT_PAIRS {
            return Err(H3LocalImportError::Inspection(format!(
                "本地批量最多支持 {MAX_LOCAL_IMPORT_PAIRS} 项"
            )));
        }

        let generation_mode = H3CommitGenerationMode::parse(request.generation_mode.as_deref())?;
        validate_local_import_mode(generation_mode, inspection.mode)?;
        for pair in inspection
            .pairs
            .iter()
            .filter(|pair| pair.status.is_ready())
        {
            validate_pair_media(pair, generation_mode)?;
        }

        let seed = request.seed.clone().unwrap_or(SeedValue::Random);
        let mut items = Vec::with_capacity(inspection.ready_count);
        let mut imported_asset_count = 0usize;
        let imported_label = generation_mode.imported_asset_label();
        for pair in inspection
            .pairs
            .iter()
            .filter(|pair| pair.status.is_ready())
        {
            let prompt_text = self.read_pair_prompt(&session.root_path, pair).await?;
            let mut reference_images = Vec::new();
            let mut first_frame = None;
            let mut last_frame = None;
            let mut reference_videos = Vec::new();
            let mut reference_audios = Vec::new();

            if generation_mode.uses_first_frame() || generation_mode.uses_reference_images() {
                let image_paths = if generation_mode.uses_reference_images() {
                    pair.image_paths.clone()
                } else {
                    vec![pair.image_path.clone().ok_or_else(|| {
                        H3LocalImportError::Inspection("有效配对缺少首帧图片路径".to_owned())
                    })?]
                };
                for (index, image_path) in image_paths.iter().enumerate() {
                    let asset = self
                        .import_image_asset(
                            &session.project_id,
                            &session.root_path,
                            image_path,
                            pair.image_sha256s.get(index).map(String::as_str),
                            imported_asset_count,
                            imported_label,
                        )
                        .await?;
                    imported_asset_count += 1;
                    self.set_imported_image_prompt(
                        &session.project_id,
                        &asset,
                        &prompt_text,
                        imported_asset_count,
                        imported_label,
                    )
                    .await?;
                    if generation_mode.uses_reference_images() {
                        reference_images.push(asset);
                    } else {
                        first_frame = Some(asset);
                    }
                }
            }

            if generation_mode.uses_last_frame() {
                let image_path = pair.last_image_path.as_ref().ok_or_else(|| {
                    H3LocalImportError::Inspection("有效配对缺少末帧图片路径".to_owned())
                })?;
                let asset = self
                    .import_image_asset(
                        &session.project_id,
                        &session.root_path,
                        image_path,
                        pair.last_image_sha256.as_deref(),
                        imported_asset_count,
                        imported_label,
                    )
                    .await?;
                imported_asset_count += 1;
                self.set_imported_image_prompt(
                    &session.project_id,
                    &asset,
                    &prompt_text,
                    imported_asset_count,
                    imported_label,
                )
                .await?;
                last_frame = Some(asset);
            }

            if generation_mode.uses_reference_videos() {
                for video_path in &pair.video_paths {
                    let asset = self
                        .import_media_asset(
                            &session.project_id,
                            &session.root_path,
                            video_path,
                            true,
                            imported_asset_count,
                            imported_label,
                        )
                        .await?;
                    imported_asset_count += 1;
                    reference_videos.push(asset);
                }
            }

            if generation_mode.uses_reference_audios() {
                for audio_path in &pair.audio_paths {
                    let asset = self
                        .import_media_asset(
                            &session.project_id,
                            &session.root_path,
                            audio_path,
                            false,
                            imported_asset_count,
                            imported_label,
                        )
                        .await?;
                    imported_asset_count += 1;
                    reference_audios.push(asset);
                }
            }

            let mut values = BTreeMap::new();
            values.insert("prompt".to_owned(), GenerationInputValue::Text(prompt_text));
            match generation_mode {
                H3CommitGenerationMode::LegacyReferenceImage => {
                    values.insert(
                        "reference_image".to_owned(),
                        GenerationInputValue::ImageAsset(
                            reference_images
                                .into_iter()
                                .next()
                                .ok_or_else(|| {
                                    H3LocalImportError::Inspection(
                                        "有效配对缺少参考图片".to_owned(),
                                    )
                                })?
                                .id,
                        ),
                    );
                }
                H3CommitGenerationMode::Fl2vaTextToVideo => {}
                H3CommitGenerationMode::Fl2vaImageToVideo => {
                    values.insert(
                        "first_frame".to_owned(),
                        GenerationInputValue::ImageAsset(
                            first_frame
                                .ok_or_else(|| {
                                    H3LocalImportError::Inspection(
                                        "有效配对缺少首帧图片".to_owned(),
                                    )
                                })?
                                .id,
                        ),
                    );
                }
                H3CommitGenerationMode::Fl2vaFirstLast => {
                    values.insert(
                        "first_frame".to_owned(),
                        GenerationInputValue::ImageAsset(
                            first_frame
                                .ok_or_else(|| {
                                    H3LocalImportError::Inspection(
                                        "有效配对缺少首帧图片".to_owned(),
                                    )
                                })?
                                .id,
                        ),
                    );
                    values.insert(
                        "last_frame".to_owned(),
                        GenerationInputValue::ImageAsset(
                            last_frame
                                .ok_or_else(|| {
                                    H3LocalImportError::Inspection(
                                        "有效配对缺少末帧图片".to_owned(),
                                    )
                                })?
                                .id,
                        ),
                    );
                }
                H3CommitGenerationMode::Ref2vaImage
                | H3CommitGenerationMode::Ref2vaImageAudio
                | H3CommitGenerationMode::Ref2vaVideoImage => {
                    values.insert(
                        "reference_images".to_owned(),
                        GenerationInputValue::ImageAssets(
                            reference_images.into_iter().map(|asset| asset.id).collect(),
                        ),
                    );
                    if matches!(generation_mode, H3CommitGenerationMode::Ref2vaImageAudio) {
                        values.insert(
                            "reference_audios".to_owned(),
                            GenerationInputValue::AudioAssets(
                                reference_audios.into_iter().map(|asset| asset.id).collect(),
                            ),
                        );
                    }
                    if matches!(generation_mode, H3CommitGenerationMode::Ref2vaVideoImage) {
                        values.insert(
                            "reference_videos".to_owned(),
                            GenerationInputValue::VideoAssets(
                                reference_videos.into_iter().map(|asset| asset.id).collect(),
                            ),
                        );
                    }
                }
                H3CommitGenerationMode::Ref2vaAudio => {
                    values.insert(
                        "reference_audios".to_owned(),
                        GenerationInputValue::AudioAssets(
                            reference_audios.into_iter().map(|asset| asset.id).collect(),
                        ),
                    );
                }
            }
            values.insert(
                "width".to_owned(),
                GenerationInputValue::Integer(request.width),
            );
            values.insert(
                "height".to_owned(),
                GenerationInputValue::Integer(request.height),
            );
            values.insert(
                "duration_seconds".to_owned(),
                GenerationInputValue::Integer(request.duration_seconds),
            );
            values.insert("seed".to_owned(), GenerationInputValue::Seed(seed.clone()));
            items.push(CreateProductionBatchItem {
                workflow_version_id: request.workflow_version_id.clone(),
                recipe_id: request.recipe_id.clone(),
                values,
            });
        }

        let batch_name = request
            .batch_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| {
                format!(
                    "H3 本地批量 · {} · {}",
                    inspection.display_root_name,
                    self.clock.now().format("%Y-%m-%d %H:%M:%S")
                )
            });
        let detail = self
            .production_queue_service
            .create(CreateProductionBatchRequest {
                project_id: session.project_id.clone(),
                name: batch_name.clone(),
                continue_on_failure: true,
                items,
            })
            .await
            .map_err(|error| {
                H3LocalImportError::Queue(format!(
                    "批次创建失败（已导入 {imported_asset_count} 个{imported_label}）：{error}"
                ))
            })?;

        let mut warnings = inspection.warnings;
        let mut auto_started = false;
        if request.auto_start {
            match self
                .production_queue_service
                .start(&session.project_id, detail.batch.id.as_str())
                .await
            {
                Ok(()) => auto_started = true,
                Err(error) => warnings.push(format!("批次已创建，但自动开始失败：{error}")),
            }
        }

        Ok(H3LocalImportResult {
            batch_id: detail.batch.id.as_str().to_owned(),
            batch_name,
            item_count: detail.items.len(),
            imported_asset_count,
            auto_started,
            warnings,
        })
    }

    async fn read_pair_prompt(
        &self,
        root_path: &Path,
        pair: &H3LocalImportPair,
    ) -> Result<String, H3LocalImportError> {
        if let Some(prompt_path) = pair.prompt_path.as_ref() {
            let prompt_path = revalidate_file_path(root_path, prompt_path)?;
            let prompt_bytes = tokio::fs::read(&prompt_path)
                .await
                .map_err(|_| H3LocalImportError::Filesystem("提示词在导入前无法读取".to_owned()))?;
            return parse_prompt_bytes(&prompt_bytes)
                .map(|(text, _)| text)
                .map_err(|error| {
                    H3LocalImportError::Inspection(format!("提示词在提交前发生变化：{error}"))
                });
        }
        pair.prompt_text
            .clone()
            .ok_or_else(|| H3LocalImportError::Inspection("清单配对缺少提示词".to_owned()))
    }

    async fn import_image_asset(
        &self,
        project_id: &str,
        root_path: &Path,
        path: &Path,
        expected_sha256: Option<&str>,
        imported_asset_count: usize,
        imported_label: &str,
    ) -> Result<Asset, H3LocalImportError> {
        let image_path = revalidate_file_path(root_path, path)?;
        let image_bytes = tokio::fs::read(&image_path)
            .await
            .map_err(|_| H3LocalImportError::Filesystem("图片在导入前无法读取".to_owned()))?;
        crate::application::image_inspection::inspect_bytes(&image_bytes)
            .map_err(|error| H3LocalImportError::Inspection(format!("图片校验失败：{error}")))?;
        if let Some(expected_sha256) = expected_sha256 {
            let actual_sha256 = format!("{:x}", Sha256::digest(&image_bytes));
            if actual_sha256 != expected_sha256 {
                return Err(H3LocalImportError::Inspection(
                    "目录内容在提交前发生变化，请重新扫描".to_owned(),
                ));
            }
        }
        if u64::try_from(image_bytes.len()).unwrap_or(u64::MAX) > MAX_SOURCE_IMAGE_BYTES {
            return Err(H3LocalImportError::Inspection(
                "图片超过现有素材导入上限".to_owned(),
            ));
        }
        let original_name = image_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("local-image.png");
        self.source_asset_import_service
            .import_bytes(project_id, original_name, &image_bytes)
            .await
            .map_err(|error| {
                H3LocalImportError::AssetImport(format!(
                    "已导入 {imported_asset_count} 个{imported_label}后失败：{error}"
                ))
            })
    }

    async fn set_imported_image_prompt(
        &self,
        project_id: &str,
        asset: &Asset,
        prompt_text: &str,
        imported_asset_count: usize,
        imported_label: &str,
    ) -> Result<(), H3LocalImportError> {
        self.asset_video_prompt_service
            .set(project_id, asset.id.as_str(), prompt_text)
            .await
            .map(|_| ())
            .map_err(|error| {
                H3LocalImportError::Prompt(format!(
                    "已导入 {imported_asset_count} 个{imported_label}后失败：{error}"
                ))
            })
    }

    async fn import_media_asset(
        &self,
        project_id: &str,
        root_path: &Path,
        path: &Path,
        is_video: bool,
        imported_asset_count: usize,
        imported_label: &str,
    ) -> Result<Asset, H3LocalImportError> {
        let path = revalidate_file_path(root_path, path)?;
        let result = if is_video {
            self.source_asset_import_service
                .import_video_file(project_id, &path)
                .await
        } else {
            self.source_asset_import_service
                .import_audio_file(project_id, &path)
                .await
        };
        result.map_err(|error| {
            H3LocalImportError::AssetImport(format!(
                "已导入 {imported_asset_count} 个{imported_label}后失败：{error}"
            ))
        })
    }
}

#[derive(Clone, Debug)]
struct ScannedFile {
    path: PathBuf,
    relative_name: String,
    extension: String,
}

#[derive(Debug)]
enum PromptIssue {
    Read,
    InvalidEncoding,
    Empty,
    TooLarge,
}

impl fmt::Display for PromptIssue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Read => "提示词文件无法读取",
            Self::InvalidEncoding => "提示词必须是 UTF-8 文本",
            Self::Empty => "提示词不能为空",
            Self::TooLarge => "提示词不能超过 64 KiB",
        })
    }
}

#[derive(Debug, Deserialize)]
struct ManifestEntry {
    image: String,
    prompt: String,
}

#[derive(Debug, Deserialize)]
struct ManifestDocument(Vec<ManifestEntry>);

async fn inspect_directory(
    root_path: &Path,
    mode: H3LocalImportMode,
) -> Result<H3LocalImportInspection, H3LocalImportError> {
    let canonical_root = fs::canonicalize(root_path)
        .map_err(|_| H3LocalImportError::Filesystem("所选任务目录无法读取".to_owned()))?;
    let root_path = canonical_root.as_path();
    let scanned_files = scan_files(root_path)?;
    let image_files = scanned_files
        .iter()
        .filter(|file| is_image_extension(&file.extension))
        .cloned()
        .collect::<Vec<_>>();
    let prompt_files = scanned_files
        .iter()
        .filter(|file| is_prompt_extension(&file.extension))
        .cloned()
        .collect::<Vec<_>>();
    let video_files = scanned_files
        .iter()
        .filter(|file| is_video_extension(&file.extension))
        .cloned()
        .collect::<Vec<_>>();
    let audio_files = scanned_files
        .iter()
        .filter(|file| is_audio_extension(&file.extension))
        .cloned()
        .collect::<Vec<_>>();
    let manifest_path = root_path.join("h3-batch.json");
    let omni_manifest_path = root_path.join("h3-omni-batch.json");
    let detected_manifest = fs::symlink_metadata(&manifest_path)
        .map(|metadata| metadata.is_file())
        .unwrap_or(false);
    let display_root_name = display_root_name(root_path);

    match mode {
        H3LocalImportMode::Pairing => Ok(inspect_pairing(
            display_root_name,
            detected_manifest,
            image_files,
            prompt_files,
        )),
        H3LocalImportMode::Manifest => {
            inspect_manifest(
                display_root_name,
                detected_manifest,
                image_files,
                prompt_files,
                &manifest_path,
            )
            .await
        }
        H3LocalImportMode::Text => Ok(inspect_text_pairing(display_root_name, prompt_files)),
        H3LocalImportMode::FirstLast => Ok(inspect_first_last_pairing(
            display_root_name,
            image_files,
            prompt_files,
        )),
        H3LocalImportMode::OmniManifest => {
            inspect_omni_manifest(
                display_root_name,
                image_files,
                video_files,
                audio_files,
                &omni_manifest_path,
            )
            .await
        }
    }
}

fn inspect_pairing(
    display_root_name: String,
    detected_manifest: bool,
    image_files: Vec<ScannedFile>,
    prompt_files: Vec<ScannedFile>,
) -> H3LocalImportInspection {
    let mut image_map: BTreeMap<String, Vec<ScannedFile>> = BTreeMap::new();
    let mut prompt_map: BTreeMap<String, Vec<ScannedFile>> = BTreeMap::new();
    for file in image_files.iter().cloned() {
        image_map
            .entry(stem_key(&file.relative_name))
            .or_default()
            .push(file);
    }
    for file in prompt_files.iter().cloned() {
        prompt_map
            .entry(stem_key(&file.relative_name))
            .or_default()
            .push(file);
    }
    let mut keys = image_map
        .keys()
        .chain(prompt_map.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| natural_cmp(left, right));

    let mut pairs = Vec::with_capacity(keys.len());
    let mut errors = Vec::new();
    for (index, key) in keys.into_iter().enumerate() {
        let images = image_map.get(&key).cloned().unwrap_or_default();
        let prompts = prompt_map.get(&key).cloned().unwrap_or_default();
        let mut pair = H3LocalImportPair {
            ordinal: index + 1,
            image_display_name: images
                .first()
                .map(|file| file.relative_name.clone())
                .unwrap_or_else(|| "缺少图片".to_owned()),
            prompt_display_name: prompts
                .first()
                .map(|file| file.relative_name.clone())
                .unwrap_or_else(|| "缺少提示词".to_owned()),
            prompt_preview: None,
            prompt_text: None,
            prompt_bytes: None,
            status: H3LocalPairStatus::Ready,
            image_path: images.first().map(|file| file.path.clone()),
            prompt_path: prompts.first().map(|file| file.path.clone()),
            image_sha256: None,
            image_paths: images.iter().map(|file| file.path.clone()).collect(),
            image_sha256s: Vec::new(),
            last_image_display_name: None,
            last_image_path: None,
            last_image_sha256: None,
            video_display_names: Vec::new(),
            video_paths: Vec::new(),
            audio_display_names: Vec::new(),
            audio_paths: Vec::new(),
        };
        if images.len() > 1 {
            pair.status = H3LocalPairStatus::AmbiguousImage;
        } else if images.is_empty() {
            pair.status = H3LocalPairStatus::MissingImage;
        } else if prompts.len() > 1 {
            pair.status = H3LocalPairStatus::AmbiguousPrompt;
        } else if prompts.is_empty() {
            pair.status = H3LocalPairStatus::MissingPrompt;
        } else {
            match fs::read(&images[0].path) {
                Ok(bytes)
                    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_SOURCE_IMAGE_BYTES =>
                {
                    pair.status = H3LocalPairStatus::ImageTooLarge;
                }
                Ok(bytes) => match crate::application::image_inspection::inspect_bytes(&bytes) {
                    Ok(inspected) => {
                        pair.image_sha256 = Some(inspected.sha256.clone());
                        pair.image_sha256s.push(inspected.sha256);
                    }
                    Err(_) => pair.status = H3LocalPairStatus::InvalidImage,
                },
                Err(_) => pair.status = H3LocalPairStatus::InvalidImage,
            }
            if pair.status.is_ready() {
                match fs::read(&prompts[0].path)
                    .map_err(|_| PromptIssue::Read)
                    .and_then(|bytes| {
                        parse_prompt_bytes(&bytes).map(|(text, byte_count)| (text, byte_count))
                    }) {
                    Ok((text, byte_count)) => {
                        pair.prompt_preview = Some(prompt_preview(&text));
                        pair.prompt_text = Some(text);
                        pair.prompt_bytes = Some(byte_count);
                    }
                    Err(PromptIssue::InvalidEncoding) => {
                        pair.status = H3LocalPairStatus::InvalidPromptEncoding
                    }
                    Err(PromptIssue::Empty) => pair.status = H3LocalPairStatus::EmptyPrompt,
                    Err(PromptIssue::TooLarge) => pair.status = H3LocalPairStatus::PromptTooLarge,
                    Err(PromptIssue::Read) => {
                        pair.status = H3LocalPairStatus::InvalidPromptEncoding
                    }
                }
            }
        }
        if !pair.status.is_ready() {
            errors.push(format!(
                "第 {} 项 {}：{}",
                pair.ordinal,
                pair.image_display_name,
                pair.status.as_str()
            ));
        }
        pairs.push(pair);
    }
    if pairs.len() > MAX_LOCAL_IMPORT_PAIRS {
        errors.push(format!("本地批量最多支持 {MAX_LOCAL_IMPORT_PAIRS} 项"));
    }
    if pairs.is_empty() {
        errors.push("未找到可配对的图片和提示词文件".to_owned());
    }
    let warnings = if detected_manifest {
        vec![
            "检测到 h3-batch.json；当前仍使用自动同名配对模式。切换到 JSON 批量清单可使用它。"
                .to_owned(),
        ]
    } else {
        Vec::new()
    };
    build_inspection(
        display_root_name,
        H3LocalImportMode::Pairing,
        detected_manifest,
        image_files.len(),
        prompt_files.len(),
        pairs,
        errors,
        warnings,
    )
}

fn empty_pair(ordinal: usize) -> H3LocalImportPair {
    H3LocalImportPair {
        ordinal,
        image_display_name: "—".to_owned(),
        prompt_display_name: "缺少提示词".to_owned(),
        prompt_preview: None,
        prompt_text: None,
        prompt_bytes: None,
        status: H3LocalPairStatus::Ready,
        image_path: None,
        prompt_path: None,
        image_sha256: None,
        image_paths: Vec::new(),
        image_sha256s: Vec::new(),
        last_image_display_name: None,
        last_image_path: None,
        last_image_sha256: None,
        video_display_names: Vec::new(),
        video_paths: Vec::new(),
        audio_display_names: Vec::new(),
        audio_paths: Vec::new(),
    }
}

fn inspect_prompt_file(pair: &mut H3LocalImportPair, path: &Path) {
    pair.prompt_display_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("提示词")
        .to_owned();
    match fs::read(path)
        .map_err(|_| PromptIssue::Read)
        .and_then(|bytes| parse_prompt_bytes(&bytes))
    {
        Ok((text, byte_count)) => {
            pair.prompt_preview = Some(prompt_preview(&text));
            pair.prompt_text = Some(text);
            pair.prompt_bytes = Some(byte_count);
        }
        Err(PromptIssue::InvalidEncoding | PromptIssue::Read) => {
            pair.status = H3LocalPairStatus::InvalidPromptEncoding
        }
        Err(PromptIssue::Empty) => pair.status = H3LocalPairStatus::EmptyPrompt,
        Err(PromptIssue::TooLarge) => pair.status = H3LocalPairStatus::PromptTooLarge,
    }
}

fn inspect_image_file(file: &ScannedFile) -> Result<String, H3LocalPairStatus> {
    let bytes = fs::read(&file.path).map_err(|_| H3LocalPairStatus::InvalidImage)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_SOURCE_IMAGE_BYTES {
        return Err(H3LocalPairStatus::ImageTooLarge);
    }
    crate::application::image_inspection::inspect_bytes(&bytes)
        .map(|inspected| inspected.sha256)
        .map_err(|_| H3LocalPairStatus::InvalidImage)
}

fn inspect_text_pairing(
    display_root_name: String,
    prompt_files: Vec<ScannedFile>,
) -> H3LocalImportInspection {
    let mut prompt_map: BTreeMap<String, Vec<ScannedFile>> = BTreeMap::new();
    for file in prompt_files.iter().cloned() {
        prompt_map
            .entry(stem_key(&file.relative_name))
            .or_default()
            .push(file);
    }
    let mut pairs = Vec::with_capacity(prompt_map.len());
    let mut errors = Vec::new();
    let mut keys = prompt_map.keys().cloned().collect::<Vec<_>>();
    keys.sort_by(|left, right| natural_cmp(left, right));
    for (index, key) in keys.into_iter().enumerate() {
        let prompts = prompt_map.get(&key).cloned().unwrap_or_default();
        let mut pair = empty_pair(index + 1);
        pair.image_display_name = "仅 Prompt".to_owned();
        if prompts.len() > 1 {
            pair.status = H3LocalPairStatus::AmbiguousPrompt;
        } else if prompts.is_empty() {
            pair.status = H3LocalPairStatus::MissingPrompt;
        } else {
            pair.prompt_path = Some(prompts[0].path.clone());
            inspect_prompt_file(&mut pair, &prompts[0].path);
        }
        if !pair.status.is_ready() {
            errors.push(format!("第 {} 项：{}", pair.ordinal, pair.status.as_str()));
        }
        pairs.push(pair);
    }
    if pairs.is_empty() {
        errors.push("未找到可用的 Prompt 文件".to_owned());
    }
    build_inspection(
        display_root_name,
        H3LocalImportMode::Text,
        false,
        0,
        prompt_files.len(),
        pairs,
        errors,
        Vec::new(),
    )
}

fn inspect_first_last_pairing(
    display_root_name: String,
    image_files: Vec<ScannedFile>,
    prompt_files: Vec<ScannedFile>,
) -> H3LocalImportInspection {
    let mut firsts: BTreeMap<String, Vec<ScannedFile>> = BTreeMap::new();
    let mut lasts: BTreeMap<String, Vec<ScannedFile>> = BTreeMap::new();
    let mut prompts: BTreeMap<String, Vec<ScannedFile>> = BTreeMap::new();
    for file in image_files.iter().cloned() {
        match frame_key(&file.relative_name) {
            Some((key, true)) => firsts.entry(key).or_default().push(file),
            Some((key, false)) => lasts.entry(key).or_default().push(file),
            None => {}
        }
    }
    for file in prompt_files.iter().cloned() {
        prompts
            .entry(stem_key(&file.relative_name))
            .or_default()
            .push(file);
    }
    let mut keys = firsts
        .keys()
        .chain(lasts.keys())
        .chain(prompts.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| natural_cmp(left, right));
    let mut pairs = Vec::with_capacity(keys.len());
    let mut errors = Vec::new();
    for (index, key) in keys.into_iter().enumerate() {
        let first = firsts.get(&key).and_then(|files| files.first());
        let last = lasts.get(&key).and_then(|files| files.first());
        let prompt = prompts.get(&key).and_then(|files| files.first());
        let mut pair = empty_pair(index + 1);
        pair.image_display_name = first
            .map(|file| file.relative_name.clone())
            .unwrap_or_else(|| "缺少首帧".to_owned());
        pair.last_image_display_name = Some(
            last.map(|file| file.relative_name.clone())
                .unwrap_or_else(|| "缺少末帧".to_owned()),
        );
        if firsts.get(&key).is_some_and(|files| files.len() > 1)
            || lasts.get(&key).is_some_and(|files| files.len() > 1)
        {
            pair.status = H3LocalPairStatus::AmbiguousImage;
        } else if first.is_none() || last.is_none() {
            pair.status = H3LocalPairStatus::MissingImage;
        } else if prompts.get(&key).is_some_and(|files| files.len() > 1) {
            pair.status = H3LocalPairStatus::AmbiguousPrompt;
        } else if prompt.is_none() {
            pair.status = H3LocalPairStatus::MissingPrompt;
        } else {
            pair.image_path = first.map(|file| file.path.clone());
            pair.image_paths = first
                .map(|file| vec![file.path.clone()])
                .unwrap_or_default();
            pair.last_image_path = last.map(|file| file.path.clone());
            match inspect_image_file(first.expect("first frame exists")) {
                Ok(hash) => {
                    pair.image_sha256 = Some(hash.clone());
                    pair.image_sha256s.push(hash);
                }
                Err(status) => pair.status = status,
            }
            if pair.status.is_ready() {
                match inspect_image_file(last.expect("last frame exists")) {
                    Ok(hash) => pair.last_image_sha256 = Some(hash),
                    Err(status) => pair.status = status,
                }
            }
            if pair.status.is_ready() {
                pair.prompt_path = prompt.map(|file| file.path.clone());
                inspect_prompt_file(&mut pair, prompt.expect("prompt exists").path.as_path());
            }
        }
        if !pair.status.is_ready() {
            errors.push(format!(
                "第 {} 项 {}：{}",
                pair.ordinal,
                pair.image_display_name,
                pair.status.as_str()
            ));
        }
        pairs.push(pair);
    }
    if pairs.is_empty() {
        errors.push("未找到首尾帧图片与 Prompt 配对".to_owned());
    }
    build_inspection(
        display_root_name,
        H3LocalImportMode::FirstLast,
        false,
        image_files.len(),
        prompt_files.len(),
        pairs,
        errors,
        Vec::new(),
    )
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OmniManifestEntry {
    #[serde(default)]
    images: Vec<String>,
    #[serde(default)]
    videos: Vec<String>,
    #[serde(default)]
    audios: Vec<String>,
    #[serde(default)]
    prompt: String,
}

#[derive(Debug, Deserialize)]
struct OmniManifestDocument(Vec<OmniManifestEntry>);

async fn inspect_omni_manifest(
    display_root_name: String,
    image_files: Vec<ScannedFile>,
    video_files: Vec<ScannedFile>,
    audio_files: Vec<ScannedFile>,
    manifest_path: &Path,
) -> Result<H3LocalImportInspection, H3LocalImportError> {
    let mut errors = Vec::new();
    if !manifest_path.is_file() {
        errors.push("未找到根目录下的 h3-omni-batch.json".to_owned());
        return Ok(build_inspection(
            display_root_name,
            H3LocalImportMode::OmniManifest,
            false,
            image_files.len(),
            0,
            Vec::new(),
            errors,
            Vec::new(),
        ));
    }
    let manifest_bytes = fs::read(manifest_path)
        .map_err(|_| H3LocalImportError::Filesystem("h3-omni-batch.json 无法读取".to_owned()))?;
    if u64::try_from(manifest_bytes.len()).unwrap_or(u64::MAX) > MAX_MANIFEST_BYTES {
        errors.push("h3-omni-batch.json 不能超过 10 MiB".to_owned());
        return Ok(build_inspection(
            display_root_name,
            H3LocalImportMode::OmniManifest,
            true,
            image_files.len(),
            0,
            Vec::new(),
            errors,
            Vec::new(),
        ));
    }
    let document = match serde_json::from_slice::<OmniManifestDocument>(&manifest_bytes) {
        Ok(document) => document,
        Err(_) => {
            errors.push("h3-omni-batch.json 不是有效的 JSON 清单".to_owned());
            return Ok(build_inspection(
                display_root_name,
                H3LocalImportMode::OmniManifest,
                true,
                image_files.len(),
                0,
                Vec::new(),
                errors,
                Vec::new(),
            ));
        }
    };
    let scanned = image_files
        .into_iter()
        .chain(video_files.clone())
        .chain(audio_files.clone())
        .map(|file| (normalize_relative(&file.relative_name), file))
        .collect::<HashMap<_, _>>();
    let mut pairs = Vec::new();
    let mut seen = HashSet::new();
    for (index, entry) in document.0.into_iter().enumerate() {
        let mut pair = empty_pair(index + 1);
        let mut valid_media = true;
        pair.prompt_display_name = "清单 Prompt".to_owned();
        let image_paths = resolve_omni_paths(
            manifest_path,
            &entry.images,
            &scanned,
            "image",
            &mut seen,
            &mut pair,
            &mut errors,
        );
        let video_paths = resolve_omni_paths(
            manifest_path,
            &entry.videos,
            &scanned,
            "video",
            &mut seen,
            &mut pair,
            &mut errors,
        );
        let audio_paths = resolve_omni_paths(
            manifest_path,
            &entry.audios,
            &scanned,
            "audio",
            &mut seen,
            &mut pair,
            &mut errors,
        );
        if image_paths.is_empty() && video_paths.is_empty() && audio_paths.is_empty() {
            pair.status = H3LocalPairStatus::MissingImage;
            valid_media = false;
        }
        pair.video_paths = video_paths.iter().map(|file| file.path.clone()).collect();
        pair.video_display_names = video_paths
            .iter()
            .map(|file| file.relative_name.clone())
            .collect();
        pair.audio_paths = audio_paths.iter().map(|file| file.path.clone()).collect();
        pair.audio_display_names = audio_paths
            .iter()
            .map(|file| file.relative_name.clone())
            .collect();
        pair.image_paths = image_paths.iter().map(|file| file.path.clone()).collect();
        if let Some(first) = image_paths.first() {
            pair.image_path = Some(first.path.clone());
            pair.image_display_name = first.relative_name.clone();
            match inspect_image_file(first) {
                Ok(hash) => {
                    pair.image_sha256 = Some(hash.clone());
                    pair.image_sha256s.push(hash);
                }
                Err(status) => {
                    pair.status = status;
                    valid_media = false;
                }
            }
        }
        for image in image_paths.iter().skip(1) {
            match inspect_image_file(image) {
                Ok(hash) => pair.image_sha256s.push(hash),
                Err(status) => {
                    pair.status = status;
                    valid_media = false;
                }
            };
        }
        match parse_prompt_text(&entry.prompt) {
            Ok((text, byte_count)) if valid_media && pair.status.is_ready() => {
                pair.prompt_preview = Some(prompt_preview(&text));
                pair.prompt_text = Some(text);
                pair.prompt_bytes = Some(byte_count);
            }
            Err(PromptIssue::Empty) => pair.status = H3LocalPairStatus::EmptyPrompt,
            Err(PromptIssue::TooLarge) => pair.status = H3LocalPairStatus::PromptTooLarge,
            Err(_) => pair.status = H3LocalPairStatus::InvalidPromptEncoding,
            Ok(_) => {}
        }
        if !pair.status.is_ready() {
            errors.push(format!("第 {} 项：{}", pair.ordinal, pair.status.as_str()));
        }
        pairs.push(pair);
    }
    if pairs.len() > MAX_LOCAL_IMPORT_PAIRS {
        errors.push(format!("本地批量最多支持 {MAX_LOCAL_IMPORT_PAIRS} 项"));
    }
    if pairs.is_empty() {
        errors.push("JSON 清单没有条目".to_owned());
    }
    Ok(build_inspection(
        display_root_name,
        H3LocalImportMode::OmniManifest,
        true,
        scanned
            .values()
            .filter(|file| is_image_extension(&file.extension))
            .count(),
        0,
        pairs,
        errors,
        Vec::new(),
    ))
}

fn resolve_omni_paths<'a>(
    manifest_path: &Path,
    entries: &[String],
    scanned: &'a HashMap<String, ScannedFile>,
    kind: &str,
    seen: &mut HashSet<String>,
    pair: &mut H3LocalImportPair,
    errors: &mut Vec<String>,
) -> Vec<&'a ScannedFile> {
    let mut resolved = Vec::new();
    for entry in entries {
        let relative = match validate_manifest_media_path(entry, kind) {
            Ok(path) => path,
            Err(status) => {
                pair.status = status;
                errors.push(format!("第 {} 项：{} 路径无效", pair.ordinal, kind));
                continue;
            }
        };
        let normalized = normalize_relative(&relative.to_string_lossy());
        let Some(file) = scanned.get(&normalized) else {
            pair.status = H3LocalPairStatus::UnknownImage;
            errors.push(format!("第 {} 项：{} 文件不存在", pair.ordinal, kind));
            continue;
        };
        let canonical = match fs::canonicalize(root_join_relative(manifest_path, &relative)) {
            Ok(path) => path,
            Err(_) => {
                pair.status = H3LocalPairStatus::UnknownImage;
                continue;
            }
        };
        let root = manifest_path.parent().unwrap_or_else(|| Path::new("."));
        if !canonical.starts_with(root) {
            pair.status = H3LocalPairStatus::InvalidPath;
            continue;
        }
        let canonical_key = canonical.to_string_lossy().to_ascii_lowercase();
        if !seen.insert(canonical_key) {
            pair.status = H3LocalPairStatus::DuplicateImageEntry;
            errors.push(format!(
                "第 {} 项：清单重复引用 {}",
                pair.ordinal, normalized
            ));
            continue;
        }
        resolved.push(file);
    }
    resolved
}

async fn inspect_manifest(
    display_root_name: String,
    detected_manifest: bool,
    image_files: Vec<ScannedFile>,
    prompt_files: Vec<ScannedFile>,
    manifest_path: &Path,
) -> Result<H3LocalImportInspection, H3LocalImportError> {
    let mut errors = Vec::new();
    let mut pairs = Vec::new();
    if !detected_manifest {
        errors.push("未找到根目录下的 h3-batch.json".to_owned());
        return Ok(build_inspection(
            display_root_name,
            H3LocalImportMode::Manifest,
            false,
            image_files.len(),
            prompt_files.len(),
            pairs,
            errors,
            Vec::new(),
        ));
    }
    let manifest_bytes = fs::read(manifest_path)
        .map_err(|_| H3LocalImportError::Filesystem("h3-batch.json 无法读取".to_owned()))?;
    if u64::try_from(manifest_bytes.len()).unwrap_or(u64::MAX) > MAX_MANIFEST_BYTES {
        errors.push("h3-batch.json 不能超过 10 MiB".to_owned());
        return Ok(build_inspection(
            display_root_name,
            H3LocalImportMode::Manifest,
            true,
            image_files.len(),
            prompt_files.len(),
            pairs,
            errors,
            Vec::new(),
        ));
    }
    let document = match serde_json::from_slice::<ManifestDocument>(&manifest_bytes) {
        Ok(document) => document,
        Err(_) => {
            errors.push("h3-batch.json 不是有效的 JSON 清单".to_owned());
            return Ok(build_inspection(
                display_root_name,
                H3LocalImportMode::Manifest,
                true,
                image_files.len(),
                prompt_files.len(),
                pairs,
                errors,
                Vec::new(),
            ));
        }
    };
    let mut entries = document.0;
    entries.sort_by(|left, right| natural_cmp(&left.image, &right.image));
    if entries.len() > MAX_LOCAL_IMPORT_PAIRS {
        errors.push(format!("JSON 清单最多支持 {MAX_LOCAL_IMPORT_PAIRS} 项"));
    }
    let scanned_by_relative = image_files
        .iter()
        .map(|file| (normalize_relative(&file.relative_name), file.clone()))
        .collect::<HashMap<_, _>>();
    let mut seen_images = HashSet::new();
    for (index, entry) in entries.into_iter().enumerate() {
        let mut pair = H3LocalImportPair {
            ordinal: index + 1,
            image_display_name: "清单图片路径无效".to_owned(),
            prompt_display_name: "清单提示词".to_owned(),
            prompt_preview: None,
            prompt_text: None,
            prompt_bytes: None,
            status: H3LocalPairStatus::Ready,
            image_path: None,
            prompt_path: None,
            image_sha256: None,
            image_paths: Vec::new(),
            image_sha256s: Vec::new(),
            last_image_display_name: None,
            last_image_path: None,
            last_image_sha256: None,
            video_display_names: Vec::new(),
            video_paths: Vec::new(),
            audio_display_names: Vec::new(),
            audio_paths: Vec::new(),
        };
        let relative_path = match validate_manifest_relative_path(&entry.image) {
            Ok(path) => path,
            Err(status) => {
                pair.status = status;
                errors.push(format!("第 {} 项：清单图片路径无效", pair.ordinal));
                pairs.push(pair);
                continue;
            }
        };
        let relative_name = normalize_relative(&relative_path.to_string_lossy());
        let canonical_image =
            match fs::canonicalize(root_join_relative(manifest_path, &relative_path)) {
                Ok(path) => path,
                Err(_) => {
                    pair.status = H3LocalPairStatus::UnknownImage;
                    errors.push(format!("第 {} 项：清单引用的图片不存在", pair.ordinal));
                    pairs.push(pair);
                    continue;
                }
            };
        let root = manifest_path.parent().unwrap_or_else(|| Path::new("."));
        if !canonical_image.starts_with(root) {
            pair.status = H3LocalPairStatus::InvalidPath;
            errors.push(format!(
                "第 {} 项：清单图片路径越过任务目录边界",
                pair.ordinal
            ));
            pairs.push(pair);
            continue;
        }
        let Some(scanned) = scanned_by_relative.get(&relative_name) else {
            pair.status = H3LocalPairStatus::UnknownImage;
            errors.push(format!(
                "第 {} 项：清单引用的图片不是支持的图片文件",
                pair.ordinal
            ));
            pairs.push(pair);
            continue;
        };
        let canonical_key = canonical_image.to_string_lossy().to_ascii_lowercase();
        if !seen_images.insert(canonical_key) {
            pair.status = H3LocalPairStatus::DuplicateImageEntry;
            errors.push(format!("第 {} 项：清单重复引用同一图片", pair.ordinal));
            pairs.push(pair);
            continue;
        }
        pair.image_display_name = scanned.relative_name.clone();
        pair.image_path = Some(scanned.path.clone());
        pair.image_paths = vec![scanned.path.clone()];
        match fs::read(&scanned.path) {
            Ok(bytes)
                if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_SOURCE_IMAGE_BYTES =>
            {
                pair.status = H3LocalPairStatus::ImageTooLarge;
            }
            Ok(bytes) => match crate::application::image_inspection::inspect_bytes(&bytes) {
                Ok(inspected) => {
                    pair.image_sha256 = Some(inspected.sha256.clone());
                    pair.image_sha256s.push(inspected.sha256);
                }
                Err(_) => pair.status = H3LocalPairStatus::InvalidImage,
            },
            Err(_) => pair.status = H3LocalPairStatus::InvalidImage,
        }
        match parse_prompt_text(&entry.prompt) {
            Ok((text, byte_count)) if pair.status.is_ready() => {
                pair.prompt_preview = Some(prompt_preview(&text));
                pair.prompt_text = Some(text);
                pair.prompt_bytes = Some(byte_count);
            }
            Err(PromptIssue::Empty) => pair.status = H3LocalPairStatus::EmptyPrompt,
            Err(PromptIssue::TooLarge) => pair.status = H3LocalPairStatus::PromptTooLarge,
            Err(_) => pair.status = H3LocalPairStatus::InvalidPromptEncoding,
            Ok(_) => {}
        }
        if !pair.status.is_ready() {
            errors.push(format!(
                "第 {} 项 {}：{}",
                pair.ordinal,
                pair.image_display_name,
                pair.status.as_str()
            ));
        }
        pairs.push(pair);
    }
    if pairs.is_empty() {
        errors.push("JSON 清单没有条目".to_owned());
    }
    Ok(build_inspection(
        display_root_name,
        H3LocalImportMode::Manifest,
        true,
        image_files.len(),
        prompt_files.len(),
        pairs,
        errors,
        Vec::new(),
    ))
}

fn build_inspection(
    display_root_name: String,
    mode: H3LocalImportMode,
    detected_manifest: bool,
    image_count: usize,
    prompt_count: usize,
    pairs: Vec<H3LocalImportPair>,
    errors: Vec<String>,
    warnings: Vec<String>,
) -> H3LocalImportInspection {
    let ready_count = pairs.iter().filter(|pair| pair.status.is_ready()).count();
    H3LocalImportInspection {
        display_root_name,
        mode,
        detected_manifest,
        image_count,
        prompt_count,
        ready_count,
        error_count: errors.len(),
        pairs,
        errors,
        warnings,
    }
}

fn scan_files(root: &Path) -> Result<Vec<ScannedFile>, H3LocalImportError> {
    let mut directories = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = directories.pop() {
        let entries = fs::read_dir(&directory)
            .map_err(|_| H3LocalImportError::Filesystem("任务目录无法读取".to_owned()))?;
        for entry in entries {
            let entry = entry
                .map_err(|_| H3LocalImportError::Filesystem("任务目录内容无法读取".to_owned()))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|_| H3LocalImportError::Filesystem("任务目录内容无法读取".to_owned()))?;
            if metadata.file_type().is_symlink() {
                let canonical = fs::canonicalize(&path).map_err(|_| {
                    H3LocalImportError::FilesystemBoundary("目录中存在无法解析的链接".to_owned())
                })?;
                if !canonical.starts_with(root) {
                    return Err(H3LocalImportError::FilesystemBoundary(
                        "目录链接越过所选任务目录边界".to_owned(),
                    ));
                }
                return Err(H3LocalImportError::Inspection(
                    "本地批量不接受符号链接或目录链接".to_owned(),
                ));
            }
            let canonical = fs::canonicalize(&path)
                .map_err(|_| H3LocalImportError::Filesystem("任务目录内容无法读取".to_owned()))?;
            if !canonical.starts_with(root) {
                return Err(H3LocalImportError::FilesystemBoundary(
                    "目录内容越过所选任务目录边界".to_owned(),
                ));
            }
            if metadata.is_dir() {
                directories.push(path);
                continue;
            }
            if !metadata.is_file() {
                continue;
            }
            let relative_path = path.strip_prefix(root).map_err(|_| {
                H3LocalImportError::FilesystemBoundary("无法计算目录内相对路径".to_owned())
            })?;
            let Some(relative_name) = relative_path.to_str() else {
                return Err(H3LocalImportError::Inspection(
                    "目录包含无法识别的文件名".to_owned(),
                ));
            };
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.to_ascii_lowercase())
                .unwrap_or_default();
            if is_image_extension(&extension)
                || is_prompt_extension(&extension)
                || is_video_extension(&extension)
                || is_audio_extension(&extension)
            {
                let relative_name = normalize_relative(relative_name);
                files.push(ScannedFile {
                    path,
                    relative_name,
                    extension,
                });
            }
        }
    }
    files.sort_by(|left, right| natural_cmp(&left.relative_name, &right.relative_name));
    Ok(files)
}

fn canonical_directory(path: &Path) -> Result<PathBuf, H3LocalImportError> {
    let metadata = fs::metadata(path)
        .map_err(|_| H3LocalImportError::Filesystem("所选任务目录无法读取".to_owned()))?;
    if !metadata.is_dir() {
        return Err(H3LocalImportError::InvalidInput(
            "请选择文件夹作为本地任务目录".to_owned(),
        ));
    }
    fs::canonicalize(path)
        .map_err(|_| H3LocalImportError::Filesystem("所选任务目录无法读取".to_owned()))
}

fn revalidate_file_path(root: &Path, path: &Path) -> Result<PathBuf, H3LocalImportError> {
    let canonical = fs::canonicalize(path)
        .map_err(|_| H3LocalImportError::Filesystem("任务文件在提交前无法读取".to_owned()))?;
    if !canonical.starts_with(root) {
        return Err(H3LocalImportError::FilesystemBoundary(
            "任务文件在提交前越过目录边界".to_owned(),
        ));
    }
    let metadata = fs::metadata(&canonical)
        .map_err(|_| H3LocalImportError::Filesystem("任务文件在提交前无法读取".to_owned()))?;
    if !metadata.is_file() {
        return Err(H3LocalImportError::Filesystem(
            "任务文件在提交前不再是普通文件".to_owned(),
        ));
    }
    Ok(canonical)
}

fn validate_commit_request(request: &H3LocalImportCommitRequest) -> Result<(), H3LocalImportError> {
    if request.workflow_version_id.trim().is_empty() || request.recipe_id.trim().is_empty() {
        return Err(H3LocalImportError::InvalidInput(
            "H3 Recipe 标识不能为空".to_owned(),
        ));
    }
    if request.width <= 0
        || request.height <= 0
        || request.width > 16_384
        || request.height > 16_384
    {
        return Err(H3LocalImportError::InvalidInput(
            "输出分辨率不合法".to_owned(),
        ));
    }
    if !(1..=15).contains(&request.duration_seconds) {
        return Err(H3LocalImportError::InvalidInput(
            "H3 视频时长必须为 1–15 秒".to_owned(),
        ));
    }
    if let Some(name) = request.batch_name.as_deref() {
        if name.trim().chars().count() > 120 {
            return Err(H3LocalImportError::InvalidInput(
                "批次名称不能超过 120 个字符".to_owned(),
            ));
        }
    }
    H3CommitGenerationMode::parse(request.generation_mode.as_deref())?;
    Ok(())
}

fn validate_local_import_mode(
    generation_mode: H3CommitGenerationMode,
    import_mode: H3LocalImportMode,
) -> Result<(), H3LocalImportError> {
    let valid = match generation_mode {
        H3CommitGenerationMode::LegacyReferenceImage => {
            matches!(
                import_mode,
                H3LocalImportMode::Pairing | H3LocalImportMode::Manifest
            )
        }
        H3CommitGenerationMode::Fl2vaTextToVideo => import_mode == H3LocalImportMode::Text,
        H3CommitGenerationMode::Fl2vaFirstLast => import_mode == H3LocalImportMode::FirstLast,
        H3CommitGenerationMode::Fl2vaImageToVideo | H3CommitGenerationMode::Ref2vaImage => {
            matches!(
                import_mode,
                H3LocalImportMode::Pairing
                    | H3LocalImportMode::Manifest
                    | H3LocalImportMode::OmniManifest
            )
        }
        H3CommitGenerationMode::Ref2vaAudio
        | H3CommitGenerationMode::Ref2vaImageAudio
        | H3CommitGenerationMode::Ref2vaVideoImage => {
            import_mode == H3LocalImportMode::OmniManifest
        }
    };
    if valid {
        Ok(())
    } else {
        Err(H3LocalImportError::Inspection(
            "本地导入方式与当前 H3 生成模式不匹配，请重新扫描任务目录".to_owned(),
        ))
    }
}

fn validate_pair_media(
    pair: &H3LocalImportPair,
    generation_mode: H3CommitGenerationMode,
) -> Result<(), H3LocalImportError> {
    let has_image = !pair.image_paths.is_empty() || pair.image_path.is_some();
    let has_last_image = pair.last_image_path.is_some();
    let has_video = !pair.video_paths.is_empty();
    let has_audio = !pair.audio_paths.is_empty();
    let valid = match generation_mode {
        H3CommitGenerationMode::LegacyReferenceImage
        | H3CommitGenerationMode::Fl2vaImageToVideo
        | H3CommitGenerationMode::Ref2vaImage => has_image,
        H3CommitGenerationMode::Fl2vaTextToVideo => true,
        H3CommitGenerationMode::Fl2vaFirstLast => has_image && has_last_image,
        H3CommitGenerationMode::Ref2vaAudio => has_audio,
        H3CommitGenerationMode::Ref2vaImageAudio => has_image && has_audio,
        H3CommitGenerationMode::Ref2vaVideoImage => has_image && has_video,
    };
    if valid {
        Ok(())
    } else {
        Err(H3LocalImportError::Inspection(format!(
            "第 {} 项缺少当前 H3 模式需要的媒体输入",
            pair.ordinal
        )))
    }
}

fn parse_prompt_bytes(bytes: &[u8]) -> Result<(String, usize), PromptIssue> {
    if bytes.len() > crate::application::asset_video_prompt_service::MAX_ASSET_VIDEO_PROMPT_BYTES {
        return Err(PromptIssue::TooLarge);
    }
    let text = String::from_utf8(bytes.to_vec()).map_err(|_| PromptIssue::InvalidEncoding)?;
    parse_prompt_text_with_bytes(text, bytes.len())
}

fn parse_prompt_text(value: &str) -> Result<(String, usize), PromptIssue> {
    let value = value.strip_prefix('\u{feff}').unwrap_or(value);
    let bytes = value.as_bytes();
    if bytes.len() > crate::application::asset_video_prompt_service::MAX_ASSET_VIDEO_PROMPT_BYTES {
        return Err(PromptIssue::TooLarge);
    }
    parse_prompt_text_with_bytes(value.to_owned(), bytes.len())
}

fn parse_prompt_text_with_bytes(
    text: String,
    byte_count: usize,
) -> Result<(String, usize), PromptIssue> {
    let text = text
        .strip_prefix('\u{feff}')
        .unwrap_or(&text)
        .trim()
        .to_owned();
    if text.is_empty() {
        return Err(PromptIssue::Empty);
    }
    Ok((text, byte_count))
}

fn prompt_preview(text: &str) -> String {
    let mut preview = text.chars().take(240).collect::<String>();
    if text.chars().count() > 240 {
        preview.push('…');
    }
    preview
}

fn is_image_extension(extension: &str) -> bool {
    matches!(extension, "png" | "jpg" | "jpeg" | "webp")
}

fn is_prompt_extension(extension: &str) -> bool {
    matches!(extension, "txt" | "md")
}

fn is_video_extension(extension: &str) -> bool {
    matches!(extension, "mp4" | "mov" | "webm" | "mkv")
}

fn is_audio_extension(extension: &str) -> bool {
    matches!(extension, "wav" | "mp3" | "m4a" | "flac" | "ogg")
}

fn stem_key(relative_name: &str) -> String {
    let mut path = PathBuf::from(relative_name);
    path.set_extension("");
    normalize_relative(path.to_string_lossy().trim_end_matches('.')).to_lowercase()
}

fn frame_key(relative_name: &str) -> Option<(String, bool)> {
    let stem = stem_key(relative_name);
    if let Some(key) = stem.strip_suffix("_first") {
        return Some((key.to_owned(), true));
    }
    if let Some(key) = stem.strip_suffix("_last") {
        return Some((key.to_owned(), false));
    }
    None
}

fn normalize_relative(value: &str) -> String {
    value.replace('\\', "/")
}

fn display_root_name(root: &Path) -> String {
    root.file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("本地任务目录")
        .to_owned()
}

fn validate_manifest_relative_path(value: &str) -> Result<PathBuf, H3LocalPairStatus> {
    if value.trim().is_empty()
        || value.starts_with('/')
        || value.starts_with('\\')
        || value.contains('\\')
        || value.contains(':')
    {
        return Err(H3LocalPairStatus::InvalidPath);
    }
    let path = PathBuf::from(value);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(H3LocalPairStatus::InvalidPath);
    }
    if !is_image_extension(
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
    ) {
        return Err(H3LocalPairStatus::UnknownImage);
    }
    Ok(path)
}

fn validate_manifest_media_path(value: &str, kind: &str) -> Result<PathBuf, H3LocalPairStatus> {
    if value.trim().is_empty()
        || value.starts_with('/')
        || value.starts_with('\\')
        || value.contains('\\')
        || value.contains(':')
    {
        return Err(H3LocalPairStatus::InvalidPath);
    }
    let path = PathBuf::from(value);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(H3LocalPairStatus::InvalidPath);
    }
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let valid = match kind {
        "image" => is_image_extension(&extension),
        "video" => is_video_extension(&extension),
        "audio" => is_audio_extension(&extension),
        _ => false,
    };
    if !valid {
        return Err(H3LocalPairStatus::UnknownImage);
    }
    Ok(path)
}

fn root_join_relative(manifest_path: &Path, relative: &Path) -> PathBuf {
    manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(relative)
}

fn natural_cmp(left: &str, right: &str) -> Ordering {
    let left = left.to_ascii_lowercase().into_bytes();
    let right = right.to_ascii_lowercase().into_bytes();
    let mut left_index = 0usize;
    let mut right_index = 0usize;
    while left_index < left.len() && right_index < right.len() {
        let left_digit = left[left_index].is_ascii_digit();
        let right_digit = right[right_index].is_ascii_digit();
        if left_digit && right_digit {
            let left_start = left_index;
            let right_start = right_index;
            while left_index < left.len() && left[left_index].is_ascii_digit() {
                left_index += 1;
            }
            while right_index < right.len() && right[right_index].is_ascii_digit() {
                right_index += 1;
            }
            let left_number = trim_leading_zeroes(&left[left_start..left_index]);
            let right_number = trim_leading_zeroes(&right[right_start..right_index]);
            match left_number.len().cmp(&right_number.len()) {
                Ordering::Equal => match left_number.cmp(right_number) {
                    Ordering::Equal => {}
                    other => return other,
                },
                other => return other,
            }
            continue;
        }
        match left[left_index].cmp(&right[right_index]) {
            Ordering::Equal => {
                left_index += 1;
                right_index += 1;
            }
            other => return other,
        }
    }
    left.len().cmp(&right.len())
}

fn trim_leading_zeroes(value: &[u8]) -> &[u8] {
    let index = value
        .iter()
        .position(|byte| *byte != b'0')
        .unwrap_or(value.len().saturating_sub(1));
    &value[index..]
}

#[cfg(test)]
fn validate_manifest_relative_path_for_test(value: &str) -> bool {
    validate_manifest_relative_path(value).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::asset_video_prompt_service::AssetVideoPromptService;
    use crate::application::generation_service::GenerationService;
    use crate::application::ports::{
        AssetRepository, AssetVideoPromptRepository, Clock, ComfyAdapter, ComfyAdapterError,
        ComfyEventSubscription, ComfyHealth, ComfyHistory, ComfyOutputData, ComfyOutputFile,
        NoopTaskUpdateSink, PromptSubmission, SystemStats,
    };
    use crate::application::production_queue_service::ProductionQueueService;
    use crate::application::source_asset_import_service::SourceAssetImportService;
    use crate::application::task_recovery_service::TaskRecoveryService;
    use crate::domain::{AssetId, AssetType, ProductionBatchItemStatus, ProductionBatchStatus};
    use crate::infrastructure::database::{
        initialize, SqliteAssetRepository, SqliteAssetVideoPromptRepository,
        SqliteGenerationDefinitionRepository, SqliteGenerationSnapshotRepository,
        SqliteProductionQueueRepository, SqliteProjectRepository, SqliteTaskRepository,
    };
    use crate::infrastructure::filesystem::FileSystemAssetStore;
    use crate::infrastructure::time::SystemClock;
    use async_trait::async_trait;
    use chrono::{Duration as ChronoDuration, Utc};
    use sqlx::SqlitePool;
    use std::io::Cursor;
    use std::path::Path;
    use std::sync::Arc;
    use tempfile::tempdir;

    const PROJECT_ID: &str = "prj_default";
    const H3_TEST_RECIPE: &str = r#"
schema_version: 1
id: minimax_h3_reference_video
name: MiniMax H3 Reference Video Test
workflow:
  file: workflow_api.json
inputs:
  prompt:
    type: textarea
    label: Prompt
    required: true
    default: ""
  reference_image:
    type: image
    label: Reference Image
    required: true
  width:
    type: integer
    label: Width
    required: true
    default: 640
    min: 64
    max: 2048
    step: 1
  height:
    type: integer
    label: Height
    required: true
    default: 360
    min: 64
    max: 2048
    step: 1
  duration_seconds:
    type: integer
    label: Duration
    required: true
    default: 5
    min: 1
    max: 15
    step: 1
  seed:
    type: seed
    label: Seed
    default: random
    min: 0
    max: 1125899906842624
bindings: []
outputs:
  - id: generated_video
    type: video
    node: "1"
    required: true
"#;

    struct TestComfyAdapter;

    #[async_trait]
    impl ComfyAdapter for TestComfyAdapter {
        async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
            Ok(ComfyHealth {
                system: SystemStats {
                    comfyui_version: None,
                    python_version: None,
                    os: None,
                    ram_total: None,
                    ram_free: None,
                    devices: Vec::new(),
                },
            })
        }

        async fn get_system_stats(&self) -> Result<SystemStats, ComfyAdapterError> {
            Ok(self.health_check().await?.system)
        }

        async fn get_object_info(&self) -> Result<serde_json::Value, ComfyAdapterError> {
            Ok(serde_json::json!({}))
        }

        async fn get_history(&self, _prompt_id: &str) -> Result<ComfyHistory, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible(
                "test adapter does not execute workflows".to_owned(),
            ))
        }

        async fn download_output(
            &self,
            _file: &ComfyOutputFile,
        ) -> Result<ComfyOutputData, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible(
                "test adapter does not download outputs".to_owned(),
            ))
        }

        async fn submit_workflow(
            &self,
            _client_id: &str,
            _prompt_id: &str,
            _workflow: serde_json::Value,
        ) -> Result<PromptSubmission, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible(
                "test adapter does not submit workflows".to_owned(),
            ))
        }

        async fn subscribe_events(
            &self,
            _client_id: &str,
        ) -> Result<Box<dyn ComfyEventSubscription>, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible(
                "test adapter does not subscribe to events".to_owned(),
            ))
        }
    }

    struct H3TestHarness {
        pool: SqlitePool,
        local: Arc<H3LocalImportService>,
        queue: Arc<ProductionQueueService>,
        assets: Arc<SqliteAssetRepository>,
        prompts: Arc<SqliteAssetVideoPromptRepository>,
        prompt_service: Arc<AssetVideoPromptService>,
    }

    async fn build_harness(
        db_path: &Path,
        project_root: &Path,
        seed_database: bool,
    ) -> H3TestHarness {
        let pool = initialize(db_path)
            .await
            .expect("H3 integration database should initialize");
        if seed_database {
            crate::infrastructure::database::repositories::test_support::seed_task_dependencies(
                &pool,
            )
            .await;
            sqlx::query(
                "INSERT INTO projects (id, name, description, root_path, created_at, updated_at)
                 VALUES (?, 'H3 Default', NULL, ?, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            )
            .bind(PROJECT_ID)
            .bind(project_root.to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("valid project fixture should insert");
            sqlx::query("UPDATE recipes SET recipe_yaml = ? WHERE id = 'recipe-1'")
                .bind(H3_TEST_RECIPE)
                .execute(&pool)
                .await
                .expect("H3 test Recipe should persist");
        }

        let asset_repository = Arc::new(SqliteAssetRepository::new(pool.clone()));
        let prompt_repository = Arc::new(SqliteAssetVideoPromptRepository::new(pool.clone()));
        let project_repository = Arc::new(SqliteProjectRepository::new(pool.clone()));
        let queue_repository = Arc::new(SqliteProductionQueueRepository::new(pool.clone()));
        let task_repository = Arc::new(SqliteTaskRepository::new(pool.clone()));
        let snapshot_repository = Arc::new(SqliteGenerationSnapshotRepository::new(pool.clone()));
        let definition_repository =
            Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone()));
        let asset_store = Arc::new(FileSystemAssetStore::new());
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let comfy_adapter: Arc<dyn ComfyAdapter> = Arc::new(TestComfyAdapter);
        let generation_service = Arc::new(GenerationService::new(
            task_repository.clone(),
            snapshot_repository.clone(),
            definition_repository.clone(),
            comfy_adapter.clone(),
            project_repository.clone(),
            asset_store.clone(),
            asset_repository.clone(),
            clock.clone(),
        ));
        let task_recovery_service = Arc::new(TaskRecoveryService::new(
            task_repository.clone(),
            snapshot_repository,
            asset_repository.clone(),
            comfy_adapter,
            project_repository.clone(),
            asset_store.clone(),
            clock.clone(),
            Arc::new(NoopTaskUpdateSink),
        ));
        let queue = Arc::new(ProductionQueueService::new(
            queue_repository.clone(),
            task_repository,
            definition_repository,
            generation_service,
            queue_repository,
            task_recovery_service,
            clock.clone(),
        ));
        let prompt_service = Arc::new(AssetVideoPromptService::new(
            prompt_repository.clone(),
            asset_repository.clone(),
            clock.clone(),
        ));
        let source_asset_import_service = Arc::new(SourceAssetImportService::new(
            project_repository,
            asset_store,
            asset_repository.clone(),
            clock.clone(),
        ));
        let local = Arc::new(H3LocalImportService::new(
            source_asset_import_service,
            prompt_service.clone(),
            queue.clone(),
            clock,
        ));
        H3TestHarness {
            pool,
            local,
            queue,
            assets: asset_repository,
            prompts: prompt_repository,
            prompt_service,
        }
    }

    fn write_pairing_fixture(root: &Path) {
        fs::create_dir_all(root).expect("local fixture directory should exist");
        fs::write(root.join("001.png"), png_bytes()).expect("001 image should write");
        fs::write(root.join("001.txt"), "  Prompt A first line\nsecond line  ")
            .expect("001 prompt should write");
        fs::write(root.join("002.png"), png_bytes()).expect("002 image should write");
        fs::write(root.join("002.txt"), "Prompt A second line\ncontinuation")
            .expect("002 prompt should write");
    }

    async fn count_rows(pool: &SqlitePool, table: &str) -> i64 {
        let query = match table {
            "assets" => "SELECT COUNT(*) FROM assets WHERE project_id = ?",
            "asset_video_prompts" => {
                "SELECT COUNT(*) FROM asset_video_prompts WHERE project_id = ?"
            }
            "production_batches" => "SELECT COUNT(*) FROM production_batches WHERE project_id = ?",
            _ => panic!("unexpected test table"),
        };
        sqlx::query_scalar(query)
            .bind(PROJECT_ID)
            .fetch_one(pool)
            .await
            .expect("row count should be readable")
    }

    fn png_bytes() -> Vec<u8> {
        let image = image::RgbImage::from_pixel(2, 2, image::Rgb([80, 120, 180]));
        let mut output = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut output, image::ImageFormat::Png)
            .expect("fixture png should encode");
        output.into_inner()
    }

    #[tokio::test]
    async fn pairing_uses_natural_sort_and_preserves_internal_newlines() {
        let directory = tempfile::tempdir().expect("fixture directory should exist");
        for (name, prompt) in [
            ("10.png", "ten\n\ninternal"),
            ("10.txt", "  Ten prompt  "),
            ("1.png", "unused"),
            ("1.txt", "  one\nline  "),
            ("2.jpg", "unused"),
            ("2.txt", "two"),
        ] {
            let path = directory.path().join(name);
            if name.ends_with(".png") || name.ends_with(".jpg") {
                fs::write(path, png_bytes()).expect("fixture image should write");
            } else {
                fs::write(path, prompt.as_bytes()).expect("fixture prompt should write");
            }
        }
        let inspection = inspect_directory(directory.path(), H3LocalImportMode::Pairing)
            .await
            .expect("inspection should succeed");
        assert_eq!(inspection.ready_count, 3);
        assert_eq!(inspection.pairs[0].image_display_name, "1.png");
        assert_eq!(inspection.pairs[1].image_display_name, "2.jpg");
        assert_eq!(inspection.pairs[2].image_display_name, "10.png");
        assert_eq!(
            inspection.pairs[0].prompt_text.as_deref(),
            Some("one\nline")
        );
        assert_eq!(inspection.pairs[0].prompt_bytes, Some(12));
    }

    #[tokio::test]
    async fn pairing_reports_missing_ambiguous_and_invalid_inputs() {
        let directory = tempfile::tempdir().expect("fixture directory should exist");
        fs::write(directory.path().join("missing.png"), png_bytes()).expect("image should write");
        fs::write(directory.path().join("ambiguous.png"), png_bytes()).expect("image should write");
        fs::write(directory.path().join("ambiguous.webp"), png_bytes())
            .expect("image should write");
        fs::write(directory.path().join("bad.txt"), [0xff, 0xfe]).expect("prompt should write");
        fs::write(directory.path().join("bad.png"), png_bytes()).expect("image should write");
        let inspection = inspect_directory(directory.path(), H3LocalImportMode::Pairing)
            .await
            .expect("inspection should succeed");
        assert!(inspection
            .pairs
            .iter()
            .any(|pair| pair.status == H3LocalPairStatus::MissingPrompt));
        assert!(inspection
            .pairs
            .iter()
            .any(|pair| pair.status == H3LocalPairStatus::AmbiguousImage));
        assert!(inspection
            .pairs
            .iter()
            .any(|pair| pair.status == H3LocalPairStatus::InvalidPromptEncoding));
        assert_eq!(inspection.ready_count, 0);
    }

    #[tokio::test]
    async fn manifest_import_preview_keeps_multiline_prompt_and_rejects_duplicates() {
        let directory = tempfile::tempdir().expect("fixture directory should exist");
        fs::write(directory.path().join("001.png"), png_bytes()).expect("image should write");
        fs::write(directory.path().join("002.png"), png_bytes()).expect("image should write");
        fs::write(
            directory.path().join("h3-batch.json"),
            serde_json::to_vec(&serde_json::json!([
                { "image": "001.png", "prompt": "line1\nline2" },
                { "image": "001.png", "prompt": "duplicate" },
                { "image": "missing.png", "prompt": "unknown" }
            ]))
            .expect("manifest should encode"),
        )
        .expect("manifest should write");
        let inspection = inspect_directory(directory.path(), H3LocalImportMode::Manifest)
            .await
            .expect("manifest inspection should succeed");
        assert_eq!(
            inspection.pairs[0].prompt_preview.as_deref(),
            Some("line1\nline2")
        );
        assert!(inspection
            .pairs
            .iter()
            .any(|pair| pair.status == H3LocalPairStatus::DuplicateImageEntry));
        assert!(inspection
            .pairs
            .iter()
            .any(|pair| pair.status == H3LocalPairStatus::UnknownImage));
        assert_eq!(inspection.ready_count, 1);
        assert!(inspection.error_count >= 2);
    }

    #[tokio::test]
    async fn inspections_cover_text_first_last_and_omni_media_modes() {
        let text_directory = tempfile::tempdir().expect("text fixture directory should exist");
        fs::write(text_directory.path().join("001.txt"), "line one\nline two")
            .expect("text prompt should write");
        let text = inspect_directory(text_directory.path(), H3LocalImportMode::Text)
            .await
            .expect("text inspection should succeed");
        assert_eq!(text.ready_count, 1);
        assert_eq!(text.image_count, 0);
        assert_eq!(text.pairs[0].image_display_name, "仅 Prompt");
        assert_eq!(
            text.pairs[0].prompt_preview.as_deref(),
            Some("line one\nline two")
        );

        let frame_directory = tempfile::tempdir().expect("frame fixture directory should exist");
        fs::write(frame_directory.path().join("001_first.png"), png_bytes())
            .expect("first frame should write");
        fs::write(frame_directory.path().join("001_last.png"), png_bytes())
            .expect("last frame should write");
        fs::write(frame_directory.path().join("001.txt"), "frame motion")
            .expect("frame prompt should write");
        let first_last = inspect_directory(frame_directory.path(), H3LocalImportMode::FirstLast)
            .await
            .expect("first-last inspection should succeed");
        assert_eq!(first_last.ready_count, 1);
        assert_eq!(first_last.pairs[0].image_display_name, "001_first.png");
        assert_eq!(
            first_last.pairs[0].last_image_display_name.as_deref(),
            Some("001_last.png")
        );

        let omni_directory = tempfile::tempdir().expect("omni fixture directory should exist");
        fs::write(omni_directory.path().join("ref.png"), png_bytes())
            .expect("omni image should write");
        fs::write(omni_directory.path().join("ref.mp4"), b"video placeholder")
            .expect("omni video should write");
        fs::write(omni_directory.path().join("ref.wav"), b"audio placeholder")
            .expect("omni audio should write");
        fs::write(
            omni_directory.path().join("h3-omni-batch.json"),
            serde_json::to_vec(&serde_json::json!([
                {
                    "images": ["ref.png"],
                    "videos": ["ref.mp4"],
                    "audios": ["ref.wav"],
                    "prompt": "omni motion"
                }
            ]))
            .expect("omni manifest should encode"),
        )
        .expect("omni manifest should write");
        let omni = inspect_directory(omni_directory.path(), H3LocalImportMode::OmniManifest)
            .await
            .expect("omni inspection should succeed");
        assert_eq!(omni.ready_count, 1);
        assert_eq!(omni.pairs[0].video_display_names, vec!["ref.mp4"]);
        assert_eq!(omni.pairs[0].audio_display_names, vec!["ref.wav"]);
    }

    #[test]
    fn manifest_rejects_absolute_parent_and_non_image_paths() {
        assert!(!validate_manifest_relative_path_for_test("C:/outside.png"));
        assert!(!validate_manifest_relative_path_for_test("../outside.png"));
        assert!(!validate_manifest_relative_path_for_test("/outside.png"));
        assert!(!validate_manifest_relative_path_for_test("prompt.txt"));
        assert!(validate_manifest_relative_path_for_test("nested/001.webp"));
    }

    #[test]
    fn natural_sort_orders_numeric_runs() {
        let mut values = vec!["10.png", "2.png", "1.png", "11.png"];
        values.sort_by(|left, right| natural_cmp(left, right));
        assert_eq!(values, vec!["1.png", "2.png", "10.png", "11.png"]);
    }

    #[tokio::test]
    async fn local_import_commit_is_read_only_until_commit_and_freezes_queue_values() {
        let directory = tempdir().expect("integration directory should exist");
        let project_root = directory.path().join("project");
        let fixture_root = directory.path().join("local-fixture");
        fs::create_dir_all(&project_root).expect("project root should exist");
        write_pairing_fixture(&fixture_root);
        let db_path = directory.path().join("app.db");
        let harness = build_harness(&db_path, &project_root, true).await;

        let before = (
            count_rows(&harness.pool, "assets").await,
            count_rows(&harness.pool, "asset_video_prompts").await,
            count_rows(&harness.pool, "production_batches").await,
        );
        let (session_id, inspection) = harness
            .local
            .pick(PROJECT_ID, fixture_root.clone(), H3LocalImportMode::Pairing)
            .await
            .expect("inspection should succeed");
        assert_eq!(inspection.ready_count, 2);
        assert_eq!(
            inspection.pairs[0].prompt_text.as_deref(),
            Some("Prompt A first line\nsecond line")
        );
        let after_inspection = (
            count_rows(&harness.pool, "assets").await,
            count_rows(&harness.pool, "asset_video_prompts").await,
            count_rows(&harness.pool, "production_batches").await,
        );
        assert_eq!(before, after_inspection, "inspection must be read-only");

        let result = harness
            .local
            .commit(
                &session_id,
                H3LocalImportCommitRequest {
                    batch_name: Some("H3 local integration".to_owned()),
                    workflow_version_id: "workflow-version-1".to_owned(),
                    recipe_id: "recipe-1".to_owned(),
                    width: 640,
                    height: 360,
                    duration_seconds: 5,
                    seed: Some(SeedValue::Fixed(123)),
                    auto_start: false,
                    generation_mode: None,
                },
            )
            .await
            .expect("commit should succeed");
        assert_eq!(result.imported_asset_count, 2);
        assert_eq!(result.item_count, 2);
        assert!(!result.auto_started);
        assert!(harness.local.sessions.lock().await.is_empty());

        let detail = harness
            .queue
            .get(PROJECT_ID, &result.batch_id)
            .await
            .expect("created batch should be readable");
        assert_eq!(detail.batch.status, ProductionBatchStatus::Ready);
        assert!(detail.batch.continue_on_failure);
        assert_eq!(detail.items.len(), 2);
        assert!(detail
            .items
            .iter()
            .all(|item| item.status == ProductionBatchItemStatus::Pending));

        let mut asset_ids = Vec::new();
        for (index, item) in detail.items.iter().enumerate() {
            assert_eq!(item.ordinal as usize, index);
            let expected_prompt = match index {
                0 => "Prompt A first line\nsecond line",
                1 => "Prompt A second line\ncontinuation",
                _ => unreachable!(),
            };
            assert_eq!(item.values_json["prompt"]["value"], expected_prompt);
            assert_eq!(item.values_json["reference_image"]["type"], "image_asset");
            let asset_id = item.values_json["reference_image"]["assetId"]
                .as_str()
                .expect("queue item should contain an asset id")
                .to_owned();
            asset_ids.push(asset_id.clone());
            assert_eq!(item.values_json["width"]["value"], 640);
            assert_eq!(item.values_json["height"]["value"], 360);
            assert_eq!(item.values_json["duration_seconds"]["value"], 5);
            assert_eq!(item.values_json["seed"]["type"], "seed_fixed");
            assert_eq!(item.values_json["seed"]["value"], "123");
            let values_text =
                serde_json::to_string(&item.values_json).expect("queue values should serialize");
            assert!(!values_text.contains(&fixture_root.to_string_lossy().to_string()));
            assert!(!values_text.contains("temporary_h3_asset"));
            assert!(!values_text.contains("local_asset"));

            let asset = harness
                .assets
                .find_by_id(&AssetId::parse(asset_id).expect("asset id should parse"))
                .await
                .expect("asset query should succeed")
                .expect("imported asset should exist");
            assert_eq!(asset.asset_type, AssetType::Image);
            assert_eq!(asset.category, crate::domain::SOURCE_IMAGE_CATEGORY);
            assert!(Path::new(&asset.storage_path).is_file());
            let prompt = harness
                .prompts
                .find(PROJECT_ID, asset.id.as_str())
                .await
                .expect("prompt query should succeed")
                .expect("asset prompt should exist");
            assert_eq!(prompt.prompt_text, expected_prompt);
        }
        assert_eq!(count_rows(&harness.pool, "assets").await, 2);
        assert_eq!(count_rows(&harness.pool, "asset_video_prompts").await, 2);
        assert_eq!(count_rows(&harness.pool, "production_batches").await, 1);

        fs::write(
            fixture_root.join("001.txt"),
            "Prompt B\nchanged after commit",
        )
        .expect("changed prompt should write");
        harness
            .prompt_service
            .set(PROJECT_ID, &asset_ids[0], "Prompt C\nasset prompt changed")
            .await
            .expect("asset prompt update should succeed");
        let frozen_detail = harness
            .queue
            .get(PROJECT_ID, &result.batch_id)
            .await
            .expect("frozen batch should remain readable");
        assert_eq!(
            frozen_detail.items[0].values_json["prompt"]["value"],
            "Prompt A first line\nsecond line"
        );
        assert_eq!(
            harness
                .prompts
                .find(PROJECT_ID, &asset_ids[0])
                .await
                .unwrap()
                .unwrap()
                .prompt_text,
            "Prompt C\nasset prompt changed"
        );

        fs::remove_dir_all(&fixture_root).expect("original local fixture should be removable");
        assert!(!fixture_root.exists());
        let deleted_folder_detail = harness
            .queue
            .get(PROJECT_ID, &result.batch_id)
            .await
            .expect("queue must not depend on the source folder");
        for item in deleted_folder_detail.items {
            let compiled = harness
                .queue
                .prepare_queue_values_for_test(
                    &item.workflow_version_id,
                    &item.recipe_id,
                    &item.values_json,
                )
                .await
                .expect("persisted Generation values should compile after folder removal");
            assert_eq!(compiled.len(), 6);
        }

        harness.pool.close().await;
        drop(harness);
        let restarted = build_harness(&db_path, &project_root, false).await;
        let persisted_assets = restarted
            .assets
            .list_recent(PROJECT_ID, 10)
            .await
            .expect("restarted asset repository should read assets");
        assert_eq!(persisted_assets.len(), 2);
        assert!(persisted_assets
            .iter()
            .all(|asset| asset.category == crate::domain::SOURCE_IMAGE_CATEGORY));
        for asset in &persisted_assets {
            assert!(Path::new(&asset.storage_path).is_file());
            assert!(restarted
                .prompts
                .find(PROJECT_ID, asset.id.as_str())
                .await
                .unwrap()
                .is_some());
        }
        let restarted_detail = restarted
            .queue
            .get(PROJECT_ID, &result.batch_id)
            .await
            .expect("batch should persist across repository restart");
        assert_eq!(restarted_detail.items.len(), 2);
        for item in restarted_detail.items {
            restarted
                .queue
                .prepare_queue_values_for_test(
                    &item.workflow_version_id,
                    &item.recipe_id,
                    &item.values_json,
                )
                .await
                .expect("restarted Generation values should compile");
        }
    }

    #[tokio::test]
    async fn local_import_partial_failure_keeps_imported_assets_and_explains_count() {
        let directory = tempdir().expect("partial failure directory should exist");
        let project_root = directory.path().join("project");
        let fixture_root = directory.path().join("partial-fixture");
        fs::create_dir_all(&project_root).expect("project root should exist");
        write_pairing_fixture(&fixture_root);
        let db_path = directory.path().join("app.db");
        let harness = build_harness(&db_path, &project_root, true).await;
        sqlx::query(
            "CREATE TRIGGER fail_h3_second_asset
             BEFORE INSERT ON assets
             WHEN NEW.original_name = '002.png'
             BEGIN SELECT RAISE(ABORT, 'forced second asset failure'); END",
        )
        .execute(&harness.pool)
        .await
        .expect("partial failure trigger should install");

        let (session_id, inspection) = harness
            .local
            .pick(PROJECT_ID, fixture_root, H3LocalImportMode::Pairing)
            .await
            .expect("partial failure inspection should succeed");
        assert_eq!(inspection.ready_count, 2);
        let error = harness
            .local
            .commit(
                &session_id,
                H3LocalImportCommitRequest {
                    batch_name: Some("partial failure".to_owned()),
                    workflow_version_id: "workflow-version-1".to_owned(),
                    recipe_id: "recipe-1".to_owned(),
                    width: 640,
                    height: 360,
                    duration_seconds: 5,
                    seed: Some(SeedValue::Fixed(123)),
                    auto_start: false,
                    generation_mode: None,
                },
            )
            .await
            .expect_err("second import should fail");
        assert!(error.to_string().contains("已导入 1 个图片素材后失败"));
        assert_eq!(count_rows(&harness.pool, "assets").await, 1);
        assert_eq!(count_rows(&harness.pool, "asset_video_prompts").await, 1);
        assert_eq!(count_rows(&harness.pool, "production_batches").await, 0);
        assert!(harness.local.sessions.lock().await.is_empty());
        let imported_assets = harness
            .assets
            .list_recent(PROJECT_ID, 10)
            .await
            .expect("successful partial asset should remain readable");
        assert_eq!(imported_assets.len(), 1);
        assert_eq!(imported_assets[0].original_name, "001.png");
    }

    #[tokio::test]
    async fn expired_local_import_sessions_are_removed_lazily() {
        let directory = tempdir().expect("session cleanup directory should exist");
        let project_root = directory.path().join("project");
        let first_fixture = directory.path().join("first-fixture");
        let second_fixture = directory.path().join("second-fixture");
        fs::create_dir_all(&project_root).expect("project root should exist");
        write_pairing_fixture(&first_fixture);
        write_pairing_fixture(&second_fixture);
        let harness = build_harness(&directory.path().join("app.db"), &project_root, true).await;

        let (expired_session_id, _) = harness
            .local
            .pick(PROJECT_ID, first_fixture, H3LocalImportMode::Pairing)
            .await
            .expect("first session should be created");
        {
            let mut sessions = harness.local.sessions.lock().await;
            sessions
                .get_mut(&expired_session_id)
                .expect("expired session should exist")
                .expires_at = Utc::now() - ChronoDuration::seconds(1);
        }
        let (active_session_id, _) = harness
            .local
            .pick(PROJECT_ID, second_fixture, H3LocalImportMode::Pairing)
            .await
            .expect("second session should be created");
        let sessions = harness.local.sessions.lock().await;
        assert!(!sessions.contains_key(&expired_session_id));
        assert!(sessions.contains_key(&active_session_id));
    }
}
