use crate::application::ports::{AssetStore, AssetStoreError, AssetWriteSession, StoredAssetFile};
use crate::domain::AssetId;
use async_trait::async_trait;
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

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
        let target = Self::target_path(project_root, asset_id, extension, "generated", "video")?;
        let parent = target.parent().ok_or_else(|| {
            AssetStoreError::InvalidPath("video target has no parent directory".to_owned())
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

#[cfg(test)]
mod tests {
    use super::FileSystemAssetStore;
    use crate::application::ports::AssetStore;
    use crate::domain::AssetId;
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
}
