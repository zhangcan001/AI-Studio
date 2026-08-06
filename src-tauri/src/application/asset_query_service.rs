use crate::application::ports::{
    AssetRepository, AssetStore, AssetStoreError, GenerationDefinitionRepository, RepositoryError,
    TaskOutputAssetMapping, TaskRepository,
};
use crate::compiler::RecipeParser;
use crate::domain::{AssetId, AssetType, TaskId};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::{error::Error, fmt, sync::Arc};

pub struct AssetQueryService {
    asset_repository: Arc<dyn AssetRepository>,
    asset_store: Arc<dyn AssetStore>,
    task_repository: Option<Arc<dyn TaskRepository>>,
    definition_repository: Option<Arc<dyn GenerationDefinitionRepository>>,
}

impl AssetQueryService {
    pub fn new(
        asset_repository: Arc<dyn AssetRepository>,
        asset_store: Arc<dyn AssetStore>,
    ) -> Self {
        Self {
            asset_repository,
            asset_store,
            task_repository: None,
            definition_repository: None,
        }
    }

    pub fn with_output_order_repositories(
        mut self,
        task_repository: Arc<dyn TaskRepository>,
        definition_repository: Arc<dyn GenerationDefinitionRepository>,
    ) -> Self {
        self.task_repository = Some(task_repository);
        self.definition_repository = Some(definition_repository);
        self
    }

    pub async fn list_by_task(
        &self,
        project_id: &str,
        task_id: &str,
    ) -> Result<Vec<AssetView>, AssetQueryError> {
        validate_project_id(project_id)?;
        let task_id = TaskId::parse(task_id.to_owned())
            .map_err(|error| AssetQueryError::InvalidTaskId(error.to_string()))?;
        let mut mapped = self.asset_repository.list_mapped_assets(&task_id).await?;
        if mapped.is_empty() {
            return Ok(self
                .asset_repository
                .list_by_source_task(&task_id)
                .await?
                .into_iter()
                .filter(|asset| asset.project_id == project_id)
                .map(AssetView::from)
                .collect());
        }

        let output_order = self.output_order(&task_id).await?;
        mapped.sort_by(|(left, _), (right, _)| compare_mappings(left, right, &output_order));
        Ok(mapped
            .into_iter()
            .filter(|(_, asset)| asset.project_id == project_id)
            .map(|(_, asset)| AssetView::from(asset))
            .collect())
    }

    async fn output_order(&self, task_id: &TaskId) -> Result<Vec<String>, AssetQueryError> {
        let (Some(task_repository), Some(definition_repository)) =
            (&self.task_repository, &self.definition_repository)
        else {
            return Ok(Vec::new());
        };
        let Some(task) = task_repository.find_by_id(task_id).await? else {
            return Ok(Vec::new());
        };
        let Some(definition) = definition_repository
            .find(&task.workflow_version_id, &task.recipe_id)
            .await?
        else {
            return Ok(Vec::new());
        };
        Ok(RecipeParser::parse(&definition.recipe_yaml)
            .map(|recipe| recipe.outputs.into_iter().map(|output| output.id).collect())
            .unwrap_or_default())
    }

    pub async fn list_recent(
        &self,
        project_id: &str,
        limit: u32,
    ) -> Result<Vec<AssetView>, AssetQueryError> {
        if project_id.trim().is_empty() {
            return Err(AssetQueryError::InvalidProjectId(
                "project id must not be empty".to_owned(),
            ));
        }
        Ok(self
            .asset_repository
            .list_recent(project_id, limit.min(100))
            .await?
            .into_iter()
            .map(AssetView::from)
            .collect())
    }

    pub async fn get(
        &self,
        project_id: &str,
        asset_id: &str,
    ) -> Result<AssetSummaryView, AssetQueryError> {
        if project_id.trim().is_empty() {
            return Err(AssetQueryError::InvalidProjectId(
                "project id must not be empty".to_owned(),
            ));
        }
        let asset_id = AssetId::parse(asset_id.to_owned())
            .map_err(|error| AssetQueryError::InvalidAssetId(error.to_string()))?;
        let asset = self
            .asset_repository
            .find_by_id(&asset_id)
            .await?
            .ok_or_else(|| AssetQueryError::NotFound(asset_id.as_str().to_owned()))?;
        if asset.project_id != project_id {
            return Err(AssetQueryError::NotFound(asset_id.as_str().to_owned()));
        }
        Ok(AssetSummaryView::from(asset))
    }

    pub async fn read_image(
        &self,
        project_id: &str,
        asset_id: &str,
    ) -> Result<AssetBinary, AssetQueryError> {
        validate_project_id(project_id)?;
        let asset_id = AssetId::parse(asset_id.to_owned())
            .map_err(|error| AssetQueryError::InvalidAssetId(error.to_string()))?;
        let asset = self
            .asset_repository
            .find_by_id(&asset_id)
            .await?
            .ok_or_else(|| AssetQueryError::NotFound(asset_id.as_str().to_owned()))?;
        if asset.project_id != project_id {
            return Err(AssetQueryError::NotFound(asset_id.as_str().to_owned()));
        }
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

    pub async fn read_thumbnail(
        &self,
        project_id: &str,
        asset_id: &str,
    ) -> Result<AssetBinary, AssetQueryError> {
        validate_project_id(project_id)?;
        let asset_id = AssetId::parse(asset_id.to_owned())
            .map_err(|error| AssetQueryError::InvalidAssetId(error.to_string()))?;
        let asset = self
            .asset_repository
            .find_by_id(&asset_id)
            .await?
            .ok_or_else(|| AssetQueryError::NotFound(asset_id.as_str().to_owned()))?;
        if asset.project_id != project_id {
            return Err(AssetQueryError::NotFound(asset_id.as_str().to_owned()));
        }
        if !matches!(asset.asset_type, AssetType::Image | AssetType::Video) {
            return Err(AssetQueryError::NotImage(asset_id.as_str().to_owned()));
        }
        let path = asset
            .thumbnail_path
            .ok_or_else(|| AssetQueryError::ThumbnailNotAvailable(asset_id.as_str().to_owned()))?;
        let bytes = self
            .asset_store
            .read(std::path::Path::new(&path))
            .await
            .map_err(AssetQueryError::Read)?;
        Ok(AssetBinary { bytes })
    }
}

fn compare_mappings(
    left: &TaskOutputAssetMapping,
    right: &TaskOutputAssetMapping,
    output_order: &[String],
) -> std::cmp::Ordering {
    let left_rank = output_order
        .iter()
        .position(|output_id| output_id == &left.output_id)
        .unwrap_or(usize::MAX);
    let right_rank = output_order
        .iter()
        .position(|output_id| output_id == &right.output_id)
        .unwrap_or(usize::MAX);
    left_rank
        .cmp(&right_rank)
        .then_with(|| left.ordinal.cmp(&right.ordinal))
        .then_with(|| left.output_id.cmp(&right.output_id))
}

fn validate_project_id(project_id: &str) -> Result<(), AssetQueryError> {
    if project_id.trim().is_empty() {
        return Err(AssetQueryError::InvalidProjectId(
            "project id must not be empty".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetSummaryView {
    pub id: String,
    pub asset_type: String,
    pub category: String,
    pub name: String,
    pub original_name: String,
    pub mime_type: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_ms: Option<u64>,
    pub file_size: u64,
    pub created_at: DateTime<Utc>,
    pub thumbnail_available: bool,
}

impl From<crate::domain::Asset> for AssetSummaryView {
    fn from(asset: crate::domain::Asset) -> Self {
        let original_name = if matches!(
            asset.category.as_str(),
            crate::domain::asset::GENERATED_IMAGE_CATEGORY
                | crate::domain::asset::GENERATED_VIDEO_CATEGORY
        ) {
            asset.name.clone()
        } else {
            asset.original_name.clone()
        };
        Self {
            id: asset.id.as_str().to_owned(),
            asset_type: asset.asset_type.as_str().to_owned(),
            category: asset.category,
            name: asset.name,
            original_name,
            mime_type: asset.mime_type,
            width: (asset.width > 0).then_some(asset.width),
            height: (asset.height > 0).then_some(asset.height),
            duration_ms: asset.duration_ms,
            file_size: asset.file_size,
            created_at: asset.created_at,
            thumbnail_available: asset.thumbnail_path.is_some(),
        }
    }
}

pub type AssetView = AssetSummaryView;

pub struct AssetBinary {
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
pub enum AssetQueryError {
    InvalidTaskId(String),
    InvalidProjectId(String),
    InvalidAssetId(String),
    NotFound(String),
    NotImage(String),
    ThumbnailNotAvailable(String),
    Repository(RepositoryError),
    Read(AssetStoreError),
}

impl fmt::Display for AssetQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTaskId(message) => write!(formatter, "INVALID_TASK_ID: {message}"),
            Self::InvalidProjectId(message) => write!(formatter, "INVALID_PROJECT_ID: {message}"),
            Self::InvalidAssetId(message) => write!(formatter, "INVALID_ASSET_ID: {message}"),
            Self::NotFound(id) => write!(formatter, "ASSET_NOT_FOUND: asset {id} was not found"),
            Self::NotImage(id) => write!(formatter, "ASSET_NOT_IMAGE: asset {id} is not an image"),
            Self::ThumbnailNotAvailable(id) => write!(
                formatter,
                "THUMBNAIL_NOT_AVAILABLE: asset {id} has no thumbnail"
            ),
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
    use crate::application::ports::{
        AssetRepository, AssetStore, AssetStoreError, StoredAssetFile, TaskRepository,
    };
    use crate::domain::{Asset, AssetId, Task};
    use crate::infrastructure::database::{
        initialize, repositories::test_support, SqliteAssetRepository, SqliteTaskRepository,
    };
    use crate::infrastructure::filesystem::FileSystemAssetStore;
    use async_trait::async_trait;
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use std::path::Path;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tempfile::tempdir;

    #[derive(Clone)]
    struct CountingAssetStore {
        reads: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl AssetStore for CountingAssetStore {
        async fn write_image(
            &self,
            _project_root: &Path,
            _asset_id: &AssetId,
            _extension: &str,
            _bytes: &[u8],
        ) -> Result<StoredAssetFile, AssetStoreError> {
            unreachable!()
        }

        async fn delete(&self, _path: &Path) -> Result<(), AssetStoreError> {
            unreachable!()
        }

        async fn read(&self, _path: &Path) -> Result<Vec<u8>, AssetStoreError> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            Ok(Vec::new())
        }
    }

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
            "ComfyUI_00001.png",
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
            service
                .read_image("project-1", "ast_read_test")
                .await
                .unwrap()
                .bytes,
            bytes
        );
        assert!(matches!(
            service.read_image("project-1", "ast_missing").await,
            Err(AssetQueryError::NotFound(_))
        ));

        let summary = service.get("project-1", "ast_read_test").await.unwrap();
        let json = serde_json::to_string(&summary).unwrap();
        for forbidden in [
            "storagePath",
            "thumbnailPath",
            "sha256",
            "metadata",
            "metadataJson",
            "comfyFilename",
            "nodeId",
            "ComfyUI_",
        ] {
            assert!(
                !json.contains(forbidden),
                "secure asset DTO leaked {forbidden}"
            );
        }
        assert!(matches!(
            service.get("project-2", "ast_read_test").await,
            Err(AssetQueryError::NotFound(_))
        ));
        assert!(matches!(
            service.read_image("project-2", "ast_read_test").await,
            Err(AssetQueryError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn cross_project_binary_read_fails_before_filesystem_access() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        let task_repository = SqliteTaskRepository::new(pool.clone());
        let task = Task::new(
            "project-1",
            "workflow-1",
            "workflow-version-1",
            "recipe-1",
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        );
        task_repository
            .create(&task, &task.created_event())
            .await
            .unwrap();
        let path = directory.path().join("image.png");
        std::fs::write(&path, b"image").unwrap();
        let asset = Asset::new_image(
            AssetId::parse("ast_cross_project").unwrap(),
            "project-1",
            "image",
            "image.png",
            path.to_string_lossy(),
            "a".repeat(64),
            "image/png",
            1,
            1,
            5,
            task.id,
            json!({}),
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 1).unwrap(),
        )
        .unwrap();
        let repository = SqliteAssetRepository::new(pool);
        repository.insert_many(&[asset]).await.unwrap();
        let reads = Arc::new(AtomicUsize::new(0));
        let service = AssetQueryService::new(
            Arc::new(repository),
            Arc::new(CountingAssetStore {
                reads: reads.clone(),
            }),
        );

        assert!(matches!(
            service.read_image("project-2", "ast_cross_project").await,
            Err(AssetQueryError::NotFound(_))
        ));
        assert_eq!(reads.load(Ordering::SeqCst), 0);
    }
}
