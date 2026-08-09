use super::{i64_to_u64, map_domain_error, map_sqlx_error, parse_datetime, parse_json};
use crate::application::pagination::{PageCursor, PageResult};
use crate::application::ports::{
    AssetBrowseRepository, AssetCreatedOrder, AssetLibraryQuery, AssetSourceFilter, RepositoryError,
};
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
        request: AssetLibraryQuery,
    ) -> Result<PageResult<Asset>, RepositoryError> {
        let requested_limit = request.limit.clamp(1, 100);
        let created_order = request.created_order;
        let mut query = QueryBuilder::<Sqlite>::new(ASSET_BROWSE_SELECT);
        query
            .push(" WHERE project_id = ")
            .push_bind(request.project_id);
        if let Some(category) = request.category.category() {
            query.push(" AND category = ").push_bind(category);
        }
        if let Some(asset_type) = request.media_type.asset_type() {
            query.push(" AND asset_type = ").push_bind(asset_type);
        }
        match request.source_kind {
            AssetSourceFilter::All => {}
            AssetSourceFilter::Source => {
                query.push(" AND category LIKE 'source_%'");
            }
            AssetSourceFilter::Generated => {
                query.push(" AND category LIKE 'generated_%'");
            }
        }
        if request.favorite_only {
            query.push(" AND EXISTS (SELECT 1 FROM asset_favorites f WHERE f.asset_id = assets.id AND f.project_id = assets.project_id)");
        }
        if let Some(tag_id) = request
            .tag_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            query.push(" AND EXISTS (SELECT 1 FROM asset_tag_links l WHERE l.asset_id = assets.id AND l.project_id = assets.project_id AND l.tag_id = ")
                .push_bind(tag_id)
                .push(")");
        }
        if let Some(keyword) = request.keyword.and_then(|keyword| {
            let keyword = keyword.trim().to_owned();
            (!keyword.is_empty()).then_some(keyword)
        }) {
            let pattern = format!("%{keyword}%");
            query
                .push(" AND (name LIKE ")
                .push_bind(pattern.clone())
                .push(" OR COALESCE(original_name, '') LIKE ")
                .push_bind(pattern)
                .push(")");
        }
        if let Some(cursor) = request.cursor {
            let created_at = cursor.created_at.to_rfc3339();
            match created_order {
                AssetCreatedOrder::Newest => query
                    .push(" AND (created_at < ")
                    .push_bind(created_at.clone())
                    .push(" OR (created_at = ")
                    .push_bind(created_at)
                    .push(" AND id < ")
                    .push_bind(cursor.id)
                    .push("))"),
                AssetCreatedOrder::Oldest => query
                    .push(" AND (created_at > ")
                    .push_bind(created_at.clone())
                    .push(" OR (created_at = ")
                    .push_bind(created_at)
                    .push(" AND id > ")
                    .push_bind(cursor.id)
                    .push("))"),
            };
        }
        query.push(" ORDER BY created_at ");
        match created_order {
            AssetCreatedOrder::Newest => {
                query.push("DESC, id DESC LIMIT ");
            }
            AssetCreatedOrder::Oldest => {
                query.push("ASC, id ASC LIMIT ");
            }
        }
        query.push_bind(i64::from(requested_limit) + 1);
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
    use crate::application::ports::{
        AssetBrowseRepository, AssetCategoryFilter, AssetCreatedOrder, AssetLibraryQuery,
        AssetMediaTypeFilter, AssetRepository, AssetSourceFilter,
    };
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
            .list_page(AssetLibraryQuery {
                project_id: "project-1".to_owned(),
                category: AssetCategoryFilter::GeneratedImage,
                keyword: None,
                media_type: AssetMediaTypeFilter::All,
                source_kind: AssetSourceFilter::All,
                favorite_only: false,
                tag_id: None,
                created_order: AssetCreatedOrder::Newest,
                cursor: None,
                limit: 1,
            })
            .await
            .unwrap();
        assert_eq!(first.items.len(), 1);
        let second = browser
            .list_page(AssetLibraryQuery {
                project_id: "project-1".to_owned(),
                category: AssetCategoryFilter::GeneratedImage,
                keyword: None,
                media_type: AssetMediaTypeFilter::All,
                source_kind: AssetSourceFilter::All,
                favorite_only: false,
                tag_id: None,
                created_order: AssetCreatedOrder::Newest,
                cursor: first.next_cursor,
                limit: 1,
            })
            .await
            .unwrap();
        assert_eq!(second.items.len(), 1);
        assert_ne!(first.items[0].id, second.items[0].id);
        assert!(
            browser
                .list_page(AssetLibraryQuery {
                    project_id: "project-1".to_owned(),
                    category: AssetCategoryFilter::SourceImage,
                    keyword: None,
                    media_type: AssetMediaTypeFilter::All,
                    source_kind: AssetSourceFilter::All,
                    favorite_only: false,
                    tag_id: None,
                    created_order: AssetCreatedOrder::Newest,
                    cursor: None,
                    limit: 10,
                })
                .await
                .unwrap()
                .items
                .len()
                == 1
        );
    }

    #[tokio::test]
    async fn searches_filters_and_paginates_stably_in_both_directions() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        sqlx::query(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at)
             VALUES ('project-2', 'Other', 'C:/other', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let repository = SqliteAssetRepository::new(pool.clone());
        let base = Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap();
        sqlx::query("INSERT INTO tasks (id, project_id, workflow_id, workflow_version_id, recipe_id, status, progress_mode, created_at) VALUES ('tsk_source', 'project-1', 'workflow-1', 'workflow-version-1', 'recipe-1', 'CREATED', 'indeterminate', ?)")
            .bind(base.to_rfc3339())
            .execute(&pool)
            .await
            .unwrap();
        for (index, (project_id, category, name, asset_type)) in [
            ("project-1", "source_image", "人物参考图.png", "image"),
            ("project-1", "generated_image", "cat-result.png", "image"),
            ("project-1", "generated_video", "cat-video.mp4", "video"),
            ("project-2", "generated_image", "cat-other.png", "image"),
        ]
        .into_iter()
        .enumerate()
        {
            let asset = if asset_type == "video" {
                Asset::new_generated_video(
                    AssetId::parse(format!("ast_search_{index}")).unwrap(),
                    project_id,
                    name,
                    name,
                    format!("C:/search-{index}.mp4"),
                    "a".repeat(64),
                    "video/mp4",
                    Some(320),
                    Some(240),
                    Some(1_000),
                    1,
                    crate::domain::TaskId::parse("tsk_source").unwrap(),
                    json!({}),
                    base + Duration::seconds(index as i64),
                )
            } else if category == "source_image" {
                Asset::new_source_image(
                    AssetId::parse(format!("ast_search_{index}")).unwrap(),
                    project_id,
                    name,
                    name,
                    format!("C:/search-{index}.png"),
                    "a".repeat(64),
                    "image/png",
                    1,
                    1,
                    1,
                    json!({}),
                    base + Duration::seconds(index as i64),
                )
            } else {
                Asset::new_image(
                    AssetId::parse(format!("ast_search_{index}")).unwrap(),
                    project_id,
                    name,
                    name,
                    format!("C:/search-{index}.png"),
                    "a".repeat(64),
                    "image/png",
                    1,
                    1,
                    1,
                    crate::domain::TaskId::parse("tsk_source").unwrap(),
                    json!({}),
                    base + Duration::seconds(index as i64),
                )
            }
            .unwrap();
            repository.insert_many(&[asset]).await.unwrap();
        }
        let browser = SqliteAssetBrowseRepository::new(pool);
        let keyword_page = browser
            .list_page(AssetLibraryQuery {
                project_id: "project-1".to_owned(),
                category: AssetCategoryFilter::All,
                keyword: Some(" cat ".to_owned()),
                media_type: AssetMediaTypeFilter::Image,
                source_kind: AssetSourceFilter::Generated,
                favorite_only: false,
                tag_id: None,
                created_order: AssetCreatedOrder::Newest,
                cursor: None,
                limit: 1,
            })
            .await
            .unwrap();
        assert_eq!(keyword_page.items.len(), 1);
        assert_eq!(keyword_page.items[0].name, "cat-result.png");
        assert!(keyword_page.next_cursor.is_none());

        let oldest = browser
            .list_page(AssetLibraryQuery {
                project_id: "project-1".to_owned(),
                category: AssetCategoryFilter::All,
                keyword: None,
                media_type: AssetMediaTypeFilter::Image,
                source_kind: AssetSourceFilter::All,
                favorite_only: false,
                tag_id: None,
                created_order: AssetCreatedOrder::Oldest,
                cursor: None,
                limit: 1,
            })
            .await
            .unwrap();
        assert_eq!(oldest.items[0].name, "人物参考图.png");
        let next_oldest = browser
            .list_page(AssetLibraryQuery {
                project_id: "project-1".to_owned(),
                category: AssetCategoryFilter::All,
                keyword: None,
                media_type: AssetMediaTypeFilter::Image,
                source_kind: AssetSourceFilter::All,
                favorite_only: false,
                tag_id: None,
                created_order: AssetCreatedOrder::Oldest,
                cursor: oldest.next_cursor,
                limit: 1,
            })
            .await
            .unwrap();
        assert_eq!(next_oldest.items[0].name, "cat-result.png");
        assert!(browser
            .list_page(AssetLibraryQuery {
                project_id: "project-2".to_owned(),
                category: AssetCategoryFilter::All,
                keyword: Some("cat".to_owned()),
                media_type: AssetMediaTypeFilter::All,
                source_kind: AssetSourceFilter::All,
                favorite_only: false,
                tag_id: None,
                created_order: AssetCreatedOrder::Newest,
                cursor: None,
                limit: 10,
            })
            .await
            .unwrap()
            .items
            .iter()
            .all(|asset| asset.project_id == "project-2"));
    }

    #[tokio::test]
    async fn keeps_synthetic_thousand_asset_query_bounded_by_keyset_page() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        let repository = SqliteAssetRepository::new(pool.clone());
        let base = Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap();
        let assets = (0..1_000)
            .map(|index| {
                Asset::new_source_image(
                    AssetId::parse(format!("ast_perf_{index:04}")).unwrap(),
                    "project-1",
                    format!("synthetic-{index:04}.png"),
                    format!("synthetic-{index:04}.png"),
                    format!("C:/synthetic-{index:04}.png"),
                    format!("{index:064x}"),
                    "image/png",
                    64,
                    64,
                    4_096,
                    json!({"synthetic": true}),
                    base + Duration::seconds(index as i64),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        repository.insert_many(&assets).await.unwrap();

        for index in 0..100 {
            sqlx::query("INSERT INTO asset_tags (id, project_id, name, normalized_name, created_at, updated_at) VALUES (?, 'project-1', ?, ?, '2026-03-01T00:00:00+00:00', '2026-03-01T00:00:00+00:00')")
                .bind(format!("tag_perf_{index:03}")).bind(format!("性能{index:03}")).bind(format!("性能{index:03}")).execute(&pool).await.unwrap();
        }
        for index in 0..1_000 {
            sqlx::query("INSERT INTO asset_tag_links (asset_id, tag_id, project_id, created_at) VALUES (?, ?, 'project-1', '2026-03-01T00:00:00+00:00')")
                .bind(format!("ast_perf_{index:04}")).bind(format!("tag_perf_{:03}", index % 100)).execute(&pool).await.unwrap();
            if index % 2 == 0 {
                sqlx::query("INSERT INTO asset_favorites (asset_id, project_id, created_at) VALUES (?, 'project-1', '2026-03-01T00:00:00+00:00')")
                    .bind(format!("ast_perf_{index:04}")).execute(&pool).await.unwrap();
            }
        }

        let browser = SqliteAssetBrowseRepository::new(pool);
        let page = browser
            .list_page(AssetLibraryQuery {
                project_id: "project-1".to_owned(),
                category: AssetCategoryFilter::SourceImage,
                keyword: Some("synthetic-".to_owned()),
                media_type: AssetMediaTypeFilter::Image,
                source_kind: AssetSourceFilter::Source,
                favorite_only: false,
                tag_id: None,
                created_order: AssetCreatedOrder::Newest,
                cursor: None,
                limit: 30,
            })
            .await
            .unwrap();

        assert_eq!(page.items.len(), 30);
        assert!(page.next_cursor.is_some());
        assert_eq!(page.items[0].name, "synthetic-0999.png");
        assert_eq!(page.items[29].name, "synthetic-0970.png");
        let organized = browser
            .list_page(AssetLibraryQuery {
                project_id: "project-1".to_owned(),
                category: AssetCategoryFilter::SourceImage,
                keyword: Some("synthetic-".to_owned()),
                media_type: AssetMediaTypeFilter::Image,
                source_kind: AssetSourceFilter::Source,
                favorite_only: true,
                tag_id: Some("tag_perf_000".to_owned()),
                created_order: AssetCreatedOrder::Newest,
                cursor: None,
                limit: 5,
            })
            .await
            .unwrap();
        assert_eq!(organized.items.len(), 5);
        assert!(organized.next_cursor.is_some());
    }

    #[tokio::test]
    async fn combines_favorite_tag_keyword_media_source_order_cursor_and_project_filters() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        sqlx::query("INSERT INTO projects (id, name, root_path, created_at, updated_at) VALUES ('project-2', 'Other', 'C:/other', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO tasks (id, project_id, workflow_id, workflow_version_id, recipe_id, status, progress_mode, created_at) VALUES ('tsk_browse_org', 'project-1', 'workflow-1', 'workflow-version-1', 'recipe-1', 'SUCCEEDED', 'indeterminate', '2026-01-01T00:00:00Z')").execute(&pool).await.unwrap();
        for (id, project, category, kind, name, created) in [
            (
                "ast_a",
                "project-1",
                "source_image",
                "image",
                "人物参考图",
                "2026-01-01T00:00:01+00:00",
            ),
            (
                "ast_b",
                "project-1",
                "generated_image",
                "image",
                "人物成图",
                "2026-01-01T00:00:02+00:00",
            ),
            (
                "ast_c",
                "project-1",
                "generated_video",
                "video",
                "最终成片",
                "2026-01-01T00:00:03+00:00",
            ),
            (
                "ast_d",
                "project-2",
                "source_image",
                "image",
                "人物其他项目",
                "2026-01-01T00:00:04+00:00",
            ),
        ] {
            sqlx::query("INSERT INTO assets (id, project_id, type, category, name, original_name, storage_path, sha256, mime_type, width, height, file_size, source_task_id, metadata_json, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1, 1, 1, ?, '{}', ?, ?)")
                .bind(id).bind(project).bind(kind).bind(category).bind(name).bind(name).bind(format!("C:/{id}")).bind("a".repeat(64)).bind(if kind == "video" { "video/mp4" } else { "image/png" }).bind(category.starts_with("generated_").then_some("tsk_browse_org")).bind(created).bind(created).execute(&pool).await.unwrap();
        }
        for (id, project, name) in [
            ("tag_people", "project-1", "人物"),
            ("tag_finish", "project-1", "成片"),
            ("tag_other", "project-2", "人物"),
        ] {
            sqlx::query("INSERT INTO asset_tags (id, project_id, name, normalized_name, created_at, updated_at) VALUES (?, ?, ?, ?, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')").bind(id).bind(project).bind(name).bind(name).execute(&pool).await.unwrap();
        }
        for (asset, tag, project) in [
            ("ast_a", "tag_people", "project-1"),
            ("ast_b", "tag_people", "project-1"),
            ("ast_c", "tag_finish", "project-1"),
            ("ast_d", "tag_other", "project-2"),
        ] {
            sqlx::query("INSERT INTO asset_tag_links (asset_id, tag_id, project_id, created_at) VALUES (?, ?, ?, '2026-01-01T00:00:00Z')").bind(asset).bind(tag).bind(project).execute(&pool).await.unwrap();
        }
        for (asset, project) in [
            ("ast_a", "project-1"),
            ("ast_c", "project-1"),
            ("ast_d", "project-2"),
        ] {
            sqlx::query("INSERT INTO asset_favorites (asset_id, project_id, created_at) VALUES (?, ?, '2026-01-01T00:00:00Z')").bind(asset).bind(project).execute(&pool).await.unwrap();
        }
        let browser = SqliteAssetBrowseRepository::new(pool);
        let base = |favorite_only,
                    tag_id: Option<&str>,
                    keyword: Option<&str>,
                    media_type,
                    source_kind,
                    created_order,
                    cursor,
                    limit| AssetLibraryQuery {
            project_id: "project-1".to_owned(),
            category: AssetCategoryFilter::All,
            keyword: keyword.map(str::to_owned),
            media_type,
            source_kind,
            favorite_only,
            tag_id: tag_id.map(str::to_owned),
            created_order,
            cursor,
            limit,
        };
        let favorites = browser
            .list_page(base(
                true,
                None,
                None,
                AssetMediaTypeFilter::All,
                AssetSourceFilter::All,
                AssetCreatedOrder::Newest,
                None,
                10,
            ))
            .await
            .unwrap();
        assert_eq!(
            favorites
                .items
                .iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>(),
            ["ast_c", "ast_a"]
        );
        let people = browser
            .list_page(base(
                false,
                Some("tag_people"),
                None,
                AssetMediaTypeFilter::All,
                AssetSourceFilter::All,
                AssetCreatedOrder::Newest,
                None,
                10,
            ))
            .await
            .unwrap();
        assert_eq!(people.items.len(), 2);
        let combined = browser
            .list_page(base(
                true,
                Some("tag_people"),
                Some("参考"),
                AssetMediaTypeFilter::Image,
                AssetSourceFilter::Source,
                AssetCreatedOrder::Oldest,
                None,
                10,
            ))
            .await
            .unwrap();
        assert_eq!(combined.items[0].id.as_str(), "ast_a");
        let first = browser
            .list_page(base(
                false,
                Some("tag_people"),
                None,
                AssetMediaTypeFilter::Image,
                AssetSourceFilter::All,
                AssetCreatedOrder::Oldest,
                None,
                1,
            ))
            .await
            .unwrap();
        let second = browser
            .list_page(base(
                false,
                Some("tag_people"),
                None,
                AssetMediaTypeFilter::Image,
                AssetSourceFilter::All,
                AssetCreatedOrder::Oldest,
                first.next_cursor,
                1,
            ))
            .await
            .unwrap();
        assert_eq!(first.items[0].id.as_str(), "ast_a");
        assert_eq!(second.items[0].id.as_str(), "ast_b");
        let isolated = browser
            .list_page(AssetLibraryQuery {
                project_id: "project-2".to_owned(),
                category: AssetCategoryFilter::All,
                keyword: None,
                media_type: AssetMediaTypeFilter::All,
                source_kind: AssetSourceFilter::All,
                favorite_only: true,
                tag_id: Some("tag_other".to_owned()),
                created_order: AssetCreatedOrder::Newest,
                cursor: None,
                limit: 10,
            })
            .await
            .unwrap();
        assert_eq!(isolated.items.len(), 1);
        assert_eq!(isolated.items[0].project_id, "project-2");
    }
}
