use crate::application::asset_video_prompt_service::AssetVideoPromptService;
use crate::application::generation_input_preparer::GenerationInputValue;
use crate::application::media_probe::{CommandMediaProbe, MediaProbe};
use crate::application::ports::Clock;
use crate::application::production_queue_service::{
    CreateProductionBatchItem, CreateProductionBatchRequest, ProductionQueueService,
};
use crate::application::source_asset_import_service::{
    SourceAssetImportService, MAX_SOURCE_AUDIO_BYTES, MAX_SOURCE_IMAGE_BYTES,
    MAX_SOURCE_VIDEO_BYTES,
};
use crate::domain::{Asset, ProductionPackageProvenance, SeedValue};
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
use tokio::io::AsyncReadExt;
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
    ProjectFolder,
}

const MAX_PROJECT_SEGMENTS: usize = 100;
const PROJECT_DEFAULT_DURATION_SECONDS: i64 = 5;
const PROJECT_DEFAULT_WIDTH: i64 = 960;
const PROJECT_DEFAULT_HEIGHT: i64 = 544;
const H3_OUTPUT_RESOLUTIONS: [(i64, i64); 14] = [
    (608, 352),
    (736, 416),
    (864, 480),
    (960, 544),
    (1056, 608),
    (1152, 640),
    (1216, 672),
    (1280, 736),
    (1344, 768),
    (1376, 768),
    (1504, 832),
    (1664, 928),
    (1824, 1024),
    (1920, 1088),
];
const PROJECT_IMAGE_MAX_ITEMS: usize = 9;
const PROJECT_VIDEO_MAX_ITEMS: usize = 3;
const PROJECT_AUDIO_MAX_ITEMS: usize = 3;

#[derive(Clone, Debug, Default)]
struct ProjectFrontMatter {
    mode: Option<String>,
    duration_seconds: Option<i64>,
    resolution: Option<(i64, i64)>,
    warnings: Vec<String>,
}

#[derive(Clone, Debug)]
struct ProjectPromptData {
    text: String,
    bytes: usize,
    front_matter: ProjectFrontMatter,
    prompt_spec: ProjectPromptSpec,
}

#[derive(Clone, Debug, Default)]
struct ProjectPromptSpec {
    duration_seconds: Option<i64>,
    resolution: Option<(i64, i64)>,
    duration_rounded: bool,
    duration_out_of_range: Option<f64>,
    unsupported_resolution: Option<(i64, i64)>,
    warnings: Vec<String>,
}

async fn inspect_project_folder(
    root_path: &Path,
    display_root_name: String,
) -> Result<H3LocalImportInspection, H3LocalImportError> {
    let directories = scan_project_segment_directories(root_path)?;
    let segment_count = directories.len();
    let mut segments = Vec::with_capacity(segment_count);
    for (ordinal, (folder_name, segment_path)) in directories.into_iter().enumerate() {
        segments.push(
            inspect_project_segment(ordinal + 1, folder_name, segment_path, root_path).await?,
        );
    }

    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    if segment_count == 0 {
        errors.push("未找到一级 Segment 文件夹。".to_owned());
    }
    if segment_count > MAX_PROJECT_SEGMENTS {
        errors.push("单次最多生成100段，请拆分项目文件夹或分批导入。".to_owned());
    }
    for segment in &segments {
        warnings.extend(segment.warnings.iter().cloned());
        if segment.status != "READY" {
            errors.push(format!(
                "第 {} 段 {}：{}",
                segment.ordinal,
                segment.folder_name,
                if segment.errors.is_empty() {
                    "Segment 检查未通过".to_owned()
                } else {
                    segment.errors.join("；")
                }
            ));
        }
    }

    let ready_count = segments
        .iter()
        .filter(|segment| segment.status == "READY")
        .count();
    let project = H3ProjectFolderInspection {
        display_root_name: display_root_name.clone(),
        segment_count,
        ready_count,
        error_count: errors.len(),
        segments,
        errors: errors.clone(),
        warnings: warnings.clone(),
    };
    let image_count = project
        .segments
        .iter()
        .flat_map(|segment| segment.all_media.iter())
        .filter(|media| media.kind == H3ProjectMediaKind::Image)
        .count();
    let prompt_count = project
        .segments
        .iter()
        .filter(|segment| segment.prompt_path.is_some())
        .count();
    Ok(H3LocalImportInspection {
        display_root_name,
        mode: H3LocalImportMode::ProjectFolder,
        detected_manifest: false,
        image_count,
        prompt_count,
        ready_count,
        error_count: errors.len(),
        pairs: Vec::new(),
        project_folder: Some(project),
        errors,
        warnings,
    })
}

fn scan_project_segment_directories(
    root_path: &Path,
) -> Result<Vec<(String, PathBuf)>, H3LocalImportError> {
    let mut directories = Vec::new();
    let entries = fs::read_dir(root_path)
        .map_err(|_| H3LocalImportError::Filesystem("项目文件夹无法读取".to_owned()))?;
    for entry in entries {
        let entry = entry
            .map_err(|_| H3LocalImportError::Filesystem("项目文件夹内容无法读取".to_owned()))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| H3LocalImportError::Filesystem("项目文件夹内容无法读取".to_owned()))?;
        if metadata.file_type().is_symlink() {
            let canonical = fs::canonicalize(&path).map_err(|_| {
                H3LocalImportError::FilesystemBoundary("Segment 链接无法解析".to_owned())
            })?;
            if !canonical.starts_with(root_path) {
                return Err(H3LocalImportError::FilesystemBoundary(
                    "Segment 链接越过项目文件夹边界".to_owned(),
                ));
            }
            return Err(H3LocalImportError::Inspection(
                "PROJECT_FOLDER 不接受符号链接或目录链接".to_owned(),
            ));
        }
        if !metadata.is_dir() {
            // ProjectRoot 下的普通文件是说明文件或用户杂项，按约定忽略。
            continue;
        }
        let canonical = fs::canonicalize(&path)
            .map_err(|_| H3LocalImportError::Filesystem("Segment 目录无法读取".to_owned()))?;
        if !canonical.starts_with(root_path) {
            return Err(H3LocalImportError::FilesystemBoundary(
                "Segment 目录越过项目文件夹边界".to_owned(),
            ));
        }
        let folder_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| H3LocalImportError::Inspection("Segment 文件夹名称无法识别".to_owned()))?
            .to_owned();
        directories.push((folder_name, canonical));
    }
    directories.sort_by(|left, right| natural_cmp(&left.0, &right.0));
    Ok(directories)
}

async fn inspect_project_segment(
    ordinal: usize,
    folder_name: String,
    segment_path: PathBuf,
    _root_path: &Path,
) -> Result<H3ProjectSegmentInspection, H3LocalImportError> {
    let segment_id = project_segment_id(&folder_name);
    let mut segment = empty_project_segment(ordinal, segment_id, folder_name, segment_path);
    let scanned_files = match scan_files(&segment.segment_path) {
        Ok(files) => files,
        Err(error) => {
            segment.errors.push(error.to_string());
            segment.status = "BLOCKED".to_owned();
            return Ok(segment);
        }
    };
    let prompt_files = scanned_files
        .iter()
        .filter(|file| is_prompt_extension(&file.extension))
        .cloned()
        .collect::<Vec<_>>();
    let prompt_file = select_project_prompt_file(&prompt_files);
    if prompt_files.len() > 1 && prompt_file.is_none() {
        segment.errors.push("AMBIGUOUS_PROMPT：Segment 内存在多个普通 Prompt 文件，请保留一个或命名为 prompt.txt / prompt.md。".to_owned());
    }
    if let Some(prompt_file) = prompt_file {
        segment.prompt_display_name = Some(prompt_file.relative_name.clone());
        segment.prompt_path = Some(prompt_file.path.clone());
        match fs::read(&prompt_file.path) {
            Ok(bytes) => match parse_project_prompt_bytes(&bytes) {
                Ok(prompt) => {
                    segment.prompt = Some(prompt.text);
                    segment.prompt_bytes = Some(prompt.bytes);
                    segment.prompt_sha256 = Some(hash_bytes(&bytes));
                    segment
                        .warnings
                        .extend(prompt.front_matter.warnings.clone());
                    segment.warnings.extend(prompt.prompt_spec.warnings.clone());
                    segment.front_matter = Some(prompt.front_matter);
                    segment.prompt_spec = Some(prompt.prompt_spec);
                }
                Err(error) => segment.errors.push(error),
            },
            Err(_) => segment.errors.push("Prompt 文件无法读取".to_owned()),
        }
    } else if prompt_files.is_empty() {
        segment
            .errors
            .push("MISSING_PROMPT：Segment 缺少 Prompt 文件。".to_owned());
    }

    for file in scanned_files
        .iter()
        .filter(|file| is_media_extension(&file.extension))
    {
        match inspect_project_media(file, &segment.segment_path).await {
            Ok(media) => segment.all_media.push(media),
            Err(error) => segment.errors.push(error),
        }
    }
    sort_project_media(&mut segment.all_media);
    initialize_project_segment_inputs(&mut segment, _root_path);
    recompute_project_segment_status(&mut segment);
    Ok(segment)
}

fn empty_project_segment(
    ordinal: usize,
    segment_id: String,
    folder_name: String,
    segment_path: PathBuf,
) -> H3ProjectSegmentInspection {
    H3ProjectSegmentInspection {
        ordinal,
        segment_id,
        folder_name,
        generation_mode: "FL2VA_TEXT_TO_VIDEO".to_owned(),
        inferred_mode: "FL2VA_TEXT_TO_VIDEO".to_owned(),
        mode_source: "AUTO_INFERENCE".to_owned(),
        prompt: None,
        prompt_display_name: None,
        prompt_bytes: None,
        width: PROJECT_DEFAULT_WIDTH,
        height: PROJECT_DEFAULT_HEIGHT,
        resolution_source: "RECIPE_DEFAULT".to_owned(),
        duration_seconds: PROJECT_DEFAULT_DURATION_SECONDS,
        duration_source: "RECIPE_DEFAULT".to_owned(),
        first_frame: None,
        last_frame: None,
        reference_images: Vec::new(),
        reference_audios: Vec::new(),
        reference_videos: Vec::new(),
        status: "BLOCKED".to_owned(),
        errors: Vec::new(),
        warnings: Vec::new(),
        segment_path,
        prompt_path: None,
        prompt_sha256: None,
        all_media: Vec::new(),
        front_matter: None,
        prompt_spec: None,
        base_errors: Vec::new(),
        base_warnings: Vec::new(),
    }
}

impl H3LocalImportMode {
    pub fn parse(value: &str) -> Result<Self, H3LocalImportError> {
        match value.trim() {
            "PAIRING" | "pairing" => Ok(Self::Pairing),
            "MANIFEST" | "manifest" => Ok(Self::Manifest),
            "TEXT" | "text" => Ok(Self::Text),
            "FIRST_LAST" | "first_last" => Ok(Self::FirstLast),
            "OMNI_MANIFEST" | "omni_manifest" => Ok(Self::OmniManifest),
            "PROJECT_FOLDER" | "project_folder" => Ok(Self::ProjectFolder),
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
            Self::ProjectFolder => "PROJECT_FOLDER",
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
    pub project_folder: Option<H3ProjectFolderInspection>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum H3ProjectMediaKind {
    Image,
    Audio,
    Video,
}

impl H3ProjectMediaKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Audio => "audio",
            Self::Video => "video",
        }
    }
}

#[derive(Clone, Debug)]
pub struct H3ProjectMedia {
    pub id: String,
    pub display_name: String,
    pub kind: H3ProjectMediaKind,
    pub path: PathBuf,
    pub sha256: String,
    pub size_bytes: u64,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct H3ProjectSegmentInspection {
    pub ordinal: usize,
    pub segment_id: String,
    pub folder_name: String,
    pub generation_mode: String,
    pub inferred_mode: String,
    pub mode_source: String,
    pub prompt: Option<String>,
    pub prompt_display_name: Option<String>,
    pub prompt_bytes: Option<usize>,
    pub width: i64,
    pub height: i64,
    pub resolution_source: String,
    pub duration_seconds: i64,
    pub duration_source: String,
    pub first_frame: Option<H3ProjectMedia>,
    pub last_frame: Option<H3ProjectMedia>,
    pub reference_images: Vec<H3ProjectMedia>,
    pub reference_audios: Vec<H3ProjectMedia>,
    pub reference_videos: Vec<H3ProjectMedia>,
    pub status: String,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub(crate) segment_path: PathBuf,
    pub(crate) prompt_path: Option<PathBuf>,
    pub(crate) prompt_sha256: Option<String>,
    pub(crate) all_media: Vec<H3ProjectMedia>,
    front_matter: Option<ProjectFrontMatter>,
    prompt_spec: Option<ProjectPromptSpec>,
    base_errors: Vec<String>,
    base_warnings: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct H3ProjectFolderInspection {
    pub display_root_name: String,
    pub segment_count: usize,
    pub ready_count: usize,
    pub error_count: usize,
    pub segments: Vec<H3ProjectSegmentInspection>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct H3ProjectSegmentDraft {
    pub segment_id: String,
    pub mode: Option<String>,
    pub prompt: Option<String>,
    pub duration_seconds: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub reference_image_ids: Option<Vec<String>>,
    pub reference_audio_ids: Option<Vec<String>>,
    pub reference_video_ids: Option<Vec<String>>,
    pub first_frame_id: Option<String>,
    pub last_frame_id: Option<String>,
    pub reset_auto_detection: bool,
}

#[derive(Clone, Debug)]
pub struct H3QualityRecipeSelection {
    pub mode: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
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
    pub fl2va_workflow_version_id: Option<String>,
    pub fl2va_recipe_id: Option<String>,
    pub ref2va_workflow_version_id: Option<String>,
    pub ref2va_recipe_id: Option<String>,
    pub quality_profile: Option<String>,
    pub quality_recipes: Vec<H3QualityRecipeSelection>,
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

    fn as_str(self) -> &'static str {
        match self {
            Self::LegacyReferenceImage => "REF2VA_IMAGE",
            Self::Fl2vaTextToVideo => "FL2VA_TEXT_TO_VIDEO",
            Self::Fl2vaImageToVideo => "FL2VA_IMAGE_TO_VIDEO",
            Self::Fl2vaFirstLast => "FL2VA_FIRST_LAST",
            Self::Ref2vaImage => "REF2VA_IMAGE",
            Self::Ref2vaAudio => "REF2VA_AUDIO",
            Self::Ref2vaImageAudio => "REF2VA_IMAGE_AUDIO",
            Self::Ref2vaVideoImage => "REF2VA_VIDEO_IMAGE",
        }
    }

    fn is_fl2va(self) -> bool {
        matches!(
            self,
            Self::Fl2vaTextToVideo | Self::Fl2vaImageToVideo | Self::Fl2vaFirstLast
        )
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
    project_drafts: HashMap<String, H3ProjectSegmentDraft>,
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
            project_drafts: HashMap::new(),
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
        let (root_path, project_drafts) = {
            let sessions = self.sessions.lock().await;
            let session = sessions
                .get(session_id)
                .ok_or(H3LocalImportError::SessionNotFound)?;
            if session.expires_at <= self.clock.now() {
                return Err(H3LocalImportError::SessionExpired);
            }
            (session.root_path.clone(), session.project_drafts.clone())
        };
        let mut inspection = inspect_directory(&root_path, mode).await?;
        if mode == H3LocalImportMode::ProjectFolder {
            apply_project_drafts(&mut inspection, &project_drafts)?;
        }
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or(H3LocalImportError::SessionNotFound)?;
        if session.expires_at <= self.clock.now() {
            sessions.remove(session_id);
            return Err(H3LocalImportError::SessionExpired);
        }
        session.inspection = inspection.clone();
        if mode != H3LocalImportMode::ProjectFolder {
            session.project_drafts.clear();
        }
        Ok(inspection)
    }

    pub async fn update_h3_project_segment_draft(
        &self,
        session_id: &str,
        draft: H3ProjectSegmentDraft,
    ) -> Result<H3LocalImportInspection, H3LocalImportError> {
        self.cleanup_expired_sessions(Some(session_id)).await;
        let (root_path, mut drafts) = {
            let sessions = self.sessions.lock().await;
            let session = sessions
                .get(session_id)
                .ok_or(H3LocalImportError::SessionNotFound)?;
            if session.expires_at <= self.clock.now() {
                return Err(H3LocalImportError::SessionExpired);
            }
            if session.inspection.mode != H3LocalImportMode::ProjectFolder {
                return Err(H3LocalImportError::Inspection(
                    "只有 PROJECT_FOLDER 导入会话支持 Segment 编辑".to_owned(),
                ));
            }
            (session.root_path.clone(), session.project_drafts.clone())
        };

        if draft.reset_auto_detection {
            drafts.remove(&draft.segment_id);
        } else {
            validate_project_segment_draft(&draft)?;
            drafts.insert(draft.segment_id.clone(), draft);
        }
        let mut inspection =
            inspect_directory(&root_path, H3LocalImportMode::ProjectFolder).await?;
        apply_project_drafts(&mut inspection, &drafts)?;
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or(H3LocalImportError::SessionNotFound)?;
        if session.expires_at <= self.clock.now() {
            sessions.remove(session_id);
            return Err(H3LocalImportError::SessionExpired);
        }
        session.project_drafts = drafts;
        session.inspection = inspection.clone();
        Ok(inspection)
    }

    pub async fn commit(
        &self,
        session_id: &str,
        request: H3LocalImportCommitRequest,
    ) -> Result<H3LocalImportResult, H3LocalImportError> {
        self.commit_with_provenance(session_id, request, None).await
    }

    pub async fn commit_with_provenance(
        &self,
        session_id: &str,
        request: H3LocalImportCommitRequest,
        provenance: Option<ProductionPackageProvenance>,
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
        let mut inspection = inspect_directory(&session.root_path, session.inspection.mode).await?;
        if session.inspection.mode == H3LocalImportMode::ProjectFolder {
            apply_project_drafts(&mut inspection, &session.project_drafts)?;
            return self
                .commit_project_folder(session, inspection, request, provenance)
                .await;
        }
        if provenance.is_some() {
            return Err(H3LocalImportError::InvalidInput(
                "production package provenance requires PROJECT_FOLDER import mode".to_owned(),
            ));
        }
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

    async fn commit_project_folder(
        &self,
        session: LocalImportSession,
        inspection: H3LocalImportInspection,
        request: H3LocalImportCommitRequest,
        provenance: Option<ProductionPackageProvenance>,
    ) -> Result<H3LocalImportResult, H3LocalImportError> {
        let project = inspection.project_folder.as_ref().ok_or_else(|| {
            H3LocalImportError::Inspection("PROJECT_FOLDER 扫描结果缺少 Segment 数据".to_owned())
        })?;
        if project.segment_count > MAX_PROJECT_SEGMENTS {
            return Err(H3LocalImportError::Inspection(
                "单次最多生成100段，请拆分项目文件夹或分批导入。".to_owned(),
            ));
        }
        if project.error_count > 0 || project.ready_count == 0 {
            return Err(H3LocalImportError::Inspection(format!(
                "项目文件夹检查未通过：{}",
                if project.errors.is_empty() {
                    "没有可生成的 Segment".to_owned()
                } else {
                    project.errors.join("；")
                }
            )));
        }

        let seed = request.seed.clone().unwrap_or(SeedValue::Random);
        let mut items = Vec::with_capacity(project.ready_count);
        let mut imported_asset_count = 0usize;
        for segment in project
            .segments
            .iter()
            .filter(|item| item.status == "READY")
        {
            let mode = H3CommitGenerationMode::parse(Some(&segment.generation_mode))?;
            let prompt_override = session
                .project_drafts
                .get(&segment.segment_id)
                .and_then(|draft| draft.prompt.as_deref());
            let prompt_text = self
                .read_project_segment_prompt(&session.root_path, segment, prompt_override)
                .await
                .map_err(|error| {
                    H3LocalImportError::Inspection(format!(
                        "第 {} 段 {}：{error}",
                        segment.ordinal, segment.folder_name
                    ))
                })?;

            let mut reference_images = Vec::new();
            let mut first_frame = None;
            let mut last_frame = None;
            let mut reference_videos = Vec::new();
            let mut reference_audios = Vec::new();
            let image_media = match mode {
                H3CommitGenerationMode::Fl2vaImageToVideo => {
                    segment.first_frame.iter().collect::<Vec<_>>()
                }
                H3CommitGenerationMode::Fl2vaFirstLast => segment
                    .first_frame
                    .iter()
                    .chain(segment.last_frame.iter())
                    .collect::<Vec<_>>(),
                H3CommitGenerationMode::Ref2vaImage
                | H3CommitGenerationMode::Ref2vaImageAudio
                | H3CommitGenerationMode::Ref2vaVideoImage => {
                    segment.reference_images.iter().collect::<Vec<_>>()
                }
                _ => Vec::new(),
            };
            for media in image_media {
                self.revalidate_project_media(&session.root_path, media)
                    .await?;
                let asset = self
                    .import_image_asset(
                        &session.project_id,
                        &session.root_path,
                        &media.path,
                        Some(media.sha256.as_str()),
                        imported_asset_count,
                        mode.imported_asset_label(),
                    )
                    .await?;
                imported_asset_count += 1;
                self.set_imported_image_prompt(
                    &session.project_id,
                    &asset,
                    &prompt_text,
                    imported_asset_count,
                    mode.imported_asset_label(),
                )
                .await?;
                if mode == H3CommitGenerationMode::Fl2vaImageToVideo {
                    first_frame = Some(asset);
                } else if mode == H3CommitGenerationMode::Fl2vaFirstLast && first_frame.is_none() {
                    first_frame = Some(asset);
                } else if mode == H3CommitGenerationMode::Fl2vaFirstLast {
                    last_frame = Some(asset);
                } else {
                    reference_images.push(asset);
                }
            }
            for media in &segment.reference_videos {
                self.revalidate_project_media(&session.root_path, media)
                    .await?;
                let asset = self
                    .import_media_asset(
                        &session.project_id,
                        &session.root_path,
                        &media.path,
                        true,
                        imported_asset_count,
                        mode.imported_asset_label(),
                    )
                    .await?;
                imported_asset_count += 1;
                reference_videos.push(asset);
            }
            for media in &segment.reference_audios {
                self.revalidate_project_media(&session.root_path, media)
                    .await?;
                let asset = self
                    .import_media_asset(
                        &session.project_id,
                        &session.root_path,
                        &media.path,
                        false,
                        imported_asset_count,
                        mode.imported_asset_label(),
                    )
                    .await?;
                imported_asset_count += 1;
                reference_audios.push(asset);
            }

            let mut values = BTreeMap::new();
            values.insert("prompt".to_owned(), GenerationInputValue::Text(prompt_text));
            match mode {
                H3CommitGenerationMode::Fl2vaTextToVideo => {}
                H3CommitGenerationMode::Fl2vaImageToVideo => {
                    values.insert(
                        "first_frame".to_owned(),
                        GenerationInputValue::ImageAsset(
                            first_frame
                                .ok_or_else(|| {
                                    H3LocalImportError::Inspection(
                                        "Segment 缺少首帧图片".to_owned(),
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
                                        "Segment 缺少首帧图片".to_owned(),
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
                                        "Segment 缺少末帧图片".to_owned(),
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
                    if matches!(mode, H3CommitGenerationMode::Ref2vaImageAudio) {
                        values.insert(
                            "reference_audios".to_owned(),
                            GenerationInputValue::AudioAssets(
                                reference_audios.into_iter().map(|asset| asset.id).collect(),
                            ),
                        );
                    }
                    if matches!(mode, H3CommitGenerationMode::Ref2vaVideoImage) {
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
                H3CommitGenerationMode::LegacyReferenceImage => unreachable!(),
            }
            values.insert(
                "width".to_owned(),
                GenerationInputValue::Integer(segment.width),
            );
            values.insert(
                "height".to_owned(),
                GenerationInputValue::Integer(segment.height),
            );
            values.insert(
                "duration_seconds".to_owned(),
                GenerationInputValue::Integer(segment.duration_seconds),
            );
            values.insert("seed".to_owned(), GenerationInputValue::Seed(seed.clone()));
            let (workflow_version_id, recipe_id) = project_recipe_ids(&request, mode)?;
            items.push(CreateProductionBatchItem {
                workflow_version_id,
                recipe_id,
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
                    "H3 项目文件夹 · {} · {}",
                    inspection.display_root_name,
                    self.clock.now().format("%Y-%m-%d %H:%M:%S")
                )
            });
        let detail = self
            .production_queue_service
            .create_with_provenance(
                CreateProductionBatchRequest {
                    project_id: session.project_id.clone(),
                    name: batch_name.clone(),
                    continue_on_failure: true,
                    items,
                },
                provenance,
            )
            .await
            .map_err(|error| {
                H3LocalImportError::Queue(format!(
                    "批次创建失败（已导入 {imported_asset_count} 个素材）：{error}"
                ))
            })?;
        let mut warnings = project.warnings.clone();
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

    async fn read_project_segment_prompt(
        &self,
        root_path: &Path,
        segment: &H3ProjectSegmentInspection,
        prompt_override: Option<&str>,
    ) -> Result<String, String> {
        let path = segment
            .prompt_path
            .as_ref()
            .ok_or_else(|| "Prompt 文件不存在".to_owned())?;
        let path = revalidate_file_path(root_path, path).map_err(|error| error.to_string())?;
        let bytes = tokio::fs::read(&path)
            .await
            .map_err(|_| "Prompt 文件在提交前无法读取".to_owned())?;
        if segment.prompt_sha256.as_deref() != Some(hash_bytes(&bytes).as_str()) {
            return Err("Segment changed：Prompt 在检查后发生变化，请重新扫描。".to_owned());
        }
        if let Some(prompt) = prompt_override {
            return parse_prompt_text(prompt)
                .map(|(text, _)| text)
                .map_err(format_project_prompt_issue);
        }
        parse_project_prompt_bytes(&bytes)
            .map(|prompt| prompt.text)
            .map_err(|error| format!("Segment changed：{error}"))
    }

    async fn revalidate_project_media(
        &self,
        root_path: &Path,
        media: &H3ProjectMedia,
    ) -> Result<(), H3LocalImportError> {
        let path = revalidate_file_path(root_path, &media.path)?;
        let current_hash = hash_file(&path)
            .await
            .map_err(H3LocalImportError::Filesystem)?;
        if current_hash != media.sha256 {
            return Err(H3LocalImportError::Inspection(format!(
                "Segment changed：{} 在检查后发生变化，请重新扫描。",
                media.display_name
            )));
        }
        Ok(())
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
        H3LocalImportMode::ProjectFolder => {
            inspect_project_folder(root_path, display_root_name).await
        }
    }
}

fn select_project_prompt_file<'a>(files: &'a [ScannedFile]) -> Option<&'a ScannedFile> {
    files
        .iter()
        .find(|file| {
            file.path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("prompt.txt"))
        })
        .or_else(|| {
            files.iter().find(|file| {
                file.path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.eq_ignore_ascii_case("prompt.md"))
            })
        })
        .or_else(|| (files.len() == 1).then(|| &files[0]))
}

fn parse_project_prompt_bytes(bytes: &[u8]) -> Result<ProjectPromptData, String> {
    if bytes.len() > crate::application::asset_video_prompt_service::MAX_ASSET_VIDEO_PROMPT_BYTES {
        return Err("PROMPT_TOO_LARGE：Prompt 不能超过 64 KiB。".to_owned());
    }
    let text = String::from_utf8(bytes.to_vec())
        .map_err(|_| "INVALID_PROMPT_ENCODING：Prompt 必须是 UTF-8 文本。".to_owned())?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let mut front_matter = ProjectFrontMatter::default();
    let body = if first_line_is_front_matter(text) {
        let lines = text.split_inclusive('\n').collect::<Vec<_>>();
        let mut closing_index = None;
        for (index, line) in lines.iter().enumerate().skip(1) {
            if trim_line(line) == "---" {
                closing_index = Some(index);
                break;
            }
        }
        let Some(closing_index) = closing_index else {
            return Err("FRONT_MATTER_INVALID：Prompt front matter 缺少结束标记。".to_owned());
        };
        for line in lines.iter().take(closing_index).skip(1) {
            let line = trim_line(line);
            if line.is_empty() {
                continue;
            }
            let Some((key, value)) = line.split_once(':') else {
                return Err(format!("FRONT_MATTER_INVALID：无法解析字段「{line}」。"));
            };
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();
            match key.as_str() {
                "mode" => {
                    let mode = value.to_ascii_lowercase();
                    if mode != "auto" && project_mode_from_front_matter(&mode).is_none() {
                        return Err(format!("FRONT_MATTER_INVALID：不支持的 mode「{value}」。"));
                    }
                    front_matter.mode = Some(mode);
                }
                "duration" => {
                    let duration = value
                        .parse::<i64>()
                        .map_err(|_| "FRONT_MATTER_INVALID：duration 必须是整数。".to_owned())?;
                    front_matter.duration_seconds = Some(duration);
                }
                "resolution" => {
                    let (width, height) = find_exact_resolution(value, 0).ok_or_else(|| {
                        "FRONT_MATTER_INVALID：resolution 必须是 widthxheight。".to_owned()
                    })?;
                    front_matter.resolution = Some((width, height));
                }
                _ => front_matter
                    .warnings
                    .push(format!("忽略未知 front matter 字段「{key}」。")),
            }
        }
        lines
            .iter()
            .skip(closing_index + 1)
            .copied()
            .collect::<String>()
    } else {
        text.to_owned()
    };
    let (prompt, _) = parse_prompt_text_with_bytes(body, bytes.len())
        .map_err(|issue| format_project_prompt_issue(issue))?;
    let prompt_spec = parse_project_prompt_spec(&prompt);
    Ok(ProjectPromptData {
        text: prompt,
        bytes: bytes.len(),
        front_matter,
        prompt_spec,
    })
}

fn parse_project_prompt_spec(prompt: &str) -> ProjectPromptSpec {
    let mut spec = ProjectPromptSpec::default();
    for line in project_prompt_scan_lines(prompt) {
        let is_spec_line = is_project_spec_line(line) || is_compact_project_spec_line(line);
        if !is_spec_line
            && !has_project_parameter_key(line, true)
            && !has_project_parameter_key(line, false)
        {
            continue;
        }

        if spec.duration_seconds.is_none() && spec.duration_out_of_range.is_none() {
            let duration = if is_spec_line {
                find_duration_with_unit(line, 0)
            } else {
                find_explicit_parameter_value(line, true)
                    .and_then(|start| find_duration_value(line, start, false))
            };
            if let Some((value, raw)) = duration {
                apply_prompt_duration(&mut spec, value, &raw);
            }
        }

        if spec.resolution.is_none() {
            let resolution = if is_spec_line {
                find_resolution_or_alias(line, 0, &mut spec.warnings)
            } else {
                find_explicit_parameter_value(line, false)
                    .and_then(|start| find_resolution_or_alias(line, start, &mut spec.warnings))
            };
            if let Some(resolution) = resolution {
                spec.resolution = Some(resolution);
                if !valid_project_resolution(resolution.0, resolution.1) {
                    spec.unsupported_resolution = Some(resolution);
                }
            }
        }
    }
    spec
}

fn project_prompt_scan_lines(prompt: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    for (index, line) in prompt.lines().enumerate() {
        if index >= 30 || is_project_timeline_line(line) {
            break;
        }
        lines.push(line);
    }
    lines
}

fn is_project_timeline_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.contains("时间线") || contains_ascii_word(trimmed, "timeline") {
        return true;
    }
    let bytes = trimmed.as_bytes();
    let mut index = 0;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    if index == 0 {
        return false;
    }
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    let separator = trimmed.get(index..).and_then(|value| value.chars().next());
    if !separator.is_some_and(|value| matches!(value, '-' | '–' | '—')) {
        return false;
    }
    index += separator.unwrap().len_utf8();
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    let second_start = index;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    if second_start == index {
        return false;
    }
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    bytes
        .get(index)
        .is_some_and(|byte| *byte == b's' || *byte == b'S')
        || trimmed
            .get(index..)
            .is_some_and(|value| value.starts_with('秒'))
}

fn is_project_spec_line(line: &str) -> bool {
    line.contains("规格")
        || contains_ascii_word(line, "spec")
        || contains_ascii_word(line, "specification")
}

fn is_compact_project_spec_line(line: &str) -> bool {
    find_duration_with_unit(line, 0).is_some() && find_exact_resolution(line, 0).is_some()
}

fn contains_ascii_word(value: &str, word: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let mut offset = 0;
    while let Some(found) = lower[offset..].find(&word.to_ascii_lowercase()) {
        let start = offset + found;
        let end = start + word.len();
        let before_ok = start == 0 || !lower.as_bytes()[start - 1].is_ascii_alphanumeric();
        let after_ok = end >= lower.len() || !lower.as_bytes()[end].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
        offset = end;
        if offset >= lower.len() {
            break;
        }
    }
    false
}

fn has_project_parameter_key(line: &str, duration: bool) -> bool {
    let keys = if duration {
        &["视频时长", "总时长", "时长", "video duration", "duration"][..]
    } else {
        &["输出分辨率", "分辨率", "video resolution", "resolution"][..]
    };
    keys.iter().any(|key| find_project_key(line, key).is_some())
}

fn find_project_key(line: &str, key: &str) -> Option<usize> {
    if key.is_ascii() {
        let lower = line.to_ascii_lowercase();
        let key_lower = key.to_ascii_lowercase();
        let mut offset = 0;
        while let Some(found) = lower[offset..].find(&key_lower) {
            let start = offset + found;
            let end = start + key_lower.len();
            let before_ok = start == 0 || !lower.as_bytes()[start - 1].is_ascii_alphanumeric();
            let after_ok = end >= lower.len() || !lower.as_bytes()[end].is_ascii_alphanumeric();
            if before_ok && after_ok {
                return Some(end);
            }
            offset = end;
            if offset >= lower.len() {
                break;
            }
        }
        None
    } else {
        line.find(key).map(|start| start + key.len())
    }
}

fn find_explicit_parameter_value(line: &str, duration: bool) -> Option<usize> {
    let keys = if duration {
        &["视频时长", "总时长", "时长", "video duration", "duration"][..]
    } else {
        &["输出分辨率", "分辨率", "video resolution", "resolution"][..]
    };
    keys.iter().find_map(|key| {
        find_project_key(line, key).map(|mut index| {
            while index < line.len() {
                let character = line[index..]
                    .chars()
                    .next()
                    .expect("parameter separator should be a character");
                if character.is_whitespace() || matches!(character, ':' | '=' | '：' | '｜' | '|')
                {
                    index += character.len_utf8();
                } else {
                    break;
                }
            }
            index
        })
    })
}

fn find_duration_with_unit(line: &str, start: usize) -> Option<(f64, String)> {
    find_duration_value(line, start, true)
}

fn find_duration_value(line: &str, start: usize, require_unit: bool) -> Option<(f64, String)> {
    let tail = line.get(start..)?;
    for (offset, character) in tail.char_indices() {
        if !character.is_ascii_digit() && character != '-' && character != '+' {
            continue;
        }
        let candidate = &tail[offset..];
        let mut end = 0;
        let mut dot_count = 0;
        for (index, character) in candidate.char_indices() {
            if character.is_ascii_digit()
                || ((character == '.' || character == ',') && dot_count == 0)
            {
                if character == '.' || character == ',' {
                    dot_count += 1;
                }
                end = index + character.len_utf8();
            } else {
                break;
            }
        }
        if end == 0 || candidate[..end].ends_with(['+', '-']) {
            continue;
        }
        let raw = candidate[..end].replace(',', ".");
        let value = raw.parse::<f64>().ok()?;
        let rest = candidate[end..].trim_start();
        let has_unit = rest.starts_with('秒') || rest.starts_with('s') || rest.starts_with('S');
        if !require_unit || has_unit {
            return Some((value, candidate[..end].to_owned()));
        }
    }
    None
}

fn apply_prompt_duration(spec: &mut ProjectPromptSpec, value: f64, raw: &str) {
    if !value.is_finite() {
        return;
    }
    let rounded = value.round();
    let rounded_i64 = rounded as i64;
    if !(1..=15).contains(&rounded_i64) {
        spec.duration_out_of_range = Some(value);
        return;
    }
    spec.duration_seconds = Some(rounded_i64);
    if (value - rounded).abs() > f64::EPSILON {
        spec.duration_rounded = true;
        spec.warnings.push(format!(
            "Prompt时长{raw}秒已按H3整数秒要求取整为{rounded_i64}秒。"
        ));
    }
}

fn format_prompt_number(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}

fn find_resolution_or_alias(
    line: &str,
    start: usize,
    warnings: &mut Vec<String>,
) -> Option<(i64, i64)> {
    if let Some(resolution) = find_exact_resolution(line, start) {
        return Some(resolution);
    }
    let lower = line.get(start..)?.to_ascii_lowercase();
    for alias in ["1080p", "2k", "1k"] {
        if contains_project_alias(&lower, alias) {
            return h3_resolution_alias(alias);
        }
    }
    if contains_unknown_project_alias(&lower) {
        warnings.push("Prompt 指定了未定义的模糊分辨率别名，已回退到其他参数来源。".to_owned());
    }
    None
}

fn contains_project_alias(value: &str, alias: &str) -> bool {
    let mut offset = 0;
    while let Some(found) = value[offset..].find(alias) {
        let start = offset + found;
        let end = start + alias.len();
        let before_ok = start == 0 || !value.as_bytes()[start - 1].is_ascii_alphanumeric();
        let after_ok = end >= value.len() || !value.as_bytes()[end].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
        offset = end;
        if offset >= value.len() {
            break;
        }
    }
    false
}

fn contains_unknown_project_alias(value: &str) -> bool {
    let bytes = value.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if !byte.is_ascii_digit() || (index > 0 && bytes[index - 1].is_ascii_alphanumeric()) {
            continue;
        }
        let mut end = index;
        while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b'.') {
            end += 1;
        }
        if end < bytes.len() && matches!(bytes[end], b'k' | b'K' | b'p' | b'P') {
            return true;
        }
    }
    false
}

fn h3_resolution_alias(alias: &str) -> Option<(i64, i64)> {
    match alias.to_ascii_lowercase().as_str() {
        "2k" | "1080p" => Some((1920, 1088)),
        "1k" => Some((PROJECT_DEFAULT_WIDTH, PROJECT_DEFAULT_HEIGHT)),
        _ => None,
    }
}

fn find_exact_resolution(line: &str, start: usize) -> Option<(i64, i64)> {
    let tail = line.get(start..)?;
    let bytes = tail.as_bytes();
    for (offset, character) in tail.char_indices() {
        if !character.is_ascii_digit() {
            continue;
        }
        let mut index = offset;
        let width_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        let width = tail.get(width_start..index)?.parse::<i64>().ok()?;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() {
            continue;
        }
        let separator = tail[index..].chars().next()?;
        if !matches!(separator, 'x' | 'X' | '×' | '*') {
            continue;
        }
        index += separator.len_utf8();
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let height_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if height_start == index {
            continue;
        }
        let height = tail.get(height_start..index)?.parse::<i64>().ok()?;
        return Some((width, height));
    }
    None
}

fn first_line_is_front_matter(text: &str) -> bool {
    text.split_inclusive('\n')
        .next()
        .is_some_and(|line| trim_line(line) == "---")
}

fn trim_line(line: &str) -> &str {
    line.trim_end_matches(['\r', '\n']).trim()
}

fn format_project_prompt_issue(issue: PromptIssue) -> String {
    match issue {
        PromptIssue::Empty => "EMPTY_PROMPT：Prompt 不能为空。".to_owned(),
        PromptIssue::TooLarge => "PROMPT_TOO_LARGE：Prompt 不能超过 64 KiB。".to_owned(),
        PromptIssue::InvalidEncoding => {
            "INVALID_PROMPT_ENCODING：Prompt 必须是 UTF-8 文本。".to_owned()
        }
        PromptIssue::Read => "Prompt 文件无法读取".to_owned(),
    }
}

fn project_mode_from_front_matter(value: &str) -> Option<H3CommitGenerationMode> {
    match value {
        "text" => Some(H3CommitGenerationMode::Fl2vaTextToVideo),
        "image" => Some(H3CommitGenerationMode::Fl2vaImageToVideo),
        "first_last" => Some(H3CommitGenerationMode::Fl2vaFirstLast),
        "ref_image" => Some(H3CommitGenerationMode::Ref2vaImage),
        "ref_audio" => Some(H3CommitGenerationMode::Ref2vaAudio),
        "ref_image_audio" => Some(H3CommitGenerationMode::Ref2vaImageAudio),
        "ref_video_image" => Some(H3CommitGenerationMode::Ref2vaVideoImage),
        _ => None,
    }
}

fn project_segment_id(folder_name: &str) -> String {
    let digest = Sha256::digest(folder_name.to_ascii_lowercase().as_bytes());
    format!("seg_{:x}", digest)[..20].to_owned()
}

fn project_media_id(kind: H3ProjectMediaKind, display_name: &str) -> String {
    let source = format!("{}:{}", kind.as_str(), display_name.to_ascii_lowercase());
    let digest = Sha256::digest(source.as_bytes());
    format!("med_{:x}", digest)[..20].to_owned()
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

async fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|_| "文件无法读取".to_owned())?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .await
            .map_err(|_| "文件无法读取".to_owned())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

async fn inspect_project_media(
    file: &ScannedFile,
    segment_root: &Path,
) -> Result<H3ProjectMedia, String> {
    let canonical = fs::canonicalize(&file.path)
        .map_err(|_| "MEDIA_UNREADABLE：媒体文件无法解析。".to_owned())?;
    if !canonical.starts_with(segment_root) {
        return Err("FILESYSTEM_BOUNDARY_ERROR：媒体文件越过 Segment 边界。".to_owned());
    }
    let metadata =
        fs::metadata(&canonical).map_err(|_| "MEDIA_UNREADABLE：媒体文件无法读取。".to_owned())?;
    if !metadata.is_file() {
        return Err("MEDIA_UNREADABLE：媒体文件不再是普通文件。".to_owned());
    }
    let kind = if is_image_extension(&file.extension) {
        H3ProjectMediaKind::Image
    } else if is_audio_extension(&file.extension) {
        H3ProjectMediaKind::Audio
    } else {
        H3ProjectMediaKind::Video
    };
    let size_bytes = metadata.len();
    let max_bytes = match kind {
        H3ProjectMediaKind::Image => MAX_SOURCE_IMAGE_BYTES,
        H3ProjectMediaKind::Audio => MAX_SOURCE_AUDIO_BYTES,
        H3ProjectMediaKind::Video => MAX_SOURCE_VIDEO_BYTES,
    };
    if size_bytes > max_bytes {
        return Err(format!(
            "MEDIA_TOO_LARGE：{} 超过允许的文件大小。",
            file.relative_name
        ));
    }
    let (sha256, width, height, duration_ms) = match kind {
        H3ProjectMediaKind::Image => {
            let bytes =
                fs::read(&canonical).map_err(|_| "INVALID_IMAGE：图片无法读取。".to_owned())?;
            let inspected = crate::application::image_inspection::inspect_bytes(&bytes)
                .map_err(|error| format!("INVALID_IMAGE：{error}"))?;
            (
                inspected.sha256,
                Some(i64::from(inspected.width)),
                Some(i64::from(inspected.height)),
                None,
            )
        }
        H3ProjectMediaKind::Audio => (
            hash_file(&canonical).await?,
            None,
            None,
            CommandMediaProbe::default()
                .probe_audio(&canonical)
                .await
                .duration_ms,
        ),
        H3ProjectMediaKind::Video => {
            let metadata = CommandMediaProbe::default().probe_video(&canonical).await;
            (
                hash_file(&canonical).await?,
                metadata.width.map(i64::from),
                metadata.height.map(i64::from),
                metadata.duration_ms,
            )
        }
    };
    Ok(H3ProjectMedia {
        id: project_media_id(kind, &file.relative_name),
        display_name: file.relative_name.clone(),
        kind,
        path: canonical,
        sha256,
        size_bytes,
        width,
        height,
        duration_ms,
    })
}

fn is_media_extension(extension: &str) -> bool {
    is_image_extension(extension) || is_video_extension(extension) || is_audio_extension(extension)
}

fn sort_project_media(media: &mut [H3ProjectMedia]) {
    media.sort_by(|left, right| {
        let prefix = |kind: H3ProjectMediaKind, name: &str| -> (u8, String) {
            let lower = name.to_ascii_lowercase();
            let marker = match kind {
                H3ProjectMediaKind::Image => 'p',
                H3ProjectMediaKind::Audio => 'a',
                H3ProjectMediaKind::Video => 'v',
            };
            let bytes = lower.as_bytes();
            let mut digit_end = 1usize;
            while digit_end < bytes.len() && bytes[digit_end].is_ascii_digit() {
                digit_end += 1;
            }
            let is_prefixed = lower.starts_with(marker)
                && digit_end > 1
                && bytes.get(digit_end).is_some_and(|byte| *byte == b'_');
            (if is_prefixed { 0 } else { 1 }, lower)
        };
        let left_key = prefix(left.kind, &left.display_name);
        let right_key = prefix(right.kind, &right.display_name);
        left_key
            .0
            .cmp(&right_key.0)
            .then_with(|| natural_cmp(&left_key.1, &right_key.1))
    });
}

fn frame_candidates(media: &[H3ProjectMedia], first: bool) -> Vec<H3ProjectMedia> {
    media
        .iter()
        .filter(|item| item.kind == H3ProjectMediaKind::Image)
        .filter(|item| is_frame_name(&item.display_name, first))
        .cloned()
        .collect()
}

fn is_frame_name(name: &str, first: bool) -> bool {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let tokens = stem
        .split(|character: char| matches!(character, '_' | '-' | ' '))
        .collect::<Vec<_>>();
    let aliases: &[&str] = if first {
        &[
            "first",
            "first_frame",
            "start",
            "start_frame",
            "首帧",
            "开始帧",
        ]
    } else {
        &["last", "last_frame", "end", "end_frame", "尾帧", "结束帧"]
    };
    aliases.iter().any(|alias| {
        stem == *alias
            || stem.ends_with(&format!("_{alias}"))
            || tokens.last().is_some_and(|token| *token == *alias)
    })
}

fn infer_project_mode(media: &[H3ProjectMedia]) -> (H3CommitGenerationMode, Vec<String>) {
    let images = media
        .iter()
        .filter(|item| item.kind == H3ProjectMediaKind::Image)
        .count();
    let audios = media
        .iter()
        .filter(|item| item.kind == H3ProjectMediaKind::Audio)
        .count();
    let videos = media
        .iter()
        .filter(|item| item.kind == H3ProjectMediaKind::Video)
        .count();
    let firsts = frame_candidates(media, true);
    let lasts = frame_candidates(media, false);
    let mut errors = Vec::new();
    if firsts.len() > 1 {
        errors.push("AMBIGUOUS_FIRST_FRAME：首帧候选不唯一。".to_owned());
    }
    if lasts.len() > 1 {
        errors.push("AMBIGUOUS_LAST_FRAME：末帧候选不唯一。".to_owned());
    }
    if firsts.len() == 1 && lasts.len() == 1 {
        return (H3CommitGenerationMode::Fl2vaFirstLast, errors);
    }
    let mode = match (images, audios, videos) {
        (0, 0, 0) => H3CommitGenerationMode::Fl2vaTextToVideo,
        (1, 0, 0) => H3CommitGenerationMode::Fl2vaImageToVideo,
        (2.., 0, 0) => H3CommitGenerationMode::Ref2vaImage,
        (0, 1.., 0) => H3CommitGenerationMode::Ref2vaAudio,
        (1.., 1.., 0) => H3CommitGenerationMode::Ref2vaImageAudio,
        (1.., 0, 1..) => H3CommitGenerationMode::Ref2vaVideoImage,
        _ => {
            errors.push(
                "AMBIGUOUS_MEDIA_COMBINATION：当前素材同时包含图片、音频和视频，请手动选择生成模式或调整素材。"
                    .to_owned(),
            );
            H3CommitGenerationMode::Ref2vaVideoImage
        }
    };
    (mode, errors)
}

fn initialize_project_segment_inputs(segment: &mut H3ProjectSegmentInspection, _root_path: &Path) {
    segment.base_errors = segment.errors.clone();
    segment.base_warnings = segment.warnings.clone();
    let front_matter_duration = segment
        .front_matter
        .as_ref()
        .and_then(|matter| matter.duration_seconds);
    let front_matter_resolution = segment
        .front_matter
        .as_ref()
        .and_then(|matter| matter.resolution);
    if front_matter_duration.is_some() {
        segment
            .warnings
            .retain(|warning| !warning.starts_with("Prompt时长"));
    }
    if front_matter_resolution.is_some() {
        segment
            .warnings
            .retain(|warning| !warning.starts_with("Prompt 指定了未定义"));
    }
    segment.base_warnings = segment.warnings.clone();
    if let Some(prompt_spec) = segment.prompt_spec.as_ref() {
        if front_matter_duration.is_none() {
            if let Some(duration) = prompt_spec.duration_out_of_range {
                segment.base_errors.push(format!(
                    "PROMPT_DURATION_UNSUPPORTED：Prompt指定{}秒，当前 H3 只支持 1–15 秒。",
                    format_prompt_number(duration)
                ));
            }
        }
        if front_matter_resolution.is_none() {
            if let Some((width, height)) = prompt_spec.unsupported_resolution {
                segment.base_errors.push(format!(
                    "PROMPT_RESOLUTION_UNSUPPORTED：Prompt指定{}×{}，但当前H3不支持该输出尺寸，请选择合法分辨率。",
                    width, height
                ));
            }
        }
    }
    let (inferred, inference_errors) = infer_project_mode(&segment.all_media);
    segment.inferred_mode = inferred.as_str().to_owned();
    let front_mode = segment
        .front_matter
        .as_ref()
        .and_then(|matter| matter.mode.as_deref())
        .filter(|mode| *mode != "auto")
        .and_then(project_mode_from_front_matter);
    let effective = front_mode.unwrap_or(inferred);
    segment.generation_mode = effective.as_str().to_owned();
    segment.mode_source = if front_mode.is_some() {
        "FRONT_MATTER".to_owned()
    } else {
        "AUTO_INFERENCE".to_owned()
    };
    let firsts = frame_candidates(&segment.all_media, true);
    let lasts = frame_candidates(&segment.all_media, false);
    select_project_media_for_mode(segment, effective, firsts, lasts);
    let (width, height, resolution_source) = project_resolution_for_segment(segment, effective);
    segment.width = width;
    segment.height = height;
    segment.resolution_source = resolution_source;
    let (duration, duration_source) = project_duration_for_segment(segment, effective);
    segment.duration_seconds = duration;
    segment.duration_source = duration_source;
    segment.errors = segment.base_errors.clone();
    if front_mode.is_none() {
        segment.errors.extend(inference_errors);
    }
}

fn select_project_media_for_mode(
    segment: &mut H3ProjectSegmentInspection,
    mode: H3CommitGenerationMode,
    firsts: Vec<H3ProjectMedia>,
    lasts: Vec<H3ProjectMedia>,
) {
    segment.first_frame = None;
    segment.last_frame = None;
    segment.reference_images.clear();
    segment.reference_audios.clear();
    segment.reference_videos.clear();
    let images = segment
        .all_media
        .iter()
        .filter(|item| item.kind == H3ProjectMediaKind::Image)
        .cloned()
        .collect::<Vec<_>>();
    let audios = segment
        .all_media
        .iter()
        .filter(|item| item.kind == H3ProjectMediaKind::Audio)
        .cloned()
        .collect::<Vec<_>>();
    let videos = segment
        .all_media
        .iter()
        .filter(|item| item.kind == H3ProjectMediaKind::Video)
        .cloned()
        .collect::<Vec<_>>();
    match mode {
        H3CommitGenerationMode::Fl2vaTextToVideo => {}
        H3CommitGenerationMode::Fl2vaImageToVideo => {
            segment.first_frame = images.first().cloned();
        }
        H3CommitGenerationMode::Fl2vaFirstLast => {
            segment.first_frame = firsts.first().cloned();
            segment.last_frame = lasts.first().cloned();
        }
        H3CommitGenerationMode::Ref2vaImage => segment.reference_images = images,
        H3CommitGenerationMode::Ref2vaAudio => segment.reference_audios = audios,
        H3CommitGenerationMode::Ref2vaImageAudio => {
            segment.reference_images = images;
            segment.reference_audios = audios;
        }
        H3CommitGenerationMode::Ref2vaVideoImage => {
            segment.reference_images = images;
            segment.reference_videos = videos;
        }
        H3CommitGenerationMode::LegacyReferenceImage => {
            segment.reference_images = images.into_iter().take(1).collect();
        }
    }
}

fn project_resolution_for_segment(
    segment: &H3ProjectSegmentInspection,
    mode: H3CommitGenerationMode,
) -> (i64, i64, String) {
    if let Some((width, height)) = segment
        .front_matter
        .as_ref()
        .and_then(|matter| matter.resolution)
    {
        return (width, height, "FRONT_MATTER".to_owned());
    }
    if let Some((width, height)) = segment
        .prompt_spec
        .as_ref()
        .and_then(|spec| spec.resolution)
    {
        return (width, height, "PROMPT_SPEC".to_owned());
    }
    let source = match mode {
        H3CommitGenerationMode::Fl2vaFirstLast => segment.first_frame.as_ref(),
        H3CommitGenerationMode::Ref2vaVideoImage => segment.reference_videos.first(),
        H3CommitGenerationMode::Ref2vaImage
        | H3CommitGenerationMode::Ref2vaImageAudio
        | H3CommitGenerationMode::Fl2vaImageToVideo => segment
            .reference_images
            .first()
            .or(segment.first_frame.as_ref()),
        _ => None,
    };
    source
        .and_then(|media| media.width.zip(media.height))
        .map(|(width, height)| {
            let (width, height) = nearest_project_resolution(width as f64 / height as f64);
            (width, height, "SOURCE_ASPECT".to_owned())
        })
        .unwrap_or((
            PROJECT_DEFAULT_WIDTH,
            PROJECT_DEFAULT_HEIGHT,
            "RECIPE_DEFAULT".to_owned(),
        ))
}

fn nearest_project_resolution(aspect: f64) -> (i64, i64) {
    H3_OUTPUT_RESOLUTIONS
        .iter()
        .copied()
        .min_by(|left, right| {
            let left_delta = ((left.0 as f64 / left.1 as f64) - aspect).abs();
            let right_delta = ((right.0 as f64 / right.1 as f64) - aspect).abs();
            left_delta
                .partial_cmp(&right_delta)
                .unwrap_or(Ordering::Equal)
                .then_with(|| {
                    let left_cost =
                        (left.0 * left.1 - PROJECT_DEFAULT_WIDTH * PROJECT_DEFAULT_HEIGHT).abs();
                    let right_cost =
                        (right.0 * right.1 - PROJECT_DEFAULT_WIDTH * PROJECT_DEFAULT_HEIGHT).abs();
                    left_cost.cmp(&right_cost)
                })
        })
        .unwrap_or((PROJECT_DEFAULT_WIDTH, PROJECT_DEFAULT_HEIGHT))
}

fn project_duration_for_segment(
    segment: &H3ProjectSegmentInspection,
    mode: H3CommitGenerationMode,
) -> (i64, String) {
    if let Some(duration) = segment
        .front_matter
        .as_ref()
        .and_then(|matter| matter.duration_seconds)
    {
        return (duration, "FRONT_MATTER".to_owned());
    }
    if let Some(prompt_spec) = segment.prompt_spec.as_ref() {
        if let Some(duration) = prompt_spec.duration_seconds {
            return (
                duration,
                if prompt_spec.duration_rounded {
                    "PROMPT_SPEC_ROUNDED".to_owned()
                } else {
                    "PROMPT_SPEC".to_owned()
                },
            );
        }
    }
    if mode == H3CommitGenerationMode::Ref2vaVideoImage {
        if let Some(duration_ms) = segment
            .reference_videos
            .first()
            .and_then(|media| media.duration_ms)
        {
            let rounded = ((duration_ms as f64 / 1000.0).round() as i64).clamp(1, 15);
            return (rounded, "REFERENCE_VIDEO".to_owned());
        }
    }
    (
        PROJECT_DEFAULT_DURATION_SECONDS,
        "RECIPE_DEFAULT".to_owned(),
    )
}

fn recompute_project_segment_status(segment: &mut H3ProjectSegmentInspection) {
    let (inferred, inference_errors) = infer_project_mode(&segment.all_media);
    segment.inferred_mode = inferred.as_str().to_owned();
    let mode = match H3CommitGenerationMode::parse(Some(&segment.generation_mode)) {
        Ok(mode) => mode,
        Err(_) => {
            segment.errors = segment.base_errors.clone();
            segment.errors.push("生成模式无效".to_owned());
            segment.status = "BLOCKED".to_owned();
            return;
        }
    };
    let mut errors = segment.base_errors.clone();
    if segment.mode_source == "AUTO_INFERENCE" {
        errors.extend(inference_errors);
    }
    if segment
        .prompt
        .as_deref()
        .is_none_or(|prompt| prompt.trim().is_empty())
    {
        errors.push("MISSING_PROMPT：Segment 缺少有效 Prompt。".to_owned());
    }
    if !(1..=15).contains(&segment.duration_seconds) {
        errors.push("duration 必须在 1–15 秒范围内。".to_owned());
    }
    if !valid_project_resolution(segment.width, segment.height) {
        errors.push("resolution 不符合当前 H3 Recipe 的合法范围。".to_owned());
    }
    if matches!(mode, H3CommitGenerationMode::Fl2vaImageToVideo) && segment.first_frame.is_none() {
        errors.push("当前模式需要一张图片。".to_owned());
    }
    if matches!(mode, H3CommitGenerationMode::Fl2vaFirstLast)
        && (segment.first_frame.is_none()
            || segment.last_frame.is_none()
            || segment.first_frame.as_ref().map(|item| &item.id)
                == segment.last_frame.as_ref().map(|item| &item.id))
    {
        errors.push("当前模式需要唯一的首帧和末帧图片。".to_owned());
    }
    if matches!(
        mode,
        H3CommitGenerationMode::Ref2vaImage
            | H3CommitGenerationMode::Ref2vaImageAudio
            | H3CommitGenerationMode::Ref2vaVideoImage
    ) && segment.reference_images.is_empty()
    {
        errors.push("当前模式至少需要一张参考图片。".to_owned());
    }
    if matches!(
        mode,
        H3CommitGenerationMode::Ref2vaAudio | H3CommitGenerationMode::Ref2vaImageAudio
    ) && segment.reference_audios.is_empty()
    {
        errors.push("当前模式至少需要一个参考音频。".to_owned());
    }
    if mode == H3CommitGenerationMode::Ref2vaVideoImage && segment.reference_videos.is_empty() {
        errors.push("当前模式至少需要一个参考视频。".to_owned());
    }
    if segment.reference_images.len() > PROJECT_IMAGE_MAX_ITEMS {
        errors.push("参考图片超过当前 Recipe 上限 9 个。".to_owned());
    }
    if segment.reference_videos.len() > PROJECT_VIDEO_MAX_ITEMS {
        errors.push("参考视频超过当前 Recipe 上限 3 个。".to_owned());
    }
    if segment.reference_audios.len() > PROJECT_AUDIO_MAX_ITEMS {
        errors.push("参考音频超过当前 Recipe 上限 3 个。".to_owned());
    }
    segment.errors = errors;
    segment.warnings = segment.base_warnings.clone();
    let selected = selected_project_media_ids(segment);
    for media in &segment.all_media {
        if !selected.contains(&media.id) {
            segment
                .warnings
                .push(format!("该模式不会使用 {} 文件。", media.display_name));
        }
    }
    segment.status = if segment.errors.is_empty() {
        "READY".to_owned()
    } else {
        "BLOCKED".to_owned()
    };
}

fn selected_project_media_ids(segment: &H3ProjectSegmentInspection) -> HashSet<String> {
    segment
        .first_frame
        .iter()
        .chain(segment.last_frame.iter())
        .chain(segment.reference_images.iter())
        .chain(segment.reference_audios.iter())
        .chain(segment.reference_videos.iter())
        .map(|media| media.id.clone())
        .collect()
}

fn is_h3_output_resolution(width: i64, height: i64) -> bool {
    H3_OUTPUT_RESOLUTIONS.contains(&(width, height))
}

pub fn is_supported_h3_output_resolution(width: i64, height: i64) -> bool {
    is_h3_output_resolution(width, height)
}

fn valid_project_resolution(width: i64, height: i64) -> bool {
    is_h3_output_resolution(width, height)
}

fn project_mode_from_id(value: &str) -> Result<H3CommitGenerationMode, H3LocalImportError> {
    H3CommitGenerationMode::parse(Some(value))
}

fn project_recipe_ids(
    request: &H3LocalImportCommitRequest,
    mode: H3CommitGenerationMode,
) -> Result<(String, String), H3LocalImportError> {
    if request.quality_profile.as_deref() == Some("QUALITY") {
        let selection = request
            .quality_recipes
            .iter()
            .find(|selection| selection.mode == mode.as_str())
            .ok_or_else(|| {
                H3LocalImportError::Inspection(format!("QUALITY Recipe 缺失：{}", mode.as_str()))
            })?;
        if selection.workflow_version_id.trim().is_empty() || selection.recipe_id.trim().is_empty()
        {
            return Err(H3LocalImportError::Inspection(format!(
                "QUALITY Recipe 标识无效：{}",
                mode.as_str()
            )));
        }
        return Ok((
            selection.workflow_version_id.clone(),
            selection.recipe_id.clone(),
        ));
    }
    if mode.is_fl2va() {
        Ok((
            request
                .fl2va_workflow_version_id
                .clone()
                .unwrap_or_else(|| request.workflow_version_id.clone()),
            request
                .fl2va_recipe_id
                .clone()
                .unwrap_or_else(|| request.recipe_id.clone()),
        ))
    } else {
        Ok((
            request
                .ref2va_workflow_version_id
                .clone()
                .unwrap_or_else(|| request.workflow_version_id.clone()),
            request
                .ref2va_recipe_id
                .clone()
                .unwrap_or_else(|| request.recipe_id.clone()),
        ))
    }
}

fn validate_project_segment_draft(draft: &H3ProjectSegmentDraft) -> Result<(), H3LocalImportError> {
    if draft.segment_id.trim().is_empty() {
        return Err(H3LocalImportError::InvalidInput(
            "Segment 标识不能为空".to_owned(),
        ));
    }
    if let Some(mode) = draft.mode.as_deref() {
        project_mode_from_id(mode)?;
    }
    if let Some(prompt) = draft.prompt.as_deref() {
        let bytes = prompt.as_bytes();
        parse_prompt_text(prompt).map_err(|issue| {
            H3LocalImportError::InvalidInput(format_project_prompt_issue(issue))
        })?;
        if bytes.len()
            > crate::application::asset_video_prompt_service::MAX_ASSET_VIDEO_PROMPT_BYTES
        {
            return Err(H3LocalImportError::InvalidInput(
                "Prompt 不能超过 64 KiB".to_owned(),
            ));
        }
    }
    if let Some(duration) = draft.duration_seconds {
        if !(1..=15).contains(&duration) {
            return Err(H3LocalImportError::InvalidInput(
                "duration 必须在 1–15 秒范围内".to_owned(),
            ));
        }
    }
    if let (Some(width), Some(height)) = (draft.width, draft.height) {
        if !valid_project_resolution(width, height) {
            return Err(H3LocalImportError::InvalidInput(
                "resolution 不符合当前 H3 Recipe 约束".to_owned(),
            ));
        }
    }
    Ok(())
}

fn apply_project_drafts(
    inspection: &mut H3LocalImportInspection,
    drafts: &HashMap<String, H3ProjectSegmentDraft>,
) -> Result<(), H3LocalImportError> {
    let Some(project) = inspection.project_folder.as_mut() else {
        return Ok(());
    };
    for segment in &mut project.segments {
        if let Some(draft) = drafts.get(&segment.segment_id) {
            apply_project_segment_draft(segment, draft)?;
        }
    }
    rebuild_project_folder_counts(inspection);
    Ok(())
}

fn apply_project_segment_draft(
    segment: &mut H3ProjectSegmentInspection,
    draft: &H3ProjectSegmentDraft,
) -> Result<(), H3LocalImportError> {
    if let Some(mode) = draft.mode.as_deref() {
        let mode = project_mode_from_id(mode)?;
        segment.generation_mode = mode.as_str().to_owned();
        segment.mode_source = "USER_OVERRIDE".to_owned();
    }
    if let Some(prompt) = draft.prompt.as_deref() {
        let (prompt, _) = parse_prompt_text(prompt).map_err(|issue| {
            H3LocalImportError::InvalidInput(format_project_prompt_issue(issue))
        })?;
        segment.prompt = Some(prompt);
    }
    if let Some(duration) = draft.duration_seconds {
        segment.duration_seconds = duration;
        segment.duration_source = "USER_OVERRIDE".to_owned();
    }
    if let (Some(width), Some(height)) = (draft.width, draft.height) {
        segment.width = width;
        segment.height = height;
        segment.resolution_source = "USER_OVERRIDE".to_owned();
    }
    let find_media = |id: &str| {
        segment
            .all_media
            .iter()
            .find(|media| media.id == id)
            .cloned()
    };
    if let Some(ids) = draft.reference_image_ids.as_ref() {
        segment.reference_images = ids
            .iter()
            .filter_map(|id| find_media(id))
            .filter(|media| media.kind == H3ProjectMediaKind::Image)
            .collect();
    }
    if let Some(ids) = draft.reference_audio_ids.as_ref() {
        segment.reference_audios = ids
            .iter()
            .filter_map(|id| find_media(id))
            .filter(|media| media.kind == H3ProjectMediaKind::Audio)
            .collect();
    }
    if let Some(ids) = draft.reference_video_ids.as_ref() {
        segment.reference_videos = ids
            .iter()
            .filter_map(|id| find_media(id))
            .filter(|media| media.kind == H3ProjectMediaKind::Video)
            .collect();
    }
    if let Some(id) = draft.first_frame_id.as_deref() {
        segment.first_frame =
            find_media(id).filter(|media| media.kind == H3ProjectMediaKind::Image);
    }
    if let Some(id) = draft.last_frame_id.as_deref() {
        segment.last_frame = find_media(id).filter(|media| media.kind == H3ProjectMediaKind::Image);
    }
    recompute_project_segment_status(segment);
    Ok(())
}

fn rebuild_project_folder_counts(inspection: &mut H3LocalImportInspection) {
    let Some(project) = inspection.project_folder.as_mut() else {
        return;
    };
    project.ready_count = project
        .segments
        .iter()
        .filter(|segment| segment.status == "READY")
        .count();
    project.errors = project
        .segments
        .iter()
        .filter(|segment| segment.status != "READY")
        .map(|segment| {
            format!(
                "第 {} 段 {}：{}",
                segment.ordinal,
                segment.folder_name,
                segment.errors.join("；")
            )
        })
        .collect();
    if project.segment_count > MAX_PROJECT_SEGMENTS {
        project
            .errors
            .push("单次最多生成100段，请拆分项目文件夹或分批导入。".to_owned());
    }
    project.warnings = project
        .segments
        .iter()
        .flat_map(|segment| segment.warnings.iter().cloned())
        .collect();
    project.error_count = project.errors.len();
    inspection.ready_count = project.ready_count;
    inspection.error_count = project.error_count;
    inspection.errors = project.errors.clone();
    inspection.warnings = project.warnings.clone();
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
        project_folder: None,
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
    if !is_h3_output_resolution(request.width, request.height) {
        return Err(H3LocalImportError::InvalidInput(
            "输出视频分辨率必须选择图片规格中的 14 档 16:9 分辨率".to_owned(),
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
    if let Some(profile) = request.quality_profile.as_deref() {
        if !matches!(profile, "QUALITY" | "FAST") {
            return Err(H3LocalImportError::InvalidInput(
                "H3 生成质量必须是 QUALITY 或 FAST".to_owned(),
            ));
        }
    }
    for selection in &request.quality_recipes {
        if selection.mode.trim().is_empty()
            || selection.workflow_version_id.trim().is_empty()
            || selection.recipe_id.trim().is_empty()
        {
            return Err(H3LocalImportError::InvalidInput(
                "QUALITY Recipe 选择不能包含空标识".to_owned(),
            ));
        }
        H3CommitGenerationMode::parse(Some(&selection.mode))?;
    }
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

    #[test]
    fn quality_profile_freezes_a_recipe_per_project_segment_mode() {
        let request = H3LocalImportCommitRequest {
            batch_name: None,
            workflow_version_id: "fast-workflow".to_owned(),
            recipe_id: "fast-recipe".to_owned(),
            width: 960,
            height: 544,
            duration_seconds: 5,
            seed: None,
            auto_start: false,
            generation_mode: Some("FL2VA_TEXT_TO_VIDEO".to_owned()),
            fl2va_workflow_version_id: Some("fast-fl2va-workflow".to_owned()),
            fl2va_recipe_id: Some("fast-fl2va-recipe".to_owned()),
            ref2va_workflow_version_id: Some("fast-ref-workflow".to_owned()),
            ref2va_recipe_id: Some("fast-ref-recipe".to_owned()),
            quality_profile: Some("QUALITY".to_owned()),
            quality_recipes: vec![
                H3QualityRecipeSelection {
                    mode: "FL2VA_TEXT_TO_VIDEO".to_owned(),
                    workflow_version_id: "quality-t2v-workflow".to_owned(),
                    recipe_id: "quality-t2v-recipe".to_owned(),
                },
                H3QualityRecipeSelection {
                    mode: "REF2VA_IMAGE".to_owned(),
                    workflow_version_id: "quality-ref-workflow".to_owned(),
                    recipe_id: "quality-ref-recipe".to_owned(),
                },
            ],
        };

        assert_eq!(
            project_recipe_ids(&request, H3CommitGenerationMode::Fl2vaTextToVideo).unwrap(),
            (
                "quality-t2v-workflow".to_owned(),
                "quality-t2v-recipe".to_owned()
            )
        );
        assert_eq!(
            project_recipe_ids(&request, H3CommitGenerationMode::Ref2vaImage).unwrap(),
            (
                "quality-ref-workflow".to_owned(),
                "quality-ref-recipe".to_owned()
            )
        );
    }

    #[test]
    fn project_folder_auto_resolution_uses_the_closest_h3_ladder_entry() {
        assert_eq!(nearest_project_resolution(16.0 / 9.0), (1824, 1024));
        assert_eq!(h3_resolution_alias("2K"), Some((1920, 1088)));
        assert_eq!(h3_resolution_alias("1080p"), Some((1920, 1088)));
        assert_eq!(h3_resolution_alias("1K"), Some((960, 544)));
    }

    #[test]
    fn project_prompt_spec_extracts_duration_resolution_without_touching_prompt_body() {
        let chinese = parse_project_prompt_bytes("视频规格：10秒，分辨率：1344×768".as_bytes())
            .expect("Chinese prompt spec should parse");
        assert_eq!(chinese.prompt_spec.duration_seconds, Some(10));
        assert_eq!(chinese.prompt_spec.resolution, Some((1344, 768)));
        assert_eq!(chinese.prompt_spec.duration_rounded, false);
        assert_eq!(chinese.text, "视频规格：10秒，分辨率：1344×768");

        let english = parse_project_prompt_bytes(b"Duration: 8s\nResolution: 960x544")
            .expect("English prompt spec should parse");
        assert_eq!(english.prompt_spec.duration_seconds, Some(8));
        assert_eq!(english.prompt_spec.resolution, Some((960, 544)));

        let compact =
            parse_project_prompt_bytes("规格：15秒｜1920×1088｜16:9｜原生立体声".as_bytes())
                .expect("compact prompt spec should parse");
        assert_eq!(compact.prompt_spec.duration_seconds, Some(15));
        assert_eq!(compact.prompt_spec.resolution, Some((1920, 1088)));

        let slash = parse_project_prompt_bytes(b"10s / 1344x768")
            .expect("compact slash prompt spec should parse");
        assert_eq!(slash.prompt_spec.duration_seconds, Some(10));
        assert_eq!(slash.prompt_spec.resolution, Some((1344, 768)));
    }

    #[test]
    fn project_prompt_spec_ignores_timeline_and_rounds_fractional_duration() {
        let data = parse_project_prompt_bytes(
            "视频规格：10秒，1344×768\n\n0–2秒：人物转身\n2–5秒：镜头推进\n5–10秒：人物停下"
                .as_bytes(),
        )
        .expect("timeline prompt should parse");
        assert_eq!(data.prompt_spec.duration_seconds, Some(10));
        assert_eq!(data.prompt_spec.resolution, Some((1344, 768)));

        let rounded = parse_project_prompt_bytes("时长：7.5秒".as_bytes())
            .expect("fractional duration should parse");
        assert_eq!(rounded.prompt_spec.duration_seconds, Some(8));
        assert!(rounded.prompt_spec.duration_rounded);
        assert!(rounded
            .prompt_spec
            .warnings
            .iter()
            .any(|warning| warning.contains("7.5秒") && warning.contains("8秒")));
    }

    #[test]
    fn project_prompt_spec_prefers_exact_resolution_over_alias_and_blocks_unsupported_size() {
        let exact = parse_project_prompt_bytes("规格：2K，1344×768".as_bytes())
            .expect("exact resolution should win over alias");
        assert_eq!(exact.prompt_spec.resolution, Some((1344, 768)));

        let unsupported = parse_project_prompt_bytes("分辨率：1280×720".as_bytes())
            .expect("unsupported resolution should remain inspectable");
        assert_eq!(unsupported.prompt_spec.resolution, Some((1280, 720)));
        assert_eq!(
            unsupported.prompt_spec.unsupported_resolution,
            Some((1280, 720))
        );
    }

    #[test]
    fn project_prompt_spec_uses_no_default_when_prompt_has_no_explicit_spec() {
        let data = parse_project_prompt_bytes("人物缓慢向前行走。".as_bytes())
            .expect("ordinary prompt should parse");
        assert_eq!(data.prompt_spec.duration_seconds, None);
        assert_eq!(data.prompt_spec.resolution, None);
    }

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

    const H3_PROJECT_TEST_RECIPE: &str = r#"
schema_version: 1
id: h3_project_test_recipe
name: H3 Project Folder Test
workflow:
  file: workflow_api.json
inputs:
  prompt:
    type: textarea
    label: Prompt
    required: true
    default: ""
  first_frame:
    type: image
    label: First frame
    required: false
  last_frame:
    type: image
    label: Last frame
    required: false
  reference_images:
    type: images
    label: Reference images
    required: false
    min_items: 0
    max_items: 9
  reference_videos:
    type: videos
    label: Reference videos
    required: false
    min_items: 0
    max_items: 3
  reference_audios:
    type: audios
    label: Reference audios
    required: false
    min_items: 0
    max_items: 3
  width:
    type: integer
    label: Width
    required: true
    default: 1344
    min: 32
    max: 2048
    step: 32
  height:
    type: integer
    label: Height
    required: true
    default: 768
    min: 32
    max: 2048
    step: 32
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

    fn write_project_fixture(root: &Path) {
        let segments = [
            ("001_text", &["prompt.txt"] as &[&str]),
            ("002_i2v", &["prompt.txt", "P01.png"]),
            ("003_first_last", &["prompt.txt", "first.png", "last.png"]),
            (
                "004_ref_image",
                &["prompt.txt", "P10.png", "P02.png", "P01.png"],
            ),
            ("005_ref_audio", &["prompt.txt", "A01.wav"]),
            ("006_ref_image_audio", &["prompt.txt", "P01.png", "A01.wav"]),
            ("007_ref_video_image", &["prompt.txt", "P01.png", "V01.mp4"]),
        ];
        for (folder, files) in segments {
            let directory = root.join(folder);
            fs::create_dir_all(&directory).expect("project segment should exist");
            fs::write(
                directory.join("prompt.txt"),
                format!("{folder} prompt\nsecond line"),
            )
            .expect("project prompt should write");
            for file in files.iter().copied().filter(|file| *file != "prompt.txt") {
                let bytes = if file.ends_with(".png") {
                    png_bytes()
                } else if file.ends_with(".wav") {
                    b"RIFF audio placeholder".to_vec()
                } else {
                    b"video placeholder".to_vec()
                };
                fs::write(directory.join(file), bytes).expect("project media should write");
            }
        }
        fs::write(root.join("README.txt"), "ignored root file").expect("root file should write");
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
                    width: 960,
                    height: 544,
                    duration_seconds: 5,
                    seed: Some(SeedValue::Fixed(123)),
                    auto_start: false,
                    generation_mode: None,
                    fl2va_workflow_version_id: None,
                    fl2va_recipe_id: None,
                    ref2va_workflow_version_id: None,
                    ref2va_recipe_id: None,
                    quality_profile: None,
                    quality_recipes: Vec::new(),
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
            assert_eq!(item.values_json["width"]["value"], 960);
            assert_eq!(item.values_json["height"]["value"], 544);
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
                    width: 960,
                    height: 544,
                    duration_seconds: 5,
                    seed: Some(SeedValue::Fixed(123)),
                    auto_start: false,
                    generation_mode: None,
                    fl2va_workflow_version_id: None,
                    fl2va_recipe_id: None,
                    ref2va_workflow_version_id: None,
                    ref2va_recipe_id: None,
                    quality_profile: None,
                    quality_recipes: Vec::new(),
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

    #[tokio::test]
    async fn project_folder_inspection_infers_all_h3_modes_without_db_writes() {
        let directory = tempdir().expect("project inspection directory should exist");
        let project_root = directory.path().join("project");
        let fixture_root = directory.path().join("ProjectRoot");
        fs::create_dir_all(&project_root).expect("project root should exist");
        write_project_fixture(&fixture_root);
        let harness = build_harness(&directory.path().join("app.db"), &project_root, true).await;
        let before = (
            count_rows(&harness.pool, "assets").await,
            count_rows(&harness.pool, "asset_video_prompts").await,
            count_rows(&harness.pool, "production_batches").await,
        );
        let (_, inspection) = harness
            .local
            .pick(PROJECT_ID, fixture_root, H3LocalImportMode::ProjectFolder)
            .await
            .expect("project folder inspection should succeed");
        let project = inspection
            .project_folder
            .expect("project folder data should be returned");
        assert_eq!(project.segment_count, 7);
        assert_eq!(project.segments[0].folder_name, "001_text");
        assert_eq!(project.segments[0].generation_mode, "FL2VA_TEXT_TO_VIDEO");
        assert_eq!(project.segments[1].generation_mode, "FL2VA_IMAGE_TO_VIDEO");
        assert_eq!(project.segments[2].generation_mode, "FL2VA_FIRST_LAST");
        assert_eq!(project.segments[3].generation_mode, "REF2VA_IMAGE");
        assert_eq!(project.segments[4].generation_mode, "REF2VA_AUDIO");
        assert_eq!(project.segments[5].generation_mode, "REF2VA_IMAGE_AUDIO");
        assert_eq!(project.segments[6].generation_mode, "REF2VA_VIDEO_IMAGE");
        assert_eq!(
            project.segments[3]
                .reference_images
                .iter()
                .map(|media| media.display_name.as_str())
                .collect::<Vec<_>>(),
            vec!["P01.png", "P02.png", "P10.png"]
        );
        assert_eq!(
            project.segments[2]
                .first_frame
                .as_ref()
                .map(|media| media.display_name.as_str()),
            Some("first.png")
        );
        assert_eq!(
            project.segments[2]
                .last_frame
                .as_ref()
                .map(|media| media.display_name.as_str()),
            Some("last.png")
        );
        assert!(
            project.errors.is_empty(),
            "unexpected project errors: {:?}",
            project.errors
        );
        assert_eq!(
            before,
            (
                count_rows(&harness.pool, "assets").await,
                count_rows(&harness.pool, "asset_video_prompts").await,
                count_rows(&harness.pool, "production_batches").await,
            )
        );
    }

    #[tokio::test]
    async fn project_folder_auto_detection_supports_arbitrary_i2v_and_explicit_frame_aliases() {
        let directory = tempdir().expect("project auto detection directory should exist");
        let project_root = directory.path().join("project");
        let fixture_root = directory.path().join("ProjectRoot");
        fs::create_dir_all(&project_root).expect("project root should exist");

        let i2v_segment = fixture_root.join("001_arbitrary_i2v");
        fs::create_dir_all(&i2v_segment).expect("i2v segment should exist");
        fs::write(i2v_segment.join("prompt.md"), "single image prompt")
            .expect("i2v prompt should write");
        fs::write(i2v_segment.join("character.png"), png_bytes())
            .expect("arbitrary image should write");

        let first_last_segment = fixture_root.join("002_chinese_frames");
        fs::create_dir_all(&first_last_segment).expect("first/last segment should exist");
        fs::write(first_last_segment.join("prompt.txt"), "frame prompt")
            .expect("frame prompt should write");
        fs::write(first_last_segment.join("001_首帧.png"), png_bytes())
            .expect("first frame should write");
        fs::write(first_last_segment.join("001_尾帧.png"), png_bytes())
            .expect("last frame should write");

        let reference_segment = fixture_root.join("003_plain_references");
        fs::create_dir_all(&reference_segment).expect("reference segment should exist");
        fs::write(reference_segment.join("prompt.txt"), "reference prompt")
            .expect("reference prompt should write");
        fs::write(reference_segment.join("person.png"), png_bytes())
            .expect("first reference image should write");
        fs::write(reference_segment.join("scene.png"), png_bytes())
            .expect("second reference image should write");

        let override_segment = fixture_root.join("004_explicit_image");
        fs::create_dir_all(&override_segment).expect("override segment should exist");
        fs::write(
            override_segment.join("prompt.txt"),
            "---\nmode: image\n---\nexplicit image mode",
        )
        .expect("override prompt should write");
        fs::write(override_segment.join("character.png"), png_bytes())
            .expect("override image should write");
        fs::write(override_segment.join("unused.wav"), b"audio")
            .expect("unused audio should write");

        let harness = build_harness(&directory.path().join("app.db"), &project_root, true).await;
        let (_, inspection) = harness
            .local
            .pick(PROJECT_ID, fixture_root, H3LocalImportMode::ProjectFolder)
            .await
            .expect("project folder inspection should succeed");
        let project = inspection
            .project_folder
            .expect("project folder data should be returned");

        assert_eq!(project.segment_count, 4);
        assert_eq!(project.segments[0].generation_mode, "FL2VA_IMAGE_TO_VIDEO");
        assert_eq!(
            project.segments[0]
                .first_frame
                .as_ref()
                .map(|media| media.display_name.as_str()),
            Some("character.png")
        );
        assert_eq!(project.segments[1].generation_mode, "FL2VA_FIRST_LAST");
        assert_eq!(
            project.segments[1]
                .first_frame
                .as_ref()
                .map(|media| media.display_name.as_str()),
            Some("001_首帧.png")
        );
        assert_eq!(
            project.segments[1]
                .last_frame
                .as_ref()
                .map(|media| media.display_name.as_str()),
            Some("001_尾帧.png")
        );
        assert_eq!(project.segments[2].generation_mode, "REF2VA_IMAGE");
        assert_eq!(
            project.segments[2]
                .reference_images
                .iter()
                .map(|media| media.display_name.as_str())
                .collect::<Vec<_>>(),
            vec!["person.png", "scene.png"]
        );
        assert_eq!(project.segments[3].generation_mode, "FL2VA_IMAGE_TO_VIDEO");
        assert!(project.segments[3]
            .warnings
            .iter()
            .any(|warning| warning.contains("unused.wav")));
        assert!(
            project.errors.is_empty(),
            "unexpected project errors: {:?}",
            project.errors
        );
    }

    #[tokio::test]
    async fn project_folder_front_matter_and_ambiguous_segments_are_explicitly_blocked() {
        let directory = tempdir().expect("front matter directory should exist");
        let project_root = directory.path().join("project");
        let fixture_root = directory.path().join("ProjectRoot");
        fs::create_dir_all(&project_root).expect("project root should exist");
        let front_matter_segment = fixture_root.join("001_front_matter");
        fs::create_dir_all(&front_matter_segment).expect("front matter segment should exist");
        fs::write(
            front_matter_segment.join("prompt.txt"),
            "---\nmode: text\nduration: 8\nresolution: 1376x768\n---\n视频规格：10秒，1344×768\nline one\nline two",
        )
        .expect("front matter prompt should write");
        let invalid_segment = fixture_root.join("002_invalid_resolution");
        fs::create_dir_all(&invalid_segment).expect("invalid segment should exist");
        fs::write(
            invalid_segment.join("prompt.txt"),
            "---\nresolution: 777x999\n---\ninvalid resolution",
        )
        .expect("invalid prompt should write");
        let ambiguous_prompt_segment = fixture_root.join("003_ambiguous_prompt");
        fs::create_dir_all(&ambiguous_prompt_segment)
            .expect("ambiguous prompt segment should exist");
        fs::write(ambiguous_prompt_segment.join("a.txt"), "a").expect("prompt should write");
        fs::write(ambiguous_prompt_segment.join("b.md"), "b").expect("prompt should write");
        let ambiguous_media_segment = fixture_root.join("004_ambiguous_media");
        fs::create_dir_all(&ambiguous_media_segment).expect("ambiguous media segment should exist");
        fs::write(ambiguous_media_segment.join("prompt.txt"), "ambiguous")
            .expect("prompt should write");
        fs::write(ambiguous_media_segment.join("P01.png"), png_bytes())
            .expect("image should write");
        fs::write(ambiguous_media_segment.join("A01.wav"), b"audio").expect("audio should write");
        fs::write(ambiguous_media_segment.join("V01.mp4"), b"video").expect("video should write");
        let harness = build_harness(&directory.path().join("app.db"), &project_root, true).await;
        let (_, inspection) = harness
            .local
            .pick(PROJECT_ID, fixture_root, H3LocalImportMode::ProjectFolder)
            .await
            .expect("front matter inspection should succeed");
        let project = inspection.project_folder.unwrap();
        let front = &project.segments[0];
        assert_eq!(front.generation_mode, "FL2VA_TEXT_TO_VIDEO");
        assert_eq!(front.duration_seconds, 8);
        assert_eq!((front.width, front.height), (1376, 768));
        assert_eq!(
            front.prompt.as_deref(),
            Some("视频规格：10秒，1344×768\nline one\nline two")
        );
        assert_eq!(front.mode_source, "FRONT_MATTER");
        assert_eq!(front.duration_source, "FRONT_MATTER");
        assert_eq!(front.resolution_source, "FRONT_MATTER");
        assert!(project.segments[1]
            .errors
            .iter()
            .any(|error| error.contains("resolution")));
        assert!(project.segments[2]
            .errors
            .iter()
            .any(|error| error.contains("AMBIGUOUS_PROMPT")));
        assert!(project.segments[3]
            .errors
            .iter()
            .any(|error| error.contains("AMBIGUOUS_MEDIA_COMBINATION")));
        assert!(project.error_count >= 3);
    }

    #[tokio::test]
    async fn project_folder_prompt_specs_are_independent_per_segment() {
        let directory = tempdir().expect("prompt spec directory should exist");
        let project_root = directory.path().join("project");
        let fixture_root = directory.path().join("ProjectRoot");
        fs::create_dir_all(&project_root).expect("project root should exist");
        for (folder, prompt) in [
            ("001", "视频规格：5秒，960×544"),
            ("002", "Duration: 10s\nResolution: 1344x768"),
            ("003", "规格：15秒｜1920×1088｜16:9｜原生立体声"),
            ("004", "分辨率：1280×720"),
        ] {
            let segment = fixture_root.join(folder);
            fs::create_dir_all(&segment).expect("segment should exist");
            fs::write(segment.join("prompt.txt"), prompt).expect("prompt should write");
        }
        let harness = build_harness(&directory.path().join("app.db"), &project_root, true).await;
        let (_, inspection) = harness
            .local
            .pick(PROJECT_ID, fixture_root, H3LocalImportMode::ProjectFolder)
            .await
            .expect("project folder inspection should succeed");
        let project = inspection
            .project_folder
            .expect("project folder data should be returned");
        assert!(project
            .segments
            .iter()
            .find(|segment| segment.folder_name == "004")
            .expect("unsupported segment should exist")
            .errors
            .iter()
            .any(|error| error.contains("PROMPT_RESOLUTION_UNSUPPORTED")));
        assert_eq!(project.ready_count, 3);
        assert_eq!(
            project
                .segments
                .iter()
                .take(3)
                .map(|segment| (segment.duration_seconds, segment.width, segment.height))
                .collect::<Vec<_>>(),
            vec![(5, 960, 544), (10, 1344, 768), (15, 1920, 1088)]
        );
        assert!(project
            .segments
            .iter()
            .take(3)
            .all(|segment| segment.duration_source == "PROMPT_SPEC"));
        assert!(project
            .segments
            .iter()
            .take(3)
            .all(|segment| segment.resolution_source == "PROMPT_SPEC"));
    }

    #[tokio::test]
    async fn project_folder_commit_freezes_independent_segment_values_and_survives_source_delete() {
        let directory = tempdir().expect("project commit directory should exist");
        let project_root = directory.path().join("project");
        let fixture_root = directory.path().join("ProjectRoot");
        fs::create_dir_all(&project_root).expect("project root should exist");
        for (folder, image, prompt) in [
            ("001_text", None, "Prompt A\nline two"),
            ("002_i2v", Some("P01.png"), "Prompt B\nline two"),
        ] {
            let segment = fixture_root.join(folder);
            fs::create_dir_all(&segment).expect("segment should exist");
            fs::write(segment.join("prompt.txt"), prompt).expect("prompt should write");
            if let Some(image) = image {
                fs::write(segment.join(image), png_bytes()).expect("image should write");
            }
        }
        let first_last_segment = fixture_root.join("003_first_last");
        fs::create_dir_all(&first_last_segment).expect("first/last segment should exist");
        fs::write(first_last_segment.join("prompt.txt"), "Prompt C\nline two")
            .expect("first/last prompt should write");
        fs::write(first_last_segment.join("first.png"), png_bytes())
            .expect("first frame should write");
        fs::write(first_last_segment.join("last.png"), png_bytes())
            .expect("last frame should write");
        let db_path = directory.path().join("app.db");
        let harness = build_harness(&db_path, &project_root, true).await;
        sqlx::query("UPDATE recipes SET recipe_yaml = ? WHERE id = 'recipe-1'")
            .bind(H3_PROJECT_TEST_RECIPE)
            .execute(&harness.pool)
            .await
            .expect("project Recipe should persist");
        let before = (
            count_rows(&harness.pool, "assets").await,
            count_rows(&harness.pool, "asset_video_prompts").await,
            count_rows(&harness.pool, "production_batches").await,
        );
        let (session_id, inspection) = harness
            .local
            .pick(
                PROJECT_ID,
                fixture_root.clone(),
                H3LocalImportMode::ProjectFolder,
            )
            .await
            .expect("project inspection should succeed");
        assert_eq!(
            before,
            (
                count_rows(&harness.pool, "assets").await,
                count_rows(&harness.pool, "asset_video_prompts").await,
                count_rows(&harness.pool, "production_batches").await,
            )
        );
        let segments = inspection
            .project_folder
            .as_ref()
            .expect("project inspection should exist")
            .segments
            .clone();
        let first = harness
            .local
            .update_h3_project_segment_draft(
                &session_id,
                H3ProjectSegmentDraft {
                    segment_id: segments[0].segment_id.clone(),
                    mode: Some("FL2VA_TEXT_TO_VIDEO".to_owned()),
                    prompt: Some("Prompt A\nline two".to_owned()),
                    duration_seconds: Some(5),
                    width: Some(1344),
                    height: Some(768),
                    reference_image_ids: Some(Vec::new()),
                    reference_audio_ids: Some(Vec::new()),
                    reference_video_ids: Some(Vec::new()),
                    first_frame_id: None,
                    last_frame_id: None,
                    reset_auto_detection: false,
                },
            )
            .await
            .expect("first Segment draft should save");
        assert_eq!(first.ready_count, 3);
        let second_segment = first
            .project_folder
            .as_ref()
            .unwrap()
            .segments
            .iter()
            .find(|segment| segment.folder_name == "002_i2v")
            .unwrap()
            .clone();
        let second = harness
            .local
            .update_h3_project_segment_draft(
                &session_id,
                H3ProjectSegmentDraft {
                    segment_id: second_segment.segment_id.clone(),
                    mode: Some("FL2VA_IMAGE_TO_VIDEO".to_owned()),
                    prompt: Some("Prompt B\nline two".to_owned()),
                    duration_seconds: Some(8),
                    width: Some(1376),
                    height: Some(768),
                    reference_image_ids: Some(Vec::new()),
                    reference_audio_ids: Some(Vec::new()),
                    reference_video_ids: Some(Vec::new()),
                    first_frame_id: second_segment
                        .all_media
                        .iter()
                        .find(|media| media.kind == H3ProjectMediaKind::Image)
                        .map(|media| media.id.clone()),
                    last_frame_id: None,
                    reset_auto_detection: false,
                },
            )
            .await
            .expect("second Segment draft should save");
        assert_eq!(second.error_count, 0);
        let third_segment = second
            .project_folder
            .as_ref()
            .unwrap()
            .segments
            .iter()
            .find(|segment| segment.folder_name == "003_first_last")
            .unwrap()
            .clone();
        let third = harness
            .local
            .update_h3_project_segment_draft(
                &session_id,
                H3ProjectSegmentDraft {
                    segment_id: third_segment.segment_id.clone(),
                    mode: Some("FL2VA_FIRST_LAST".to_owned()),
                    prompt: Some("Prompt C\nline two".to_owned()),
                    duration_seconds: Some(6),
                    width: Some(960),
                    height: Some(544),
                    reference_image_ids: Some(Vec::new()),
                    reference_audio_ids: Some(Vec::new()),
                    reference_video_ids: Some(Vec::new()),
                    first_frame_id: third_segment
                        .all_media
                        .iter()
                        .find(|media| media.display_name == "first.png")
                        .map(|media| media.id.clone()),
                    last_frame_id: third_segment
                        .all_media
                        .iter()
                        .find(|media| media.display_name == "last.png")
                        .map(|media| media.id.clone()),
                    reset_auto_detection: false,
                },
            )
            .await
            .expect("third Segment draft should save");
        assert_eq!(third.error_count, 0);
        let result = harness
            .local
            .commit(
                &session_id,
                H3LocalImportCommitRequest {
                    batch_name: Some("H3 project folder integration".to_owned()),
                    workflow_version_id: "workflow-version-1".to_owned(),
                    recipe_id: "recipe-1".to_owned(),
                    width: 1344,
                    height: 768,
                    duration_seconds: 5,
                    seed: None,
                    auto_start: false,
                    generation_mode: Some("FL2VA_TEXT_TO_VIDEO".to_owned()),
                    fl2va_workflow_version_id: None,
                    fl2va_recipe_id: None,
                    ref2va_workflow_version_id: None,
                    ref2va_recipe_id: None,
                    quality_profile: None,
                    quality_recipes: Vec::new(),
                },
            )
            .await
            .expect("project commit should succeed");
        assert_eq!(result.imported_asset_count, 3);
        assert_eq!(result.item_count, 3);
        let detail = harness
            .queue
            .get(PROJECT_ID, &result.batch_id)
            .await
            .expect("project batch should be readable");
        assert_eq!(detail.batch.status, ProductionBatchStatus::Ready);
        assert!(detail.batch.continue_on_failure);
        assert_eq!(detail.items.len(), 3);
        assert_eq!(detail.items[0].values_json["duration_seconds"]["value"], 5);
        assert_eq!(detail.items[0].values_json["width"]["value"], 1344);
        assert_eq!(detail.items[1].values_json["duration_seconds"]["value"], 8);
        assert_eq!(detail.items[1].values_json["width"]["value"], 1376);
        assert_eq!(detail.items[1].values_json["height"]["value"], 768);
        assert_eq!(
            detail.items[1].values_json["first_frame"]["type"],
            "image_asset"
        );
        assert_eq!(detail.items[2].values_json["duration_seconds"]["value"], 6);
        assert_eq!(detail.items[2].values_json["width"]["value"], 960);
        assert_eq!(detail.items[2].values_json["height"]["value"], 544);
        assert_eq!(
            detail.items[2].values_json["first_frame"]["type"],
            "image_asset"
        );
        assert_eq!(
            detail.items[2].values_json["last_frame"]["type"],
            "image_asset"
        );
        let second_asset_id = detail.items[1].values_json["first_frame"]["assetId"]
            .as_str()
            .expect("I2V item should contain an asset id")
            .to_owned();
        fs::write(
            fixture_root.join("002_i2v").join("prompt.txt"),
            "Prompt D\nchanged after commit",
        )
        .expect("changed project prompt should write");
        harness
            .prompt_service
            .set(
                PROJECT_ID,
                &second_asset_id,
                "Prompt E\nasset prompt changed",
            )
            .await
            .expect("project asset prompt update should succeed");
        let frozen_detail = harness
            .queue
            .get(PROJECT_ID, &result.batch_id)
            .await
            .expect("frozen project batch should remain readable");
        assert_eq!(
            frozen_detail.items[1].values_json["prompt"]["value"],
            "Prompt B\nline two"
        );
        let values_text = detail
            .items
            .iter()
            .map(|item| {
                serde_json::to_string(&item.values_json).expect("queue values should serialize")
            })
            .collect::<String>();
        assert!(!values_text.contains(&fixture_root.to_string_lossy().to_string()));
        fs::remove_dir_all(&fixture_root).expect("source project should be removable");
        let deleted_folder_detail = harness
            .queue
            .get(PROJECT_ID, &result.batch_id)
            .await
            .expect("batch should survive source deletion");
        for item in &deleted_folder_detail.items {
            harness
                .queue
                .prepare_queue_values_for_test(
                    &item.workflow_version_id,
                    &item.recipe_id,
                    &item.values_json,
                )
                .await
                .expect("persisted project values should compile after folder removal");
        }
        harness.pool.close().await;
        drop(harness);
        let restarted = build_harness(&db_path, &project_root, false).await;
        let persisted_assets = restarted
            .assets
            .list_recent(PROJECT_ID, 10)
            .await
            .expect("restarted project asset repository should read assets");
        assert_eq!(persisted_assets.len(), 3);
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
            .expect("project batch should persist across repository restart");
        assert_eq!(restarted_detail.items.len(), 3);
        for item in restarted_detail.items {
            restarted
                .queue
                .prepare_queue_values_for_test(
                    &item.workflow_version_id,
                    &item.recipe_id,
                    &item.values_json,
                )
                .await
                .expect("restarted project values should compile");
        }
    }
}
