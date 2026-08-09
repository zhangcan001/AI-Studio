use super::{
    format_datetime, i64_to_u64, map_domain_error, map_sqlx_error, parse_datetime, parse_json,
    parse_optional_datetime, serialize_json,
};
use crate::application::ports::{ProductionQueueRepository, RepositoryError};
use crate::domain::{
    ProductionBatch, ProductionBatchDetail, ProductionBatchId, ProductionBatchItem,
    ProductionBatchItemId, ProductionBatchItemStatus, ProductionBatchStatus,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct SqliteProductionQueueRepository {
    pool: SqlitePool,
}

impl SqliteProductionQueueRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProductionQueueRepository for SqliteProductionQueueRepository {
    async fn insert(
        &self,
        batch: &ProductionBatch,
        items: &[ProductionBatchItem],
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        sqlx::query(
            "INSERT INTO production_batches (id, project_id, name, status, continue_on_failure, archived_at, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(batch.id.as_str())
        .bind(&batch.project_id)
        .bind(&batch.name)
        .bind(batch.status.as_str())
        .bind(if batch.continue_on_failure { 1_i64 } else { 0_i64 })
        .bind(batch.archived_at.as_ref().map(|value| format_datetime(value.to_owned())))
        .bind(format_datetime(batch.created_at))
        .bind(format_datetime(batch.updated_at))
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;

        for item in items {
            let values_json =
                serialize_json("production batch item values", Some(&item.values_json))?
                    .ok_or_else(|| {
                        RepositoryError::serialization(
                            "production batch item values",
                            "missing value",
                        )
                    })?;
            sqlx::query(
                "INSERT INTO production_batch_items
                 (id, batch_id, ordinal, workflow_version_id, recipe_id, values_json, status, task_id, retry_of_item_id, error_code, error_message, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(item.id.as_str())
            .bind(item.batch_id.as_str())
            .bind(i64::from(item.ordinal))
            .bind(&item.workflow_version_id)
            .bind(&item.recipe_id)
            .bind(values_json)
            .bind(item.status.as_str())
            .bind(&item.task_id)
            .bind(&item.retry_of_item_id)
            .bind(&item.error_code)
            .bind(&item.error_message)
            .bind(format_datetime(item.created_at))
            .bind(format_datetime(item.updated_at))
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
        }
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn list(&self, project_id: &str) -> Result<Vec<ProductionBatch>, RepositoryError> {
        let rows = sqlx::query_as::<_, BatchRow>(
            "SELECT id, project_id, name, status, continue_on_failure, archived_at, created_at, updated_at
             FROM production_batches WHERE project_id = ? ORDER BY updated_at DESC, id ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter().map(BatchRow::try_into_domain).collect()
    }

    async fn list_running(&self) -> Result<Vec<ProductionBatch>, RepositoryError> {
        let rows = sqlx::query_as::<_, BatchRow>(
            "SELECT id, project_id, name, status, continue_on_failure, archived_at, created_at, updated_at
             FROM production_batches WHERE status = 'RUNNING' AND archived_at IS NULL ORDER BY updated_at ASC, id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter().map(BatchRow::try_into_domain).collect()
    }

    async fn find_detail(
        &self,
        project_id: &str,
        batch_id: &ProductionBatchId,
    ) -> Result<Option<ProductionBatchDetail>, RepositoryError> {
        let batch = sqlx::query_as::<_, BatchRow>(
            "SELECT id, project_id, name, status, continue_on_failure, archived_at, created_at, updated_at
             FROM production_batches WHERE project_id = ? AND id = ?",
        )
        .bind(project_id)
        .bind(batch_id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        let Some(batch) = batch else {
            return Ok(None);
        };
        let items = sqlx::query_as::<_, ItemRow>(
            "SELECT id, batch_id, ordinal, workflow_version_id, recipe_id, values_json, status,
                    task_id, retry_of_item_id, error_code, error_message, created_at, updated_at
             FROM production_batch_items WHERE batch_id = ? ORDER BY ordinal ASC",
        )
        .bind(batch_id.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(Some(ProductionBatchDetail {
            batch: batch.try_into_domain()?,
            items: items
                .into_iter()
                .map(ItemRow::try_into_domain)
                .collect::<Result<Vec<_>, _>>()?,
        }))
    }

    async fn set_batch_status(
        &self,
        project_id: &str,
        batch_id: &ProductionBatchId,
        status: ProductionBatchStatus,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query(
            "UPDATE production_batches SET status = ?, updated_at = ? WHERE project_id = ? AND id = ?",
        )
        .bind(status.as_str())
        .bind(format_datetime(updated_at))
        .bind(project_id)
        .bind(batch_id.as_str())
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(result.rows_affected() > 0)
    }

    async fn set_item_dispatching(
        &self,
        item_id: &ProductionBatchItemId,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query(
            "UPDATE production_batch_items
             SET status = 'DISPATCHING', error_code = NULL, error_message = NULL, updated_at = ?
             WHERE id = ? AND status = 'PENDING'",
        )
        .bind(format_datetime(updated_at))
        .bind(item_id.as_str())
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(result.rows_affected() > 0)
    }

    async fn link_item_task(
        &self,
        item_id: &ProductionBatchItemId,
        task_id: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query(
            "UPDATE production_batch_items
             SET status = 'DISPATCHED', task_id = ?, updated_at = ?
             WHERE id = ? AND status = 'DISPATCHING' AND task_id IS NULL",
        )
        .bind(task_id)
        .bind(format_datetime(updated_at))
        .bind(item_id.as_str())
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(result.rows_affected() > 0)
    }

    async fn finish_item(
        &self,
        item_id: &ProductionBatchItemId,
        status: ProductionBatchItemStatus,
        error_code: Option<&str>,
        error_message: Option<&str>,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        if !status.is_terminal() {
            return Err(RepositoryError::integrity(
                "production queue finish_item requires terminal status",
            ));
        }
        let result = sqlx::query(
            "UPDATE production_batch_items
             SET status = ?, error_code = ?, error_message = ?, updated_at = ?
             WHERE id = ? AND status IN ('DISPATCHING', 'DISPATCHED')",
        )
        .bind(status.as_str())
        .bind(error_code)
        .bind(error_message)
        .bind(format_datetime(updated_at))
        .bind(item_id.as_str())
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(result.rows_affected() > 0)
    }

    async fn set_item_skipped(
        &self,
        item_id: &ProductionBatchItemId,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query(
            "UPDATE production_batch_items
             SET status = 'SKIPPED', updated_at = ?
             WHERE id = ? AND status IN ('FAILED', 'CANCELLED')",
        )
        .bind(format_datetime(updated_at))
        .bind(item_id.as_str())
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(result.rows_affected() > 0)
    }

    async fn append_requeue_item(
        &self,
        item: &ProductionBatchItem,
        updated_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let values_json = serialize_json("production batch item values", Some(&item.values_json))?
            .ok_or_else(|| {
                RepositoryError::serialization("production batch item values", "missing value")
            })?;
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        sqlx::query(
            "INSERT INTO production_batch_items
             (id, batch_id, ordinal, workflow_version_id, recipe_id, values_json, status, task_id, retry_of_item_id, error_code, error_message, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, 'PENDING', NULL, ?, NULL, NULL, ?, ?)",
        )
        .bind(item.id.as_str())
        .bind(item.batch_id.as_str())
        .bind(i64::from(item.ordinal))
        .bind(&item.workflow_version_id)
        .bind(&item.recipe_id)
        .bind(values_json)
        .bind(&item.retry_of_item_id)
        .bind(format_datetime(item.created_at))
        .bind(format_datetime(item.updated_at))
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        sqlx::query(
            "UPDATE production_batches SET status = 'PAUSED', archived_at = NULL, updated_at = ? WHERE id = ?",
        )
        .bind(format_datetime(updated_at))
        .bind(item.batch_id.as_str())
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn set_archived_at(
        &self,
        project_id: &str,
        batch_id: &ProductionBatchId,
        archived_at: Option<DateTime<Utc>>,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query(
            "UPDATE production_batches SET archived_at = ?, updated_at = ? WHERE project_id = ? AND id = ?",
        )
        .bind(archived_at.map(format_datetime))
        .bind(format_datetime(updated_at))
        .bind(project_id)
        .bind(batch_id.as_str())
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(result.rows_affected() > 0)
    }

    async fn delete_batch(
        &self,
        project_id: &str,
        batch_id: &ProductionBatchId,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM production_batches WHERE project_id = ? AND id = ?")
            .bind(project_id)
            .bind(batch_id.as_str())
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        Ok(result.rows_affected() > 0)
    }

    async fn recover_uncertain_dispatches(
        &self,
        updated_at: DateTime<Utc>,
    ) -> Result<Vec<ProductionBatchId>, RepositoryError> {
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT batch_id FROM production_batch_items WHERE status = 'DISPATCHING' AND task_id IS NULL",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        let at = format_datetime(updated_at);
        for batch_id in &rows {
            sqlx::query(
                "UPDATE production_batch_items
                 SET status = 'FAILED', error_code = 'QUEUE_DISPATCH_UNCERTAIN',
                     error_message = 'Application restarted while dispatch outcome was uncertain; automatic duplicate dispatch is blocked.',
                     updated_at = ?
                 WHERE batch_id = ? AND status = 'DISPATCHING' AND task_id IS NULL",
            )
            .bind(&at)
            .bind(batch_id)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
            sqlx::query(
                "UPDATE production_batches SET status = 'PAUSED', updated_at = ? WHERE id = ?",
            )
            .bind(&at)
            .bind(batch_id)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        }
        rows.into_iter()
            .map(|id| {
                ProductionBatchId::parse(id)
                    .map_err(|error| map_domain_error("production batch id", error))
            })
            .collect()
    }
}

#[derive(sqlx::FromRow)]
struct BatchRow {
    id: String,
    project_id: String,
    name: String,
    status: String,
    continue_on_failure: i64,
    archived_at: Option<String>,
    created_at: String,
    updated_at: String,
}

impl BatchRow {
    fn try_into_domain(self) -> Result<ProductionBatch, RepositoryError> {
        Ok(ProductionBatch {
            id: ProductionBatchId::parse(self.id)
                .map_err(|error| map_domain_error("production batch id", error))?,
            project_id: self.project_id,
            name: self.name,
            status: ProductionBatchStatus::parse(&self.status)
                .map_err(|error| map_domain_error("production batch status", error))?,
            continue_on_failure: self.continue_on_failure != 0,
            archived_at: parse_optional_datetime(
                "production batch archived_at",
                self.archived_at.as_deref(),
            )?,
            created_at: parse_datetime("production batch created_at", &self.created_at)?,
            updated_at: parse_datetime("production batch updated_at", &self.updated_at)?,
        })
    }
}

#[derive(sqlx::FromRow)]
struct ItemRow {
    id: String,
    batch_id: String,
    ordinal: i64,
    workflow_version_id: String,
    recipe_id: String,
    values_json: String,
    status: String,
    task_id: Option<String>,
    retry_of_item_id: Option<String>,
    error_code: Option<String>,
    error_message: Option<String>,
    created_at: String,
    updated_at: String,
}

impl ItemRow {
    fn try_into_domain(self) -> Result<ProductionBatchItem, RepositoryError> {
        let ordinal = i64_to_u64("production batch item ordinal", self.ordinal)?;
        Ok(ProductionBatchItem {
            id: ProductionBatchItemId::parse(self.id)
                .map_err(|error| map_domain_error("production batch item id", error))?,
            batch_id: ProductionBatchId::parse(self.batch_id)
                .map_err(|error| map_domain_error("production batch item batch id", error))?,
            ordinal: u32::try_from(ordinal).map_err(|_| {
                RepositoryError::serialization("production batch item ordinal", "value exceeds u32")
            })?,
            workflow_version_id: self.workflow_version_id,
            recipe_id: self.recipe_id,
            values_json: parse_json("production batch item values", Some(&self.values_json))?
                .ok_or_else(|| {
                    RepositoryError::serialization("production batch item values", "missing value")
                })?,
            status: ProductionBatchItemStatus::parse(&self.status)
                .map_err(|error| map_domain_error("production batch item status", error))?,
            task_id: self.task_id,
            retry_of_item_id: self.retry_of_item_id,
            error_code: self.error_code,
            error_message: self.error_message,
            created_at: parse_datetime("production batch item created_at", &self.created_at)?,
            updated_at: parse_datetime("production batch item updated_at", &self.updated_at)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteProductionQueueRepository;
    use crate::application::ports::ProductionQueueRepository;
    use crate::domain::{
        ProductionBatch, ProductionBatchId, ProductionBatchItem, ProductionBatchItemId,
        ProductionBatchItemStatus, ProductionBatchStatus,
    };
    use crate::infrastructure::database::{
        pool::initialize, repositories::test_support::seed_task_dependencies,
    };
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use tempfile::tempdir;

    #[tokio::test]
    async fn uncertain_dispatch_is_failed_and_batch_is_paused_on_recovery() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("queue.db"))
            .await
            .unwrap();
        seed_task_dependencies(&pool).await;
        let repository = SqliteProductionQueueRepository::new(pool.clone());
        let now = Utc.with_ymd_and_hms(2026, 8, 8, 12, 0, 0).unwrap();
        let batch_id = ProductionBatchId::new();
        let item_id = ProductionBatchItemId::new();
        let batch = ProductionBatch {
            id: batch_id.clone(),
            project_id: "project-1".to_owned(),
            name: "Recovery test".to_owned(),
            status: ProductionBatchStatus::Running,
            continue_on_failure: false,
            archived_at: None,
            created_at: now,
            updated_at: now,
        };
        let item = ProductionBatchItem {
            id: item_id.clone(),
            batch_id: batch_id.clone(),
            ordinal: 0,
            workflow_version_id: "workflow-version-1".to_owned(),
            recipe_id: "recipe-1".to_owned(),
            values_json: json!({"prompt": {"type": "string", "value": "test"}}),
            status: ProductionBatchItemStatus::Pending,
            task_id: None,
            retry_of_item_id: None,
            error_code: None,
            error_message: None,
            created_at: now,
            updated_at: now,
        };
        repository.insert(&batch, &[item]).await.unwrap();
        assert!(repository
            .set_item_dispatching(&item_id, now)
            .await
            .unwrap());

        let recovered = repository.recover_uncertain_dispatches(now).await.unwrap();
        assert_eq!(recovered, vec![batch_id.clone()]);
        let detail = repository
            .find_detail("project-1", &batch_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(detail.batch.status, ProductionBatchStatus::Paused);
        assert_eq!(detail.items[0].status, ProductionBatchItemStatus::Failed);
        assert_eq!(
            detail.items[0].error_code.as_deref(),
            Some("QUEUE_DISPATCH_UNCERTAIN")
        );
        pool.close().await;
    }

    #[tokio::test]
    async fn archive_restore_and_requeue_preserve_original_failure_evidence() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("operations.db"))
            .await
            .unwrap();
        seed_task_dependencies(&pool).await;
        let repository = SqliteProductionQueueRepository::new(pool.clone());
        let now = Utc.with_ymd_and_hms(2026, 8, 8, 13, 0, 0).unwrap();
        let batch_id = ProductionBatchId::new();
        let source_id = ProductionBatchItemId::new();
        let batch = ProductionBatch {
            id: batch_id.clone(),
            project_id: "project-1".to_owned(),
            name: "Operations test".to_owned(),
            status: ProductionBatchStatus::Paused,
            continue_on_failure: false,
            archived_at: None,
            created_at: now,
            updated_at: now,
        };
        let source = ProductionBatchItem {
            id: source_id.clone(),
            batch_id: batch_id.clone(),
            ordinal: 0,
            workflow_version_id: "workflow-version-1".to_owned(),
            recipe_id: "recipe-1".to_owned(),
            values_json: json!({"prompt": {"type": "string", "value": "retry me"}}),
            status: ProductionBatchItemStatus::Failed,
            task_id: None,
            retry_of_item_id: None,
            error_code: Some("COMFY_TIMEOUT".to_owned()),
            error_message: Some("timeout".to_owned()),
            created_at: now,
            updated_at: now,
        };
        repository.insert(&batch, &[source]).await.unwrap();

        assert!(repository
            .set_archived_at("project-1", &batch_id, Some(now), now)
            .await
            .unwrap());
        let archived = repository
            .find_detail("project-1", &batch_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(archived.batch.archived_at, Some(now));

        assert!(repository
            .set_archived_at("project-1", &batch_id, None, now)
            .await
            .unwrap());
        let retry = ProductionBatchItem {
            id: ProductionBatchItemId::new(),
            batch_id: batch_id.clone(),
            ordinal: 1,
            workflow_version_id: "workflow-version-1".to_owned(),
            recipe_id: "recipe-1".to_owned(),
            values_json: json!({"prompt": {"type": "string", "value": "retry me"}}),
            status: ProductionBatchItemStatus::Pending,
            task_id: None,
            retry_of_item_id: Some(source_id.as_str().to_owned()),
            error_code: None,
            error_message: None,
            created_at: now,
            updated_at: now,
        };
        repository.append_requeue_item(&retry, now).await.unwrap();

        let detail = repository
            .find_detail("project-1", &batch_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(detail.batch.archived_at, None);
        assert_eq!(detail.batch.status, ProductionBatchStatus::Paused);
        assert_eq!(detail.items.len(), 2);
        assert_eq!(detail.items[0].status, ProductionBatchItemStatus::Failed);
        assert_eq!(detail.items[0].error_code.as_deref(), Some("COMFY_TIMEOUT"));
        assert_eq!(detail.items[1].status, ProductionBatchItemStatus::Pending);
        assert_eq!(
            detail.items[1].retry_of_item_id.as_deref(),
            Some(source_id.as_str())
        );
        pool.close().await;
    }
}
