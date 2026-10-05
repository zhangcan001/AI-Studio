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

/// Internal typed inspection; paths never cross the Product DTO boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssetReadInspection {
    Present { canonical_path: PathBuf },
    Missing,
    Unreadable,
    UnsafePath,
}

/// Shared path policy for all reads and readonly integrity inspection. Resolve
/// the nearest existing ancestor before classifying a missing managed file.
fn resolve_asset_read_path(
    project_root: &Path,
    candidate: &Path,
) -> Result<PathBuf, AssetReadInspection> {
    use AssetReadInspection::*;
    if project_root.as_os_str().is_empty()
        || candidate.as_os_str().is_empty()
        || candidate
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(UnsafePath);
    }
    if fs::symlink_metadata(project_root)
        .map_err(|_| UnsafePath)?
        .file_type()
        .is_symlink()
    {
        return Err(UnsafePath);
    }
    let root = fs::canonicalize(project_root).map_err(|_| UnsafePath)?;
    let lexical = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    };
    let mut ancestor = lexical.as_path();
    let mut missing = false;
    let metadata = loop {
        match fs::symlink_metadata(ancestor) {
            Ok(m) => break m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                missing = true;
                ancestor = ancestor.parent().ok_or(UnsafePath)?;
            }
            Err(_) => return Err(UnsafePath),
        }
    };
    if metadata.file_type().is_symlink() {
        return Err(UnsafePath);
    }
    let canonical = fs::canonicalize(ancestor).map_err(|_| UnsafePath)?;
    if !canonical.starts_with(&root) {
        return Err(UnsafePath);
    }
    // Reject links in intermediate components too, including links resolving
    // back inside the root. No alternative inspection-only path rules.
    let mut parent = if canonical == root {
        None
    } else {
        ancestor.parent()
    };
    while let Some(p) = parent {
        let resolved = fs::canonicalize(p).map_err(|_| UnsafePath)?;
        if fs::symlink_metadata(p)
            .map_err(|_| UnsafePath)?
            .file_type()
            .is_symlink()
        {
            return Err(UnsafePath);
        }
        if resolved == root {
            break;
        }
        if !resolved.starts_with(&root) {
            return Err(UnsafePath);
        }
        parent = p.parent();
    }
    if missing {
        return Err(Missing);
    }
    if !metadata.is_file() {
        return Err(UnsafePath);
    }
    Ok(canonical)
}

pub fn validate_asset_read_path(
    project_root: &Path,
    candidate: &Path,
) -> Result<PathBuf, AssetStoreError> {
    resolve_asset_read_path(project_root, candidate).map_err(|outcome| match outcome {
        AssetReadInspection::UnsafePath => {
            AssetStoreError::FilesystemBoundary("unsafe managed asset path".into())
        }
        _ => AssetStoreError::Read("managed asset unavailable".into()),
    })
}

pub fn inspect_asset_read_path(project_root: &Path, candidate: &Path) -> AssetReadInspection {
    match resolve_asset_read_path(project_root, candidate) {
        Ok(canonical_path) => match fs::File::open(&canonical_path) {
            Ok(_) => AssetReadInspection::Present { canonical_path },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => AssetReadInspection::Missing,
            Err(_) => AssetReadInspection::Unreadable,
        },
        Err(outcome) => outcome,
    }
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

    async fn begin_audio_write(
        &self,
        _project_root: &Path,
        _asset_id: &AssetId,
        _extension: &str,
    ) -> Result<Box<dyn AssetWriteSession>, AssetStoreError> {
        Err(AssetStoreError::Write(
            "generated audio streaming storage is not available".to_owned(),
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

    /// Default is fail closed; adapters/fakes must explicitly support inspection.
    async fn inspect_read(&self, _project_root: &Path, _path: &Path) -> AssetReadInspection {
        AssetReadInspection::UnsafePath
    }

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

#[cfg(test)]
mod inspection_tests {
    use super::*;
    #[test]
    fn inspection_keeps_missing_inside_root_distinct_and_rejects_unsafe_missing_paths() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(
            inspect_asset_read_path(root.path(), Path::new("missing/file.png")),
            AssetReadInspection::Missing
        );
        assert_eq!(
            inspect_asset_read_path(root.path(), Path::new("../outside.png")),
            AssetReadInspection::UnsafePath
        );
        let outside = tempfile::tempdir().unwrap();
        assert_eq!(
            inspect_asset_read_path(root.path(), &outside.path().join("missing.png")),
            AssetReadInspection::UnsafePath
        );
        assert_eq!(
            inspect_asset_read_path(root.path(), root.path()),
            AssetReadInspection::UnsafePath
        );
        let file = root.path().join("managed.png");
        fs::write(&file, b"managed").unwrap();
        let AssetReadInspection::Present { canonical_path } =
            inspect_asset_read_path(root.path(), &file)
        else {
            panic!("safe file")
        };
        assert_eq!(
            canonical_path,
            validate_asset_read_path(root.path(), &file).unwrap()
        );
    }
    #[test]
    fn inspection_rejects_symlinks_including_in_root_aliases_when_platform_permits() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("real.bin");
        fs::write(&file, b"owned").unwrap();
        let link = root.path().join("alias.bin");
        #[cfg(unix)]
        let result = std::os::unix::fs::symlink(&file, &link);
        #[cfg(windows)]
        let result = std::os::windows::fs::symlink_file(&file, &link);
        if let Err(e) = result {
            #[cfg(windows)]
            {
                assert_eq!(e.kind(), std::io::ErrorKind::PermissionDenied);
                eprintln!("SYMLINK_FIXTURE=NOT_AVAILABLE_WINDOWS_PERMISSION; production rejection unchanged");
                return;
            }
            #[cfg(not(windows))]
            panic!("owned symlink fixture failed: {e}");
        }
        assert_eq!(
            inspect_asset_read_path(root.path(), &link),
            AssetReadInspection::UnsafePath
        );
        assert!(matches!(
            validate_asset_read_path(root.path(), &link),
            Err(AssetStoreError::FilesystemBoundary(_))
        ));
        let real = root.path().join("real");
        fs::create_dir(&real).unwrap();
        fs::write(real.join("file.bin"), b"owned").unwrap();
        let dir_link = root.path().join("alias-dir");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real, &dir_link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&real, &dir_link).unwrap();
        assert_eq!(
            inspect_asset_read_path(root.path(), &dir_link.join("file.bin")),
            AssetReadInspection::UnsafePath
        );
        assert_eq!(
            inspect_asset_read_path(root.path(), &dir_link.join("missing.bin")),
            AssetReadInspection::UnsafePath
        );
    }
}
