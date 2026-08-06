use crate::application::ports::{AssetStore, AssetStoreError, StoredAssetFile};
use crate::domain::AssetId;
use async_trait::async_trait;
use std::{
    fs,
    io::Write,
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
            .join("image")
            .join(format!("{}.{}", asset_id.as_str(), extension)))
    }

    async fn write_to_path(
        project_root: &Path,
        asset_id: &AssetId,
        extension: &str,
        category: &str,
        bytes: &[u8],
    ) -> Result<StoredAssetFile, AssetStoreError> {
        let target = Self::target_path(project_root, asset_id, extension, category)?;
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

#[async_trait]
impl AssetStore for FileSystemAssetStore {
    async fn write_image(
        &self,
        project_root: &Path,
        asset_id: &AssetId,
        extension: &str,
        bytes: &[u8],
    ) -> Result<StoredAssetFile, AssetStoreError> {
        Self::write_to_path(project_root, asset_id, extension, "generated", bytes).await
    }

    async fn write_source_image(
        &self,
        project_root: &Path,
        asset_id: &AssetId,
        extension: &str,
        bytes: &[u8],
    ) -> Result<StoredAssetFile, AssetStoreError> {
        Self::write_to_path(project_root, asset_id, extension, "source", bytes).await
    }

    async fn write_thumbnail(
        &self,
        project_root: &Path,
        asset_id: &AssetId,
        bytes: &[u8],
    ) -> Result<StoredAssetFile, AssetStoreError> {
        Self::write_to_path(project_root, asset_id, "png", "thumbnails", bytes).await
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
