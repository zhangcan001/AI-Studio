//! DEV-059: the external production-package adapter.
//!
//! A package is an input document, not a persistence entity. This service
//! owns only the short-lived inspection session and the final revalidation.
//! Selected items are materialized into the existing H3 `PROJECT_FOLDER`
//! import contract so SourceAssetImportService, generation values, batch
//! creation, and ProductionQueueService remain single-source behavior.

use crate::application::h3_local_import_service::{
    H3LocalImportCommitRequest, H3LocalImportError, H3LocalImportMode, H3LocalImportService,
    H3QualityRecipeSelection,
};
use crate::application::ports::Clock;
use crate::application::production_package_inspector::{
    ProductionPackageDiagnostic, ProductionPackageInspection, ProductionPackageInspectionError,
    ProductionPackageInspector, ProductionPackageItemInspection, ProductionPackageItemStatus,
};
use crate::application::production_queue_service::ProductionQueueService;
use crate::application::source_asset_import_service::SourceAssetImportService;
use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::Mutex;
use uuid::Uuid;

pub const MAX_PRODUCTION_PACKAGE_ITEMS: usize = 500;
pub const MAX_PRODUCTION_PACKAGE_SESSION_MINUTES: i64 = 20;
const DEFAULT_DURATION_SECONDS: i64 = 5;
const DEFAULT_WIDTH: i64 = 960;
const DEFAULT_HEIGHT: i64 = 544;
const SESSION_TTL: Duration = Duration::minutes(MAX_PRODUCTION_PACKAGE_SESSION_MINUTES);

/// The recipe identifiers are application configuration, never package input.
/// `ProductionPackageService` passes them to the existing H3 importer only
/// after the package session and every selected file have been revalidated.
#[derive(Clone, Debug)]
pub struct ProductionPackageH3Config {
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub fl2va_workflow_version_id: Option<String>,
    pub fl2va_recipe_id: Option<String>,
    pub ref2va_workflow_version_id: Option<String>,
    pub ref2va_recipe_id: Option<String>,
    pub quality_profile: Option<String>,
    pub quality_recipes: Vec<H3QualityRecipeSelection>,
}

impl ProductionPackageH3Config {
    fn validate(&self) -> Result<(), ProductionPackageError> {
        if self.workflow_version_id.trim().is_empty() || self.recipe_id.trim().is_empty() {
            return Err(ProductionPackageError::InvalidInput(
                "H3 workflow version and recipe are required".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductionPackageItemMapping {
    pub package_item_id: String,
    pub batch_id: String,
    pub batch_item_id: String,
    pub imported_asset_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductionPackageBatchMapping {
    pub batch_id: String,
    pub batch_name: String,
    pub item_count: usize,
    pub item_mappings: Vec<ProductionPackageItemMapping>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductionPackageCreateBatchesResult {
    pub package_name: String,
    pub batch_count: usize,
    pub item_count: usize,
    pub auto_started: bool,
    pub batches: Vec<ProductionPackageBatchMapping>,
    pub item_mappings: Vec<ProductionPackageItemMapping>,
    pub warnings: Vec<ProductionPackageDiagnostic>,
}

#[derive(Debug)]
pub enum ProductionPackageError {
    InvalidInput(String),
    Inspection(ProductionPackageInspectionError),
    MediaChanged(String),
    PromptChanged(String),
    ModeChanged(String),
    ItemNotFound(String),
    ItemBlocked(String),
    DuplicateItemId(String),
    SessionNotFound,
    SessionExpired,
    Filesystem(String),
    H3(String),
    Queue(String),
}

impl ProductionPackageError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput(_) => "PACKAGE_INVALID_INPUT",
            Self::Inspection(error) => error.code(),
            Self::MediaChanged(_) => "PACKAGE_MEDIA_CHANGED",
            Self::PromptChanged(_) => "PACKAGE_PROMPT_CHANGED",
            Self::ModeChanged(_) => "PACKAGE_MODE_CHANGED",
            Self::ItemNotFound(_) => "PACKAGE_ITEM_NOT_FOUND",
            Self::ItemBlocked(_) => "PACKAGE_ITEM_BLOCKED",
            Self::DuplicateItemId(_) => "PACKAGE_DUPLICATE_ITEM_ID",
            Self::SessionNotFound => "PACKAGE_SESSION_NOT_FOUND",
            Self::SessionExpired => "PACKAGE_SESSION_EXPIRED",
            Self::Filesystem(_) => "PACKAGE_FILESYSTEM_ERROR",
            Self::H3(_) => "PACKAGE_H3_IMPORT_ERROR",
            Self::Queue(_) => "PACKAGE_QUEUE_ERROR",
        }
    }
}

impl fmt::Display for ProductionPackageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inspection(error) => error.fmt(formatter),
            Self::InvalidInput(message)
            | Self::MediaChanged(message)
            | Self::PromptChanged(message)
            | Self::ModeChanged(message)
            | Self::ItemNotFound(message)
            | Self::ItemBlocked(message)
            | Self::DuplicateItemId(message)
            | Self::Filesystem(message)
            | Self::H3(message)
            | Self::Queue(message) => write!(formatter, "{}: {message}", self.code()),
            Self::SessionNotFound => write!(
                formatter,
                "{}: inspection session was not found",
                self.code()
            ),
            Self::SessionExpired => {
                write!(formatter, "{}: inspection session expired", self.code())
            }
        }
    }
}

impl Error for ProductionPackageError {}

impl From<ProductionPackageInspectionError> for ProductionPackageError {
    fn from(error: ProductionPackageInspectionError) -> Self {
        Self::Inspection(error)
    }
}

struct PackageSession {
    project_id: String,
    root_path: PathBuf,
    inspection: ProductionPackageInspection,
    expires_at: DateTime<Utc>,
}

pub struct ProductionPackageService {
    inspector: ProductionPackageInspector,
    h3_local_import_service: Arc<H3LocalImportService>,
    // H3LocalImportService owns the actual SourceAssetImport call. Keeping
    // the same instance explicit here prevents a second asset-import path at
    // the application boundary.
    _source_asset_import_service: Arc<SourceAssetImportService>,
    production_queue_service: Arc<ProductionQueueService>,
    h3_config: ProductionPackageH3Config,
    clock: Arc<dyn Clock>,
    sessions: Arc<Mutex<HashMap<String, PackageSession>>>,
}

impl ProductionPackageService {
    pub fn new(
        inspector: ProductionPackageInspector,
        h3_local_import_service: Arc<H3LocalImportService>,
        source_asset_import_service: Arc<SourceAssetImportService>,
        production_queue_service: Arc<ProductionQueueService>,
        h3_config: ProductionPackageH3Config,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            inspector,
            h3_local_import_service,
            _source_asset_import_service: source_asset_import_service,
            production_queue_service,
            h3_config,
            clock,
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn inspect_session(
        &self,
        project_id: &str,
        package_root: PathBuf,
    ) -> Result<(String, ProductionPackageInspection), ProductionPackageError> {
        validate_project_id(project_id)?;
        self.cleanup_expired_sessions(None).await;
        let inspection = self.inspector.inspect(&package_root).await?;
        let root_path = inspection.package_root.clone();
        let session_id = format!("production_package_{}", Uuid::new_v4());
        self.sessions.lock().await.insert(
            session_id.clone(),
            PackageSession {
                project_id: project_id.to_owned(),
                root_path,
                inspection: inspection.clone(),
                expires_at: self.clock.now() + SESSION_TTL,
            },
        );
        Ok((session_id, inspection))
    }

    pub async fn inspect(
        &self,
        project_id: &str,
        package_root: PathBuf,
    ) -> Result<(String, ProductionPackageInspection), ProductionPackageError> {
        self.inspect_session(project_id, package_root).await
    }

    pub async fn create_batches(
        &self,
        session_id: &str,
        selected_item_ids: &[String],
    ) -> Result<ProductionPackageCreateBatchesResult, ProductionPackageError> {
        self.cleanup_expired_sessions(Some(session_id)).await;
        self.h3_config.validate()?;
        if selected_item_ids.is_empty() {
            return Err(ProductionPackageError::InvalidInput(
                "at least one package item must be selected".to_owned(),
            ));
        }
        let session = {
            let sessions = self.sessions.lock().await;
            let session = sessions
                .get(session_id)
                .ok_or(ProductionPackageError::SessionNotFound)?;
            if session.expires_at <= self.clock.now() {
                return Err(ProductionPackageError::SessionExpired);
            }
            PackageSession {
                project_id: session.project_id.clone(),
                root_path: session.root_path.clone(),
                inspection: session.inspection.clone(),
                expires_at: session.expires_at,
            }
        };
        let selected = select_items(&session.inspection, selected_item_ids)?;
        if selected.len() > MAX_PRODUCTION_PACKAGE_ITEMS {
            return Err(ProductionPackageError::InvalidInput(format!(
                "at most {MAX_PRODUCTION_PACKAGE_ITEMS} package items may be committed"
            )));
        }

        let current = self.inspector.inspect(&session.root_path).await?;
        if current.manifest_sha256 != session.inspection.manifest_sha256 {
            return Err(ProductionPackageError::MediaChanged(
                "production-package.json".to_owned(),
            ));
        }
        let current_by_id = current
            .items
            .iter()
            .map(|item| (item.id.as_str(), item))
            .collect::<HashMap<_, _>>();
        let mut current_selected = Vec::with_capacity(selected.len());
        for item in selected {
            let current_item = current_by_id
                .get(item.id.as_str())
                .copied()
                .ok_or_else(|| ProductionPackageError::ItemNotFound(item.id.clone()))?;
            ensure_item_unchanged(item, current_item)?;
            if current_item.status == ProductionPackageItemStatus::Blocked {
                return Err(ProductionPackageError::ItemBlocked(item.id.clone()));
            }
            for media in item_media(item) {
                self.inspector
                    .revalidate_media(&session.root_path, media)
                    .await
                    .map_err(|error| match error {
                        ProductionPackageInspectionError::PackageMediaChanged => {
                            ProductionPackageError::MediaChanged(media.relative_path.clone())
                        }
                        other => ProductionPackageError::Inspection(other),
                    })?;
            }
            current_selected.push(current_item.clone());
        }

        // Consume the package session before side effects. A retry after a
        // partial downstream failure must be explicit, never an accidental
        // duplicate batch import.
        self.sessions.lock().await.remove(session_id);

        let chunk_size = MAX_PRODUCTION_PACKAGE_ITEMS.min(100);
        let chunk_count = current_selected.len().div_ceil(chunk_size);
        let mut batches = Vec::with_capacity(chunk_count);
        let mut item_mappings = Vec::with_capacity(current_selected.len());
        for (chunk_index, chunk) in current_selected.chunks(chunk_size).enumerate() {
            let staging_root = stage_chunk(chunk)?;
            let batch_name = format!(
                "{} · {}/{}",
                current.package_name,
                chunk_index + 1,
                chunk_count
            );
            let result = self
                .commit_staged_chunk(&session.project_id, &staging_root, &batch_name, chunk)
                .await;
            let _ = fs::remove_dir_all(&staging_root);
            let batch = result?;
            item_mappings.extend(batch.item_mappings.iter().cloned());
            batches.push(batch);
        }

        Ok(ProductionPackageCreateBatchesResult {
            package_name: current.package_name,
            batch_count: batches.len(),
            item_count: item_mappings.len(),
            auto_started: false,
            batches,
            item_mappings,
            warnings: current.warnings,
        })
    }

    async fn commit_staged_chunk(
        &self,
        project_id: &str,
        staging_root: &Path,
        package_batch_name: &str,
        package_items: &[ProductionPackageItemInspection],
    ) -> Result<ProductionPackageBatchMapping, ProductionPackageError> {
        let (h3_session_id, inspection) = self
            .h3_local_import_service
            .pick(
                project_id,
                staging_root.to_path_buf(),
                H3LocalImportMode::ProjectFolder,
            )
            .await
            .map_err(map_h3_error)?;
        if inspection.error_count > 0 || inspection.ready_count != package_items.len() {
            return Err(ProductionPackageError::H3(format!(
                "staged H3 inspection failed: {}",
                inspection.errors.join("; ")
            )));
        }
        let result = self
            .h3_local_import_service
            .commit(
                &h3_session_id,
                H3LocalImportCommitRequest {
                    batch_name: Some(package_batch_name.to_owned()),
                    workflow_version_id: self.h3_config.workflow_version_id.clone(),
                    recipe_id: self.h3_config.recipe_id.clone(),
                    width: DEFAULT_WIDTH,
                    height: DEFAULT_HEIGHT,
                    duration_seconds: DEFAULT_DURATION_SECONDS,
                    seed: None,
                    auto_start: false,
                    generation_mode: None,
                    fl2va_workflow_version_id: self.h3_config.fl2va_workflow_version_id.clone(),
                    fl2va_recipe_id: self.h3_config.fl2va_recipe_id.clone(),
                    ref2va_workflow_version_id: self.h3_config.ref2va_workflow_version_id.clone(),
                    ref2va_recipe_id: self.h3_config.ref2va_recipe_id.clone(),
                    quality_profile: self.h3_config.quality_profile.clone(),
                    quality_recipes: self.h3_config.quality_recipes.clone(),
                },
            )
            .await
            .map_err(map_h3_error)?;
        if result.auto_started {
            return Err(ProductionPackageError::H3(
                "package import unexpectedly started a batch".to_owned(),
            ));
        }
        let detail = self
            .production_queue_service
            .get(project_id, &result.batch_id)
            .await
            .map_err(|error| ProductionPackageError::Queue(error.to_string()))?;
        if detail.items.len() != package_items.len() {
            return Err(ProductionPackageError::Queue(format!(
                "H3 batch item count {} does not match package count {}",
                detail.items.len(),
                package_items.len()
            )));
        }
        let item_mappings = package_items
            .iter()
            .zip(detail.items.iter())
            .map(|(package_item, batch_item)| ProductionPackageItemMapping {
                package_item_id: package_item.id.clone(),
                batch_id: detail.batch.id.as_str().to_owned(),
                batch_item_id: batch_item.id.as_str().to_owned(),
                imported_asset_ids: asset_ids_from_values(&batch_item.values_json),
            })
            .collect::<Vec<_>>();
        Ok(ProductionPackageBatchMapping {
            batch_id: detail.batch.id.as_str().to_owned(),
            batch_name: detail.batch.name,
            item_count: detail.items.len(),
            item_mappings,
        })
    }

    async fn cleanup_expired_sessions(&self, keep_session_id: Option<&str>) {
        let now = self.clock.now();
        self.sessions.lock().await.retain(|session_id, session| {
            keep_session_id.is_some_and(|keep| keep == session_id) || session.expires_at > now
        });
    }
}

fn validate_project_id(project_id: &str) -> Result<(), ProductionPackageError> {
    crate::domain::validate_project_id(project_id)
        .map_err(|error| ProductionPackageError::InvalidInput(error.to_string()))
}

fn select_items<'a>(
    inspection: &'a ProductionPackageInspection,
    selected_item_ids: &[String],
) -> Result<Vec<&'a ProductionPackageItemInspection>, ProductionPackageError> {
    let mut requested = HashSet::<String>::with_capacity(selected_item_ids.len());
    for id in selected_item_ids {
        if !requested.insert(id.clone()) {
            return Err(ProductionPackageError::DuplicateItemId(id.clone()));
        }
    }
    let selected = inspection
        .items
        .iter()
        .filter(|item| requested.contains(&item.id))
        .collect::<Vec<_>>();
    if selected.len() != requested.len() {
        let missing = requested
            .into_iter()
            .find(|id| !inspection.items.iter().any(|item| item.id == *id))
            .unwrap_or_default();
        return Err(ProductionPackageError::ItemNotFound(missing));
    }
    Ok(selected)
}

fn ensure_item_unchanged(
    inspected: &ProductionPackageItemInspection,
    current: &ProductionPackageItemInspection,
) -> Result<(), ProductionPackageError> {
    if inspected.video_prompt != current.video_prompt {
        return Err(ProductionPackageError::PromptChanged(inspected.id.clone()));
    }
    if inspected.mode != current.mode {
        return Err(ProductionPackageError::ModeChanged(inspected.id.clone()));
    }
    if inspected.duration_seconds != current.duration_seconds
        || inspected.width != current.width
        || inspected.height != current.height
    {
        return Err(ProductionPackageError::ModeChanged(inspected.id.clone()));
    }
    let old_media = item_media(inspected).collect::<Vec<_>>();
    let current_media = item_media(current).collect::<Vec<_>>();
    if old_media.len() != current_media.len()
        || old_media
            .iter()
            .zip(current_media.iter())
            .any(|(old, new)| {
                old.relative_path != new.relative_path
                    || old.size_bytes != new.size_bytes
                    || old.sha256 != new.sha256
            })
    {
        return Err(ProductionPackageError::MediaChanged(inspected.id.clone()));
    }
    Ok(())
}

fn item_media(
    item: &ProductionPackageItemInspection,
) -> impl Iterator<
    Item = &crate::application::production_package_inspector::ProductionPackageMediaInspection,
> {
    item.first_frame
        .iter()
        .chain(item.last_frame.iter())
        .chain(item.reference_images.iter())
        .chain(item.reference_audios.iter())
        .chain(item.reference_videos.iter())
}

fn stage_chunk(
    items: &[ProductionPackageItemInspection],
) -> Result<PathBuf, ProductionPackageError> {
    let staging_root =
        std::env::temp_dir().join(format!("ai-studio-production-package-{}", Uuid::new_v4()));
    fs::create_dir_all(&staging_root)
        .map_err(|error| ProductionPackageError::Filesystem(error.to_string()))?;
    for (index, item) in items.iter().enumerate() {
        let segment_root = staging_root.join(format!("segment-{:04}", index + 1));
        fs::create_dir_all(&segment_root)
            .map_err(|error| ProductionPackageError::Filesystem(error.to_string()))?;
        let prompt = format!(
            "---\nmode: {}\nduration: {}\nresolution: {}x{}\n---\n{}",
            front_matter_mode(&item.mode),
            item.duration_seconds,
            item.width,
            item.height,
            item.video_prompt
        );
        fs::write(segment_root.join("prompt.txt"), prompt)
            .map_err(|error| ProductionPackageError::Filesystem(error.to_string()))?;
        if let Some(media) = item.first_frame.as_ref() {
            copy_staged_media(media, &segment_root.join(with_extension("first", media)))?;
        }
        if let Some(media) = item.last_frame.as_ref() {
            copy_staged_media(media, &segment_root.join(with_extension("last", media)))?;
        }
        for (index, media) in item.reference_images.iter().enumerate() {
            copy_staged_media(
                media,
                &segment_root.join(with_extension(&format!("reference-{index:03}"), media)),
            )?;
        }
        for (index, media) in item.reference_audios.iter().enumerate() {
            copy_staged_media(
                media,
                &segment_root.join(with_extension(
                    &format!("reference-audio-{index:03}"),
                    media,
                )),
            )?;
        }
        for (index, media) in item.reference_videos.iter().enumerate() {
            copy_staged_media(
                media,
                &segment_root.join(with_extension(
                    &format!("reference-video-{index:03}"),
                    media,
                )),
            )?;
        }
    }
    Ok(staging_root)
}

fn front_matter_mode(mode: &str) -> &'static str {
    match mode {
        "FL2VA_TEXT_TO_VIDEO" => "text",
        "FL2VA_IMAGE_TO_VIDEO" => "image",
        "FL2VA_FIRST_LAST" => "first_last",
        "REF2VA_IMAGE" => "ref_image",
        "REF2VA_AUDIO" => "ref_audio",
        "REF2VA_IMAGE_AUDIO" => "ref_image_audio",
        "REF2VA_VIDEO_IMAGE" => "ref_video_image",
        _ => "text",
    }
}

fn with_extension(
    stem: &str,
    media: &crate::application::production_package_inspector::ProductionPackageMediaInspection,
) -> String {
    let extension = Path::new(&media.relative_path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("bin");
    format!("{stem}.{extension}")
}

fn copy_staged_media(
    source: &crate::application::production_package_inspector::ProductionPackageMediaInspection,
    destination: &Path,
) -> Result<(), ProductionPackageError> {
    let source_path = source
        .resolved_path
        .as_ref()
        .ok_or_else(|| ProductionPackageError::MediaChanged(source.relative_path.clone()))?;
    fs::copy(source_path, destination)
        .map_err(|_| ProductionPackageError::MediaChanged(source.relative_path.clone()))?;
    let bytes = fs::read(destination)
        .map_err(|error| ProductionPackageError::Filesystem(error.to_string()))?;
    if source.sha256.as_deref() != Some(sha256(&bytes).as_str())
        || source.size_bytes != Some(bytes.len() as u64)
    {
        return Err(ProductionPackageError::MediaChanged(
            source.relative_path.clone(),
        ));
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn asset_ids_from_values(values: &Value) -> Vec<String> {
    [
        "first_frame",
        "last_frame",
        "reference_images",
        "reference_videos",
        "reference_audios",
    ]
    .into_iter()
    .filter_map(|key| values.get(key))
    .flat_map(asset_ids_from_value)
    .collect()
}

fn asset_ids_from_value(value: &Value) -> Vec<String> {
    let Some(object) = value.as_object() else {
        return Vec::new();
    };
    match object.get("type").and_then(Value::as_str) {
        Some("image_asset" | "video_asset" | "audio_asset") => object
            .get("assetId")
            .and_then(Value::as_str)
            .map(|value| vec![value.to_owned()])
            .unwrap_or_default(),
        Some("image_assets" | "video_assets" | "audio_assets") => object
            .get("assetIds")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn map_h3_error(error: H3LocalImportError) -> ProductionPackageError {
    match error {
        H3LocalImportError::FilesystemBoundary(message) => {
            ProductionPackageError::MediaChanged(message)
        }
        H3LocalImportError::Queue(message) => ProductionPackageError::Queue(message),
        H3LocalImportError::Filesystem(message)
        | H3LocalImportError::InvalidInput(message)
        | H3LocalImportError::Inspection(message)
        | H3LocalImportError::Prompt(message)
        | H3LocalImportError::AssetImport(message) => ProductionPackageError::H3(message),
        H3LocalImportError::SessionNotFound => {
            ProductionPackageError::H3("H3 session not found".to_owned())
        }
        H3LocalImportError::SessionExpired => {
            ProductionPackageError::H3("H3 session expired".to_owned())
        }
    }
}
