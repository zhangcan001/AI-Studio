use crate::application::ports::{
    AssetDeletionReferences, AssetDeletionRepository, AssetRepository, AssetStore, AssetStoreError,
    ProjectRepository, RepositoryError, StagedAssetFile,
};
use crate::domain::{validate_project_id, Asset, AssetId};
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

const MAX_DELETE_ASSETS: usize = 100;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetDeleteInspectionItem {
    pub asset_id: String,
    pub name: String,
    pub asset_type: String,
    pub file_size: u64,
    pub can_delete: bool,
    pub blocking_reasons: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetDeleteInspection {
    pub items: Vec<AssetDeleteInspectionItem>,
    pub deletable: Vec<String>,
    pub blocked: Vec<String>,
    pub historical_references: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetDeleteResult {
    pub deleted_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum AssetDeletionError {
    InvalidInput(String),
    NotFound(String),
    Blocked(AssetDeleteInspection),
    FilesystemBoundary(String),
    Store(AssetStoreError),
    Repository(RepositoryError),
}

impl std::fmt::Display for AssetDeletionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(message) => {
                write!(formatter, "invalid asset deletion input: {message}")
            }
            Self::NotFound(message) => {
                write!(formatter, "asset deletion target not found: {message}")
            }
            Self::Blocked(_) => formatter.write_str("one or more assets are still in use"),
            Self::FilesystemBoundary(message) => {
                write!(formatter, "FILESYSTEM_BOUNDARY_ERROR: {message}")
            }
            Self::Store(error) => write!(formatter, "asset storage operation failed: {error}"),
            Self::Repository(error) => {
                write!(formatter, "asset deletion repository failed: {error}")
            }
        }
    }
}

impl std::error::Error for AssetDeletionError {}

pub struct AssetDeletionService {
    asset_repository: Arc<dyn AssetRepository>,
    reference_repository: Arc<dyn AssetDeletionRepository>,
    project_repository: Arc<dyn ProjectRepository>,
    asset_store: Arc<dyn AssetStore>,
}

impl AssetDeletionService {
    pub fn new(
        asset_repository: Arc<dyn AssetRepository>,
        reference_repository: Arc<dyn AssetDeletionRepository>,
        project_repository: Arc<dyn ProjectRepository>,
        asset_store: Arc<dyn AssetStore>,
    ) -> Self {
        Self {
            asset_repository,
            reference_repository,
            project_repository,
            asset_store,
        }
    }

    pub async fn inspect(
        &self,
        project_id: &str,
        asset_id_values: &[String],
    ) -> Result<AssetDeleteInspection, AssetDeletionError> {
        let (asset_ids, assets, project_root) =
            self.load_targets(project_id, asset_id_values).await?;
        let references = self
            .reference_repository
            .references_for(project_id, &asset_ids)
            .await
            .map_err(AssetDeletionError::Repository)?;
        let references_by_id = references
            .into_iter()
            .map(|reference| (reference.asset_id.as_str().to_owned(), reference))
            .collect::<std::collections::HashMap<_, _>>();

        let mut items = Vec::with_capacity(assets.len());
        let mut deletable = Vec::new();
        let mut blocked = Vec::new();
        let mut historical_references = Vec::new();
        for asset in assets {
            let reference = references_by_id
                .get(asset.id.as_str())
                .cloned()
                .unwrap_or_else(|| AssetDeletionReferences {
                    asset_id: asset.id.clone(),
                    ..Default::default()
                });
            let mut blocking_reasons = Vec::new();
            if !reference.active_production_item_ids.is_empty() {
                blocking_reasons.push(
                    "该素材正在被生产队列使用，请等待任务完成或取消对应批次后再删除。".to_owned(),
                );
            }
            if !reference.active_task_ids.is_empty() {
                blocking_reasons
                    .push("该素材正被活动任务使用，请等待任务完成或取消后再删除。".to_owned());
            }
            self.asset_store
                .validate_delete_paths(&project_root, &asset)
                .await
                .map_err(map_store_error)?;

            let mut warnings = Vec::new();
            if !reference.historical_task_ids.is_empty() {
                historical_references.push(asset.id.as_str().to_owned());
                warnings.push(
                    "该素材已被历史生成任务使用。删除后历史记录仍保留，但无法再次读取该素材，基于该历史输入的重试可能需要重新选择素材。"
                        .to_owned(),
                );
            }
            let can_delete = blocking_reasons.is_empty();
            if can_delete {
                deletable.push(asset.id.as_str().to_owned());
            } else {
                blocked.push(asset.id.as_str().to_owned());
            }
            items.push(AssetDeleteInspectionItem {
                asset_id: asset.id.as_str().to_owned(),
                name: asset.name,
                asset_type: asset.asset_type.as_str().to_owned(),
                file_size: asset.file_size,
                can_delete,
                blocking_reasons,
                warnings,
            });
        }

        Ok(AssetDeleteInspection {
            items,
            deletable,
            blocked,
            historical_references,
        })
    }

    pub async fn delete(
        &self,
        project_id: &str,
        asset_id_values: &[String],
    ) -> Result<AssetDeleteResult, AssetDeletionError> {
        let inspection = self.inspect(project_id, asset_id_values).await?;
        if !inspection.blocked.is_empty() {
            return Err(AssetDeletionError::Blocked(inspection));
        }
        let (asset_ids, assets, project_root) =
            self.load_targets(project_id, asset_id_values).await?;
        let operation_id = Uuid::new_v4().simple().to_string();
        let mut staged = Vec::<StagedAssetFile>::new();
        for asset in &assets {
            match self
                .asset_store
                .stage_for_delete(&project_root, &operation_id, asset)
                .await
            {
                Ok(mut files) => staged.append(&mut files),
                Err(error) => {
                    self.restore_after_failure(&staged).await;
                    return Err(map_store_error(error));
                }
            }
        }

        if let Err(error) = self
            .asset_repository
            .delete_by_ids(project_id, &asset_ids)
            .await
        {
            self.restore_after_failure(&staged).await;
            return Err(AssetDeletionError::Repository(error));
        }

        let mut warnings = Vec::new();
        if let Err(error) = self.asset_store.commit_staged_delete(&staged).await {
            tracing::warn!(
                error = %error,
                project_id,
                asset_count = assets.len(),
                "asset database deletion succeeded but staged file cleanup needs attention"
            );
            warnings.push(format!("数据库记录已删除，但部分素材文件清理失败：{error}"));
        }
        for item in inspection.items {
            warnings.extend(item.warnings);
        }
        Ok(AssetDeleteResult {
            deleted_count: asset_ids.len(),
            warnings,
        })
    }

    async fn load_targets(
        &self,
        project_id: &str,
        asset_id_values: &[String],
    ) -> Result<(Vec<AssetId>, Vec<Asset>, std::path::PathBuf), AssetDeletionError> {
        validate_project_id(project_id)
            .map_err(|error| AssetDeletionError::InvalidInput(error.to_string()))?;
        if asset_id_values.is_empty() {
            return Err(AssetDeletionError::InvalidInput(
                "至少选择一个素材".to_owned(),
            ));
        }
        if asset_id_values.len() > MAX_DELETE_ASSETS {
            return Err(AssetDeletionError::InvalidInput(format!(
                "一次最多删除 {MAX_DELETE_ASSETS} 个素材"
            )));
        }
        let mut asset_ids = Vec::with_capacity(asset_id_values.len());
        for value in asset_id_values {
            let asset_id = AssetId::parse(value.clone())
                .map_err(|error| AssetDeletionError::InvalidInput(error.to_string()))?;
            if asset_ids
                .iter()
                .any(|candidate: &AssetId| candidate == &asset_id)
            {
                return Err(AssetDeletionError::InvalidInput(
                    "素材列表不能包含重复项".to_owned(),
                ));
            }
            asset_ids.push(asset_id);
        }
        let project_root = self
            .project_repository
            .get_storage_root(project_id)
            .await
            .map_err(AssetDeletionError::Repository)?
            .ok_or_else(|| AssetDeletionError::NotFound(format!("project {project_id}")))?;
        let mut assets = Vec::with_capacity(asset_ids.len());
        for asset_id in &asset_ids {
            let asset = self
                .asset_repository
                .find_by_id(asset_id)
                .await
                .map_err(AssetDeletionError::Repository)?
                .ok_or_else(|| AssetDeletionError::NotFound(format!("asset {asset_id}")))?;
            if asset.project_id != project_id {
                return Err(AssetDeletionError::NotFound(format!(
                    "asset {} does not belong to project {}",
                    asset_id.as_str(),
                    project_id
                )));
            }
            assets.push(asset);
        }
        Ok((asset_ids, assets, project_root))
    }

    async fn restore_after_failure(&self, staged: &[StagedAssetFile]) {
        if staged.is_empty() {
            return;
        }
        if let Err(error) = self.asset_store.restore_staged_delete(staged).await {
            tracing::error!(error = %error, "asset deletion rollback could not restore staged files");
        }
    }
}

fn map_store_error(error: AssetStoreError) -> AssetDeletionError {
    match error {
        AssetStoreError::FilesystemBoundary(message) => {
            AssetDeletionError::FilesystemBoundary(message)
        }
        other => AssetDeletionError::Store(other),
    }
}

#[cfg(test)]
mod tests {
    use super::{AssetDeleteInspection, AssetDeletionError, AssetDeletionService};
    use crate::application::ports::{
        AssetDeletionReferences, AssetDeletionRepository, AssetRepository, AssetStore,
        AssetStoreError, ProjectRecord, ProjectRepository, RepositoryError, StagedAssetFile,
        StoredAssetFile,
    };
    use crate::domain::{Asset, AssetId, TaskId};
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use serde_json::json;
    use std::{
        collections::HashMap,
        path::{Path, PathBuf},
        sync::{Arc, Mutex},
    };
    use tempfile::tempdir;

    struct FakeAssetRepository {
        assets: Mutex<HashMap<String, Asset>>,
        fail_delete: bool,
    }

    #[async_trait]
    impl AssetRepository for FakeAssetRepository {
        async fn insert_many(&self, _assets: &[Asset]) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn find_by_id(&self, asset_id: &AssetId) -> Result<Option<Asset>, RepositoryError> {
            Ok(self
                .assets
                .lock()
                .expect("asset lock")
                .get(asset_id.as_str())
                .cloned())
        }

        async fn list_by_source_task(
            &self,
            _task_id: &TaskId,
        ) -> Result<Vec<Asset>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn list_recent(
            &self,
            _project_id: &str,
            _limit: u32,
        ) -> Result<Vec<Asset>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn delete_by_ids(
            &self,
            _project_id: &str,
            asset_ids: &[AssetId],
        ) -> Result<(), RepositoryError> {
            if self.fail_delete {
                return Err(RepositoryError::database("injected database failure"));
            }
            let mut assets = self.assets.lock().expect("asset lock");
            for asset_id in asset_ids {
                assets.remove(asset_id.as_str());
            }
            Ok(())
        }
    }

    struct FakeReferenceRepository {
        references: Vec<AssetDeletionReferences>,
    }

    #[async_trait]
    impl AssetDeletionRepository for FakeReferenceRepository {
        async fn references_for(
            &self,
            _project_id: &str,
            _asset_ids: &[AssetId],
        ) -> Result<Vec<AssetDeletionReferences>, RepositoryError> {
            Ok(self.references.clone())
        }
    }

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
            Ok((project_id == "prj_default").then(|| self.record()))
        }

        async fn insert(&self, _project: &ProjectRecord) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn update_metadata(
            &self,
            _project_id: &str,
            _name: &str,
            _description: Option<&str>,
            _updated_at: DateTime<Utc>,
        ) -> Result<Option<ProjectRecord>, RepositoryError> {
            Ok(None)
        }

        async fn get_storage_root(
            &self,
            project_id: &str,
        ) -> Result<Option<PathBuf>, RepositoryError> {
            Ok((project_id == "prj_default").then(|| self.root.clone()))
        }

        async fn ensure_default_project(
            &self,
            _project_id: &str,
            _name: &str,
            _root_path: &PathBuf,
            _created_at: DateTime<Utc>,
        ) -> Result<ProjectRecord, RepositoryError> {
            Ok(self.record())
        }
    }

    impl FakeProjectRepository {
        fn record(&self) -> ProjectRecord {
            let now = Utc::now();
            ProjectRecord {
                id: "prj_default".to_owned(),
                name: "Project".to_owned(),
                description: None,
                root_path: self.root.clone(),
                created_at: now,
                updated_at: now,
            }
        }
    }

    struct CleanupFailStore;

    #[async_trait]
    impl AssetStore for CleanupFailStore {
        async fn write_image(
            &self,
            _project_root: &Path,
            _asset_id: &AssetId,
            _extension: &str,
            _bytes: &[u8],
        ) -> Result<StoredAssetFile, AssetStoreError> {
            Err(AssetStoreError::Write("not used".to_owned()))
        }

        async fn delete(&self, _path: &Path) -> Result<(), AssetStoreError> {
            Ok(())
        }

        async fn read(&self, _path: &Path) -> Result<Vec<u8>, AssetStoreError> {
            Ok(Vec::new())
        }

        async fn stage_for_delete(
            &self,
            _project_root: &Path,
            _operation_id: &str,
            _asset: &Asset,
        ) -> Result<Vec<StagedAssetFile>, AssetStoreError> {
            Ok(Vec::new())
        }

        async fn commit_staged_delete(
            &self,
            _staged: &[StagedAssetFile],
        ) -> Result<(), AssetStoreError> {
            Err(AssetStoreError::Delete(
                "injected cleanup failure".to_owned(),
            ))
        }
    }

    fn image_asset(root: &Path, project_id: &str, id: &str) -> Asset {
        Asset::new_source_image(
            AssetId::parse(id).expect("asset id"),
            project_id,
            "Reference",
            "reference.png",
            root.join(format!("{id}.png")).to_string_lossy().to_string(),
            "a".repeat(64),
            "image/png",
            1,
            1,
            4,
            json!({}),
            Utc::now(),
        )
        .expect("asset should be valid")
    }

    fn service(
        root: &Path,
        asset: Asset,
        references: Vec<AssetDeletionReferences>,
        fail_delete: bool,
        store: Arc<dyn AssetStore>,
    ) -> AssetDeletionService {
        let mut assets = HashMap::new();
        assets.insert(asset.id.as_str().to_owned(), asset);
        AssetDeletionService::new(
            Arc::new(FakeAssetRepository {
                assets: Mutex::new(assets),
                fail_delete,
            }),
            Arc::new(FakeReferenceRepository { references }),
            Arc::new(FakeProjectRepository {
                root: root.to_path_buf(),
            }),
            store,
        )
    }

    #[tokio::test]
    async fn rejects_empty_unknown_cross_project_and_over_limit_inputs() {
        let root = tempdir().expect("root");
        let asset = image_asset(root.path(), "prj_default", "ast_one");
        let initial_service = service(
            root.path(),
            asset.clone(),
            Vec::new(),
            false,
            Arc::new(CleanupFailStore),
        );
        assert!(matches!(
            initial_service.inspect("prj_default", &[]).await,
            Err(AssetDeletionError::InvalidInput(_))
        ));
        assert!(matches!(
            initial_service
                .inspect("prj_default", &["ast_missing".to_owned()])
                .await,
            Err(AssetDeletionError::NotFound(_))
        ));
        assert!(matches!(
            initial_service
                .inspect("prj_default", &["ast_one".to_owned(), "ast_one".to_owned()])
                .await,
            Err(AssetDeletionError::InvalidInput(_))
        ));
        let over_limit = (0..101)
            .map(|index| format!("ast_{index}"))
            .collect::<Vec<_>>();
        assert!(matches!(
            initial_service.inspect("prj_default", &over_limit).await,
            Err(AssetDeletionError::InvalidInput(_))
        ));
        let cross_project_service = service(
            root.path(),
            image_asset(root.path(), "prj_other", "ast_cross"),
            Vec::new(),
            false,
            Arc::new(CleanupFailStore),
        );
        assert!(matches!(
            cross_project_service
                .inspect("prj_default", &["ast_cross".to_owned()])
                .await,
            Err(AssetDeletionError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn active_reference_blocks_and_historical_reference_warns() {
        let root = tempdir().expect("root");
        let asset = image_asset(root.path(), "prj_default", "ast_reference");
        let active_service = service(
            root.path(),
            asset.clone(),
            vec![AssetDeletionReferences {
                asset_id: asset.id.clone(),
                active_production_item_ids: vec!["pbi_active".to_owned()],
                ..Default::default()
            }],
            false,
            Arc::new(CleanupFailStore),
        );
        let error = active_service
            .delete("prj_default", &[asset.id.as_str().to_owned()])
            .await
            .expect_err("active reference should block deletion");
        assert!(
            matches!(error, AssetDeletionError::Blocked(AssetDeleteInspection { blocked, .. }) if blocked == vec!["ast_reference"])
        );

        let historical = service(
            root.path(),
            asset.clone(),
            vec![AssetDeletionReferences {
                asset_id: asset.id.clone(),
                historical_task_ids: vec![TaskId::parse("tsk_history").expect("task id")],
                ..Default::default()
            }],
            false,
            Arc::new(CleanupFailStore),
        );
        let inspection = historical
            .inspect("prj_default", &[asset.id.as_str().to_owned()])
            .await
            .expect("historical reference should be deletable");
        assert!(inspection.blocked.is_empty());
        assert_eq!(inspection.historical_references, vec!["ast_reference"]);
        assert!(inspection.items[0].warnings[0].contains("基于该历史输入的重试"));
    }

    #[tokio::test]
    async fn database_failure_restores_staged_files() {
        let root = tempdir().expect("root");
        let asset = image_asset(root.path(), "prj_default", "ast_rollback");
        std::fs::write(&asset.storage_path, b"asset").expect("asset file");
        let service = service(
            root.path(),
            asset.clone(),
            Vec::new(),
            true,
            Arc::new(crate::infrastructure::filesystem::FileSystemAssetStore::new()),
        );
        assert!(matches!(
            service
                .delete("prj_default", &[asset.id.as_str().to_owned()])
                .await,
            Err(AssetDeletionError::Repository(_))
        ));
        assert_eq!(
            std::fs::read(&asset.storage_path).expect("restored asset"),
            b"asset"
        );
    }

    #[tokio::test]
    async fn cleanup_failure_keeps_database_success_and_returns_warning() {
        let root = tempdir().expect("root");
        let asset = image_asset(root.path(), "prj_default", "ast_cleanup_warning");
        let service = service(
            root.path(),
            asset.clone(),
            Vec::new(),
            false,
            Arc::new(CleanupFailStore),
        );
        let result = service
            .delete("prj_default", &[asset.id.as_str().to_owned()])
            .await
            .expect("database delete should remain successful");
        assert_eq!(result.deleted_count, 1);
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.contains("清理失败")));
    }
}
