use crate::application::ports::{
    AssetReadStream, AssetStore, AssetStoreError, AssetWriteSession, StagedAssetFile,
    StoredAssetFile,
};
use crate::domain::{Asset, AssetId};
use async_trait::async_trait;
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
use tokio::io::AsyncReadExt;

const ASSET_READ_CHUNK_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Default)]
pub struct FileSystemAssetStore;

impl FileSystemAssetStore {
    pub fn new() -> Self {
        Self
    }

    fn target_path(
        project_root: &Path,
        asset_id: &AssetId,
        extension: &str,
        category: &str,
        media_type: &str,
    ) -> Result<PathBuf, AssetStoreError> {
        if project_root.as_os_str().is_empty()
            || extension.is_empty()
            || category.is_empty()
            || !extension
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
        {
            return Err(AssetStoreError::InvalidPath(
                "project root and a simple file extension are required".to_owned(),
            ));
        }

        Ok(project_root
            .join("assets")
            .join(category)
            .join(media_type)
            .join(format!("{}.{}", asset_id.as_str(), extension)))
    }

    async fn write_to_path(
        project_root: &Path,
        asset_id: &AssetId,
        extension: &str,
        category: &str,
        media_type: &str,
        bytes: &[u8],
    ) -> Result<StoredAssetFile, AssetStoreError> {
        let target = Self::target_path(project_root, asset_id, extension, category, media_type)?;
        let parent = target.parent().ok_or_else(|| {
            AssetStoreError::InvalidPath("asset target has no parent directory".to_owned())
        })?;
        fs::create_dir_all(parent).map_err(|error| {
            AssetStoreError::Write(format!("create {}: {error}", parent.display()))
        })?;

        if target.exists() {
            return Err(AssetStoreError::Write(format!(
                "asset target already exists: {}",
                target.display()
            )));
        }

        let temporary = parent.join(format!(".{}.tmp", asset_id.as_str()));
        let write_result = (|| {
            let mut file = fs::File::create(&temporary)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            fs::rename(&temporary, &target)?;
            Ok::<(), std::io::Error>(())
        })();

        if let Err(error) = write_result {
            let _ = fs::remove_file(&temporary);
            return Err(AssetStoreError::Write(format!(
                "publish {}: {error}",
                target.display()
            )));
        }

        Ok(StoredAssetFile { path: target })
    }
}

struct FileSystemVideoWriteSession {
    temporary: PathBuf,
    target: PathBuf,
    file: Option<fs::File>,
}

struct FileSystemAssetReadStream {
    file: tokio::fs::File,
}

#[async_trait]
impl AssetReadStream for FileSystemAssetReadStream {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, AssetStoreError> {
        let mut chunk = vec![0; ASSET_READ_CHUNK_BYTES];
        let read = self
            .file
            .read(&mut chunk)
            .await
            .map_err(|error| AssetStoreError::Read(format!("read asset stream: {error}")))?;
        if read == 0 {
            return Ok(None);
        }
        chunk.truncate(read);
        Ok(Some(chunk))
    }
}

#[async_trait]
impl AssetWriteSession for FileSystemVideoWriteSession {
    async fn write_chunk(&mut self, bytes: &[u8]) -> Result<(), AssetStoreError> {
        self.file
            .as_mut()
            .ok_or_else(|| AssetStoreError::Write("video writer is already closed".to_owned()))?
            .write_all(bytes)
            .map_err(|error| AssetStoreError::Write(format!("write video chunk: {error}")))
    }

    async fn commit(mut self: Box<Self>) -> Result<StoredAssetFile, AssetStoreError> {
        let mut file = self
            .file
            .take()
            .ok_or_else(|| AssetStoreError::Write("video writer is already closed".to_owned()))?;
        let result = (|| {
            file.flush()?;
            file.sync_all()?;
            fs::rename(&self.temporary, &self.target)?;
            Ok::<(), std::io::Error>(())
        })();
        if let Err(error) = result {
            let _ = fs::remove_file(&self.temporary);
            return Err(AssetStoreError::Write(format!(
                "publish {}: {error}",
                self.target.display()
            )));
        }
        Ok(StoredAssetFile {
            path: self.target.clone(),
        })
    }

    async fn abort(mut self: Box<Self>) -> Result<(), AssetStoreError> {
        self.file.take();
        match fs::remove_file(&self.temporary) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(AssetStoreError::Delete(format!(
                "remove {}: {error}",
                self.temporary.display()
            ))),
        }
    }
}

impl Drop for FileSystemVideoWriteSession {
    fn drop(&mut self) {
        if self.file.is_some() {
            let _ = fs::remove_file(&self.temporary);
        }
    }
}

#[async_trait]
impl AssetStore for FileSystemAssetStore {
    async fn write_image(
        &self,
        project_root: &Path,
        asset_id: &AssetId,
        extension: &str,
        bytes: &[u8],
    ) -> Result<StoredAssetFile, AssetStoreError> {
        Self::write_to_path(
            project_root,
            asset_id,
            extension,
            "generated",
            "image",
            bytes,
        )
        .await
    }

    async fn write_source_image(
        &self,
        project_root: &Path,
        asset_id: &AssetId,
        extension: &str,
        bytes: &[u8],
    ) -> Result<StoredAssetFile, AssetStoreError> {
        Self::write_to_path(project_root, asset_id, extension, "source", "image", bytes).await
    }

    async fn write_thumbnail(
        &self,
        project_root: &Path,
        asset_id: &AssetId,
        bytes: &[u8],
    ) -> Result<StoredAssetFile, AssetStoreError> {
        Self::write_to_path(project_root, asset_id, "png", "thumbnails", "image", bytes).await
    }

    async fn begin_video_write(
        &self,
        project_root: &Path,
        asset_id: &AssetId,
        extension: &str,
    ) -> Result<Box<dyn AssetWriteSession>, AssetStoreError> {
        Self::begin_stream_write(project_root, asset_id, extension, "generated", "video").await
    }

    async fn begin_source_video_write(
        &self,
        project_root: &Path,
        asset_id: &AssetId,
        extension: &str,
    ) -> Result<Box<dyn AssetWriteSession>, AssetStoreError> {
        Self::begin_stream_write(project_root, asset_id, extension, "source", "video").await
    }

    async fn begin_source_audio_write(
        &self,
        project_root: &Path,
        asset_id: &AssetId,
        extension: &str,
    ) -> Result<Box<dyn AssetWriteSession>, AssetStoreError> {
        Self::begin_stream_write(project_root, asset_id, extension, "source", "audio").await
    }

    async fn open_read_stream(
        &self,
        path: &Path,
    ) -> Result<Box<dyn AssetReadStream>, AssetStoreError> {
        let file = tokio::fs::File::open(path)
            .await
            .map_err(|error| AssetStoreError::Read(format!("open {}: {error}", path.display())))?;
        Ok(Box::new(FileSystemAssetReadStream { file }))
    }

    async fn write_video_poster(
        &self,
        project_root: &Path,
        asset_id: &AssetId,
        bytes: &[u8],
    ) -> Result<StoredAssetFile, AssetStoreError> {
        Self::write_to_path(project_root, asset_id, "png", "thumbnails", "video", bytes).await
    }

    async fn delete(&self, path: &Path) -> Result<(), AssetStoreError> {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(AssetStoreError::Delete(format!(
                "remove {}: {error}",
                path.display()
            ))),
        }
    }

    async fn validate_delete_paths(
        &self,
        project_root: &Path,
        asset: &Asset,
    ) -> Result<(), AssetStoreError> {
        let root = Self::canonical_root(project_root)?;
        let mut paths = BTreeSet::new();
        paths.insert(PathBuf::from(&asset.storage_path));
        if let Some(thumbnail_path) = asset.thumbnail_path.as_deref() {
            if !thumbnail_path.trim().is_empty() {
                paths.insert(PathBuf::from(thumbnail_path));
            }
        }
        for path in paths {
            Self::validated_delete_path(&root, &path)?;
        }
        Ok(())
    }

    async fn stage_for_delete(
        &self,
        project_root: &Path,
        operation_id: &str,
        asset: &Asset,
    ) -> Result<Vec<StagedAssetFile>, AssetStoreError> {
        let root = Self::canonical_root(project_root)?;
        Self::validate_operation_id(operation_id)?;
        let mut staged = Vec::new();
        let mut paths = BTreeSet::new();
        paths.insert(PathBuf::from(&asset.storage_path));
        if let Some(thumbnail_path) = asset.thumbnail_path.as_deref() {
            if !thumbnail_path.trim().is_empty() {
                paths.insert(PathBuf::from(thumbnail_path));
            }
        }

        let trash_root = root.join(".project-trash").join(operation_id);
        for path in paths {
            let Some(original) = Self::validated_delete_path(&root, &path)? else {
                continue;
            };
            let relative = original.strip_prefix(&root).map_err(|_| {
                AssetStoreError::FilesystemBoundary(format!(
                    "asset path is outside project root: {}",
                    original.display()
                ))
            })?;
            let staged_path = trash_root.join(relative);
            if let Some(parent) = staged_path.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    AssetStoreError::Delete(format!(
                        "create trash directory {}: {error}",
                        parent.display()
                    ))
                })?;
            }
            if let Err(error) = fs::rename(&original, &staged_path) {
                let restore_error = Self::restore_staged_files(&staged);
                if let Err(restore_error) = restore_error {
                    return Err(AssetStoreError::Delete(format!(
                        "stage {} failed: {error}; restore failed: {restore_error}",
                        original.display()
                    )));
                }
                return Err(AssetStoreError::Delete(format!(
                    "stage {} failed: {error}",
                    original.display()
                )));
            }
            staged.push(StagedAssetFile {
                original_path: original,
                staged_path,
            });
        }
        Ok(staged)
    }

    async fn restore_staged_delete(
        &self,
        staged: &[StagedAssetFile],
    ) -> Result<(), AssetStoreError> {
        Self::restore_staged_files(staged)
            .map_err(|error| AssetStoreError::Delete(format!("restore staged assets: {error}")))
    }

    async fn commit_staged_delete(
        &self,
        staged: &[StagedAssetFile],
    ) -> Result<(), AssetStoreError> {
        for file in staged {
            match fs::remove_file(&file.staged_path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(AssetStoreError::Delete(format!(
                        "remove staged file {}: {error}",
                        file.staged_path.display()
                    )))
                }
            }
        }
        let mut directories = BTreeSet::new();
        for file in staged {
            let mut current = file.staged_path.parent();
            while let Some(directory) = current {
                if directory.file_name().and_then(|name| name.to_str()) == Some(".project-trash") {
                    break;
                }
                directories.insert(directory.to_path_buf());
                current = directory.parent();
            }
        }
        for directory in directories.into_iter().rev() {
            match fs::remove_dir(&directory) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) if error.kind() == std::io::ErrorKind::DirectoryNotEmpty => {}
                Err(error) => {
                    return Err(AssetStoreError::Delete(format!(
                        "remove trash directory {}: {error}",
                        directory.display()
                    )))
                }
            }
        }
        Ok(())
    }

    async fn read(&self, path: &Path) -> Result<Vec<u8>, AssetStoreError> {
        fs::read(path)
            .map_err(|error| AssetStoreError::Read(format!("read {}: {error}", path.display())))
    }

    async fn read_range(
        &self,
        path: &Path,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, AssetStoreError> {
        let mut file = fs::File::open(path)
            .map_err(|error| AssetStoreError::Read(format!("open {}: {error}", path.display())))?;
        file.seek(SeekFrom::Start(offset))
            .map_err(|error| AssetStoreError::Read(format!("seek {}: {error}", path.display())))?;
        let length = usize::try_from(length)
            .map_err(|_| AssetStoreError::Read("range length is too large".to_owned()))?;
        let mut bytes = vec![0; length];
        let read = file.read(&mut bytes).map_err(|error| {
            AssetStoreError::Read(format!("read range {}: {error}", path.display()))
        })?;
        bytes.truncate(read);
        Ok(bytes)
    }
}

impl FileSystemAssetStore {
    fn canonical_root(project_root: &Path) -> Result<PathBuf, AssetStoreError> {
        if project_root.as_os_str().is_empty() {
            return Err(AssetStoreError::InvalidPath(
                "project root is required".to_owned(),
            ));
        }
        fs::canonicalize(project_root).map_err(|error| {
            AssetStoreError::Delete(format!(
                "canonicalize project root {}: {error}",
                project_root.display()
            ))
        })
    }

    fn validate_operation_id(operation_id: &str) -> Result<(), AssetStoreError> {
        if operation_id.is_empty()
            || !operation_id.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            })
        {
            return Err(AssetStoreError::FilesystemBoundary(
                "delete operation id must be a simple path component".to_owned(),
            ));
        }
        Ok(())
    }

    fn validated_delete_path(
        root: &Path,
        candidate: &Path,
    ) -> Result<Option<PathBuf>, AssetStoreError> {
        if candidate.as_os_str().is_empty()
            || candidate
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(AssetStoreError::FilesystemBoundary(format!(
                "path traversal is not allowed: {}",
                candidate.display()
            )));
        }
        let candidate = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            root.join(candidate)
        };
        let lexical = if candidate.is_absolute() {
            candidate.clone()
        } else {
            root.join(candidate)
        };
        let symlink_metadata = match fs::symlink_metadata(&lexical) {
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(AssetStoreError::Delete(format!(
                    "inspect {}: {error}",
                    lexical.display()
                )))
            }
        };
        if symlink_metadata
            .as_ref()
            .is_some_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err(AssetStoreError::FilesystemBoundary(format!(
                "symbolic links are not allowed: {}",
                lexical.display()
            )));
        }
        let canonical = match fs::canonicalize(&lexical) {
            Ok(path) => path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let mut parent = lexical.parent().ok_or_else(|| {
                    AssetStoreError::FilesystemBoundary(format!(
                        "asset path has no parent: {}",
                        lexical.display()
                    ))
                })?;
                while !parent.exists() {
                    parent = parent.parent().ok_or_else(|| {
                        AssetStoreError::FilesystemBoundary(format!(
                            "asset path has no existing parent: {}",
                            lexical.display()
                        ))
                    })?;
                }
                let canonical_parent = fs::canonicalize(parent).map_err(|parent_error| {
                    AssetStoreError::Delete(format!(
                        "canonicalize asset parent {}: {parent_error}",
                        parent.display()
                    ))
                })?;
                if !canonical_parent.starts_with(root) {
                    return Err(AssetStoreError::FilesystemBoundary(format!(
                        "asset path is outside project root: {}",
                        lexical.display()
                    )));
                }
                return Ok(None);
            }
            Err(error) => {
                return Err(AssetStoreError::Delete(format!(
                    "canonicalize asset path {}: {error}",
                    lexical.display()
                )))
            }
        };
        if !canonical.starts_with(root) {
            return Err(AssetStoreError::FilesystemBoundary(format!(
                "asset path is outside project root: {}",
                lexical.display()
            )));
        }
        if !symlink_metadata.is_some_and(|metadata| metadata.is_file()) {
            return Err(AssetStoreError::FilesystemBoundary(format!(
                "asset path is not a regular file: {}",
                lexical.display()
            )));
        }
        Ok(Some(canonical))
    }

    fn restore_staged_files(staged: &[StagedAssetFile]) -> Result<(), std::io::Error> {
        for file in staged.iter().rev() {
            if !file.staged_path.exists() {
                continue;
            }
            if file.original_path.exists() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    format!(
                        "original path already exists: {}",
                        file.original_path.display()
                    ),
                ));
            }
            if let Some(parent) = file.original_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::rename(&file.staged_path, &file.original_path)?;
        }
        Ok(())
    }

    async fn begin_stream_write(
        project_root: &Path,
        asset_id: &AssetId,
        extension: &str,
        category: &str,
        media_type: &str,
    ) -> Result<Box<dyn AssetWriteSession>, AssetStoreError> {
        let target = Self::target_path(project_root, asset_id, extension, category, media_type)?;
        let parent = target.parent().ok_or_else(|| {
            AssetStoreError::InvalidPath("media target has no parent directory".to_owned())
        })?;
        fs::create_dir_all(parent).map_err(|error| {
            AssetStoreError::Write(format!("create {}: {error}", parent.display()))
        })?;
        if target.exists() {
            return Err(AssetStoreError::Write(format!(
                "asset target already exists: {}",
                target.display()
            )));
        }
        let temporary = parent.join(format!(".{}.tmp", asset_id.as_str()));
        let file = fs::File::create(&temporary).map_err(|error| {
            AssetStoreError::Write(format!("create {}: {error}", temporary.display()))
        })?;
        Ok(Box::new(FileSystemVideoWriteSession {
            temporary,
            target,
            file: Some(file),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::FileSystemAssetStore;
    use crate::application::ports::AssetStore;
    use crate::domain::{Asset, AssetId};
    use chrono::Utc;
    use serde_json::json;
    use tempfile::tempdir;

    #[tokio::test]
    async fn writes_to_generated_image_path_and_deletes_only_that_file() {
        let root = tempdir().expect("temp root");
        let store = FileSystemAssetStore::new();
        let asset_id = AssetId::parse("ast_test-1").expect("asset id");
        let stored = store
            .write_image(root.path(), &asset_id, "png", b"bytes")
            .await
            .expect("asset should write");

        assert_eq!(
            stored.path,
            root.path().join("assets/generated/image/ast_test-1.png")
        );
        assert_eq!(std::fs::read(&stored.path).expect("asset file"), b"bytes");
        store
            .delete(&stored.path)
            .await
            .expect("asset should delete");
        assert!(!stored.path.exists());

        let source = store
            .write_source_image(root.path(), &asset_id, "png", b"source-bytes")
            .await
            .expect("source asset should write");
        assert_eq!(
            source.path,
            root.path().join("assets/source/image/ast_test-1.png")
        );
    }

    fn source_asset(id: &AssetId, storage_path: String, thumbnail_path: Option<String>) -> Asset {
        let mut asset = Asset::new_source_image(
            id.clone(),
            "project-1",
            "Reference",
            "reference.png",
            storage_path,
            "a".repeat(64),
            "image/png",
            1,
            1,
            4,
            json!({}),
            Utc::now(),
        )
        .expect("source asset should be valid");
        asset.thumbnail_path = thumbnail_path;
        asset
    }

    #[tokio::test]
    async fn stages_and_restores_main_file_and_thumbnail_as_one_operation() {
        let root = tempdir().expect("temp root");
        let store = FileSystemAssetStore::new();
        let asset_id = AssetId::parse("ast_transactional").expect("asset id");
        let main = store
            .write_source_image(root.path(), &asset_id, "png", b"main")
            .await
            .expect("main asset should write");
        let thumbnail = store
            .write_thumbnail(root.path(), &asset_id, b"thumb")
            .await
            .expect("thumbnail should write");
        let asset = source_asset(
            &asset_id,
            main.path.to_string_lossy().to_string(),
            Some(thumbnail.path.to_string_lossy().to_string()),
        );

        let staged = store
            .stage_for_delete(root.path(), "operation-1", &asset)
            .await
            .expect("files should stage");
        assert_eq!(staged.len(), 2);
        assert!(!main.path.exists());
        assert!(!thumbnail.path.exists());
        store
            .restore_staged_delete(&staged)
            .await
            .expect("staged files should restore");
        assert_eq!(std::fs::read(&main.path).expect("main file"), b"main");
        assert_eq!(std::fs::read(&thumbnail.path).expect("thumbnail"), b"thumb");

        let staged = store
            .stage_for_delete(root.path(), "operation-2", &asset)
            .await
            .expect("files should stage again");
        store
            .commit_staged_delete(&staged)
            .await
            .expect("staged files should commit");
        assert!(!main.path.exists());
        assert!(!thumbnail.path.exists());
    }

    #[tokio::test]
    async fn rejects_outside_and_traversal_asset_paths() {
        let root = tempdir().expect("temp root");
        let store = FileSystemAssetStore::new();
        let asset_id = AssetId::parse("ast_boundary").expect("asset id");
        let outside = tempdir().expect("outside root");
        let outside_asset = source_asset(
            &asset_id,
            outside
                .path()
                .join("outside.png")
                .to_string_lossy()
                .to_string(),
            None,
        );
        assert!(matches!(
            store
                .validate_delete_paths(root.path(), &outside_asset)
                .await,
            Err(crate::application::ports::AssetStoreError::FilesystemBoundary(_))
        ));

        let traversal_asset = source_asset(
            &asset_id,
            root.path()
                .join("assets")
                .join("..")
                .join("outside.png")
                .to_string_lossy()
                .to_string(),
            None,
        );
        assert!(matches!(
            store
                .validate_delete_paths(root.path(), &traversal_asset)
                .await,
            Err(crate::application::ports::AssetStoreError::FilesystemBoundary(_))
        ));
    }
}
