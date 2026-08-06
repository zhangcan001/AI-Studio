use crate::application::image_inspection::{generate_thumbnail, inspect_bytes, InspectedImage};
use crate::application::media_probe::{CommandMediaProbe, MediaProbe};
use crate::application::output_collector::{CollectedImage, CollectedOutput, CollectedVideo};
use crate::application::ports::{
    AssetRepository, AssetStore, Clock, ProjectRepository, RepositoryError, TaskOutputAssetMapping,
};
use crate::domain::{Asset, AssetId, TaskId};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{error::Error, fmt, path::PathBuf, sync::Arc};

#[derive(Clone, Debug, PartialEq)]
pub enum AssetImportError {
    ProjectStorageMissing { project_id: String },
    OutputImportFailed { message: String },
    AssetPersistence { message: String },
}

impl AssetImportError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ProjectStorageMissing { .. } | Self::AssetPersistence { .. } => {
                "ASSET_PERSISTENCE_ERROR"
            }
            Self::OutputImportFailed { .. } => "OUTPUT_IMPORT_FAILED",
        }
    }
}

impl fmt::Display for AssetImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectStorageMissing { project_id } => write!(
                formatter,
                "ASSET_PERSISTENCE_ERROR: project {project_id} has no storage root"
            ),
            Self::OutputImportFailed { message } => {
                write!(formatter, "OUTPUT_IMPORT_FAILED: {message}")
            }
            Self::AssetPersistence { message } => {
                write!(formatter, "ASSET_PERSISTENCE_ERROR: {message}")
            }
        }
    }
}

impl Error for AssetImportError {}

pub struct AssetImportService {
    project_repository: Arc<dyn ProjectRepository>,
    asset_store: Arc<dyn AssetStore>,
    asset_repository: Arc<dyn AssetRepository>,
    clock: Arc<dyn Clock>,
    media_probe: Arc<dyn MediaProbe>,
}

impl AssetImportService {
    pub fn new(
        project_repository: Arc<dyn ProjectRepository>,
        asset_store: Arc<dyn AssetStore>,
        asset_repository: Arc<dyn AssetRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            project_repository,
            asset_store,
            asset_repository,
            clock,
            media_probe: Arc::new(CommandMediaProbe::default()),
        }
    }

    #[allow(dead_code)]
    pub fn with_media_probe(mut self, media_probe: Arc<dyn MediaProbe>) -> Self {
        self.media_probe = media_probe;
        self
    }

    #[allow(dead_code)]
    pub async fn import(
        &self,
        project_id: &str,
        task_id: &TaskId,
        images: &[CollectedImage],
    ) -> Result<Vec<Asset>, AssetImportError> {
        if images.is_empty() {
            return Ok(Vec::new());
        }
        let project_root = self
            .project_repository
            .get_storage_root(project_id)
            .await
            .map_err(repository_error)?
            .ok_or_else(|| AssetImportError::ProjectStorageMissing {
                project_id: project_id.to_owned(),
            })?;

        let inspected = images
            .iter()
            .map(inspect_image)
            .collect::<Result<Vec<_>, _>>()?;

        let mut stored_paths = Vec::<PathBuf>::with_capacity(images.len());
        let mut assets = Vec::with_capacity(images.len());
        for (image, inspected) in images.iter().zip(inspected.iter()) {
            let asset_id = AssetId::new();
            let stored = match self
                .asset_store
                .write_image(&project_root, &asset_id, inspected.extension, &image.bytes)
                .await
            {
                Ok(stored) => stored,
                Err(error) => {
                    self.compensate(&stored_paths).await;
                    return Err(AssetImportError::AssetPersistence {
                        message: error.to_string(),
                    });
                }
            };
            stored_paths.push(stored.path.clone());
            let thumbnail_path = match generate_thumbnail(&image.bytes) {
                Ok(thumbnail) => match self
                    .asset_store
                    .write_thumbnail(&project_root, &asset_id, &thumbnail)
                    .await
                {
                    Ok(stored_thumbnail) => {
                        stored_paths.push(stored_thumbnail.path.clone());
                        Some(stored_thumbnail.path.display().to_string())
                    }
                    Err(error) => {
                        tracing::warn!(asset_id = %asset_id, error = %error, "thumbnail write skipped; full asset remains available");
                        None
                    }
                },
                Err(error) => {
                    tracing::warn!(asset_id = %asset_id, error = %error, "thumbnail generation skipped; full asset remains available");
                    None
                }
            };
            let created_at = self.clock.now();
            let metadata = json!({
                "outputId": image.output_id,
                "nodeId": image.node_id,
                "position": image.position,
                "comfyFilename": image.original_filename,
                "comfySubfolder": image.subfolder,
                "comfyType": image.folder_type,
            });
            let mut asset = match Asset::new_generated_image(
                asset_id,
                project_id,
                format!("Generated Image {}", image.position + 1),
                image.original_filename.clone(),
                stored.path.display().to_string(),
                inspected.sha256.clone(),
                inspected.mime_type,
                inspected.width,
                inspected.height,
                image.bytes.len() as u64,
                task_id.clone(),
                metadata,
                created_at,
            ) {
                Ok(asset) => asset,
                Err(error) => {
                    self.compensate(&stored_paths).await;
                    return Err(AssetImportError::OutputImportFailed {
                        message: error.to_string(),
                    });
                }
            };
            asset.thumbnail_path = thumbnail_path;
            assets.push(asset);
        }

        if let Err(error) = self.asset_repository.insert_many(&assets).await {
            self.compensate(&stored_paths).await;
            return Err(AssetImportError::AssetPersistence {
                message: error.to_string(),
            });
        }

        Ok(assets)
    }

    pub async fn import_outputs(
        &self,
        project_id: &str,
        task_id: &TaskId,
        outputs: Vec<CollectedOutput>,
    ) -> Result<Vec<Asset>, AssetImportError> {
        if outputs.is_empty() {
            return Ok(Vec::new());
        }
        let project_root = self
            .project_repository
            .get_storage_root(project_id)
            .await
            .map_err(repository_error)?
            .ok_or_else(|| AssetImportError::ProjectStorageMissing {
                project_id: project_id.to_owned(),
            })?;

        let mut stored_paths = Vec::<PathBuf>::new();
        let mut assets = Vec::with_capacity(outputs.len());
        let mut mappings = Vec::with_capacity(outputs.len());
        for output in outputs {
            let result = match output {
                CollectedOutput::Image(image) => {
                    self.persist_collected_image(project_id, task_id, &project_root, image)
                        .await
                }
                CollectedOutput::Video(video) => {
                    self.persist_collected_video(project_id, task_id, &project_root, video)
                        .await
                }
            };
            let (asset, mapping, paths) = match result {
                Ok(result) => result,
                Err(error) => {
                    self.compensate(&stored_paths).await;
                    return Err(error);
                }
            };
            stored_paths.extend(paths);
            assets.push(asset);
            mappings.push(mapping);
        }

        if let Err(error) = self
            .asset_repository
            .insert_generated_outputs(&assets, &mappings)
            .await
        {
            self.compensate(&stored_paths).await;
            return Err(AssetImportError::AssetPersistence {
                message: error.to_string(),
            });
        }
        Ok(assets)
    }

    async fn persist_collected_image(
        &self,
        project_id: &str,
        task_id: &TaskId,
        project_root: &std::path::Path,
        image: CollectedImage,
    ) -> Result<(Asset, TaskOutputAssetMapping, Vec<PathBuf>), AssetImportError> {
        let inspected = inspect_image(&image)?;
        let asset_id = AssetId::new();
        let stored = self
            .asset_store
            .write_image(project_root, &asset_id, inspected.extension, &image.bytes)
            .await
            .map_err(|error| AssetImportError::AssetPersistence {
                message: error.to_string(),
            })?;
        let mut paths = vec![stored.path.clone()];
        let thumbnail_path = match generate_thumbnail(&image.bytes) {
            Ok(thumbnail) => match self
                .asset_store
                .write_thumbnail(project_root, &asset_id, &thumbnail)
                .await
            {
                Ok(stored_thumbnail) => {
                    paths.push(stored_thumbnail.path.clone());
                    Some(stored_thumbnail.path.display().to_string())
                }
                Err(error) => {
                    tracing::warn!(asset_id = %asset_id, error = %error, "thumbnail write skipped; full asset remains available");
                    None
                }
            },
            Err(error) => {
                tracing::warn!(asset_id = %asset_id, error = %error, "thumbnail generation skipped; full asset remains available");
                None
            }
        };
        let created_at = self.clock.now();
        let metadata = json!({
            "outputId": image.output_id,
            "nodeId": image.node_id,
            "position": image.position,
            "comfyFilename": image.original_filename,
            "comfySubfolder": image.subfolder,
            "comfyType": image.folder_type,
            "mediaKind": "image",
        });
        let mut asset = Asset::new_generated_image(
            asset_id.clone(),
            project_id,
            format!("Generated Image {}", image.position + 1),
            image.original_filename,
            stored.path.display().to_string(),
            inspected.sha256,
            inspected.mime_type,
            inspected.width,
            inspected.height,
            image.bytes.len() as u64,
            task_id.clone(),
            metadata,
            created_at,
        )
        .map_err(|error| AssetImportError::OutputImportFailed {
            message: error.to_string(),
        })?;
        asset.thumbnail_path = thumbnail_path;
        Ok((
            asset,
            TaskOutputAssetMapping {
                task_id: task_id.clone(),
                output_id: image.output_id,
                ordinal: image.position as u32,
                asset_id,
                created_at,
            },
            paths,
        ))
    }

    async fn persist_collected_video(
        &self,
        project_id: &str,
        task_id: &TaskId,
        project_root: &std::path::Path,
        mut video: CollectedVideo,
    ) -> Result<(Asset, TaskOutputAssetMapping, Vec<PathBuf>), AssetImportError> {
        let (extension, mime_type) =
            validate_video_output(&video.original_filename, video.content_type.as_deref())?;
        if video
            .content_length
            .is_some_and(|length| length > MAX_VIDEO_OUTPUT_BYTES)
        {
            return Err(AssetImportError::OutputImportFailed {
                message: format!(
                    "video output exceeds the {} byte safety limit",
                    MAX_VIDEO_OUTPUT_BYTES
                ),
            });
        }

        let asset_id = AssetId::new();
        let mut writer = self
            .asset_store
            .begin_video_write(project_root, &asset_id, extension)
            .await
            .map_err(|error| AssetImportError::AssetPersistence {
                message: error.to_string(),
            })?;
        let mut hasher = Sha256::new();
        let mut file_size = 0u64;
        let mut signature = Vec::with_capacity(64);
        while let Some(chunk) = video.stream.next_chunk().await.map_err(|error| {
            AssetImportError::OutputImportFailed {
                message: error.to_string(),
            }
        })? {
            if chunk.is_empty() {
                continue;
            }
            if signature.len() < 64 {
                signature.extend_from_slice(&chunk[..chunk.len().min(64 - signature.len())]);
            }
            file_size = file_size.saturating_add(chunk.len() as u64);
            if file_size > MAX_VIDEO_OUTPUT_BYTES {
                let _ = writer.abort().await;
                return Err(AssetImportError::OutputImportFailed {
                    message: format!(
                        "video output exceeds the {} byte safety limit",
                        MAX_VIDEO_OUTPUT_BYTES
                    ),
                });
            }
            hasher.update(&chunk);
            if let Err(error) = writer.write_chunk(&chunk).await {
                let _ = writer.abort().await;
                return Err(AssetImportError::AssetPersistence {
                    message: error.to_string(),
                });
            }
        }
        if file_size == 0 || !valid_video_signature(extension, &signature) {
            let _ = writer.abort().await;
            return Err(AssetImportError::OutputImportFailed {
                message: "video output is empty or is not a recognized MP4/WEBM stream".to_owned(),
            });
        }
        let stored = writer
            .commit()
            .await
            .map_err(|error| AssetImportError::AssetPersistence {
                message: error.to_string(),
            })?;
        let mut paths = vec![stored.path.clone()];
        let probed = self.media_probe.probe_video(&stored.path).await;
        let thumbnail_path = if let Some(poster) =
            self.media_probe.generate_video_poster(&stored.path).await
        {
            match self
                .asset_store
                .write_video_poster(project_root, &asset_id, &poster)
                .await
            {
                Ok(stored_poster) => {
                    paths.push(stored_poster.path.clone());
                    Some(stored_poster.path.display().to_string())
                }
                Err(error) => {
                    tracing::warn!(asset_id = %asset_id, error = %error, "video poster write skipped");
                    None
                }
            }
        } else {
            None
        };
        let created_at = self.clock.now();
        let metadata = json!({
            "outputId": video.output_id.clone(),
            "nodeId": video.node_id.clone(),
            "position": video.position,
            "comfyFilename": video.original_filename.clone(),
            "comfySubfolder": video.subfolder.clone(),
            "comfyType": video.folder_type.clone(),
            "mediaKind": "video",
        });
        let mut asset = match Asset::new_generated_video(
            asset_id.clone(),
            project_id,
            format!("Generated Video {}", video.position + 1),
            video.original_filename.clone(),
            stored.path.display().to_string(),
            format!("{:x}", hasher.finalize()),
            mime_type,
            probed.width,
            probed.height,
            probed.duration_ms,
            file_size,
            task_id.clone(),
            metadata,
            created_at,
        ) {
            Ok(asset) => asset,
            Err(error) => {
                self.compensate(&paths).await;
                return Err(AssetImportError::OutputImportFailed {
                    message: error.to_string(),
                });
            }
        };
        asset.thumbnail_path = thumbnail_path;
        Ok((
            asset,
            TaskOutputAssetMapping {
                task_id: task_id.clone(),
                output_id: video.output_id.clone(),
                ordinal: video.position as u32,
                asset_id,
                created_at,
            },
            paths,
        ))
    }

    async fn compensate(&self, paths: &[PathBuf]) {
        for path in paths {
            if let Err(error) = self.asset_store.delete(path).await {
                tracing::error!(
                    path = %path.display(),
                    error = %error,
                    "asset compensation delete failed"
                );
            }
        }
    }
}

pub const MAX_VIDEO_OUTPUT_BYTES: u64 = 4 * 1024 * 1024 * 1024;

fn validate_video_output(
    filename: &str,
    content_type: Option<&str>,
) -> Result<(&'static str, &'static str), AssetImportError> {
    let extension = std::path::Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    let mime = content_type.map(|value| {
        value
            .split(';')
            .next()
            .unwrap_or(value)
            .trim()
            .to_ascii_lowercase()
    });
    if mime
        .as_deref()
        .is_some_and(|value| value == "text/html" || value == "application/json")
    {
        return Err(AssetImportError::OutputImportFailed {
            message: "video response is HTML/JSON rather than media".to_owned(),
        });
    }
    match (extension.as_deref(), mime.as_deref()) {
        (Some("mp4"), None | Some("video/mp4") | Some("application/octet-stream")) => {
            Ok(("mp4", "video/mp4"))
        }
        (Some("webm"), None | Some("video/webm") | Some("application/octet-stream")) => {
            Ok(("webm", "video/webm"))
        }
        (Some("mp4"), Some(other)) | (Some("webm"), Some(other)) => {
            Err(AssetImportError::OutputImportFailed {
                message: format!("video extension/content-type mismatch: {filename} / {other}"),
            })
        }
        _ => Err(AssetImportError::OutputImportFailed {
            message: format!("unsupported video output format: {filename}"),
        }),
    }
}

fn valid_video_signature(extension: &str, bytes: &[u8]) -> bool {
    match extension {
        "webm" => bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]),
        "mp4" => bytes
            .get(4..)
            .is_some_and(|header| header.windows(4).any(|window| window == b"ftyp")),
        _ => false,
    }
}

fn inspect_image(image: &CollectedImage) -> Result<InspectedImage, AssetImportError> {
    inspect_bytes(&image.bytes).map_err(|error| invalid_image(error.to_string()))
}

fn invalid_image(message: impl Into<String>) -> AssetImportError {
    AssetImportError::OutputImportFailed {
        message: message.into(),
    }
}

fn repository_error(error: RepositoryError) -> AssetImportError {
    AssetImportError::AssetPersistence {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{AssetImportError, AssetImportService};
    use crate::application::output_collector::{CollectedImage, CollectedOutput, CollectedVideo};
    use crate::application::ports::{
        AssetRepository, Clock, ComfyAdapterError, ComfyOutputStream, ProjectRecord,
        ProjectRepository, RepositoryError,
    };
    use crate::domain::{Asset, AssetId, TaskId};
    use crate::infrastructure::filesystem::FileSystemAssetStore;
    use async_trait::async_trait;
    use chrono::{DateTime, TimeZone, Utc};
    use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    #[derive(Clone)]
    struct FakeProjectRepository {
        root: PathBuf,
    }

    #[async_trait]
    impl ProjectRepository for FakeProjectRepository {
        async fn list(&self) -> Result<Vec<ProjectRecord>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn find_by_id(
            &self,
            project_id: &str,
        ) -> Result<Option<ProjectRecord>, RepositoryError> {
            Ok(Some(ProjectRecord {
                id: project_id.to_owned(),
                name: "Test Project".to_owned(),
                description: None,
                root_path: self.root.clone(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }))
        }

        async fn insert(&self, _project: &ProjectRecord) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn update_metadata(
            &self,
            project_id: &str,
            name: &str,
            description: Option<&str>,
            updated_at: DateTime<Utc>,
        ) -> Result<Option<ProjectRecord>, RepositoryError> {
            Ok(Some(ProjectRecord {
                id: project_id.to_owned(),
                name: name.to_owned(),
                description: description.map(str::to_owned),
                root_path: self.root.clone(),
                created_at: updated_at,
                updated_at,
            }))
        }

        async fn get_storage_root(
            &self,
            _project_id: &str,
        ) -> Result<Option<PathBuf>, RepositoryError> {
            Ok(Some(self.root.clone()))
        }

        async fn ensure_default_project(
            &self,
            project_id: &str,
            name: &str,
            root_path: &PathBuf,
            created_at: DateTime<Utc>,
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
    struct FakeAssetRepository {
        assets: Arc<Mutex<Vec<Asset>>>,
        fail: bool,
    }

    #[async_trait]
    impl AssetRepository for FakeAssetRepository {
        async fn insert_many(&self, assets: &[Asset]) -> Result<(), RepositoryError> {
            if self.fail {
                return Err(RepositoryError::database("forced asset database failure"));
            }
            self.assets.lock().unwrap().extend_from_slice(assets);
            Ok(())
        }

        async fn find_by_id(&self, asset_id: &AssetId) -> Result<Option<Asset>, RepositoryError> {
            Ok(self
                .assets
                .lock()
                .unwrap()
                .iter()
                .find(|asset| asset.id == *asset_id)
                .cloned())
        }

        async fn list_by_source_task(
            &self,
            task_id: &TaskId,
        ) -> Result<Vec<Asset>, RepositoryError> {
            Ok(self
                .assets
                .lock()
                .unwrap()
                .iter()
                .filter(|asset| asset.source_task_id.as_ref() == Some(task_id))
                .cloned()
                .collect())
        }

        async fn list_recent(
            &self,
            _project_id: &str,
            _limit: u32,
        ) -> Result<Vec<Asset>, RepositoryError> {
            Ok(self.assets.lock().unwrap().clone())
        }
    }

    #[derive(Clone)]
    struct FixedClock(DateTime<Utc>);

    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            self.0
        }
    }

    fn task_id() -> TaskId {
        TaskId::parse("tsk_test-task").unwrap()
    }

    fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        let image = RgbImage::from_pixel(width, height, Rgb([10, 20, 30]));
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(image)
            .write_to(&mut bytes, ImageFormat::Png)
            .expect("png should encode");
        bytes.into_inner()
    }

    fn image(bytes: Vec<u8>) -> CollectedImage {
        CollectedImage {
            output_id: "generated_image".to_owned(),
            node_id: "9".to_owned(),
            original_filename: "ComfyUI_00001.jpg".to_owned(),
            bytes,
            content_type: Some("image/png".to_owned()),
            position: 0,
            subfolder: String::new(),
            folder_type: "output".to_owned(),
        }
    }

    fn service(root: &Path, repository: FakeAssetRepository) -> AssetImportService {
        AssetImportService::new(
            Arc::new(FakeProjectRepository {
                root: root.to_path_buf(),
            }),
            Arc::new(FileSystemAssetStore::new()),
            Arc::new(repository),
            Arc::new(FixedClock(
                Utc.with_ymd_and_hms(2026, 1, 1, 10, 0, 0).unwrap(),
            )),
        )
    }

    use std::io::Cursor;

    struct SyntheticVideoStream {
        remaining: usize,
        chunk_size: usize,
        emitted: usize,
        fail_after: Option<usize>,
    }

    #[async_trait]
    impl ComfyOutputStream for SyntheticVideoStream {
        fn content_type(&self) -> Option<&str> {
            Some("video/mp4")
        }

        fn content_length(&self) -> Option<u64> {
            Some(self.remaining as u64 + self.emitted as u64)
        }

        async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, ComfyAdapterError> {
            if self.fail_after.is_some_and(|limit| self.emitted >= limit) {
                return Err(ComfyAdapterError::OutputDownload(
                    "synthetic stream interrupted".to_owned(),
                ));
            }
            if self.remaining == 0 {
                return Ok(None);
            }
            let length = self.remaining.min(self.chunk_size);
            let mut bytes = vec![0u8; length];
            if self.emitted == 0 && length >= 12 {
                bytes[4..8].copy_from_slice(b"ftyp");
            }
            self.remaining -= length;
            self.emitted += length;
            Ok(Some(bytes))
        }
    }

    fn video(output_id: &str, size: usize, fail_after: Option<usize>) -> CollectedOutput {
        CollectedOutput::Video(CollectedVideo {
            output_id: output_id.to_owned(),
            node_id: "11".to_owned(),
            original_filename: "ComfyUI_00001.mp4".to_owned(),
            content_type: Some("video/mp4".to_owned()),
            content_length: Some(size as u64),
            position: 0,
            subfolder: String::new(),
            folder_type: "output".to_owned(),
            stream: Box::new(SyntheticVideoStream {
                remaining: size,
                chunk_size: 1024 * 1024,
                emitted: 0,
                fail_after,
            }),
        })
    }

    #[tokio::test]
    async fn validates_actual_image_format_and_records_hash_dimensions_and_extension() {
        let root = tempdir().unwrap();
        let repository = FakeAssetRepository::default();
        let assets = service(root.path(), repository.clone())
            .import("project-1", &task_id(), &[image(png_bytes(2, 3))])
            .await
            .expect("image import should succeed");

        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].mime_type, "image/png");
        assert_eq!((assets[0].width, assets[0].height), (2, 3));
        assert_eq!(
            assets[0].file_size,
            std::fs::metadata(&assets[0].storage_path).unwrap().len()
        );
        assert!(assets[0].storage_path.ends_with(".png"));
        let thumbnail_path = assets[0]
            .thumbnail_path
            .as_ref()
            .expect("thumbnail should be recorded");
        assert!(
            thumbnail_path.contains("assets/thumbnails/image")
                || thumbnail_path.contains("assets\\thumbnails\\image")
        );
        let (thumbnail_width, thumbnail_height) = image::image_dimensions(thumbnail_path).unwrap();
        assert!(thumbnail_width <= 384 && thumbnail_height <= 384);
        assert_eq!(repository.assets.lock().unwrap().len(), 1);
        assert!(Path::new(&assets[0].storage_path).is_file());
    }

    #[tokio::test]
    async fn invalid_image_is_rejected_before_any_asset_file_or_row() {
        let root = tempdir().unwrap();
        let repository = FakeAssetRepository::default();
        let error = service(root.path(), repository.clone())
            .import(
                "project-1",
                &task_id(),
                &[image(b"<html>not an image</html>".to_vec())],
            )
            .await
            .expect_err("invalid image should fail");
        assert!(matches!(error, AssetImportError::OutputImportFailed { .. }));
        assert!(repository.assets.lock().unwrap().is_empty());
        assert!(!root.path().join("assets/generated/image").exists());
    }

    #[tokio::test]
    async fn database_failure_compensates_new_files_from_this_run() {
        let root = tempdir().unwrap();
        let repository = FakeAssetRepository {
            assets: Arc::new(Mutex::new(Vec::new())),
            fail: true,
        };
        let error = service(root.path(), repository)
            .import(
                "project-1",
                &task_id(),
                &[image(png_bytes(1, 1)), image(png_bytes(1, 1))],
            )
            .await
            .expect_err("database failure should be returned");
        assert!(matches!(error, AssetImportError::AssetPersistence { .. }));
        let directory = root.path().join("assets/generated/image");
        assert!(directory.is_dir());
        assert_eq!(
            std::fs::read_dir(directory)
                .unwrap()
                .filter_map(Result::ok)
                .count(),
            0
        );
    }

    #[tokio::test]
    async fn streams_video_in_bounded_chunks_and_persists_generated_video_asset() {
        let root = tempdir().unwrap();
        let repository = FakeAssetRepository::default();
        let size = 128 * 1024 * 1024;
        let assets = service(root.path(), repository.clone())
            .import_outputs(
                "project-1",
                &task_id(),
                vec![video("generated_video", size, None)],
            )
            .await
            .expect("video import should succeed");
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].asset_type, crate::domain::AssetType::Video);
        assert_eq!(assets[0].category, "generated_video");
        assert_eq!(assets[0].file_size, size as u64);
        assert!(assets[0].duration_ms.is_none());
        assert!(
            assets[0]
                .storage_path
                .contains("assets/generated/video/ast_")
                || assets[0]
                    .storage_path
                    .contains("assets\\generated\\video\\ast_")
        );
        assert!(Path::new(&assets[0].storage_path).is_file());
        assert!(!Path::new(&assets[0].storage_path)
            .parent()
            .unwrap()
            .join(format!(".{}.tmp", assets[0].id))
            .exists());
        assert_eq!(repository.assets.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn interrupted_video_stream_removes_temp_file_and_persists_no_asset() {
        let root = tempdir().unwrap();
        let repository = FakeAssetRepository::default();
        let error = service(root.path(), repository.clone())
            .import_outputs(
                "project-1",
                &task_id(),
                vec![video(
                    "generated_video",
                    8 * 1024 * 1024,
                    Some(3 * 1024 * 1024),
                )],
            )
            .await
            .expect_err("interrupted stream should fail");
        assert!(matches!(error, AssetImportError::OutputImportFailed { .. }));
        assert!(repository.assets.lock().unwrap().is_empty());
        let video_directory = root.path().join("assets/generated/video");
        if video_directory.exists() {
            assert_eq!(std::fs::read_dir(video_directory).unwrap().count(), 0);
        }
    }

    #[tokio::test]
    async fn rejects_video_html_or_extension_content_type_mismatch_before_write() {
        let root = tempdir().unwrap();
        let repository = FakeAssetRepository::default();
        let mut output = match video("generated_video", 1024, None) {
            CollectedOutput::Video(output) => output,
            CollectedOutput::Image(_) => unreachable!(),
        };
        output.content_type = Some("text/html".to_owned());
        let error = service(root.path(), repository.clone())
            .import_outputs(
                "project-1",
                &task_id(),
                vec![CollectedOutput::Video(output)],
            )
            .await
            .expect_err("HTML response should be rejected");
        assert!(matches!(error, AssetImportError::OutputImportFailed { .. }));
        assert!(repository.assets.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn video_database_failure_cleans_published_video_without_mapping() {
        let root = tempdir().unwrap();
        let repository = FakeAssetRepository {
            assets: Arc::new(Mutex::new(Vec::new())),
            fail: true,
        };
        let error = service(root.path(), repository.clone())
            .import_outputs(
                "project-1",
                &task_id(),
                vec![video("generated_video", 1024 * 1024, None)],
            )
            .await
            .expect_err("database failure should be returned");
        assert!(matches!(error, AssetImportError::AssetPersistence { .. }));
        assert!(repository.assets.lock().unwrap().is_empty());
        let video_directory = root.path().join("assets/generated/video");
        if video_directory.exists() {
            assert_eq!(std::fs::read_dir(video_directory).unwrap().count(), 0);
        }
    }
}
