use super::RepositoryError;
use crate::domain::{
    Asset, AssetId, AssetRelation, AssetRelationId, AssetVersion, AssetVersionId, TaskId,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskOutputAssetMapping {
    pub task_id: TaskId,
    pub output_id: String,
    pub ordinal: u32,
    pub asset_id: AssetId,
    pub created_at: DateTime<Utc>,
}

#[async_trait]
pub trait AssetRepository: Send + Sync {
    async fn insert_many(&self, assets: &[Asset]) -> Result<(), RepositoryError>;

    async fn find_by_id(&self, asset_id: &AssetId) -> Result<Option<Asset>, RepositoryError>;

    /// Batch lookup used by project-local asset memberships. SQLite provides
    /// one `IN (...)` query; the default keeps small test repositories
    /// source-compatible.
    async fn find_many_by_ids(&self, asset_ids: &[AssetId]) -> Result<Vec<Asset>, RepositoryError> {
        let mut assets = Vec::with_capacity(asset_ids.len());
        for asset_id in asset_ids {
            if let Some(asset) = self.find_by_id(asset_id).await? {
                assets.push(asset);
            }
        }
        Ok(assets)
    }

    async fn list_by_source_task(&self, task_id: &TaskId) -> Result<Vec<Asset>, RepositoryError>;

    /// Loads generated candidates for a set of source tasks. SQLite overrides
    /// this with one set-based query; the default keeps lightweight fakes
    /// source-compatible.
    async fn list_by_source_tasks(
        &self,
        task_ids: &[TaskId],
    ) -> Result<Vec<Asset>, RepositoryError> {
        let mut assets = Vec::new();
        for task_id in task_ids {
            assets.extend(self.list_by_source_task(task_id).await?);
        }
        Ok(assets)
    }

    async fn list_recent(
        &self,
        project_id: &str,
        limit: u32,
    ) -> Result<Vec<Asset>, RepositoryError>;

    async fn insert_asset_version(&self, _version: &AssetVersion) -> Result<(), RepositoryError> {
        Err(RepositoryError::database(
            "asset version persistence is not supported by this repository",
        ))
    }

    async fn find_asset_version_by_id(
        &self,
        _project_id: &str,
        _version_id: &AssetVersionId,
    ) -> Result<Option<AssetVersion>, RepositoryError> {
        Err(RepositoryError::database(
            "asset version lookup is not supported by this repository",
        ))
    }

    async fn list_asset_versions(
        &self,
        _project_id: &str,
        _asset_id: &AssetId,
    ) -> Result<Vec<AssetVersion>, RepositoryError> {
        Err(RepositoryError::database(
            "asset version queries are not supported by this repository",
        ))
    }

    async fn current_asset_version(
        &self,
        _project_id: &str,
        _asset_id: &AssetId,
    ) -> Result<Option<AssetVersion>, RepositoryError> {
        Err(RepositoryError::database(
            "current asset version queries are not supported by this repository",
        ))
    }

    async fn insert_asset_relation(
        &self,
        _relation: &AssetRelation,
    ) -> Result<(), RepositoryError> {
        Err(RepositoryError::database(
            "asset relation persistence is not supported by this repository",
        ))
    }

    async fn list_asset_relations(
        &self,
        _project_id: &str,
        _asset_id: &AssetId,
    ) -> Result<Vec<AssetRelation>, RepositoryError> {
        Err(RepositoryError::database(
            "asset relation queries are not supported by this repository",
        ))
    }

    async fn delete_asset_relation(
        &self,
        _project_id: &str,
        _relation_id: &AssetRelationId,
    ) -> Result<(), RepositoryError> {
        Err(RepositoryError::database(
            "asset relation deletion is not supported by this repository",
        ))
    }

    /// Deletes only the selected asset rows and their project-local relation rows.
    ///
    /// The default keeps lightweight test repositories source-compatible; the
    /// production SQLite implementation provides the transactional delete.
    async fn delete_by_ids(
        &self,
        _project_id: &str,
        _asset_ids: &[AssetId],
    ) -> Result<(), RepositoryError> {
        Err(RepositoryError::database(
            "asset deletion is not supported by this repository",
        ))
    }

    async fn insert_generated_outputs(
        &self,
        assets: &[Asset],
        mappings: &[TaskOutputAssetMapping],
    ) -> Result<(), RepositoryError> {
        let _ = mappings;
        self.insert_many(assets).await
    }

    async fn list_output_mappings(
        &self,
        task_id: &TaskId,
    ) -> Result<Vec<TaskOutputAssetMapping>, RepositoryError> {
        let _ = task_id;
        Ok(Vec::new())
    }

    async fn list_mapped_assets(
        &self,
        task_id: &TaskId,
    ) -> Result<Vec<(TaskOutputAssetMapping, Asset)>, RepositoryError> {
        let mappings = self.list_output_mappings(task_id).await?;
        let mut assets = Vec::with_capacity(mappings.len());
        for mapping in mappings {
            if let Some(asset) = self.find_by_id(&mapping.asset_id).await? {
                assets.push((mapping, asset));
            }
        }
        Ok(assets)
    }
}
