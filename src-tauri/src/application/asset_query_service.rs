use crate::application::ports::{AssetRepository, AssetStore, AssetStoreError, RepositoryError};
use crate::domain::{AssetId, AssetType, TaskId};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use std::{error::Error, fmt, sync::Arc};

pub struct AssetQueryService {
    asset_repository: Arc<dyn AssetRepository>,
    asset_store: Arc<dyn AssetStore>,
}

impl AssetQueryService {
    pub fn new(
        asset_repository: Arc<dyn AssetRepository>,
        asset_store: Arc<dyn AssetStore>,
    ) -> Self {
        Self {
            asset_repository,
            asset_store,
        }
    }

    pub async fn list_by_task(&self, task_id: &str) -> Result<Vec<AssetView>, AssetQueryError> {
        let task_id = TaskId::parse(task_id.to_owned())
            .map_err(|error| AssetQueryError::InvalidTaskId(error.to_string()))?;
        Ok(self
            .asset_repository
            .list_by_source_task(&task_id)
            .await?
            .into_iter()
            .map(AssetView::from)
            .collect())
    }

    pub async fn read_image(&self, asset_id: &str) -> Result<AssetBinary, AssetQueryError> {
        let asset_id = AssetId::parse(asset_id.to_owned())
            .map_err(|error| AssetQueryError::InvalidAssetId(error.to_string()))?;
        let asset = self
            .asset_repository
            .find_by_id(&asset_id)
            .await?
            .ok_or_else(|| AssetQueryError::NotFound(asset_id.as_str().to_owned()))?;
        if asset.asset_type != AssetType::Image {
            return Err(AssetQueryError::NotImage(asset_id.as_str().to_owned()));
        }
        let bytes = self
            .asset_store
            .read(std::path::Path::new(&asset.storage_path))
            .await
            .map_err(AssetQueryError::Read)?;
        Ok(AssetBinary { bytes })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetView {
    pub id: String,
    pub name: String,
    pub original_name: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    pub file_size: u64,
    pub created_at: DateTime<Utc>,
    pub metadata: Value,
}

impl From<crate::domain::Asset> for AssetView {
    fn from(asset: crate::domain::Asset) -> Self {
        Self {
            id: asset.id.as_str().to_owned(),
            name: asset.name,
            original_name: asset.original_name,
            mime_type: asset.mime_type,
            width: asset.width,
            height: asset.height,
            file_size: asset.file_size,
            created_at: asset.created_at,
            metadata: asset.metadata_json,
        }
    }
}

pub struct AssetBinary {
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
pub enum AssetQueryError {
    InvalidTaskId(String),
    InvalidAssetId(String),
    NotFound(String),
    NotImage(String),
    Repository(RepositoryError),
    Read(AssetStoreError),
}

impl fmt::Display for AssetQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTaskId(message) => write!(formatter, "INVALID_TASK_ID: {message}"),
            Self::InvalidAssetId(message) => write!(formatter, "INVALID_ASSET_ID: {message}"),
            Self::NotFound(id) => write!(formatter, "ASSET_NOT_FOUND: asset {id} was not found"),
            Self::NotImage(id) => write!(formatter, "ASSET_NOT_IMAGE: asset {id} is not an image"),
            Self::Repository(error) => write!(formatter, "{error}"),
            Self::Read(error) => write!(formatter, "ASSET_READ_FAILED: {error}"),
        }
    }
}

impl Error for AssetQueryError {}

impl From<RepositoryError> for AssetQueryError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{AssetQueryError, AssetQueryService};
    use crate::application::ports::{AssetRepository, TaskRepository};
    use crate::domain::{Asset, AssetId, Task};
    use crate::infrastructure::database::{
        initialize, repositories::test_support, SqliteAssetRepository, SqliteTaskRepository,
    };
    use crate::infrastructure::filesystem::FileSystemAssetStore;
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[tokio::test]
    async fn reads_image_by_asset_id_and_rejects_unknown_asset() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        let task = Task::new(
            "project-1",
            "workflow-1",
            "workflow-version-1",
            "recipe-1",
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        );
        let task_repository = SqliteTaskRepository::new(pool.clone());
        task_repository
            .create(&task, &task.created_event())
            .await
            .unwrap();

        let path = directory.path().join("image.png");
        let bytes = b"image-bytes".to_vec();
        std::fs::write(&path, &bytes).unwrap();
        let asset = Asset::new_image(
            AssetId::parse("ast_read_test").unwrap(),
            "project-1",
            "image",
            "image.png",
            path.to_string_lossy(),
            "a".repeat(64),
            "image/png",
            1,
            1,
            bytes.len() as u64,
            task.id.clone(),
            json!({}),
            task.created_at,
        )
        .unwrap();
        let repository = SqliteAssetRepository::new(pool);
        repository.insert_many(&[asset]).await.unwrap();
        let service =
            AssetQueryService::new(Arc::new(repository), Arc::new(FileSystemAssetStore::new()));

        assert_eq!(
            service.read_image("ast_read_test").await.unwrap().bytes,
            bytes
        );
        assert!(matches!(
            service.read_image("ast_missing").await,
            Err(AssetQueryError::NotFound(_))
        ));
    }
}
