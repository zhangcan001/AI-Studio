use crate::domain::AssetId;
use async_trait::async_trait;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredAssetFile {
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssetStoreError {
    InvalidPath(String),
    Write(String),
    Delete(String),
    Read(String),
}

impl std::fmt::Display for AssetStoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPath(message) => write!(formatter, "invalid asset path: {message}"),
            Self::Write(message) => write!(formatter, "asset write failed: {message}"),
            Self::Delete(message) => write!(formatter, "asset delete failed: {message}"),
            Self::Read(message) => write!(formatter, "asset read failed: {message}"),
        }
    }
}

impl std::error::Error for AssetStoreError {}

#[async_trait]
pub trait AssetStore: Send + Sync {
    async fn write_image(
        &self,
        project_root: &Path,
        asset_id: &AssetId,
        extension: &str,
        bytes: &[u8],
    ) -> Result<StoredAssetFile, AssetStoreError>;

    async fn delete(&self, path: &Path) -> Result<(), AssetStoreError>;

    async fn read(&self, path: &Path) -> Result<Vec<u8>, AssetStoreError>;
}
