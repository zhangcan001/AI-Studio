use super::{
    format_datetime, i64_to_u64, map_domain_error, map_sqlx_error, parse_datetime, parse_json,
    serialize_json,
};
use crate::application::ports::{AssetRepository, RepositoryError};
use crate::domain::{Asset, AssetId, AssetType, TaskId};
use async_trait::async_trait;
use sqlx::{Sqlite, SqlitePool, Transaction};

#[derive(Clone)]
pub struct SqliteAssetRepository {
    pool: SqlitePool,
}

impl SqliteAssetRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AssetRepository for SqliteAssetRepository {
    async fn insert_many(&self, assets: &[Asset]) -> Result<(), RepositoryError> {
        if assets.is_empty() {
            return Ok(());
        }

        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        for asset in assets {
            asset
                .validate()
                .map_err(|error| map_domain_error("asset validation", error))?;
            insert_asset(&mut transaction, asset).await?;
        }
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn find_by_id(&self, asset_id: &AssetId) -> Result<Option<Asset>, RepositoryError> {
        let row = sqlx::query_as::<_, AssetRow>(&format!("{ASSET_SELECT} WHERE id = ?"))
            .bind(asset_id.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        row.map(AssetRow::try_into_domain).transpose()
    }

    async fn list_by_source_task(&self, task_id: &TaskId) -> Result<Vec<Asset>, RepositoryError> {
        let rows = sqlx::query_as::<_, AssetRow>(&format!(
            "{ASSET_SELECT} WHERE source_task_id = ? ORDER BY created_at ASC, id ASC"
        ))
        .bind(task_id.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter().map(AssetRow::try_into_domain).collect()
    }

    async fn list_recent(
        &self,
        project_id: &str,
        limit: u32,
    ) -> Result<Vec<Asset>, RepositoryError> {
        let rows = sqlx::query_as::<_, AssetRow>(&format!(
            "{ASSET_SELECT} WHERE project_id = ? ORDER BY created_at DESC, id DESC LIMIT ?"
        ))
        .bind(project_id)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter().map(AssetRow::try_into_domain).collect()
    }
}

const ASSET_SELECT: &str = "SELECT
    id, project_id, type, category, name, original_name, storage_path,
    thumbnail_path, sha256, mime_type, width, height, file_size,
    source_task_id, metadata_json, created_at, updated_at
    FROM assets";

async fn insert_asset(
    transaction: &mut Transaction<'_, Sqlite>,
    asset: &Asset,
) -> Result<(), RepositoryError> {
    let metadata_json = serialize_json("asset metadata_json", Some(&asset.metadata_json))?
        .ok_or_else(|| RepositoryError::serialization("asset metadata_json", "missing value"))?;
    sqlx::query(
        "INSERT INTO assets (
            id, project_id, type, category, name, original_name, storage_path,
            thumbnail_path, sha256, mime_type, width, height, file_size,
            source_task_id, metadata_json, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(asset.id.as_str())
    .bind(&asset.project_id)
    .bind(asset.asset_type.as_str())
    .bind(&asset.category)
    .bind(&asset.name)
    .bind(&asset.original_name)
    .bind(&asset.storage_path)
    .bind(&asset.thumbnail_path)
    .bind(&asset.sha256)
    .bind(&asset.mime_type)
    .bind(i64::from(asset.width))
    .bind(i64::from(asset.height))
    .bind(
        i64::try_from(asset.file_size).map_err(|_| {
            RepositoryError::integrity("asset file_size exceeds SQLite integer range")
        })?,
    )
    .bind(asset.source_task_id.as_ref().map(TaskId::as_str))
    .bind(metadata_json)
    .bind(format_datetime(asset.created_at))
    .bind(format_datetime(asset.updated_at))
    .execute(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct AssetRow {
    id: String,
    project_id: String,
    r#type: String,
    category: Option<String>,
    name: String,
    original_name: Option<String>,
    storage_path: String,
    thumbnail_path: Option<String>,
    sha256: String,
    mime_type: Option<String>,
    width: Option<i64>,
    height: Option<i64>,
    file_size: Option<i64>,
    source_task_id: Option<String>,
    metadata_json: Option<String>,
    created_at: String,
    updated_at: String,
}

impl AssetRow {
    fn try_into_domain(self) -> Result<Asset, RepositoryError> {
        let width = self
            .width
            .ok_or_else(|| RepositoryError::serialization("asset width", "missing image width"))?;
        let height = self.height.ok_or_else(|| {
            RepositoryError::serialization("asset height", "missing image height")
        })?;
        let file_size = self.file_size.ok_or_else(|| {
            RepositoryError::serialization("asset file_size", "missing file size")
        })?;
        let source_task_id = self
            .source_task_id
            .map(|value| {
                TaskId::parse(value)
                    .map_err(|error| map_domain_error("asset source_task_id", error))
            })
            .transpose()?;
        let metadata_json = parse_json("asset metadata_json", self.metadata_json.as_deref())?
            .ok_or_else(|| {
                RepositoryError::serialization("asset metadata_json", "missing value")
            })?;
        let asset = Asset {
            id: AssetId::parse(self.id).map_err(|error| map_domain_error("asset id", error))?,
            project_id: self.project_id,
            asset_type: AssetType::try_from_db(&self.r#type)
                .map_err(|error| map_domain_error("asset type", error))?,
            category: self.category.ok_or_else(|| {
                RepositoryError::serialization("asset category", "missing category")
            })?,
            name: self.name,
            original_name: self.original_name.ok_or_else(|| {
                RepositoryError::serialization("asset original_name", "missing original name")
            })?,
            storage_path: self.storage_path,
            thumbnail_path: self.thumbnail_path,
            sha256: self.sha256,
            mime_type: self.mime_type.ok_or_else(|| {
                RepositoryError::serialization("asset mime_type", "missing MIME type")
            })?,
            width: u32::try_from(width).map_err(|_| {
                RepositoryError::serialization("asset width", format!("invalid value {width}"))
            })?,
            height: u32::try_from(height).map_err(|_| {
                RepositoryError::serialization("asset height", format!("invalid value {height}"))
            })?,
            file_size: i64_to_u64("asset file_size", file_size)?,
            source_task_id,
            metadata_json,
            created_at: parse_datetime("asset created_at", &self.created_at)?,
            updated_at: parse_datetime("asset updated_at", &self.updated_at)?,
        };
        asset
            .validate()
            .map_err(|error| map_domain_error("asset integrity", error))?;
        Ok(asset)
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteAssetRepository;
    use crate::application::ports::{AssetRepository, TaskRepository};
    use crate::domain::{Asset, AssetId, Task, TaskId};
    use crate::infrastructure::database::{
        initialize,
        repositories::{test_support, SqliteTaskRepository},
    };
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use sqlx::SqlitePool;
    use tempfile::{tempdir, TempDir};

    async fn setup() -> (TempDir, SqlitePool, Task, SqliteAssetRepository) {
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
        (
            directory,
            pool.clone(),
            task,
            SqliteAssetRepository::new(pool),
        )
    }

    fn asset(task_id: &TaskId, id: &str) -> Asset {
        Asset::new_image(
            AssetId::parse(id).unwrap(),
            "project-1",
            "image",
            "ComfyUI_00001.png",
            format!("C:/project/assets/generated/image/{id}.png"),
            "a".repeat(64),
            "image/png",
            1,
            1,
            67,
            task_id.clone(),
            json!({"position": 0}),
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 1).unwrap(),
        )
        .unwrap()
    }

    fn source_asset(id: &str) -> Asset {
        Asset::new_source_image(
            AssetId::parse(id).unwrap(),
            "project-1",
            "reference.png",
            "reference.png",
            "C:/project/assets/source/image/reference.png",
            "b".repeat(64),
            "image/png",
            2,
            3,
            67,
            json!({"source": "native_import"}),
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 2).unwrap(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn insert_many_round_trips_and_lists_by_task() {
        let (_directory, _pool, task, repository) = setup().await;
        let first = asset(&task.id, "ast_one");
        let second = asset(&task.id, "ast_two");
        repository
            .insert_many(&[first.clone(), second.clone()])
            .await
            .unwrap();
        assert_eq!(repository.find_by_id(&first.id).await.unwrap(), Some(first));
        assert_eq!(
            repository
                .list_by_source_task(&task.id)
                .await
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            repository.list_recent("project-1", 1).await.unwrap().len(),
            1
        );
    }

    #[tokio::test]
    async fn insert_many_is_one_transaction() {
        let (_directory, _pool, task, repository) = setup().await;
        let first = asset(&task.id, "ast_one");
        let duplicate = asset(&task.id, "ast_one");
        assert!(repository.insert_many(&[first, duplicate]).await.is_err());
        assert!(repository
            .find_by_id(&AssetId::parse("ast_one").unwrap())
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn source_asset_round_trips_with_null_source_task() {
        let (_directory, _pool, _task, repository) = setup().await;
        let source = source_asset("ast_source_one");
        repository
            .insert_many(std::slice::from_ref(&source))
            .await
            .unwrap();
        let loaded = repository
            .find_by_id(&source.id)
            .await
            .unwrap()
            .expect("source asset should load");
        assert_eq!(loaded, source);
        assert!(loaded.source_task_id.is_none());
    }
}
