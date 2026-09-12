use super::{format_datetime, map_sqlx_error, parse_datetime};
use crate::application::ports::{
    ProductionItemReviewRecord, ProductionItemReviewRepository, ProductionReviewInboxItem,
    ProductionReviewInboxPage, RepositoryError,
};
use crate::domain::ProductionReviewStatus;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{FromRow, QueryBuilder, Sqlite, SqlitePool};

#[derive(Clone)]
pub struct SqliteProductionItemReviewRepository {
    pool: SqlitePool,
}

impl SqliteProductionItemReviewRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct ReviewRow {
    id: String,
    project_id: String,
    production_batch_id: String,
    production_batch_item_id: String,
    task_id: Option<String>,
    result_asset_id: Option<String>,
    review_status: String,
    review_note: String,
    version: i64,
    lineage_key: String,
    parent_batch_id: Option<String>,
    parent_item_id: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(FromRow)]
struct ReviewInboxRow {
    project_id: String,
    batch_id: String,
    batch_name: String,
    batch_status: String,
    item_id: String,
    ordinal: i64,
    item_status: String,
    task_id: Option<String>,
    task_status: Option<String>,
    shot_id: Option<String>,
    stage: Option<String>,
    asset_id: Option<String>,
    asset_name: Option<String>,
    asset_type: Option<String>,
    asset_mime_type: Option<String>,
    selected_asset_id: Option<String>,
    review_status: String,
    review_note: String,
    version: i64,
    workflow_version_id: String,
    recipe_id: String,
    prompt_summary: Option<String>,
    updated_at: String,
    total_count: i64,
    unreviewed_count: i64,
    regenerate_count: i64,
}

impl ReviewRow {
    fn into_record(self) -> Result<ProductionItemReviewRecord, RepositoryError> {
        Ok(ProductionItemReviewRecord {
            id: self.id,
            project_id: self.project_id,
            production_batch_id: self.production_batch_id,
            production_batch_item_id: self.production_batch_item_id,
            task_id: self.task_id,
            result_asset_id: self.result_asset_id,
            review_status: ProductionReviewStatus::parse(&self.review_status).map_err(|error| {
                RepositoryError::serialization("production item review status", error.to_string())
            })?,
            review_note: self.review_note,
            version: self.version,
            lineage_key: self.lineage_key,
            parent_batch_id: self.parent_batch_id,
            parent_item_id: self.parent_item_id,
            created_at: parse_datetime("production item review created_at", &self.created_at)?,
            updated_at: parse_datetime("production item review updated_at", &self.updated_at)?,
        })
    }
}

const REVIEW_SELECT: &str = "SELECT id, project_id, production_batch_id, production_batch_item_id,
    task_id, result_asset_id, review_status, review_note, version, lineage_key,
    parent_batch_id, parent_item_id, created_at, updated_at
    FROM production_item_reviews";

#[async_trait]
impl ProductionItemReviewRepository for SqliteProductionItemReviewRepository {
    async fn list_project_inbox(
        &self,
        project_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<ProductionReviewInboxPage, RepositoryError> {
        let limit = limit.clamp(1, 100) as i64;
        let offset = offset as i64;
        let rows = sqlx::query_as::<_, ReviewInboxRow>(
            "SELECT
                r.project_id,
                r.production_batch_id AS batch_id,
                b.name AS batch_name,
                b.status AS batch_status,
                i.id AS item_id,
                i.ordinal,
                i.status AS item_status,
                i.task_id,
                t.status AS task_status,
                link.shot_id,
                link.stage,
                a.id AS asset_id,
                a.name AS asset_name,
                a.type AS asset_type,
                a.mime_type AS asset_mime_type,
                CASE link.stage
                    WHEN 'video' THEN s.selected_video_asset_id
                    WHEN 'image' THEN s.selected_image_asset_id
                END AS selected_asset_id,
                r.review_status,
                r.review_note,
                r.version,
                i.workflow_version_id,
                i.recipe_id,
                json_extract(i.values_json, '$.prompt') AS prompt_summary,
                r.updated_at,
                COUNT(*) OVER () AS total_count,
                SUM(CASE WHEN r.review_status = 'UNREVIEWED' THEN 1 ELSE 0 END) OVER () AS unreviewed_count,
                SUM(CASE WHEN r.review_status = 'REGENERATE' THEN 1 ELSE 0 END) OVER () AS regenerate_count
             FROM production_item_reviews r
             INNER JOIN production_batches b
                ON b.id = r.production_batch_id AND b.project_id = r.project_id
             INNER JOIN production_batch_items i
                ON i.id = r.production_batch_item_id AND i.batch_id = b.id
             LEFT JOIN tasks t ON t.id = i.task_id
             LEFT JOIN shot_generation_links link
                ON link.production_batch_item_id = i.id
               AND link.id = (
                   SELECT latest.id
                   FROM shot_generation_links latest
                   WHERE latest.production_batch_item_id = i.id
                   ORDER BY latest.created_at DESC, latest.id DESC
                   LIMIT 1
               )
             LEFT JOIN shots s ON s.id = link.shot_id AND s.project_id = r.project_id
             LEFT JOIN assets a ON a.id = COALESCE(
                 r.result_asset_id,
                 (SELECT MIN(output.asset_id) FROM task_output_assets output WHERE output.task_id = i.task_id)
             ) AND a.project_id = r.project_id
             WHERE r.project_id = ?
               AND b.archived_at IS NULL
               AND r.review_status IN ('UNREVIEWED', 'REGENERATE')
             ORDER BY r.updated_at DESC, r.production_batch_id ASC, i.ordinal ASC, i.id ASC
             LIMIT ? OFFSET ?",
        )
        .bind(project_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        let total = rows
            .first()
            .map(|row| row.total_count.max(0) as usize)
            .unwrap_or(0);
        let unreviewed_count = rows
            .first()
            .map(|row| row.unreviewed_count.max(0) as usize)
            .unwrap_or(0);
        let regenerate_count = rows
            .first()
            .map(|row| row.regenerate_count.max(0) as usize)
            .unwrap_or(0);
        let items = rows
            .into_iter()
            .map(|row| {
                Ok(ProductionReviewInboxItem {
                    project_id: row.project_id,
                    batch_id: row.batch_id,
                    batch_name: row.batch_name,
                    batch_status: row.batch_status,
                    item_id: row.item_id,
                    ordinal: row.ordinal,
                    item_status: row.item_status,
                    task_id: row.task_id,
                    task_status: row.task_status,
                    shot_id: row.shot_id,
                    stage: row.stage,
                    asset_id: row.asset_id,
                    asset_name: row.asset_name,
                    asset_type: row.asset_type,
                    asset_mime_type: row.asset_mime_type,
                    selected_asset_id: row.selected_asset_id,
                    review_status: ProductionReviewStatus::parse(&row.review_status).map_err(
                        |error| {
                            RepositoryError::serialization(
                                "production review inbox status",
                                error.to_string(),
                            )
                        },
                    )?,
                    review_note: row.review_note,
                    version: row.version,
                    workflow_version_id: row.workflow_version_id,
                    recipe_id: row.recipe_id,
                    prompt_summary: row.prompt_summary,
                    updated_at: parse_datetime(
                        "production review inbox updated_at",
                        &row.updated_at,
                    )?,
                })
            })
            .collect::<Result<Vec<_>, RepositoryError>>()?;
        Ok(ProductionReviewInboxPage {
            items,
            total,
            unreviewed_count,
            regenerate_count,
        })
    }

    async fn list_for_batch(
        &self,
        project_id: &str,
        production_batch_id: &str,
    ) -> Result<Vec<ProductionItemReviewRecord>, RepositoryError> {
        let rows = sqlx::query_as::<_, ReviewRow>(&format!(
            "{REVIEW_SELECT} WHERE project_id = ? AND production_batch_id = ? ORDER BY version, production_batch_item_id"
        ))
        .bind(project_id)
        .bind(production_batch_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter().map(ReviewRow::into_record).collect()
    }

    async fn list_for_lineage(
        &self,
        project_id: &str,
        lineage_key: &str,
    ) -> Result<Vec<ProductionItemReviewRecord>, RepositoryError> {
        let rows = sqlx::query_as::<_, ReviewRow>(&format!(
            "{REVIEW_SELECT} WHERE project_id = ? AND lineage_key = ? ORDER BY version, production_batch_item_id"
        ))
        .bind(project_id)
        .bind(lineage_key)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter().map(ReviewRow::into_record).collect()
    }

    async fn list_for_lineages(
        &self,
        project_id: &str,
        lineage_keys: &[String],
    ) -> Result<Vec<ProductionItemReviewRecord>, RepositoryError> {
        if lineage_keys.is_empty() {
            return Ok(Vec::new());
        }
        let mut query = QueryBuilder::<Sqlite>::new(format!("{REVIEW_SELECT} WHERE project_id = "));
        query.push_bind(project_id).push(" AND lineage_key IN (");
        let mut separated = query.separated(", ");
        for lineage_key in lineage_keys {
            separated.push_bind(lineage_key);
        }
        separated.push_unseparated(
            ") ORDER BY lineage_key ASC, version ASC, production_batch_item_id ASC",
        );
        let rows = query
            .build_query_as::<ReviewRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        rows.into_iter().map(ReviewRow::into_record).collect()
    }

    async fn find_for_item(
        &self,
        project_id: &str,
        production_batch_item_id: &str,
    ) -> Result<Option<ProductionItemReviewRecord>, RepositoryError> {
        sqlx::query_as::<_, ReviewRow>(&format!(
            "{REVIEW_SELECT} WHERE project_id = ? AND production_batch_item_id = ?"
        ))
        .bind(project_id)
        .bind(production_batch_item_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .map(ReviewRow::into_record)
        .transpose()
    }

    async fn ensure_for_item(
        &self,
        record: &ProductionItemReviewRecord,
    ) -> Result<ProductionItemReviewRecord, RepositoryError> {
        let result = sqlx::query(
            "INSERT INTO production_item_reviews
                (id, project_id, production_batch_id, production_batch_item_id, task_id,
                 result_asset_id, review_status, review_note, version, lineage_key,
                 parent_batch_id, parent_item_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(production_batch_item_id) DO UPDATE SET
                task_id = COALESCE(excluded.task_id, production_item_reviews.task_id),
                result_asset_id = COALESCE(excluded.result_asset_id, production_item_reviews.result_asset_id),
                updated_at = excluded.updated_at
             WHERE production_item_reviews.project_id = excluded.project_id",
        )
        .bind(&record.id)
        .bind(&record.project_id)
        .bind(&record.production_batch_id)
        .bind(&record.production_batch_item_id)
        .bind(&record.task_id)
        .bind(&record.result_asset_id)
        .bind(record.review_status.as_str())
        .bind(&record.review_note)
        .bind(record.version)
        .bind(&record.lineage_key)
        .bind(&record.parent_batch_id)
        .bind(&record.parent_item_id)
        .bind(format_datetime(record.created_at))
        .bind(format_datetime(record.updated_at))
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::integrity(
                "production item review belongs to another project",
            ));
        }
        self.find_for_item(&record.project_id, &record.production_batch_item_id)
            .await?
            .ok_or_else(|| {
                RepositoryError::not_found(
                    "production item review",
                    &record.production_batch_item_id,
                )
            })
    }

    async fn ensure_for_items(
        &self,
        records: &[ProductionItemReviewRecord],
    ) -> Result<Vec<ProductionItemReviewRecord>, RepositoryError> {
        let Some(first) = records.first() else {
            return Ok(Vec::new());
        };
        if records.iter().any(|record| {
            record.project_id != first.project_id
                || record.production_batch_id != first.production_batch_id
        }) {
            return Err(RepositoryError::integrity(
                "bulk production item reviews must belong to one project and batch",
            ));
        }

        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        for record in records {
            sqlx::query(
                "INSERT INTO production_item_reviews
                    (id, project_id, production_batch_id, production_batch_item_id, task_id,
                     result_asset_id, review_status, review_note, version, lineage_key,
                     parent_batch_id, parent_item_id, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(production_batch_item_id) DO UPDATE SET
                    task_id = COALESCE(excluded.task_id, production_item_reviews.task_id),
                    result_asset_id = COALESCE(excluded.result_asset_id, production_item_reviews.result_asset_id),
                    updated_at = excluded.updated_at
                 WHERE production_item_reviews.project_id = excluded.project_id",
            )
            .bind(&record.id)
            .bind(&record.project_id)
            .bind(&record.production_batch_id)
            .bind(&record.production_batch_item_id)
            .bind(&record.task_id)
            .bind(&record.result_asset_id)
            .bind(record.review_status.as_str())
            .bind(&record.review_note)
            .bind(record.version)
            .bind(&record.lineage_key)
            .bind(&record.parent_batch_id)
            .bind(&record.parent_item_id)
            .bind(format_datetime(record.created_at))
            .bind(format_datetime(record.updated_at))
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
        }

        let mut query = QueryBuilder::<Sqlite>::new(format!("{REVIEW_SELECT} WHERE project_id = "));
        query
            .push_bind(&first.project_id)
            .push(" AND production_batch_id = ")
            .push_bind(&first.production_batch_id)
            .push(" AND production_batch_item_id IN (");
        let mut separated = query.separated(", ");
        for record in records {
            separated.push_bind(&record.production_batch_item_id);
        }
        separated.push_unseparated(") ORDER BY production_batch_item_id ASC");
        let rows = query
            .build_query_as::<ReviewRow>()
            .fetch_all(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
        transaction.commit().await.map_err(map_sqlx_error)?;
        rows.into_iter().map(ReviewRow::into_record).collect()
    }

    async fn insert(&self, record: &ProductionItemReviewRecord) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO production_item_reviews
                (id, project_id, production_batch_id, production_batch_item_id, task_id,
                 result_asset_id, review_status, review_note, version, lineage_key,
                 parent_batch_id, parent_item_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&record.id)
        .bind(&record.project_id)
        .bind(&record.production_batch_id)
        .bind(&record.production_batch_item_id)
        .bind(&record.task_id)
        .bind(&record.result_asset_id)
        .bind(record.review_status.as_str())
        .bind(&record.review_note)
        .bind(record.version)
        .bind(&record.lineage_key)
        .bind(&record.parent_batch_id)
        .bind(&record.parent_item_id)
        .bind(format_datetime(record.created_at))
        .bind(format_datetime(record.updated_at))
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn set_status(
        &self,
        project_id: &str,
        production_batch_item_id: &str,
        status: ProductionReviewStatus,
        updated_at: DateTime<Utc>,
    ) -> Result<ProductionItemReviewRecord, RepositoryError> {
        let result = sqlx::query(
            "UPDATE production_item_reviews
             SET review_status = ?, updated_at = ?
             WHERE project_id = ? AND production_batch_item_id = ?",
        )
        .bind(status.as_str())
        .bind(format_datetime(updated_at))
        .bind(project_id)
        .bind(production_batch_item_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::not_found(
                "production item review",
                production_batch_item_id,
            ));
        }
        self.find_for_item(project_id, production_batch_item_id)
            .await?
            .ok_or_else(|| {
                RepositoryError::not_found("production item review", production_batch_item_id)
            })
    }

    async fn set_note(
        &self,
        project_id: &str,
        production_batch_item_id: &str,
        note: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<ProductionItemReviewRecord, RepositoryError> {
        let result = sqlx::query(
            "UPDATE production_item_reviews
             SET review_note = ?, updated_at = ?
             WHERE project_id = ? AND production_batch_item_id = ?",
        )
        .bind(note)
        .bind(format_datetime(updated_at))
        .bind(project_id)
        .bind(production_batch_item_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::not_found(
                "production item review",
                production_batch_item_id,
            ));
        }
        self.find_for_item(project_id, production_batch_item_id)
            .await?
            .ok_or_else(|| {
                RepositoryError::not_found("production item review", production_batch_item_id)
            })
    }
}
