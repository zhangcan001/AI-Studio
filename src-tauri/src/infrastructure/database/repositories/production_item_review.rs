use super::{format_datetime, map_sqlx_error, parse_datetime};
use crate::application::ports::{
    ProductionItemReviewRecord, ProductionItemReviewRepository, RepositoryError,
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
