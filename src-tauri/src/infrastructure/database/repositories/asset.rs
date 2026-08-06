use super::{
    format_datetime, i64_to_u64, map_domain_error, map_sqlx_error, parse_datetime, parse_json,
    serialize_json,
};
use crate::application::ports::{AssetRepository, RepositoryError, TaskOutputAssetMapping};
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

    async fn insert_generated_outputs(
        &self,
        assets: &[Asset],
        mappings: &[TaskOutputAssetMapping],
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        for asset in assets {
            asset
                .validate()
                .map_err(|error| map_domain_error("asset validation", error))?;
            insert_asset(&mut transaction, asset).await?;
        }
        for mapping in mappings {
            sqlx::query(
                "INSERT INTO task_output_assets
                    (task_id, output_id, ordinal, asset_id, created_at)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(mapping.task_id.as_str())
            .bind(&mapping.output_id)
            .bind(i64::from(mapping.ordinal))
            .bind(mapping.asset_id.as_str())
            .bind(format_datetime(mapping.created_at))
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
        }
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn list_output_mappings(
        &self,
        task_id: &TaskId,
    ) -> Result<Vec<TaskOutputAssetMapping>, RepositoryError> {
        let rows = sqlx::query_as::<_, TaskOutputAssetMappingRow>(
            "SELECT task_id, output_id, ordinal, asset_id, created_at
             FROM task_output_assets
             WHERE task_id = ?
             ORDER BY output_id ASC, ordinal ASC",
        )
        .bind(task_id.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(TaskOutputAssetMappingRow::try_into_domain)
            .collect()
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
    thumbnail_path, sha256, mime_type, width, height, duration_ms, file_size,
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
            thumbnail_path, sha256, mime_type, width, height, duration_ms, file_size,
            source_task_id, metadata_json, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
        asset
            .duration_ms
            .map(|value| {
                i64::try_from(value).map_err(|_| {
                    RepositoryError::integrity("asset duration_ms exceeds SQLite integer range")
                })
            })
            .transpose()?,
    )
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
    duration_ms: Option<i64>,
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
            .map(|value| {
                u32::try_from(value).map_err(|_| {
                    RepositoryError::serialization("asset width", format!("invalid value {value}"))
                })
            })
            .transpose()?
            .unwrap_or_default();
        let height = self
            .height
            .map(|value| {
                u32::try_from(value).map_err(|_| {
                    RepositoryError::serialization("asset height", format!("invalid value {value}"))
                })
            })
            .transpose()?
            .unwrap_or_default();
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
            width,
            height,
            duration_ms: self
                .duration_ms
                .map(|value| i64_to_u64("asset duration_ms", value))
                .transpose()?,
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

#[derive(sqlx::FromRow)]
struct TaskOutputAssetMappingRow {
    task_id: String,
    output_id: String,
    ordinal: i64,
    asset_id: String,
    created_at: String,
}

impl TaskOutputAssetMappingRow {
    fn try_into_domain(self) -> Result<TaskOutputAssetMapping, RepositoryError> {
        Ok(TaskOutputAssetMapping {
            task_id: TaskId::parse(self.task_id)
                .map_err(|error| map_domain_error("task_output_assets task_id", error))?,
            output_id: self.output_id,
            ordinal: u32::try_from(self.ordinal).map_err(|_| {
                RepositoryError::serialization(
                    "task_output_assets ordinal",
                    format!("invalid value {}", self.ordinal),
                )
            })?,
            asset_id: AssetId::parse(self.asset_id)
                .map_err(|error| map_domain_error("task_output_assets asset_id", error))?,
            created_at: parse_datetime("task_output_assets created_at", &self.created_at)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteAssetRepository;
    use crate::application::ports::{AssetRepository, TaskOutputAssetMapping, TaskRepository};
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

    #[tokio::test]
    async fn generated_video_and_output_mapping_round_trip_atomically() {
        let (_directory, _pool, task, repository) = setup().await;
        let video = Asset::new_generated_video(
            AssetId::parse("ast_video_one").unwrap(),
            "project-1",
            "Generated Video 1",
            "ComfyUI_00001.mp4",
            "C:/project/assets/generated/video/ast_video_one.mp4",
            "c".repeat(64),
            "video/mp4",
            Some(1280),
            Some(720),
            Some(1500),
            1024,
            task.id.clone(),
            json!({"mediaKind": "video"}),
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 3).unwrap(),
        )
        .unwrap();
        let mapping = TaskOutputAssetMapping {
            task_id: task.id.clone(),
            output_id: "generated_video".to_owned(),
            ordinal: 0,
            asset_id: video.id.clone(),
            created_at: video.created_at,
        };
        repository
            .insert_generated_outputs(std::slice::from_ref(&video), std::slice::from_ref(&mapping))
            .await
            .unwrap();
        assert_eq!(
            repository.find_by_id(&video.id).await.unwrap(),
            Some(video.clone())
        );
        assert_eq!(
            repository.list_output_mappings(&task.id).await.unwrap(),
            vec![mapping.clone()]
        );
        assert_eq!(
            repository.list_mapped_assets(&task.id).await.unwrap(),
            vec![(mapping, video)]
        );
    }

    #[tokio::test]
    async fn mapping_conflict_rolls_back_asset_and_mapping_together() {
        let (_directory, _pool, task, repository) = setup().await;
        let first = Asset::new_generated_video(
            AssetId::parse("ast_video_one").unwrap(),
            "project-1",
            "Generated Video 1",
            "one.mp4",
            "C:/project/assets/generated/video/ast_video_one.mp4",
            "c".repeat(64),
            "video/mp4",
            None,
            None,
            None,
            1024,
            task.id.clone(),
            json!({}),
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 3).unwrap(),
        )
        .unwrap();
        let first_mapping = TaskOutputAssetMapping {
            task_id: task.id.clone(),
            output_id: "generated_video".to_owned(),
            ordinal: 0,
            asset_id: first.id.clone(),
            created_at: first.created_at,
        };
        repository
            .insert_generated_outputs(
                std::slice::from_ref(&first),
                std::slice::from_ref(&first_mapping),
            )
            .await
            .unwrap();
        let second = Asset::new_generated_video(
            AssetId::parse("ast_video_two").unwrap(),
            "project-1",
            "Generated Video 2",
            "two.mp4",
            "C:/project/assets/generated/video/ast_video_two.mp4",
            "d".repeat(64),
            "video/mp4",
            None,
            None,
            None,
            1024,
            task.id.clone(),
            json!({}),
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 4).unwrap(),
        )
        .unwrap();
        let conflict = TaskOutputAssetMapping {
            task_id: task.id.clone(),
            output_id: "generated_video".to_owned(),
            ordinal: 0,
            asset_id: second.id.clone(),
            created_at: second.created_at,
        };
        assert!(repository
            .insert_generated_outputs(
                std::slice::from_ref(&second),
                std::slice::from_ref(&conflict)
            )
            .await
            .is_err());
        assert!(repository.find_by_id(&second.id).await.unwrap().is_none());
        assert_eq!(
            repository
                .list_output_mappings(&task.id)
                .await
                .unwrap()
                .len(),
            1
        );
    }
}
