use crate::application::ports::{AssetRepository, RepositoryError};
use crate::domain::{
    AssetId, AssetRelation, AssetRelationId, AssetRelationType, AssetVersion, AssetVersionId,
};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{error::Error, fmt, sync::Arc};

pub struct AssetDataService {
    repository: Arc<dyn AssetRepository>,
}

impl AssetDataService {
    pub fn new(repository: Arc<dyn AssetRepository>) -> Self {
        Self { repository }
    }

    pub async fn create_version(
        &self,
        project_id: &str,
        asset_id: &str,
        version_number: u32,
        metadata_snapshot: Value,
        location: impl Into<String>,
        checksum: impl Into<String>,
        created_at: DateTime<Utc>,
    ) -> Result<AssetVersion, AssetDataError> {
        let asset_id = self.asset_in_project(project_id, asset_id).await?;
        let version = AssetVersion::new(
            AssetVersionId::new(),
            project_id.trim(),
            asset_id,
            version_number,
            metadata_snapshot,
            location,
            checksum,
            created_at,
        )
        .map_err(|error| AssetDataError::InvalidInput(error.to_string()))?;
        self.repository.insert_asset_version(&version).await?;
        Ok(version)
    }

    pub async fn list_versions(
        &self,
        project_id: &str,
        asset_id: &str,
    ) -> Result<Vec<AssetVersion>, AssetDataError> {
        let asset_id = self.asset_in_project(project_id, asset_id).await?;
        Ok(self
            .repository
            .list_asset_versions(project_id.trim(), &asset_id)
            .await?)
    }

    pub async fn current_version(
        &self,
        project_id: &str,
        asset_id: &str,
    ) -> Result<Option<AssetVersion>, AssetDataError> {
        let asset_id = self.asset_in_project(project_id, asset_id).await?;
        Ok(self
            .repository
            .current_asset_version(project_id.trim(), &asset_id)
            .await?)
    }

    pub async fn create_relation(
        &self,
        project_id: &str,
        source_asset_id: &str,
        target_asset_id: &str,
        relation_type: AssetRelationType,
        created_at: DateTime<Utc>,
    ) -> Result<AssetRelation, AssetDataError> {
        let source_asset_id = self.asset_in_project(project_id, source_asset_id).await?;
        let target_asset_id = self.asset_in_project(project_id, target_asset_id).await?;
        let relation = AssetRelation::new(
            AssetRelationId::new(),
            project_id.trim(),
            source_asset_id,
            target_asset_id,
            relation_type,
            created_at,
        )
        .map_err(|error| AssetDataError::InvalidInput(error.to_string()))?;
        self.repository.insert_asset_relation(&relation).await?;
        Ok(relation)
    }

    pub async fn list_relations(
        &self,
        project_id: &str,
        asset_id: &str,
    ) -> Result<Vec<AssetRelation>, AssetDataError> {
        let asset_id = self.asset_in_project(project_id, asset_id).await?;
        Ok(self
            .repository
            .list_asset_relations(project_id.trim(), &asset_id)
            .await?)
    }

    pub async fn remove_relation(
        &self,
        project_id: &str,
        relation_id: &str,
    ) -> Result<(), AssetDataError> {
        validate_project_id(project_id)?;
        let relation_id = AssetRelationId::parse(relation_id.to_owned())
            .map_err(|error| AssetDataError::InvalidInput(error.to_string()))?;
        self.repository
            .delete_asset_relation(project_id.trim(), &relation_id)
            .await?;
        Ok(())
    }

    async fn asset_in_project(
        &self,
        project_id: &str,
        asset_id: &str,
    ) -> Result<AssetId, AssetDataError> {
        validate_project_id(project_id)?;
        let asset_id = AssetId::parse(asset_id.to_owned())
            .map_err(|error| AssetDataError::InvalidInput(error.to_string()))?;
        let asset = self.repository.find_by_id(&asset_id).await?;
        match asset {
            Some(asset) if asset.project_id == project_id.trim() => Ok(asset_id),
            _ => Err(AssetDataError::NotFound(asset_id.as_str().to_owned())),
        }
    }
}

fn validate_project_id(project_id: &str) -> Result<(), AssetDataError> {
    if project_id.trim().is_empty() {
        return Err(AssetDataError::InvalidInput(
            "project id must not be empty".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Debug)]
pub enum AssetDataError {
    InvalidInput(String),
    NotFound(String),
    Repository(RepositoryError),
}

impl fmt::Display for AssetDataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(formatter, "invalid asset data input: {message}"),
            Self::NotFound(id) => write!(formatter, "asset data target was not found: {id}"),
            Self::Repository(error) => write!(formatter, "asset data repository failed: {error}"),
        }
    }
}

impl Error for AssetDataError {}

impl From<RepositoryError> for AssetDataError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

#[cfg(test)]
mod tests {
    use super::AssetDataService;
    use crate::application::ports::{AssetRepository, TaskRepository};
    use crate::domain::{Asset, AssetId, AssetRelationType, Task, TaskId};
    use crate::infrastructure::database::{
        initialize, repositories::test_support, SqliteAssetRepository, SqliteTaskRepository,
    };
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tempfile::tempdir;

    async fn setup() -> (
        tempfile::TempDir,
        AssetDataService,
        SqliteAssetRepository,
        Task,
        SqlitePool,
    ) {
        let directory = tempdir().expect("temporary directory");
        let pool = initialize(&directory.path().join("app.db"))
            .await
            .expect("database should initialize");
        test_support::seed_task_dependencies(&pool).await;
        let task = Task::new(
            "project-1",
            "workflow-1",
            "workflow-version-1",
            "recipe-1",
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        );
        SqliteTaskRepository::new(pool.clone())
            .create(&task, &task.created_event())
            .await
            .expect("task fixture");
        let repository = SqliteAssetRepository::new(pool.clone());
        let service = AssetDataService::new(Arc::new(repository.clone()));
        (directory, service, repository, task, pool)
    }

    fn asset(task_id: &TaskId, id: &str, name: &str) -> Asset {
        Asset::new_image(
            AssetId::parse(id).unwrap(),
            "project-1",
            name,
            format!("{name}.png"),
            format!("C:/project/assets/source/image/{id}.png"),
            format!("{id:0<64}"),
            "image/png",
            2,
            2,
            64,
            task_id.clone(),
            json!({"source": "test"}),
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 1).unwrap(),
        )
        .unwrap()
    }

    fn source_asset(project_id: &str, id: &str, name: &str) -> Asset {
        Asset::new_source_image(
            AssetId::parse(id).unwrap(),
            project_id,
            name,
            format!("{name}.png"),
            format!("C:/{project_id}/{id}.png"),
            format!("{id:0<64}"),
            "image/png",
            2,
            2,
            64,
            json!({"source": "test"}),
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 1).unwrap(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn create_and_query_asset_versions_and_current_version() {
        let (_directory, service, repository, task, _pool) = setup().await;
        let image = asset(&task.id, "ast_versioned", "Versioned");
        repository
            .insert_many(std::slice::from_ref(&image))
            .await
            .unwrap();

        let first = service
            .create_version(
                "project-1",
                image.id.as_str(),
                1,
                json!({"label": "first"}),
                "assets/source/image/ast_versioned-v1.png",
                "a".repeat(64),
                Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 2).unwrap(),
            )
            .await
            .unwrap();
        let second = service
            .create_version(
                "project-1",
                image.id.as_str(),
                2,
                json!({"label": "second"}),
                "assets/source/image/ast_versioned-v2.png",
                "b".repeat(64),
                Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 3).unwrap(),
            )
            .await
            .unwrap();

        assert_ne!(first.id, second.id);
        assert_eq!(
            service
                .list_versions("project-1", image.id.as_str())
                .await
                .unwrap()
                .iter()
                .map(|version| version.version_number)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(
            service
                .current_version("project-1", image.id.as_str())
                .await
                .unwrap(),
            Some(second)
        );
        assert!(service
            .create_version(
                "project-1",
                image.id.as_str(),
                2,
                json!({"label": "duplicate"}),
                "assets/source/image/duplicate.png",
                "c".repeat(64),
                Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 4).unwrap(),
            )
            .await
            .is_err());
    }

    #[tokio::test]
    async fn create_query_and_remove_asset_relations() {
        let (_directory, service, repository, task, _pool) = setup().await;
        let source = asset(&task.id, "ast_relation_source", "Source");
        let target = asset(&task.id, "ast_relation_target", "Target");
        repository
            .insert_many(&[source.clone(), target.clone()])
            .await
            .unwrap();

        service
            .create_version(
                "project-1",
                source.id.as_str(),
                1,
                json!({"label": "history"}),
                "assets/source/image/ast_relation_source-v1.png",
                "d".repeat(64),
                Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 2).unwrap(),
            )
            .await
            .unwrap();

        let relation = service
            .create_relation(
                "project-1",
                source.id.as_str(),
                target.id.as_str(),
                AssetRelationType::DerivedFrom,
                Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 2).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(relation.source_asset_id, source.id);
        assert_eq!(
            service
                .list_relations("project-1", source.id.as_str())
                .await
                .unwrap(),
            vec![relation.clone()]
        );
        assert_eq!(
            service
                .list_relations("project-1", target.id.as_str())
                .await
                .unwrap(),
            vec![relation.clone()]
        );
        service
            .remove_relation("project-1", relation.id.as_str())
            .await
            .unwrap();
        assert!(service
            .list_relations("project-1", source.id.as_str())
            .await
            .unwrap()
            .is_empty());
        assert_eq!(
            service
                .list_versions("project-1", source.id.as_str())
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn relation_rejects_self_links() {
        let (_directory, service, repository, task, _pool) = setup().await;
        let image = asset(&task.id, "ast_self_relation", "Self");
        repository
            .insert_many(std::slice::from_ref(&image))
            .await
            .unwrap();

        assert!(service
            .create_relation(
                "project-1",
                image.id.as_str(),
                image.id.as_str(),
                AssetRelationType::Related,
                Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 2).unwrap(),
            )
            .await
            .is_err());
    }

    #[tokio::test]
    async fn project_scope_rejects_cross_project_version_and_relation_access() {
        let (_directory, service, repository, _task, pool) = setup().await;
        sqlx::query(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at)
             VALUES ('project-2', 'Other', 'C:/other', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let project_one_source = source_asset("project-1", "ast_scope_a1", "A one");
        let project_one_target = source_asset("project-1", "ast_scope_a2", "A two");
        let project_two_source = source_asset("project-2", "ast_scope_b1", "B one");
        let project_two_target = source_asset("project-2", "ast_scope_b2", "B two");
        repository
            .insert_many(&[
                project_one_source.clone(),
                project_one_target.clone(),
                project_two_source.clone(),
                project_two_target.clone(),
            ])
            .await
            .unwrap();

        let a_version = service
            .create_version(
                "project-1",
                project_one_source.id.as_str(),
                1,
                json!({"project": "a"}),
                "C:/project-1/a-v1.png",
                "a".repeat(64),
                Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 2).unwrap(),
            )
            .await
            .unwrap();
        let b_version = service
            .create_version(
                "project-2",
                project_two_source.id.as_str(),
                1,
                json!({"project": "b"}),
                "C:/project-2/b-v1.png",
                "b".repeat(64),
                Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 3).unwrap(),
            )
            .await
            .unwrap();
        let a_relation = service
            .create_relation(
                "project-1",
                project_one_source.id.as_str(),
                project_one_target.id.as_str(),
                AssetRelationType::DerivedFrom,
                Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 4).unwrap(),
            )
            .await
            .unwrap();
        let b_relation = service
            .create_relation(
                "project-2",
                project_two_source.id.as_str(),
                project_two_target.id.as_str(),
                AssetRelationType::VariantOf,
                Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 5).unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            service
                .list_versions("project-1", project_one_source.id.as_str())
                .await
                .unwrap(),
            vec![a_version]
        );
        assert_eq!(
            service
                .list_versions("project-2", project_two_source.id.as_str())
                .await
                .unwrap(),
            vec![b_version]
        );
        assert!(matches!(
            service
                .list_versions("project-2", project_one_source.id.as_str())
                .await,
            Err(super::AssetDataError::NotFound(_))
        ));
        assert!(matches!(
            service
                .list_versions("project-1", project_two_source.id.as_str())
                .await,
            Err(super::AssetDataError::NotFound(_))
        ));
        assert_eq!(
            service
                .list_relations("project-1", project_one_source.id.as_str())
                .await
                .unwrap(),
            vec![a_relation]
        );
        assert_eq!(
            service
                .list_relations("project-2", project_two_source.id.as_str())
                .await
                .unwrap(),
            vec![b_relation]
        );
        assert!(matches!(
            service
                .list_relations("project-2", project_one_source.id.as_str())
                .await,
            Err(super::AssetDataError::NotFound(_))
        ));
        assert!(matches!(
            service
                .list_relations("project-1", project_two_source.id.as_str())
                .await,
            Err(super::AssetDataError::NotFound(_))
        ));
        assert!(service
            .create_relation(
                "project-1",
                project_one_source.id.as_str(),
                project_two_source.id.as_str(),
                AssetRelationType::Related,
                Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 6).unwrap(),
            )
            .await
            .is_err());
    }
}
