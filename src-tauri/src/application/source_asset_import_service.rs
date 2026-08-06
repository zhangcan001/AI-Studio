use crate::application::image_inspection::{generate_thumbnail, inspect_bytes};
use crate::application::media_probe::{CommandMediaProbe, MediaProbe};
use crate::application::ports::{
    AssetReadStream, AssetRepository, AssetStore, AssetWriteSession, Clock, ProjectRepository,
    RepositoryError,
};
use crate::domain::{Asset, AssetId};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{error::Error, fmt, path::Path, sync::Arc};
use tokio::io::AsyncReadExt;

pub const MAX_SOURCE_IMAGE_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_SOURCE_VIDEO_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const MAX_SOURCE_AUDIO_BYTES: u64 = 512 * 1024 * 1024;
const SOURCE_MEDIA_CHUNK_BYTES: usize = 1024 * 1024;
const SOURCE_SIGNATURE_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceMediaKind {
    Video,
    Audio,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceAssetImportError {
    ProjectStorageMissing { project_id: String },
    SourceImageTooLarge { max_bytes: u64, actual_bytes: u64 },
    SourceVideoTooLarge { max_bytes: u64, actual_bytes: u64 },
    SourceAudioTooLarge { max_bytes: u64, actual_bytes: u64 },
    InvalidSourceImage { message: String },
    InvalidSourceVideo { message: String },
    InvalidSourceAudio { message: String },
    AssetPersistence { message: String },
}

impl SourceAssetImportError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ProjectStorageMissing { .. } | Self::AssetPersistence { .. } => {
                "ASSET_PERSISTENCE_ERROR"
            }
            Self::SourceImageTooLarge { .. } => "SOURCE_IMAGE_TOO_LARGE",
            Self::SourceVideoTooLarge { .. } => "SOURCE_VIDEO_TOO_LARGE",
            Self::SourceAudioTooLarge { .. } => "SOURCE_AUDIO_TOO_LARGE",
            Self::InvalidSourceImage { .. } => "INVALID_SOURCE_IMAGE",
            Self::InvalidSourceVideo { .. } => "INVALID_SOURCE_VIDEO",
            Self::InvalidSourceAudio { .. } => "INVALID_SOURCE_AUDIO",
        }
    }
}

impl fmt::Display for SourceAssetImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectStorageMissing { project_id } => write!(
                formatter,
                "ASSET_PERSISTENCE_ERROR: project {project_id} has no storage root"
            ),
            Self::SourceImageTooLarge {
                max_bytes,
                actual_bytes,
            } => write!(
                formatter,
                "SOURCE_IMAGE_TOO_LARGE: image is {actual_bytes} bytes; maximum is {max_bytes}"
            ),
            Self::SourceVideoTooLarge {
                max_bytes,
                actual_bytes,
            } => write!(
                formatter,
                "SOURCE_VIDEO_TOO_LARGE: video is {actual_bytes} bytes; maximum is {max_bytes}"
            ),
            Self::SourceAudioTooLarge {
                max_bytes,
                actual_bytes,
            } => write!(
                formatter,
                "SOURCE_AUDIO_TOO_LARGE: audio is {actual_bytes} bytes; maximum is {max_bytes}"
            ),
            Self::InvalidSourceImage { message } => {
                write!(formatter, "INVALID_SOURCE_IMAGE: {message}")
            }
            Self::InvalidSourceVideo { message } => {
                write!(formatter, "INVALID_SOURCE_VIDEO: {message}")
            }
            Self::InvalidSourceAudio { message } => {
                write!(formatter, "INVALID_SOURCE_AUDIO: {message}")
            }
            Self::AssetPersistence { message } => {
                write!(formatter, "ASSET_PERSISTENCE_ERROR: {message}")
            }
        }
    }
}

impl Error for SourceAssetImportError {}

pub struct SourceAssetImportService {
    project_repository: Arc<dyn ProjectRepository>,
    asset_store: Arc<dyn AssetStore>,
    asset_repository: Arc<dyn AssetRepository>,
    clock: Arc<dyn Clock>,
}

impl SourceAssetImportService {
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
        }
    }

    pub async fn import_bytes(
        &self,
        project_id: &str,
        original_name: &str,
        bytes: &[u8],
    ) -> Result<Asset, SourceAssetImportError> {
        let original_name = safe_file_name(original_name)?;
        let actual_bytes = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if actual_bytes > MAX_SOURCE_IMAGE_BYTES {
            return Err(SourceAssetImportError::SourceImageTooLarge {
                max_bytes: MAX_SOURCE_IMAGE_BYTES,
                actual_bytes,
            });
        }
        let inspected =
            inspect_bytes(bytes).map_err(|error| SourceAssetImportError::InvalidSourceImage {
                message: error.to_string(),
            })?;
        let project_root = self
            .project_repository
            .get_storage_root(project_id)
            .await
            .map_err(repository_error)?
            .ok_or_else(|| SourceAssetImportError::ProjectStorageMissing {
                project_id: project_id.to_owned(),
            })?;

        let asset_id = AssetId::new();
        let stored = self
            .asset_store
            .write_source_image(&project_root, &asset_id, inspected.extension, bytes)
            .await
            .map_err(|error| SourceAssetImportError::AssetPersistence {
                message: error.to_string(),
            })?;
        let mut stored_paths = vec![stored.path.clone()];
        let thumbnail_path = match generate_thumbnail(bytes) {
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
        let mut asset = match Asset::new_source_image(
            asset_id,
            project_id,
            original_name.clone(),
            original_name,
            stored.path.display().to_string(),
            inspected.sha256,
            inspected.mime_type,
            inspected.width,
            inspected.height,
            bytes.len() as u64,
            json!({"source": "native_import"}),
            created_at,
        ) {
            Ok(asset) => asset,
            Err(error) => {
                self.compensate(&stored_paths).await;
                return Err(SourceAssetImportError::InvalidSourceImage {
                    message: error.to_string(),
                });
            }
        };
        asset.thumbnail_path = thumbnail_path;

        if let Err(error) = self
            .asset_repository
            .insert_many(std::slice::from_ref(&asset))
            .await
        {
            self.compensate(&stored_paths).await;
            return Err(SourceAssetImportError::AssetPersistence {
                message: error.to_string(),
            });
        }

        Ok(asset)
    }

    pub async fn import_video_file(
        &self,
        project_id: &str,
        path: &Path,
    ) -> Result<Asset, SourceAssetImportError> {
        self.import_media_file(project_id, path, SourceMediaKind::Video)
            .await
    }

    pub async fn import_audio_file(
        &self,
        project_id: &str,
        path: &Path,
    ) -> Result<Asset, SourceAssetImportError> {
        self.import_media_file(project_id, path, SourceMediaKind::Audio)
            .await
    }

    async fn import_media_file(
        &self,
        project_id: &str,
        path: &Path,
        kind: SourceMediaKind,
    ) -> Result<Asset, SourceAssetImportError> {
        let original_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| invalid_source_error(kind, "selected file has no usable name"))
            .and_then(|value| safe_file_name_for_kind(value, kind))?;
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .ok_or_else(|| invalid_source_error(kind, "file extension is required"))?;
        let max_bytes = match kind {
            SourceMediaKind::Video => MAX_SOURCE_VIDEO_BYTES,
            SourceMediaKind::Audio => MAX_SOURCE_AUDIO_BYTES,
        };
        let file_size = tokio::fs::metadata(path)
            .await
            .map_err(|error| SourceAssetImportError::AssetPersistence {
                message: format!("inspect {}: {error}", path.display()),
            })?
            .len();
        validate_source_size(kind, max_bytes, file_size)?;

        let prefix = read_source_prefix(path).await.map_err(|error| {
            invalid_source_error(kind, format!("could not inspect file signature: {error}"))
        })?;
        let (storage_extension, mime_type) = validate_source_signature(kind, &extension, &prefix)?;
        let project_root = self
            .project_repository
            .get_storage_root(project_id)
            .await
            .map_err(repository_error)?
            .ok_or_else(|| SourceAssetImportError::ProjectStorageMissing {
                project_id: project_id.to_owned(),
            })?;
        let asset_id = AssetId::new();
        let mut writer = match kind {
            SourceMediaKind::Video => {
                self.asset_store
                    .begin_source_video_write(&project_root, &asset_id, storage_extension)
                    .await
            }
            SourceMediaKind::Audio => {
                self.asset_store
                    .begin_source_audio_write(&project_root, &asset_id, storage_extension)
                    .await
            }
        }
        .map_err(|error| SourceAssetImportError::AssetPersistence {
            message: error.to_string(),
        })?;

        let stream = stream_source_file(path, writer.as_mut(), max_bytes, kind).await;
        let streamed = match stream {
            Ok(streamed) => streamed,
            Err(error) => {
                let _ = writer.abort().await;
                return Err(error);
            }
        };
        let stored = match writer.commit().await {
            Ok(stored) => stored,
            Err(error) => {
                return Err(SourceAssetImportError::AssetPersistence {
                    message: error.to_string(),
                })
            }
        };
        let mut stored_paths = vec![stored.path.clone()];
        let media_probe = CommandMediaProbe::default();
        let (width, height, duration_ms, thumbnail_path) = match kind {
            SourceMediaKind::Video => {
                let metadata = media_probe.probe_video(&stored.path).await;
                let thumbnail_path = if let Some(poster) =
                    media_probe.generate_video_poster(&stored.path).await
                {
                    match self
                        .asset_store
                        .write_video_poster(&project_root, &asset_id, &poster)
                        .await
                    {
                        Ok(stored_poster) => {
                            stored_paths.push(stored_poster.path.clone());
                            Some(stored_poster.path.display().to_string())
                        }
                        Err(error) => {
                            tracing::warn!(asset_id = %asset_id, error = %error, "source video poster write skipped");
                            None
                        }
                    }
                } else {
                    None
                };
                (
                    metadata.width,
                    metadata.height,
                    metadata.duration_ms,
                    thumbnail_path,
                )
            }
            SourceMediaKind::Audio => {
                let metadata = media_probe.probe_audio(&stored.path).await;
                (None, None, metadata.duration_ms, None)
            }
        };
        let metadata = json!({
            "source": "native_import",
            "mediaKind": match kind {
                SourceMediaKind::Video => "video",
                SourceMediaKind::Audio => "audio",
            },
        });
        let asset = match kind {
            SourceMediaKind::Video => Asset::new_source_video(
                asset_id,
                project_id,
                original_name.clone(),
                original_name,
                stored.path.display().to_string(),
                streamed.sha256,
                mime_type,
                width,
                height,
                duration_ms,
                streamed.file_size,
                metadata,
                self.clock.now(),
            ),
            SourceMediaKind::Audio => Asset::new_source_audio(
                asset_id,
                project_id,
                original_name.clone(),
                original_name,
                stored.path.display().to_string(),
                streamed.sha256,
                mime_type,
                duration_ms,
                streamed.file_size,
                metadata,
                self.clock.now(),
            ),
        };
        let mut asset = match asset {
            Ok(asset) => asset,
            Err(error) => {
                self.compensate(&stored_paths).await;
                return Err(invalid_source_error(kind, error.to_string()));
            }
        };
        asset.thumbnail_path = thumbnail_path;
        if let Err(error) = self
            .asset_repository
            .insert_many(std::slice::from_ref(&asset))
            .await
        {
            self.compensate(&stored_paths).await;
            return Err(SourceAssetImportError::AssetPersistence {
                message: error.to_string(),
            });
        }
        Ok(asset)
    }

    async fn compensate(&self, paths: &[std::path::PathBuf]) {
        for path in paths {
            if let Err(error) = self.asset_store.delete(path).await {
                tracing::error!(path = %path.display(), error = %error, "source asset compensation delete failed");
            }
        }
    }
}

fn safe_file_name(value: &str) -> Result<String, SourceAssetImportError> {
    let candidate = std::path::Path::new(value)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .trim();
    if candidate.is_empty() || candidate == "." || candidate == ".." {
        return Err(SourceAssetImportError::InvalidSourceImage {
            message: "original file name is required".to_owned(),
        });
    }
    Ok(candidate.to_owned())
}

fn safe_file_name_for_kind(
    value: &str,
    kind: SourceMediaKind,
) -> Result<String, SourceAssetImportError> {
    safe_file_name(value).map_err(|error| match error {
        SourceAssetImportError::InvalidSourceImage { message } => {
            invalid_source_error(kind, message)
        }
        other => other,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StreamedSourceFile {
    sha256: String,
    file_size: u64,
}

async fn stream_source_file(
    path: &Path,
    writer: &mut dyn AssetWriteSession,
    max_bytes: u64,
    kind: SourceMediaKind,
) -> Result<StreamedSourceFile, SourceAssetImportError> {
    let file = tokio::fs::File::open(path).await.map_err(|error| {
        SourceAssetImportError::AssetPersistence {
            message: format!("open {}: {error}", path.display()),
        }
    })?;
    let mut source = FileSourceReadStream { file };
    stream_source_chunks(&mut source, writer, max_bytes, kind).await
}

async fn stream_source_chunks(
    source: &mut dyn AssetReadStream,
    writer: &mut dyn AssetWriteSession,
    max_bytes: u64,
    kind: SourceMediaKind,
) -> Result<StreamedSourceFile, SourceAssetImportError> {
    let mut hasher = Sha256::new();
    let mut signature = Vec::with_capacity(SOURCE_SIGNATURE_BYTES);
    let mut file_size = 0u64;
    loop {
        let Some(chunk) = source.next_chunk().await.map_err(|error| {
            SourceAssetImportError::AssetPersistence {
                message: format!("read source media: {error}"),
            }
        })?
        else {
            break;
        };
        file_size = file_size.saturating_add(chunk.len() as u64);
        validate_source_size(kind, max_bytes, file_size)?;
        if signature.len() < SOURCE_SIGNATURE_BYTES {
            signature.extend_from_slice(
                &chunk[..chunk.len().min(SOURCE_SIGNATURE_BYTES - signature.len())],
            );
        }
        hasher.update(&chunk);
        writer.write_chunk(&chunk).await.map_err(|error| {
            SourceAssetImportError::AssetPersistence {
                message: error.to_string(),
            }
        })?;
    }
    if file_size == 0 || !signature_matches(kind, &signature) {
        return Err(invalid_source_error(
            kind,
            "file is empty or has an invalid media signature",
        ));
    }
    Ok(StreamedSourceFile {
        sha256: format!("{:x}", hasher.finalize()),
        file_size,
    })
}

struct FileSourceReadStream {
    file: tokio::fs::File,
}

#[async_trait::async_trait]
impl AssetReadStream for FileSourceReadStream {
    async fn next_chunk(
        &mut self,
    ) -> Result<Option<Vec<u8>>, crate::application::ports::AssetStoreError> {
        let mut chunk = vec![0; SOURCE_MEDIA_CHUNK_BYTES];
        let read =
            self.file.read(&mut chunk).await.map_err(|error| {
                crate::application::ports::AssetStoreError::Read(error.to_string())
            })?;
        if read == 0 {
            return Ok(None);
        }
        chunk.truncate(read);
        Ok(Some(chunk))
    }
}

fn validate_source_size(
    kind: SourceMediaKind,
    max_bytes: u64,
    actual_bytes: u64,
) -> Result<(), SourceAssetImportError> {
    if actual_bytes <= max_bytes {
        return Ok(());
    }
    Err(match kind {
        SourceMediaKind::Video => SourceAssetImportError::SourceVideoTooLarge {
            max_bytes,
            actual_bytes,
        },
        SourceMediaKind::Audio => SourceAssetImportError::SourceAudioTooLarge {
            max_bytes,
            actual_bytes,
        },
    })
}

async fn read_source_prefix(path: &Path) -> Result<Vec<u8>, std::io::Error> {
    let mut file = tokio::fs::File::open(path).await?;
    let mut prefix = vec![0; SOURCE_SIGNATURE_BYTES];
    let read = file.read(&mut prefix).await?;
    prefix.truncate(read);
    Ok(prefix)
}

fn validate_source_signature(
    kind: SourceMediaKind,
    extension: &str,
    prefix: &[u8],
) -> Result<(&'static str, &'static str), SourceAssetImportError> {
    let result = match kind {
        SourceMediaKind::Video => match extension {
            "mp4" if is_ftyp(prefix) => Some(("mp4", "video/mp4")),
            "webm" if is_ebml(prefix) => Some(("webm", "video/webm")),
            "mov" if is_ftyp(prefix) => Some(("mov", "video/quicktime")),
            "mkv" if is_ebml(prefix) => Some(("mkv", "video/x-matroska")),
            _ => None,
        },
        SourceMediaKind::Audio => match extension {
            "wav" if is_wav(prefix) => Some(("wav", "audio/wav")),
            "flac" if prefix.starts_with(b"fLaC") => Some(("flac", "audio/flac")),
            "mp3" if is_mp3(prefix) => Some(("mp3", "audio/mpeg")),
            "ogg" if is_ogg(prefix) => Some(("ogg", "audio/ogg")),
            "opus" if is_ogg(prefix) => Some(("opus", "audio/opus")),
            "m4a" if is_ftyp(prefix) => Some(("m4a", "audio/mp4")),
            _ => None,
        },
    };
    result.ok_or_else(|| {
        invalid_source_error(
            kind,
            format!("unsupported extension or media signature: .{extension}"),
        )
    })
}

fn signature_matches(kind: SourceMediaKind, bytes: &[u8]) -> bool {
    match kind {
        SourceMediaKind::Video => is_ftyp(bytes) || is_ebml(bytes),
        SourceMediaKind::Audio => {
            is_wav(bytes)
                || bytes.starts_with(b"fLaC")
                || is_mp3(bytes)
                || is_ogg(bytes)
                || is_ftyp(bytes)
        }
    }
}

fn is_ftyp(bytes: &[u8]) -> bool {
    bytes.get(4..8) == Some(b"ftyp")
}

fn is_ebml(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3])
}

fn is_wav(bytes: &[u8]) -> bool {
    bytes.get(0..4) == Some(b"RIFF") && bytes.get(8..12) == Some(b"WAVE")
}

fn is_mp3(bytes: &[u8]) -> bool {
    bytes.starts_with(b"ID3") || (bytes.len() >= 2 && bytes[0] == 0xff && (bytes[1] & 0xe0) == 0xe0)
}

fn is_ogg(bytes: &[u8]) -> bool {
    bytes.starts_with(b"OggS")
}

fn invalid_source_error(
    kind: SourceMediaKind,
    message: impl Into<String>,
) -> SourceAssetImportError {
    let message = message.into();
    match kind {
        SourceMediaKind::Video => SourceAssetImportError::InvalidSourceVideo { message },
        SourceMediaKind::Audio => SourceAssetImportError::InvalidSourceAudio { message },
    }
}

fn repository_error(error: RepositoryError) -> SourceAssetImportError {
    SourceAssetImportError::AssetPersistence {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        stream_source_chunks, validate_source_signature, validate_source_size,
        SourceAssetImportError, SourceAssetImportService, SourceMediaKind, MAX_SOURCE_AUDIO_BYTES,
        MAX_SOURCE_IMAGE_BYTES, MAX_SOURCE_VIDEO_BYTES,
    };
    use crate::application::ports::{
        AssetReadStream, AssetRepository, AssetStoreError, AssetWriteSession, Clock, ProjectRecord,
        ProjectRepository, RepositoryError, StoredAssetFile,
    };
    use crate::domain::{Asset, AssetId, TaskId};
    use crate::infrastructure::filesystem::FileSystemAssetStore;
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
    use std::io::Cursor;
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
                return Err(RepositoryError::database(
                    "forced source asset database failure",
                ));
            }
            self.assets.lock().unwrap().extend_from_slice(assets);
            Ok(())
        }
        async fn find_by_id(&self, id: &AssetId) -> Result<Option<Asset>, RepositoryError> {
            Ok(self
                .assets
                .lock()
                .unwrap()
                .iter()
                .find(|asset| asset.id == *id)
                .cloned())
        }
        async fn list_by_source_task(&self, id: &TaskId) -> Result<Vec<Asset>, RepositoryError> {
            Ok(self
                .assets
                .lock()
                .unwrap()
                .iter()
                .filter(|asset| asset.source_task_id.as_ref() == Some(id))
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
    struct FixedClock;
    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            Utc::now()
        }
    }

    fn png() -> Vec<u8> {
        let image = RgbImage::from_pixel(2, 3, Rgb([10, 20, 30]));
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(image)
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }

    fn mp4_prefix() -> Vec<u8> {
        vec![0, 0, 0, 24, b'f', b't', b'y', b'p', b'm', b'p', b'4', b'2']
    }

    fn wav_prefix() -> Vec<u8> {
        let mut bytes = vec![0; 44];
        bytes[0..4].copy_from_slice(b"RIFF");
        bytes[8..12].copy_from_slice(b"WAVE");
        bytes
    }

    fn service(root: &Path, repository: FakeAssetRepository) -> SourceAssetImportService {
        SourceAssetImportService::new(
            Arc::new(FakeProjectRepository {
                root: root.to_path_buf(),
            }),
            Arc::new(FileSystemAssetStore::new()),
            Arc::new(repository),
            Arc::new(FixedClock),
        )
    }

    #[tokio::test]
    async fn imports_source_image_without_persisting_source_task_or_absolute_input_path() {
        let root = tempdir().unwrap();
        let repository = FakeAssetRepository::default();
        let asset = service(root.path(), repository.clone())
            .import_bytes("project-1", r"C:\\Users\\me\\photo.png", &png())
            .await
            .unwrap();

        assert_eq!(asset.category, "source_image");
        assert!(asset.source_task_id.is_none());
        assert!(
            asset.storage_path.ends_with("assets/source/image")
                || asset.storage_path.contains("assets\\source\\image")
        );
        assert_eq!(asset.original_name, "photo.png");
        assert_eq!(repository.assets.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn rejects_invalid_image_and_oversized_input() {
        let root = tempdir().unwrap();
        let repository = FakeAssetRepository::default();
        let invalid = service(root.path(), repository.clone())
            .import_bytes("project-1", "bad.png", b"not-image")
            .await
            .unwrap_err();
        assert!(matches!(
            invalid,
            SourceAssetImportError::InvalidSourceImage { .. }
        ));

        let oversized = service(root.path(), repository)
            .import_bytes(
                "project-1",
                "big.png",
                &vec![0; (MAX_SOURCE_IMAGE_BYTES + 1) as usize],
            )
            .await
            .unwrap_err();
        assert!(matches!(
            oversized,
            SourceAssetImportError::SourceImageTooLarge { .. }
        ));
    }

    #[tokio::test]
    async fn imports_jpeg_and_webp_from_actual_bytes() {
        let root = tempdir().unwrap();
        for (extension, format) in [("jpg", ImageFormat::Jpeg), ("webp", ImageFormat::WebP)] {
            let image = RgbImage::from_pixel(2, 2, Rgb([30, 40, 50]));
            let mut bytes = Cursor::new(Vec::new());
            DynamicImage::ImageRgb8(image)
                .write_to(&mut bytes, format)
                .unwrap();
            let asset = service(root.path(), FakeAssetRepository::default())
                .import_bytes(
                    "project-1",
                    &format!("reference.{extension}"),
                    &bytes.into_inner(),
                )
                .await
                .unwrap();
            assert_eq!(
                asset.mime_type,
                if extension == "jpg" {
                    "image/jpeg"
                } else {
                    "image/webp"
                }
            );
            assert!(asset.storage_path.ends_with(&format!(".{extension}")));
        }
    }

    #[tokio::test]
    async fn database_failure_removes_only_new_source_file() {
        let root = tempdir().unwrap();
        let repository = FakeAssetRepository {
            assets: Arc::new(Mutex::new(Vec::new())),
            fail: true,
        };
        let error = service(root.path(), repository)
            .import_bytes("project-1", "reference.png", &png())
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            SourceAssetImportError::AssetPersistence { .. }
        ));
        let directory = root.path().join("assets/source/image");
        assert!(directory.is_dir());
        assert_eq!(std::fs::read_dir(directory).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn imports_source_video_and_audio_without_source_task_or_absolute_input_path() {
        let root = tempdir().unwrap();
        let repository = FakeAssetRepository::default();
        let video_path = root.path().join("reference.mp4");
        std::fs::write(&video_path, mp4_prefix()).unwrap();
        let audio_path = root.path().join("reference.wav");
        std::fs::write(&audio_path, wav_prefix()).unwrap();

        let importer = service(root.path(), repository.clone());
        let video = importer
            .import_video_file("project-1", &video_path)
            .await
            .unwrap();
        let audio = importer
            .import_audio_file("project-1", &audio_path)
            .await
            .unwrap();

        assert_eq!(video.category, "source_video");
        assert_eq!(video.asset_type.as_str(), "video");
        assert!(video.source_task_id.is_none());
        assert!(
            video.storage_path.contains("assets/source/video")
                || video.storage_path.contains("assets\\source\\video")
        );
        assert_eq!(audio.category, "source_audio");
        assert_eq!(audio.asset_type.as_str(), "audio");
        assert!(audio.source_task_id.is_none());
        assert!(
            audio.storage_path.contains("assets/source/audio")
                || audio.storage_path.contains("assets\\source\\audio")
        );
        assert!(!video
            .storage_path
            .contains(video_path.to_string_lossy().as_ref()));
        assert!(!audio
            .storage_path
            .contains(audio_path.to_string_lossy().as_ref()));
        assert_eq!(repository.assets.lock().unwrap().len(), 2);
    }

    #[test]
    fn accepts_all_declared_source_media_signatures_and_rejects_cross_kind_files() {
        for (extension, prefix) in [
            ("mp4", mp4_prefix()),
            ("webm", vec![0x1a, 0x45, 0xdf, 0xa3]),
            ("mov", mp4_prefix()),
            ("mkv", vec![0x1a, 0x45, 0xdf, 0xa3]),
        ] {
            assert!(validate_source_signature(SourceMediaKind::Video, extension, &prefix).is_ok());
        }
        for (extension, prefix) in [
            ("wav", wav_prefix()),
            ("flac", b"fLaC".to_vec()),
            ("mp3", b"ID3".to_vec()),
            ("ogg", b"OggS".to_vec()),
            ("opus", b"OggS".to_vec()),
            ("m4a", mp4_prefix()),
        ] {
            assert!(validate_source_signature(SourceMediaKind::Audio, extension, &prefix).is_ok());
        }
        assert!(matches!(
            validate_source_signature(SourceMediaKind::Video, "mp4", &wav_prefix()),
            Err(SourceAssetImportError::InvalidSourceVideo { .. })
        ));
        assert!(matches!(
            validate_source_signature(SourceMediaKind::Audio, "wav", &mp4_prefix()),
            Err(SourceAssetImportError::InvalidSourceAudio { .. })
        ));
    }

    #[test]
    fn oversized_media_is_rejected_before_streaming_without_a_large_fixture() {
        assert!(matches!(
            validate_source_size(
                SourceMediaKind::Video,
                MAX_SOURCE_VIDEO_BYTES,
                MAX_SOURCE_VIDEO_BYTES + 1
            ),
            Err(SourceAssetImportError::SourceVideoTooLarge { .. })
        ));
        assert!(matches!(
            validate_source_size(
                SourceMediaKind::Audio,
                MAX_SOURCE_AUDIO_BYTES,
                MAX_SOURCE_AUDIO_BYTES + 1
            ),
            Err(SourceAssetImportError::SourceAudioTooLarge { .. })
        ));
    }

    #[derive(Default)]
    struct CountingWriteSession {
        total: u64,
        max_chunk: usize,
    }

    #[async_trait]
    impl AssetWriteSession for CountingWriteSession {
        async fn write_chunk(&mut self, bytes: &[u8]) -> Result<(), AssetStoreError> {
            self.total += bytes.len() as u64;
            self.max_chunk = self.max_chunk.max(bytes.len());
            Ok(())
        }

        async fn commit(self: Box<Self>) -> Result<StoredAssetFile, AssetStoreError> {
            unreachable!("the stream test does not publish a file")
        }

        async fn abort(self: Box<Self>) -> Result<(), AssetStoreError> {
            Ok(())
        }
    }

    struct LogicalVideoStream {
        remaining_chunks: usize,
        chunk_size: usize,
        first: bool,
    }

    #[async_trait]
    impl AssetReadStream for LogicalVideoStream {
        async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, AssetStoreError> {
            if self.remaining_chunks == 0 {
                return Ok(None);
            }
            self.remaining_chunks -= 1;
            let mut chunk = vec![0; self.chunk_size];
            if self.first {
                self.first = false;
                chunk[4..8].copy_from_slice(b"ftyp");
            }
            Ok(Some(chunk))
        }
    }

    #[tokio::test]
    async fn processes_a_512_mib_logical_source_in_bounded_chunks() {
        let chunk_size = 1024 * 1024;
        let mut source = LogicalVideoStream {
            remaining_chunks: 512,
            chunk_size,
            first: true,
        };
        let mut writer = CountingWriteSession::default();
        let streamed = stream_source_chunks(
            &mut source,
            &mut writer,
            MAX_SOURCE_VIDEO_BYTES,
            SourceMediaKind::Video,
        )
        .await
        .unwrap();
        assert_eq!(streamed.file_size, 512 * 1024 * 1024);
        assert_eq!(writer.total, streamed.file_size);
        assert_eq!(writer.max_chunk, chunk_size);
    }

    #[tokio::test]
    async fn media_database_failure_removes_only_new_media_file() {
        let root = tempdir().unwrap();
        let repository = FakeAssetRepository {
            assets: Arc::new(Mutex::new(Vec::new())),
            fail: true,
        };
        let path = root.path().join("reference.mp4");
        std::fs::write(&path, mp4_prefix()).unwrap();
        let error = service(root.path(), repository)
            .import_video_file("project-1", &path)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            SourceAssetImportError::AssetPersistence { .. }
        ));
        let directory = root.path().join("assets/source/video");
        assert!(directory.is_dir());
        assert_eq!(std::fs::read_dir(directory).unwrap().count(), 0);
    }
}
