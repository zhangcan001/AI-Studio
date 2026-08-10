use super::{format_datetime, map_sqlx_error, parse_datetime};
use crate::application::ports::{
    AssetVideoPromptRecord, AssetVideoPromptRepository, RepositoryError,
};
use async_trait::async_trait;
use sqlx::{FromRow, QueryBuilder, Sqlite, SqlitePool};

#[derive(Clone)]
pub struct SqliteAssetVideoPromptRepository {
    pool: SqlitePool,
}

impl SqliteAssetVideoPromptRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AssetVideoPromptRepository for SqliteAssetVideoPromptRepository {
    async fn find(
        &self,
        project_id: &str,
        asset_id: &str,
    ) -> Result<Option<AssetVideoPromptRecord>, RepositoryError> {
        let row = sqlx::query_as::<_, AssetVideoPromptRow>(
            "SELECT asset_id, project_id, prompt_text, updated_at
             FROM asset_video_prompts WHERE project_id = ? AND asset_id = ?",
        )
        .bind(project_id)
        .bind(asset_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        row.map(AssetVideoPromptRow::try_into_record).transpose()
    }

    async fn list(
        &self,
        project_id: &str,
        asset_ids: &[String],
    ) -> Result<Vec<AssetVideoPromptRecord>, RepositoryError> {
        if asset_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT asset_id, project_id, prompt_text, updated_at
             FROM asset_video_prompts WHERE project_id = ",
        );
        query.push_bind(project_id).push(" AND asset_id IN (");
        let mut separated = query.separated(", ");
        for asset_id in asset_ids {
            separated.push_bind(asset_id);
        }
        separated.push_unseparated(") ORDER BY updated_at DESC, asset_id ASC");
        let rows = query
            .build_query_as::<AssetVideoPromptRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(AssetVideoPromptRow::try_into_record)
            .collect()
    }

    async fn upsert(&self, record: &AssetVideoPromptRecord) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO asset_video_prompts (asset_id, project_id, prompt_text, updated_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(asset_id) DO UPDATE SET
                 project_id = excluded.project_id,
                 prompt_text = excluded.prompt_text,
                 updated_at = excluded.updated_at",
        )
        .bind(&record.asset_id)
        .bind(&record.project_id)
        .bind(&record.prompt_text)
        .bind(format_datetime(record.updated_at))
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }
}

#[derive(FromRow)]
struct AssetVideoPromptRow {
    asset_id: String,
    project_id: String,
    prompt_text: String,
    updated_at: String,
}

impl AssetVideoPromptRow {
    fn try_into_record(self) -> Result<AssetVideoPromptRecord, RepositoryError> {
        Ok(AssetVideoPromptRecord {
            asset_id: self.asset_id,
            project_id: self.project_id,
            prompt_text: self.prompt_text,
            updated_at: parse_datetime("asset video prompt updated_at", &self.updated_at)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteAssetVideoPromptRepository;
    use crate::application::ports::{AssetVideoPromptRecord, AssetVideoPromptRepository};
    use crate::infrastructure::database::{initialize, repositories::test_support};
    use chrono::{TimeZone, Utc};
    use tempfile::tempdir;

    #[tokio::test]
    async fn upsert_is_project_scoped_and_lists_requested_assets() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        sqlx::query(
            "INSERT INTO assets (id, project_id, type, category, name, original_name, storage_path, sha256, mime_type, width, height, file_size, metadata_json, created_at, updated_at)
             VALUES ('ast_image', 'project-1', 'image', 'source_image', 'Image', 'image.png', 'assets/image.png', 'sha', 'image/png', 1, 1, 1, '{}', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let repository = SqliteAssetVideoPromptRepository::new(pool.clone());
        let now = Utc.with_ymd_and_hms(2026, 8, 10, 0, 0, 0).unwrap();
        repository
            .upsert(&AssetVideoPromptRecord {
                asset_id: "ast_image".to_owned(),
                project_id: "project-1".to_owned(),
                prompt_text: "camera moves slowly".to_owned(),
                updated_at: now,
            })
            .await
            .unwrap();

        let records = repository
            .list(
                "project-1",
                &["ast_image".to_owned(), "ast_missing".to_owned()],
            )
            .await
            .unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].prompt_text, "camera moves slowly");
        assert!(repository
            .find("project-2", "ast_image")
            .await
            .unwrap()
            .is_none());
        pool.close().await;
    }
}
