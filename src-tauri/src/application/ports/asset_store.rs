use crate::domain::{Asset, AssetId};
use async_trait::async_trait;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredAssetFile {
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StagedAssetFile {
    pub original_path: PathBuf,
    pub staged_path: PathBuf,
}

#[async_trait]
pub trait AssetWriteSession: Send {
    async fn write_chunk(&mut self, bytes: &[u8]) -> Result<(), AssetStoreError>;
    async fn commit(self: Box<Self>) -> Result<StoredAssetFile, AssetStoreError>;
    async fn abort(self: Box<Self>) -> Result<(), AssetStoreError>;
}

#[async_trait]
pub trait AssetReadStream: Send {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, AssetStoreError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssetStoreError {
    InvalidPath(String),
    FilesystemBoundary(String),
    Write(String),
    Delete(String),
    Read(String),
}

impl std::fmt::Display for AssetStoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPath(message) => write!(formatter, "invalid asset path: {message}"),
            Self::FilesystemBoundary(message) => {
                write!(formatter, "asset filesystem boundary error: {message}")
            }
            Self::Write(message) => write!(formatter, "asset write failed: {message}"),
            Self::Delete(message) => write!(formatter, "asset delete failed: {message}"),
            Self::Read(message) => write!(formatter, "asset read failed: {message}"),
        }
    }
}

impl std::error::Error for AssetStoreError {}

/// Resolve an asset path and enforce that the existing regular file remains
/// inside the owning project's canonical storage root. This is shared by the
/// application boundary and filesystem adapter so HEAD/GET/stream reads use
/// the same policy.
pub fn validate_asset_read_path(
    project_root: &Path,
    candidate: &Path,
) -> Result<PathBuf, AssetStoreError> {
    if project_root.as_os_str().is_empty() || candidate.as_os_str().is_empty() {
        return Err(AssetStoreError::InvalidPath(
            "project root and asset path are required".to_owned(),
        ));
    }
    if candidate
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(AssetStoreError::FilesystemBoundary(format!(
            "path traversal is not allowed: {}",
            candidate.display()
        )));
    }

    let root = fs::canonicalize(project_root).map_err(|error| {
        AssetStoreError::FilesystemBoundary(format!(
            "canonicalize project root {}: {error}",
            project_root.display()
        ))
    })?;
    let lexical = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    };
    let metadata = fs::symlink_metadata(&lexical).map_err(|error| {
        AssetStoreError::Read(format!("inspect {}: {error}", lexical.display()))
    })?;
    if metadata.file_type().is_symlink() {
        return Err(AssetStoreError::FilesystemBoundary(format!(
            "symbolic links are not allowed: {}",
            lexical.display()
        )));
    }
    if !metadata.is_file() {
        return Err(AssetStoreError::FilesystemBoundary(format!(
            "asset path is not a regular file: {}",
            lexical.display()
        )));
    }
    let canonical = fs::canonicalize(&lexical).map_err(|error| {
        AssetStoreError::Read(format!("canonicalize {}: {error}", lexical.display()))
    })?;
    if !canonical.starts_with(&root) {
        return Err(AssetStoreError::FilesystemBoundary(format!(
            "asset path is outside project root: {}",
            lexical.display()
        )));
    }
    Ok(canonical)
}

#[async_trait]
pub trait AssetStore: Send + Sync {
    async fn write_image(
        &self,
        project_root: &Path,
        asset_id: &AssetId,
        extension: &str,
        bytes: &[u8],
    ) -> Result<StoredAssetFile, AssetStoreError>;

    async fn write_source_image(
        &self,
        project_root: &Path,
        asset_id: &AssetId,
        extension: &str,
        bytes: &[u8],
    ) -> Result<StoredAssetFile, AssetStoreError> {
        self.write_image(project_root, asset_id, extension, bytes)
            .await
    }

    async fn write_thumbnail(
        &self,
        _project_root: &Path,
        _asset_id: &AssetId,
        _bytes: &[u8],
    ) -> Result<StoredAssetFile, AssetStoreError> {
        Err(AssetStoreError::Write(
            "thumbnail storage is not available".to_owned(),
        ))
    }

    async fn begin_video_write(
        &self,
        _project_root: &Path,
        _asset_id: &AssetId,
        _extension: &str,
    ) -> Result<Box<dyn AssetWriteSession>, AssetStoreError> {
        Err(AssetStoreError::Write(
            "video streaming storage is not available".to_owned(),
        ))
    }

    async fn begin_source_video_write(
        &self,
        _project_root: &Path,
        _asset_id: &AssetId,
        _extension: &str,
    ) -> Result<Box<dyn AssetWriteSession>, AssetStoreError> {
        Err(AssetStoreError::Write(
            "source video streaming storage is not available".to_owned(),
        ))
    }

    async fn begin_source_audio_write(
        &self,
        _project_root: &Path,
        _asset_id: &AssetId,
        _extension: &str,
    ) -> Result<Box<dyn AssetWriteSession>, AssetStoreError> {
        Err(AssetStoreError::Write(
            "source audio streaming storage is not available".to_owned(),
        ))
    }

    async fn write_video_poster(
        &self,
        _project_root: &Path,
        _asset_id: &AssetId,
        _bytes: &[u8],
    ) -> Result<StoredAssetFile, AssetStoreError> {
        Err(AssetStoreError::Write(
            "video poster storage is not available".to_owned(),
        ))
    }

    async fn delete(&self, path: &Path) -> Result<(), AssetStoreError>;

    async fn validate_delete_paths(
        &self,
        _project_root: &Path,
        _asset: &Asset,
    ) -> Result<(), AssetStoreError> {
        Ok(())
    }

    async fn stage_for_delete(
        &self,
        _project_root: &Path,
        _operation_id: &str,
        _asset: &Asset,
    ) -> Result<Vec<StagedAssetFile>, AssetStoreError> {
        Err(AssetStoreError::Delete(
            "transactional asset deletion is not available".to_owned(),
        ))
    }

    async fn restore_staged_delete(
        &self,
        _staged: &[StagedAssetFile],
    ) -> Result<(), AssetStoreError> {
        Err(AssetStoreError::Delete(
            "transactional asset restore is not available".to_owned(),
        ))
    }

    async fn commit_staged_delete(
        &self,
        _staged: &[StagedAssetFile],
    ) -> Result<(), AssetStoreError> {
        Err(AssetStoreError::Delete(
            "transactional asset cleanup is not available".to_owned(),
        ))
    }

    /// Read a stored asset only after resolving it against its owning project
    /// root. Implementations must enforce the same boundary for the actual
    /// filesystem operation, not just rely on callers to preflight the path.
    async fn read(&self, project_root: &Path, path: &Path) -> Result<Vec<u8>, AssetStoreError>;

    async fn open_read_stream(
        &self,
        _project_root: &Path,
        _path: &Path,
    ) -> Result<Box<dyn AssetReadStream>, AssetStoreError> {
        Err(AssetStoreError::Read(
            "streaming asset reads are not available".to_owned(),
        ))
    }

    async fn read_range(
        &self,
        _project_root: &Path,
        _path: &Path,
        _offset: u64,
        _length: u64,
    ) -> Result<Vec<u8>, AssetStoreError> {
        Err(AssetStoreError::Read(
            "bounded range reads are not available".to_owned(),
        ))
    }
}
