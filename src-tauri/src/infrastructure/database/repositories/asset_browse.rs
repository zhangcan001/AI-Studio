use super::{i64_to_u64, map_domain_error, map_sqlx_error, parse_datetime, parse_json};
use crate::application::pagination::{PageCursor, PageResult};
use crate::application::ports::{AssetBrowseRepository, AssetCategoryFilter, RepositoryError};
use crate::domain::{Asset, AssetId, AssetType, TaskId};
use async_trait::async_trait;
use sqlx::{QueryBuilder, Sqlite, SqlitePool};

#[derive(Clone)]
pub struct SqliteAssetBrowseRepository {
    pool: SqlitePool,
}

impl SqliteAssetBrowseRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

const ASSET_BROWSE_SELECT: &str = "SELECT
    id, project_id, type AS asset_type, category, name, original_name,
    storage_path, thumbnail_path, sha256, mime_type, width, height, duration_ms, file_size,
    source_task_id, metadata_json, created_at, updated_at
    FROM assets";

#[async_trait]
impl AssetBrowseRepository for SqliteAssetBrowseRepository {
    async fn list_page(
        &self,
        project_id: &str,
        category: AssetCategoryFilter,
        cursor: Option<PageCursor>,
        limit: u32,
    ) -> Result<PageResult<Asset>, RepositoryError> {
        let requested_limit = limit.clamp(1, 100);
        let mut query = QueryBuilder::<Sqlite>::new(ASSET_BROWSE_SELECT);
        query
            .push(" WHERE project_id = ")
            .push_bind(project_id.to_owned());
        if let Some(category) = category.category() {
            query.push(" AND category = ").push_bind(category);
        }
        if let Some(cursor) = cursor {
            let created_at = cursor.created_at.to_rfc3339();
            query
                .push(" AND (created_at < ")
                .push_bind(created_at.clone())
                .push(" OR (created_at = ")
                .push_bind(created_at)
                .push(" AND id < ")
                .push_bind(cursor.id)
                .push("))");
        }
        query
            .push(" ORDER BY created_at DESC, id DESC LIMIT ")
            .push_bind(i64::from(requested_limit) + 1);
        let mut rows = query
            .build_query_as::<AssetBrowseRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        let has_more = rows.len() > requested_limit as usize;
        if has_more {
            rows.truncate(requested_limit as usize);
        }
        let next_cursor = has_more
            .then(|| rows.last())
            .flatten()
            .map(|row| PageCursor::for_item(row.created_at_value(), row.id.clone()));
        let items = rows
            .into_iter()
            .map(AssetBrowseRow::try_into_domain)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(PageResult { items, next_cursor })
    }
}

#[derive(sqlx::FromRow)]
struct AssetBrowseRow {
    id: String,
    project_id: String,
    asset_type: String,
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

impl AssetBrowseRow {
    fn created_at_value(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(&self.created_at)
            .map(|value| value.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now())
    }

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
            asset_type: AssetType::try_from_db(&self.asset_type)
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

#[cfg(test)]
mod tests {
    use super::SqliteAssetBrowseRepository;
    use crate::application::ports::{AssetBrowseRepository, AssetCategoryFilter, AssetRepository};
    use crate::domain::{Asset, AssetId};
    use crate::infrastructure::database::{
        initialize, repositories::test_support, SqliteAssetRepository,
    };
    use chrono::{Duration, TimeZone, Utc};
    use serde_json::json;
    use tempfile::tempdir;

    #[tokio::test]
    async fn filters_by_project_and_category_with_keyset_pages() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        let repository = SqliteAssetRepository::new(pool.clone());
        let base = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        for (index, category) in ["source_image", "generated_image", "generated_image"]
            .iter()
            .enumerate()
        {
            let asset = if *category == "source_image" {
                Asset::new_source_image(
                    AssetId::parse(format!("ast_browse_{index}")).unwrap(),
                    "project-1",
                    format!("image-{index}.png"),
                    format!("image-{index}.png"),
                    format!("C:/image-{index}.png"),
                    "a".repeat(64),
                    "image/png",
                    1,
                    1,
                    1,
                    json!({"category": category}),
                    base + Duration::seconds(index as i64),
                )
            } else {
                Asset::new_image(
                    AssetId::parse(format!("ast_browse_{index}")).unwrap(),
                    "project-1",
                    "image",
                    format!("image-{index}.png"),
                    format!("C:/image-{index}.png"),
                    "a".repeat(64),
                    "image/png",
                    1,
                    1,
                    1,
                    crate::domain::TaskId::parse("tsk_source").unwrap(),
                    json!({"category": category}),
                    base + Duration::seconds(index as i64),
                )
            }
            .unwrap();
            sqlx::query("INSERT INTO tasks (id, project_id, workflow_id, workflow_version_id, recipe_id, status, progress_mode, created_at) VALUES ('tsk_source', 'project-1', 'workflow-1', 'workflow-version-1', 'recipe-1', 'CREATED', 'indeterminate', ?)")
                .bind(base.to_rfc3339()).execute(&pool).await.ok();
            repository.insert_many(&[asset]).await.unwrap();
        }
        let browser = SqliteAssetBrowseRepository::new(pool);
        let first = browser
            .list_page("project-1", AssetCategoryFilter::GeneratedImage, None, 1)
            .await
            .unwrap();
        assert_eq!(first.items.len(), 1);
        let second = browser
            .list_page(
                "project-1",
                AssetCategoryFilter::GeneratedImage,
                first.next_cursor,
                1,
            )
            .await
            .unwrap();
        assert_eq!(second.items.len(), 1);
        assert_ne!(first.items[0].id, second.items[0].id);
        assert!(
            browser
                .list_page("project-1", AssetCategoryFilter::SourceImage, None, 10)
                .await
                .unwrap()
                .items
                .len()
                == 1
        );
    }
}
