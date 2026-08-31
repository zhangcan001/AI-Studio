use super::{
    format_datetime, i64_to_u64, map_domain_error, map_sqlx_error, parse_datetime, parse_json,
    parse_optional_datetime, serialize_json,
};
use crate::application::ports::{
    ActiveProductionItem, ActiveShotBatchBinding, ProductionBatchShotLink,
    ProductionQueueRepository, RepositoryError, ShotBatchBinding, ShotBatchRepository,
};
use crate::application::production_batch_runbook_service::{
    ProductionBatchRunbookRepository, ProductionBatchRunbookSourceRow,
};
use crate::domain::{
    PreparationSnapshotRecord, PreparedShotBatchRecord, ProductionBatch, ProductionBatchDetail,
    ProductionBatchId, ProductionBatchItem, ProductionBatchItemId, ProductionBatchItemStatus,
    ProductionBatchStatus, ProductionPackageBatchBinding, ProductionPackageProvenance, ShotStage,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Sqlite, SqlitePool, Transaction};

const MAX_LOGICAL_PRODUCTION_BATCH_ITEMS: usize = 100;

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
impl ProductionBatchRunbookRepository for SqliteProductionQueueRepository {
    async fn list_project_shot_batch_runbook_rows(
        &self,
        project_id: &str,
    ) -> Result<Vec<ProductionBatchRunbookSourceRow>, RepositoryError> {
        let rows = sqlx::query_as::<_, RunbookSourceRow>(
            "SELECT
                b.id AS batch_id, b.project_id, b.name AS batch_name,
                b.status AS batch_status, b.continue_on_failure, b.archived_at,
                b.created_at AS batch_created_at, b.updated_at AS batch_updated_at,
                i.id AS item_id, i.status AS item_status,
                l.shot_id, l.stage, a.scene_id
             FROM production_batch_items i
             INNER JOIN production_batches b ON b.id = i.batch_id
             INNER JOIN shot_generation_links l ON l.production_batch_item_id = i.id
             INNER JOIN shots s ON s.id = l.shot_id
             INNER JOIN shot_scene_assignments a ON a.shot_id = l.shot_id
             WHERE b.project_id = ? AND s.project_id = ?
             ORDER BY b.created_at ASC, b.id ASC, i.ordinal ASC, i.id ASC",
        )
        .bind(project_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(RunbookSourceRow::try_into_domain)
            .collect()
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
        insert_batch_records(&mut transaction, batch, items).await?;
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn insert_with_provenance(
        &self,
        batch: &ProductionBatch,
        items: &[ProductionBatchItem],
        provenance: &ProductionPackageProvenance,
    ) -> Result<(), RepositoryError> {
        let unique_package_item_count = provenance
            .package_item_ids
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len();
        if provenance.package_item_ids.len() != items.len()
            || provenance
                .package_item_ids
                .iter()
                .any(|id| id.trim().is_empty())
            || unique_package_item_count != provenance.package_item_ids.len()
        {
            return Err(RepositoryError::integrity(
                "production package binding must contain one unique id for every batch item",
            ));
        }
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        insert_batch_records(&mut transaction, batch, items).await?;
        insert_package_binding(&mut transaction, batch, provenance).await?;
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn list_package_bindings(
        &self,
        project_id: &str,
    ) -> Result<Vec<ProductionPackageBatchBinding>, RepositoryError> {
        let rows = sqlx::query_as::<_, PackageBindingRow>(
            "SELECT project_id, package_key, package_root, manifest_sha256,
                    package_id, package_name, batch_id, chunk_index, chunk_count,
                    package_item_ids_json, created_at, source_kind
             FROM production_package_batch_bindings
             WHERE project_id = ?
             ORDER BY created_at ASC, batch_id ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(PackageBindingRow::try_into_domain)
            .collect()
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
             FROM production_batches WHERE status = 'RUNNING' AND archived_at IS NULL ORDER BY created_at ASC, id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter().map(BatchRow::try_into_domain).collect()
    }

    async fn list_active_items(&self) -> Result<Vec<ActiveProductionItem>, RepositoryError> {
        let rows = sqlx::query_as::<_, ActiveItemRow>(
            "SELECT
                b.id AS batch_id, b.project_id, b.name AS batch_name,
                b.status AS batch_status, b.continue_on_failure, b.archived_at,
                b.created_at AS batch_created_at, b.updated_at AS batch_updated_at,
                i.id AS item_id, i.ordinal, i.workflow_version_id, i.recipe_id,
                i.values_json, i.status AS item_status, i.task_id, i.retry_of_item_id,
                i.error_code, i.error_message, i.created_at AS item_created_at,
                i.updated_at AS item_updated_at
             FROM production_batch_items i
             INNER JOIN production_batches b ON b.id = i.batch_id
             WHERE i.status IN ('DISPATCHING', 'DISPATCHED')
             ORDER BY b.created_at ASC, b.id ASC, i.ordinal ASC, i.id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(ActiveItemRow::try_into_domain)
            .collect()
    }

    async fn list_non_terminal_items(&self) -> Result<Vec<ActiveProductionItem>, RepositoryError> {
        let rows = sqlx::query_as::<_, ActiveItemRow>(
            "SELECT
                b.id AS batch_id, b.project_id, b.name AS batch_name,
                b.status AS batch_status, b.continue_on_failure, b.archived_at,
                b.created_at AS batch_created_at, b.updated_at AS batch_updated_at,
                i.id AS item_id, i.ordinal, i.workflow_version_id, i.recipe_id,
                i.values_json, i.status AS item_status, i.task_id, i.retry_of_item_id,
                i.error_code, i.error_message, i.created_at AS item_created_at,
                i.updated_at AS item_updated_at
             FROM production_batch_items i
             INNER JOIN production_batches b ON b.id = i.batch_id
             ORDER BY b.created_at ASC, b.id ASC, i.ordinal ASC, i.id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(ActiveItemRow::try_into_domain)
            .filter_map(|result| match result {
                Ok(item) if !item.item.status.is_terminal() => Some(Ok(item)),
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .collect()
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

    async fn cancel_pending_items(
        &self,
        project_id: &str,
        batch_id: &ProductionBatchId,
        updated_at: DateTime<Utc>,
    ) -> Result<u64, RepositoryError> {
        let result = sqlx::query(
            "UPDATE production_batch_items
             SET status = 'CANCELLED', error_code = NULL, error_message = NULL, updated_at = ?
             WHERE batch_id = ?
               AND status = 'PENDING'
               AND EXISTS (
                   SELECT 1 FROM production_batches
                   WHERE id = ? AND project_id = ?
               )",
        )
        .bind(format_datetime(updated_at))
        .bind(batch_id.as_str())
        .bind(batch_id.as_str())
        .bind(project_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(result.rows_affected())
    }

    async fn cancel_pending_items_and_complete(
        &self,
        project_id: &str,
        batch_id: &ProductionBatchId,
        updated_at: DateTime<Utc>,
    ) -> Result<u64, RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        let cancelled = sqlx::query(
            "UPDATE production_batch_items
             SET status = 'CANCELLED', error_code = NULL, error_message = NULL, updated_at = ?
             WHERE batch_id = ?
               AND status = 'PENDING'
               AND EXISTS (
                   SELECT 1 FROM production_batches
                   WHERE id = ? AND project_id = ?
               )",
        )
        .bind(format_datetime(updated_at))
        .bind(batch_id.as_str())
        .bind(batch_id.as_str())
        .bind(project_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?
        .rows_affected();

        if cancelled == 0 {
            transaction.commit().await.map_err(map_sqlx_error)?;
            return Ok(0);
        }

        let batch_update = sqlx::query(
            "UPDATE production_batches
             SET status = 'COMPLETED', updated_at = ?
             WHERE project_id = ? AND id = ?",
        )
        .bind(format_datetime(updated_at))
        .bind(project_id)
        .bind(batch_id.as_str())
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        if batch_update.rows_affected() == 0 {
            return Err(RepositoryError::integrity(
                "cancelled production items but could not complete their batch",
            ));
        }
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(cancelled)
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

#[async_trait]
impl ShotBatchRepository for SqliteProductionQueueRepository {
    async fn insert_prepared_batch_with_bindings(
        &self,
        batch: &ProductionBatch,
        items: &[ProductionBatchItem],
        bindings: &[ShotBatchBinding],
        snapshots: &[PreparationSnapshotRecord],
    ) -> Result<(), RepositoryError> {
        validate_prepared_insert(batch, items, bindings, snapshots)?;

        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        validate_shot_batch_bindings(&mut transaction, batch, items, bindings).await?;
        insert_batch_records(&mut transaction, batch, items).await?;
        insert_shot_batch_bindings(&mut transaction, batch, bindings).await?;
        for snapshot in snapshots {
            let snapshot_json = serde_json::to_string(&snapshot.snapshot).map_err(|error| {
                RepositoryError::serialization("production preparation snapshot", error.to_string())
            })?;
            sqlx::query(
                "INSERT INTO production_preparation_snapshots
                 (id, project_id, shot_id, stage, context_hash, production_batch_id,
                  production_batch_item_id, snapshot_json, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&snapshot.id)
            .bind(&snapshot.project_id)
            .bind(&snapshot.shot_id)
            .bind(snapshot.stage.as_str())
            .bind(&snapshot.context_hash)
            .bind(&snapshot.production_batch_id)
            .bind(&snapshot.production_batch_item_id)
            .bind(snapshot_json)
            .bind(format_datetime(snapshot.created_at))
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
        }
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn list_prepared_shot_records(
        &self,
        project_id: &str,
        stage: ShotStage,
        shot_ids: &[String],
    ) -> Result<Vec<PreparedShotBatchRecord>, RepositoryError> {
        if shot_ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = std::iter::repeat_n("?", shot_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let query = format!(
            "SELECT sps.shot_id, sps.stage, sps.context_hash,
                    sps.production_batch_id, sps.production_batch_item_id,
                    sps.id AS snapshot_id, i.status, sps.created_at
             FROM production_preparation_snapshots sps
             INNER JOIN production_batches b ON b.id = sps.production_batch_id
             INNER JOIN production_batch_items i ON i.id = sps.production_batch_item_id
             INNER JOIN shots s ON s.id = sps.shot_id
             WHERE sps.project_id = ? AND b.project_id = ? AND s.project_id = ?
               AND sps.stage = ?
               AND i.status IN ('PENDING', 'DISPATCHING', 'DISPATCHED')
               AND sps.shot_id IN ({placeholders})
             ORDER BY sps.shot_id ASC, sps.created_at ASC, sps.id ASC"
        );
        let mut request = sqlx::query_as::<_, PreparedShotBatchRow>(&query)
            .bind(project_id)
            .bind(project_id)
            .bind(project_id)
            .bind(stage.as_str());
        for shot_id in shot_ids {
            request = request.bind(shot_id);
        }
        request
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?
            .into_iter()
            .map(PreparedShotBatchRow::try_into_domain)
            .collect()
    }

    async fn find_preparation_snapshot(
        &self,
        project_id: &str,
        production_batch_item_id: &str,
    ) -> Result<Option<PreparationSnapshotRecord>, RepositoryError> {
        let row = sqlx::query_as::<_, PreparationSnapshotRow>(
            "SELECT sps.id, sps.project_id, sps.shot_id, sps.stage, sps.context_hash,
                    sps.production_batch_id, sps.production_batch_item_id,
                    sps.snapshot_json, sps.created_at
             FROM production_preparation_snapshots sps
             INNER JOIN production_batches b ON b.id = sps.production_batch_id
             INNER JOIN production_batch_items i ON i.id = sps.production_batch_item_id
             WHERE sps.project_id = ? AND b.project_id = ? AND sps.production_batch_item_id = ?",
        )
        .bind(project_id)
        .bind(project_id)
        .bind(production_batch_item_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        row.map(PreparationSnapshotRow::try_into_domain).transpose()
    }

    async fn list_preparation_snapshots_for_batch(
        &self,
        project_id: &str,
        production_batch_id: &str,
    ) -> Result<Vec<PreparationSnapshotRecord>, RepositoryError> {
        let rows = sqlx::query_as::<_, PreparationSnapshotRow>(
            "SELECT sps.id, sps.project_id, sps.shot_id, sps.stage, sps.context_hash,
                    sps.production_batch_id, sps.production_batch_item_id,
                    sps.snapshot_json, sps.created_at
             FROM production_preparation_snapshots sps
             INNER JOIN production_batches b ON b.id = sps.production_batch_id
             WHERE sps.project_id = ? AND b.project_id = ? AND sps.production_batch_id = ?
             ORDER BY sps.production_batch_item_id ASC",
        )
        .bind(project_id)
        .bind(project_id)
        .bind(production_batch_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(PreparationSnapshotRow::try_into_domain)
            .collect()
    }

    async fn list_shot_links_for_batch(
        &self,
        project_id: &str,
        production_batch_id: &str,
    ) -> Result<Vec<ProductionBatchShotLink>, RepositoryError> {
        let rows = sqlx::query_as::<_, ProductionBatchShotLinkRow>(
            "SELECT l.production_batch_item_id, l.shot_id, l.stage,
                    s.selected_image_asset_id, s.selected_video_asset_id
             FROM shot_generation_links l
             INNER JOIN production_batch_items i ON i.id = l.production_batch_item_id
             INNER JOIN production_batches b ON b.id = i.batch_id
             INNER JOIN shots s ON s.id = l.shot_id
             WHERE b.project_id = ? AND i.batch_id = ? AND s.project_id = ?
               AND l.production_batch_item_id IS NOT NULL
             ORDER BY i.ordinal ASC, l.id ASC",
        )
        .bind(project_id)
        .bind(production_batch_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(ProductionBatchShotLinkRow::try_into_domain)
            .collect()
    }

    async fn insert_batch_with_bindings(
        &self,
        batch: &ProductionBatch,
        items: &[ProductionBatchItem],
        bindings: &[ShotBatchBinding],
    ) -> Result<(), RepositoryError> {
        if bindings.len() != items.len() {
            return Err(RepositoryError::integrity(
                "Shot batch must provide exactly one binding for every production item",
            ));
        }
        if items.iter().any(|item| item.batch_id != batch.id) {
            return Err(RepositoryError::integrity(
                "every Shot batch item must belong to the inserted production batch",
            ));
        }
        let item_ids = items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<std::collections::HashSet<_>>();
        let mut bound_items = std::collections::HashSet::new();
        for binding in bindings {
            if !item_ids.contains(binding.production_batch_item_id.as_str())
                || !bound_items.insert(binding.production_batch_item_id.as_str())
            {
                return Err(RepositoryError::integrity(
                    "each Shot batch item must be bound exactly once",
                ));
            }
            if !matches!(binding.stage, ShotStage::Image | ShotStage::Video) {
                return Err(RepositoryError::integrity("invalid Shot batch stage"));
            }
        }

        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        for binding in bindings {
            let shot_project =
                sqlx::query_scalar::<_, String>("SELECT project_id FROM shots WHERE id = ?")
                    .bind(&binding.shot_id)
                    .fetch_optional(&mut *transaction)
                    .await
                    .map_err(map_sqlx_error)?
                    .ok_or_else(|| RepositoryError::not_found("shot", &binding.shot_id))?;
            if shot_project != batch.project_id {
                return Err(RepositoryError::integrity(
                    "Shot batch cannot bind a Shot from another project",
                ));
            }
            let item_batch_project = sqlx::query_as::<_, (String, String, String)>(
                "SELECT b.project_id, i.batch_id, i.status
                 FROM production_batch_items i
                 JOIN production_batches b ON b.id = i.batch_id
                 WHERE i.id = ?",
            )
            .bind(&binding.production_batch_item_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
            if item_batch_project.is_some() {
                return Err(RepositoryError::integrity(
                    "production batch item already exists before Shot batch insertion",
                ));
            }
        }
        insert_batch_records(&mut transaction, batch, items).await?;
        for binding in bindings {
            let existing = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM shot_generation_links WHERE production_batch_item_id = ?",
            )
            .bind(&binding.production_batch_item_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
            if existing != 0 {
                return Err(RepositoryError::integrity(
                    "production batch item already has a Shot generation link",
                ));
            }
            sqlx::query(
                "INSERT INTO shot_generation_links
                 (id, shot_id, stage, task_id, production_batch_item_id, created_at)
                 VALUES (?, ?, ?, NULL, ?, ?)",
            )
            .bind(format!("sgl_{}", uuid::Uuid::new_v4()))
            .bind(&binding.shot_id)
            .bind(binding.stage.as_str())
            .bind(&binding.production_batch_item_id)
            .bind(format_datetime(batch.created_at))
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
        }
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn bind_shot_item_task(
        &self,
        item_id: &str,
        task_id: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        let links = sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT id, task_id FROM shot_generation_links
             WHERE production_batch_item_id = ?",
        )
        .bind(item_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        if links.is_empty() {
            transaction.rollback().await.map_err(map_sqlx_error)?;
            return Ok(false);
        }
        if links.len() != 1 {
            return Err(RepositoryError::integrity(
                "production batch item has multiple Shot generation links",
            ));
        }
        if links[0].1.is_some() {
            return Err(RepositoryError::integrity(
                "Shot generation link is already bound to a task",
            ));
        }
        let item_updated = sqlx::query(
            "UPDATE production_batch_items
             SET status = 'DISPATCHED', task_id = ?, updated_at = ?
             WHERE id = ? AND status = 'DISPATCHING' AND task_id IS NULL",
        )
        .bind(task_id)
        .bind(format_datetime(updated_at))
        .bind(item_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        if item_updated.rows_affected() != 1 {
            return Err(RepositoryError::integrity(
                "Shot batch item is no longer in the dispatching state",
            ));
        }
        let link_updated = sqlx::query(
            "UPDATE shot_generation_links SET task_id = ?
             WHERE production_batch_item_id = ? AND task_id IS NULL",
        )
        .bind(task_id)
        .bind(item_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        if link_updated.rows_affected() != 1 {
            return Err(RepositoryError::integrity(
                "Shot generation link could not be bound to the task",
            ));
        }
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(true)
    }

    async fn append_requeue_item_with_binding(
        &self,
        item: &ProductionBatchItem,
        source_item_id: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        let link = sqlx::query_as::<_, (String, String, String)>(
            "SELECT id, shot_id, stage FROM shot_generation_links
             WHERE production_batch_item_id = ?",
        )
        .bind(source_item_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        if link.is_empty() {
            transaction.rollback().await.map_err(map_sqlx_error)?;
            return Ok(false);
        }
        if link.len() != 1 {
            return Err(RepositoryError::integrity(
                "production batch item has multiple Shot generation links",
            ));
        }
        let source_item = sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT batch_id, workflow_version_id, recipe_id, values_json
             FROM production_batch_items WHERE id = ?",
        )
        .bind(source_item_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?
        .ok_or_else(|| RepositoryError::not_found("production batch item", source_item_id))?;
        if source_item.0 != item.batch_id.as_str() {
            return Err(RepositoryError::integrity(
                "Shot retry item and source item must belong to the same production batch",
            ));
        }
        let source_values = parse_json("production batch item values", Some(&source_item.3))?
            .ok_or_else(|| {
                RepositoryError::serialization("production batch item values", "missing value")
            })?;
        let mut frozen_item = item.clone();
        frozen_item.workflow_version_id = source_item.1;
        frozen_item.recipe_id = source_item.2;
        frozen_item.values_json = source_values;
        let (_, shot_id, stage) = &link[0];
        let stage = ShotStage::try_from_str(stage)
            .map_err(|error| map_domain_error("Shot generation stage", error))?;
        insert_requeue_item_record(&mut transaction, &frozen_item).await?;
        sqlx::query(
            "INSERT INTO shot_generation_links
             (id, shot_id, stage, task_id, production_batch_item_id, created_at)
             VALUES (?, ?, ?, NULL, ?, ?)",
        )
        .bind(format!("sgl_{}", uuid::Uuid::new_v4()))
        .bind(shot_id)
        .bind(stage.as_str())
        .bind(frozen_item.id.as_str())
        .bind(format_datetime(frozen_item.created_at))
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        sqlx::query(
            "UPDATE production_batches SET status = 'PAUSED', archived_at = NULL, updated_at = ? WHERE id = ?",
        )
        .bind(format_datetime(updated_at))
        .bind(frozen_item.batch_id.as_str())
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(true)
    }

    async fn append_requeue_items_with_bindings(
        &self,
        items: &[ProductionBatchItem],
        updated_at: DateTime<Utc>,
    ) -> Result<(Vec<String>, Vec<String>), RepositoryError> {
        let Some(first_item) = items.first() else {
            return Ok((Vec::new(), Vec::new()));
        };
        if items
            .iter()
            .any(|item| item.batch_id != first_item.batch_id)
        {
            return Err(RepositoryError::integrity(
                "bulk retry items must belong to one production batch",
            ));
        }
        if items.iter().any(|item| item.retry_of_item_id.is_none()) {
            return Err(RepositoryError::integrity(
                "bulk retry items must reference their source item",
            ));
        }

        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;
        let batch_exists =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_batches WHERE id = ?")
                .bind(first_item.batch_id.as_str())
                .fetch_one(&mut *transaction)
                .await
                .map_err(map_sqlx_error)?;
        if batch_exists == 0 {
            return Err(RepositoryError::not_found(
                "production batch",
                first_item.batch_id.as_str(),
            ));
        }

        let mut created_item_ids = Vec::new();
        let mut existing_retry_item_ids = Vec::new();
        for item in items {
            let source_item_id = item.retry_of_item_id.as_deref().ok_or_else(|| {
                RepositoryError::integrity("bulk retry items must reference their source item")
            })?;
            let source = sqlx::query_as::<_, (String, String, String, String)>(
                "SELECT batch_id, workflow_version_id, recipe_id, values_json
                 FROM production_batch_items WHERE id = ?",
            )
            .bind(source_item_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?
            .ok_or_else(|| RepositoryError::not_found("production batch item", source_item_id))?;
            if source.0 != item.batch_id.as_str() {
                return Err(RepositoryError::integrity(
                    "bulk retry item and source item must belong to the same production batch",
                ));
            }

            if let Some(existing_id) = sqlx::query_scalar::<_, String>(
                "SELECT id FROM production_batch_items
                 WHERE batch_id = ? AND retry_of_item_id = ?
                 ORDER BY ordinal ASC, id ASC LIMIT 1",
            )
            .bind(item.batch_id.as_str())
            .bind(source_item_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?
            {
                existing_retry_item_ids.push(existing_id);
                continue;
            }

            let links = sqlx::query_as::<_, (String, String)>(
                "SELECT shot_id, stage FROM shot_generation_links
                 WHERE production_batch_item_id = ?",
            )
            .bind(source_item_id)
            .fetch_all(&mut *transaction)
            .await
            .map_err(map_sqlx_error)?;
            if links.len() > 1 {
                return Err(RepositoryError::integrity(
                    "production batch item has multiple Shot generation links",
                ));
            }

            let source_values = parse_json("production batch item values", Some(&source.3))?
                .ok_or_else(|| {
                    RepositoryError::serialization("production batch item values", "missing value")
                })?;
            let mut frozen_item = item.clone();
            frozen_item.workflow_version_id = source.1;
            frozen_item.recipe_id = source.2;
            frozen_item.values_json = source_values;
            insert_requeue_item_record(&mut transaction, &frozen_item).await?;

            if let Some((shot_id, stage)) = links.first() {
                let stage = ShotStage::try_from_str(stage)
                    .map_err(|error| map_domain_error("Shot generation stage", error))?;
                sqlx::query(
                    "INSERT INTO shot_generation_links
                     (id, shot_id, stage, task_id, production_batch_item_id, created_at)
                     VALUES (?, ?, ?, NULL, ?, ?)",
                )
                .bind(format!("sgl_{}", uuid::Uuid::new_v4()))
                .bind(shot_id)
                .bind(stage.as_str())
                .bind(frozen_item.id.as_str())
                .bind(format_datetime(frozen_item.created_at))
                .execute(&mut *transaction)
                .await
                .map_err(map_sqlx_error)?;
            }
            created_item_ids.push(frozen_item.id.as_str().to_owned());
        }

        let batch_update = sqlx::query(
            "UPDATE production_batches
             SET status = 'PAUSED', archived_at = NULL, updated_at = ? WHERE id = ?",
        )
        .bind(format_datetime(updated_at))
        .bind(first_item.batch_id.as_str())
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;
        if batch_update.rows_affected() != 1 {
            return Err(RepositoryError::integrity(
                "bulk retry items could not pause their production batch",
            ));
        }
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok((created_item_ids, existing_retry_item_ids))
    }

    async fn has_active_shot_binding(
        &self,
        project_id: &str,
        shot_id: &str,
    ) -> Result<bool, RepositoryError> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)
             FROM shot_generation_links l
             JOIN production_batch_items i ON i.id = l.production_batch_item_id
             JOIN production_batches b ON b.id = i.batch_id
             WHERE l.shot_id = ? AND b.project_id = ?
               AND i.status IN ('PENDING', 'DISPATCHING', 'DISPATCHED')",
        )
        .bind(shot_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(count > 0)
    }

    async fn list_active_shot_bindings(
        &self,
        project_id: &str,
        stage: ShotStage,
        shot_ids: &[String],
    ) -> Result<Vec<ActiveShotBatchBinding>, RepositoryError> {
        if shot_ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = std::iter::repeat_n("?", shot_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let query = format!(
            "SELECT l.shot_id, l.production_batch_item_id, i.batch_id
             FROM shot_generation_links l
             JOIN production_batch_items i ON i.id = l.production_batch_item_id
             JOIN production_batches b ON b.id = i.batch_id
             WHERE b.project_id = ? AND l.stage = ?
               AND i.status IN ('PENDING', 'DISPATCHING', 'DISPATCHED')
               AND l.shot_id IN ({placeholders})
             ORDER BY l.shot_id ASC, i.batch_id ASC, i.id ASC"
        );
        let mut request = sqlx::query_as::<_, (String, String, String)>(&query)
            .bind(project_id)
            .bind(stage.as_str());
        for shot_id in shot_ids {
            request = request.bind(shot_id);
        }
        let rows = request
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        Ok(rows
            .into_iter()
            .map(|(shot_id, production_batch_item_id, production_batch_id)| {
                ActiveShotBatchBinding {
                    shot_id,
                    stage,
                    production_batch_id,
                    production_batch_item_id,
                }
            })
            .collect())
    }
}

fn validate_prepared_insert(
    batch: &ProductionBatch,
    items: &[ProductionBatchItem],
    bindings: &[ShotBatchBinding],
    snapshots: &[PreparationSnapshotRecord],
) -> Result<(), RepositoryError> {
    if snapshots.len() != items.len() || bindings.len() != items.len() {
        return Err(RepositoryError::integrity(
            "prepared Shot batch must provide exactly one binding and snapshot for every item",
        ));
    }
    let item_ids = items
        .iter()
        .map(|item| item.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let mut binding_item_ids = std::collections::HashSet::new();
    let mut snapshot_item_ids = std::collections::HashSet::new();
    for item in items {
        if item.batch_id != batch.id {
            return Err(RepositoryError::integrity(
                "every prepared Shot batch item must belong to the inserted production batch",
            ));
        }
    }
    for binding in bindings {
        if !item_ids.contains(binding.production_batch_item_id.as_str())
            || !binding_item_ids.insert(binding.production_batch_item_id.as_str())
        {
            return Err(RepositoryError::integrity(
                "each prepared Shot batch item must be bound exactly once",
            ));
        }
    }
    for snapshot in snapshots {
        if snapshot.project_id != batch.project_id
            || snapshot.production_batch_id != batch.id.as_str()
            || !item_ids.contains(snapshot.production_batch_item_id.as_str())
            || !snapshot_item_ids.insert(snapshot.production_batch_item_id.as_str())
            || snapshot.snapshot.project_id != snapshot.project_id
            || snapshot.snapshot.shot_id != snapshot.shot_id
            || snapshot.snapshot.stage != snapshot.stage.as_str()
            || snapshot.snapshot.context_hash != snapshot.context_hash
        {
            return Err(RepositoryError::integrity(
                "prepared snapshot identity must match its batch and item binding",
            ));
        }
    }
    Ok(())
}

async fn validate_shot_batch_bindings(
    transaction: &mut Transaction<'_, Sqlite>,
    batch: &ProductionBatch,
    items: &[ProductionBatchItem],
    bindings: &[ShotBatchBinding],
) -> Result<(), RepositoryError> {
    let item_ids = items
        .iter()
        .map(|item| item.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    if bindings
        .iter()
        .any(|binding| !item_ids.contains(binding.production_batch_item_id.as_str()))
    {
        return Err(RepositoryError::integrity(
            "prepared Shot binding references an unknown item",
        ));
    }

    let requested_shot_ids = bindings
        .iter()
        .map(|binding| binding.shot_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    if !requested_shot_ids.is_empty() {
        let placeholders = std::iter::repeat_n("?", requested_shot_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let query = format!("SELECT id, project_id FROM shots WHERE id IN ({placeholders})");
        let mut request = sqlx::query_as::<_, (String, String)>(&query);
        for shot_id in &requested_shot_ids {
            request = request.bind(*shot_id);
        }
        let shot_rows = request
            .fetch_all(&mut **transaction)
            .await
            .map_err(map_sqlx_error)?;
        let returned_valid_count = shot_rows
            .iter()
            .filter(|(_, project_id)| project_id == &batch.project_id)
            .count();
        if returned_valid_count != requested_shot_ids.len() {
            for binding in bindings {
                match shot_rows
                    .iter()
                    .find(|(shot_id, _)| shot_id == &binding.shot_id)
                {
                    None => return Err(RepositoryError::not_found("shot", &binding.shot_id)),
                    Some((_, project_id)) if project_id != &batch.project_id => {
                        return Err(RepositoryError::integrity(
                            "prepared Shot batch cannot bind a Shot from another project",
                        ));
                    }
                    Some(_) => {}
                }
            }
            return Err(RepositoryError::integrity(
                "prepared Shot batch cannot bind a Shot from another project",
            ));
        }
    }

    let requested_item_ids = bindings
        .iter()
        .map(|binding| binding.production_batch_item_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    if !requested_item_ids.is_empty() {
        let placeholders = std::iter::repeat_n("?", requested_item_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let query = format!(
            "SELECT DISTINCT production_batch_item_id
             FROM shot_generation_links
             WHERE production_batch_item_id IN ({placeholders})"
        );
        let mut request = sqlx::query_scalar::<_, String>(&query);
        for item_id in &requested_item_ids {
            request = request.bind(*item_id);
        }
        let existing_item_ids = request
            .fetch_all(&mut **transaction)
            .await
            .map_err(map_sqlx_error)?;
        if !existing_item_ids.is_empty() {
            return Err(RepositoryError::integrity(
                "production batch item already has a Shot generation link",
            ));
        }
    }
    Ok(())
}

async fn insert_shot_batch_bindings(
    transaction: &mut Transaction<'_, Sqlite>,
    batch: &ProductionBatch,
    bindings: &[ShotBatchBinding],
) -> Result<(), RepositoryError> {
    for binding in bindings {
        sqlx::query(
            "INSERT INTO shot_generation_links
             (id, shot_id, stage, task_id, production_batch_item_id, created_at)
             VALUES (?, ?, ?, NULL, ?, ?)",
        )
        .bind(format!("sgl_{}", uuid::Uuid::new_v4()))
        .bind(&binding.shot_id)
        .bind(binding.stage.as_str())
        .bind(&binding.production_batch_item_id)
        .bind(format_datetime(batch.created_at))
        .execute(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?;
    }
    Ok(())
}

async fn insert_batch_records(
    transaction: &mut Transaction<'_, Sqlite>,
    batch: &ProductionBatch,
    items: &[ProductionBatchItem],
) -> Result<(), RepositoryError> {
    let logical_item_count = items
        .iter()
        .filter(|item| item.retry_of_item_id.is_none())
        .count();
    if logical_item_count > MAX_LOGICAL_PRODUCTION_BATCH_ITEMS {
        return Err(RepositoryError::integrity(format!(
            "production batch must contain at most {MAX_LOGICAL_PRODUCTION_BATCH_ITEMS} logical items"
        )));
    }
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
    .execute(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;

    for item in items {
        let values_json = serialize_json("production batch item values", Some(&item.values_json))?
            .ok_or_else(|| {
                RepositoryError::serialization("production batch item values", "missing value")
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
        .execute(&mut **transaction)
        .await
        .map_err(map_sqlx_error)?;
    }
    Ok(())
}

async fn insert_package_binding(
    transaction: &mut Transaction<'_, Sqlite>,
    batch: &ProductionBatch,
    provenance: &ProductionPackageProvenance,
) -> Result<(), RepositoryError> {
    let existing_json = sqlx::query_scalar::<_, String>(
        "SELECT package_item_ids_json
         FROM production_package_batch_bindings
         WHERE project_id = ? AND package_key = ?",
    )
    .bind(&batch.project_id)
    .bind(&provenance.source_package_key)
    .fetch_all(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    for json in existing_json {
        let existing_ids = serde_json::from_str::<Vec<String>>(&json).map_err(|error| {
            RepositoryError::serialization("production package item ids", error.to_string())
        })?;
        if existing_ids.iter().any(|id| {
            provenance
                .package_item_ids
                .iter()
                .any(|new_id| new_id == id)
        }) {
            return Err(RepositoryError::integrity(
                "production package item is already bound to a production batch",
            ));
        }
    }
    let package_item_ids_json =
        serde_json::to_string(&provenance.package_item_ids).map_err(|error| {
            RepositoryError::serialization("production package item ids", error.to_string())
        })?;
    sqlx::query(
        "INSERT INTO production_package_batch_bindings
         (project_id, package_key, package_root, manifest_sha256, package_id,
          package_name, batch_id, chunk_index, chunk_count, package_item_ids_json,
          created_at, source_kind)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'PRODUCTION_PACKAGE')",
    )
    .bind(&batch.project_id)
    .bind(&provenance.source_package_key)
    .bind(&provenance.source_package_root)
    .bind(&provenance.source_package_manifest_sha256)
    .bind(&provenance.source_package_id)
    .bind(&provenance.source_package_name)
    .bind(batch.id.as_str())
    .bind(i64::from(provenance.source_package_chunk_index))
    .bind(i64::from(provenance.source_package_chunk_count))
    .bind(package_item_ids_json)
    .bind(format_datetime(batch.created_at))
    .execute(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

async fn insert_requeue_item_record(
    transaction: &mut Transaction<'_, Sqlite>,
    item: &ProductionBatchItem,
) -> Result<(), RepositoryError> {
    let values_json = serialize_json("production batch item values", Some(&item.values_json))?
        .ok_or_else(|| {
            RepositoryError::serialization("production batch item values", "missing value")
        })?;
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
    .execute(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
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

#[derive(sqlx::FromRow)]
struct PackageBindingRow {
    project_id: String,
    package_key: String,
    package_root: String,
    manifest_sha256: String,
    package_id: Option<String>,
    package_name: String,
    batch_id: String,
    chunk_index: i64,
    chunk_count: i64,
    package_item_ids_json: String,
    created_at: String,
    source_kind: String,
}

impl PackageBindingRow {
    fn try_into_domain(self) -> Result<ProductionPackageBatchBinding, RepositoryError> {
        let package_item_ids = serde_json::from_str::<Vec<String>>(&self.package_item_ids_json)
            .map_err(|error| {
                RepositoryError::serialization("production package item ids", error.to_string())
            })?;
        let chunk_index = u32::try_from(self.chunk_index).map_err(|_| {
            RepositoryError::serialization(
                "production package chunk index",
                format!("invalid value {}", self.chunk_index),
            )
        })?;
        let chunk_count = u32::try_from(self.chunk_count).map_err(|_| {
            RepositoryError::serialization(
                "production package chunk count",
                format!("invalid value {}", self.chunk_count),
            )
        })?;
        Ok(ProductionPackageBatchBinding {
            project_id: self.project_id,
            package_key: self.package_key,
            package_root: self.package_root,
            manifest_sha256: self.manifest_sha256,
            package_id: self.package_id,
            package_name: self.package_name,
            batch_id: self.batch_id,
            chunk_index,
            chunk_count,
            package_item_ids,
            created_at: parse_datetime("production package binding created_at", &self.created_at)?,
            source_kind: self.source_kind,
        })
    }
}

#[derive(sqlx::FromRow)]
struct RunbookSourceRow {
    batch_id: String,
    project_id: String,
    batch_name: String,
    batch_status: String,
    continue_on_failure: i64,
    archived_at: Option<String>,
    batch_created_at: String,
    batch_updated_at: String,
    item_id: String,
    item_status: String,
    shot_id: String,
    stage: String,
    scene_id: String,
}

impl RunbookSourceRow {
    fn try_into_domain(self) -> Result<ProductionBatchRunbookSourceRow, RepositoryError> {
        Ok(ProductionBatchRunbookSourceRow {
            batch: BatchRow {
                id: self.batch_id,
                project_id: self.project_id,
                name: self.batch_name,
                status: self.batch_status,
                continue_on_failure: self.continue_on_failure,
                archived_at: self.archived_at,
                created_at: self.batch_created_at,
                updated_at: self.batch_updated_at,
            }
            .try_into_domain()?,
            item_id: self.item_id,
            item_status: ProductionBatchItemStatus::parse(&self.item_status)
                .map_err(|error| map_domain_error("production batch item status", error))?,
            shot_id: self.shot_id,
            stage: ShotStage::try_from_str(&self.stage)
                .map_err(|error| map_domain_error("Shot generation stage", error))?,
            scene_id: self.scene_id,
        })
    }
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

#[derive(sqlx::FromRow)]
struct ActiveItemRow {
    batch_id: String,
    project_id: String,
    batch_name: String,
    batch_status: String,
    continue_on_failure: i64,
    archived_at: Option<String>,
    batch_created_at: String,
    batch_updated_at: String,
    item_id: String,
    ordinal: i64,
    workflow_version_id: String,
    recipe_id: String,
    values_json: String,
    item_status: String,
    task_id: Option<String>,
    retry_of_item_id: Option<String>,
    error_code: Option<String>,
    error_message: Option<String>,
    item_created_at: String,
    item_updated_at: String,
}

impl ActiveItemRow {
    fn try_into_domain(self) -> Result<ActiveProductionItem, RepositoryError> {
        let batch_id = self.batch_id.clone();
        Ok(ActiveProductionItem {
            batch: BatchRow {
                id: self.batch_id,
                project_id: self.project_id,
                name: self.batch_name,
                status: self.batch_status,
                continue_on_failure: self.continue_on_failure,
                archived_at: self.archived_at,
                created_at: self.batch_created_at,
                updated_at: self.batch_updated_at,
            }
            .try_into_domain()?,
            item: ItemRow {
                id: self.item_id,
                batch_id,
                ordinal: self.ordinal,
                workflow_version_id: self.workflow_version_id,
                recipe_id: self.recipe_id,
                values_json: self.values_json,
                status: self.item_status,
                task_id: self.task_id,
                retry_of_item_id: self.retry_of_item_id,
                error_code: self.error_code,
                error_message: self.error_message,
                created_at: self.item_created_at,
                updated_at: self.item_updated_at,
            }
            .try_into_domain()?,
        })
    }
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

#[derive(sqlx::FromRow)]
struct PreparedShotBatchRow {
    shot_id: String,
    stage: String,
    context_hash: String,
    production_batch_id: String,
    production_batch_item_id: String,
    snapshot_id: String,
    status: String,
    created_at: String,
}

impl PreparedShotBatchRow {
    fn try_into_domain(self) -> Result<PreparedShotBatchRecord, RepositoryError> {
        Ok(PreparedShotBatchRecord {
            shot_id: self.shot_id,
            stage: ShotStage::try_from_str(&self.stage)
                .map_err(|error| map_domain_error("preparation snapshot stage", error))?,
            context_hash: self.context_hash,
            production_batch_id: self.production_batch_id,
            production_batch_item_id: self.production_batch_item_id,
            item_status: ProductionBatchItemStatus::parse(&self.status)
                .map_err(|error| map_domain_error("production batch item status", error))?,
            snapshot_id: self.snapshot_id,
            created_at: parse_datetime("preparation snapshot created_at", &self.created_at)?,
        })
    }
}

#[derive(sqlx::FromRow)]
struct PreparationSnapshotRow {
    id: String,
    project_id: String,
    shot_id: String,
    stage: String,
    context_hash: String,
    production_batch_id: String,
    production_batch_item_id: String,
    snapshot_json: String,
    created_at: String,
}

#[derive(sqlx::FromRow)]
struct ProductionBatchShotLinkRow {
    production_batch_item_id: String,
    shot_id: String,
    stage: String,
    selected_image_asset_id: Option<String>,
    selected_video_asset_id: Option<String>,
}

impl ProductionBatchShotLinkRow {
    fn try_into_domain(self) -> Result<ProductionBatchShotLink, RepositoryError> {
        Ok(ProductionBatchShotLink {
            production_batch_item_id: self.production_batch_item_id,
            shot_id: self.shot_id,
            stage: ShotStage::try_from_str(&self.stage)
                .map_err(|error| map_domain_error("production batch Shot stage", error))?,
            selected_image_asset_id: self.selected_image_asset_id,
            selected_video_asset_id: self.selected_video_asset_id,
        })
    }
}

impl PreparationSnapshotRow {
    fn try_into_domain(self) -> Result<PreparationSnapshotRecord, RepositoryError> {
        let stage = ShotStage::try_from_str(&self.stage)
            .map_err(|error| map_domain_error("preparation snapshot stage", error))?;
        let snapshot: crate::domain::PreparationSnapshotV1 =
            serde_json::from_str(&self.snapshot_json).map_err(|error| {
                RepositoryError::serialization("production preparation snapshot", error.to_string())
            })?;
        if snapshot.schema_version != crate::domain::PREPARATION_SNAPSHOT_SCHEMA_VERSION
            || snapshot.project_id != self.project_id
            || snapshot.shot_id != self.shot_id
            || snapshot.stage != self.stage
            || snapshot.context_hash != self.context_hash
        {
            return Err(RepositoryError::integrity(
                "preparation snapshot JSON identity does not match its columns",
            ));
        }
        Ok(PreparationSnapshotRecord {
            id: self.id,
            project_id: self.project_id,
            shot_id: self.shot_id,
            stage,
            context_hash: self.context_hash,
            production_batch_id: self.production_batch_id,
            production_batch_item_id: self.production_batch_item_id,
            snapshot,
            created_at: parse_datetime("preparation snapshot created_at", &self.created_at)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteProductionQueueRepository;
    use crate::application::ports::{
        ProductionQueueRepository, RepositoryError, ShotBatchBinding, ShotBatchRepository,
    };
    use crate::domain::production_preparation::{
        PreparationSnapshotPrompt, PreparationSnapshotReadiness, PreparationSnapshotRecord,
        PreparationSnapshotV1, PreparationSnapshotWorkflow,
    };
    use crate::domain::{
        ProductionBatch, ProductionBatchId, ProductionBatchItem, ProductionBatchItemId,
        ProductionBatchItemStatus, ProductionBatchStatus, ProductionPackageProvenance, ShotStage,
    };
    use crate::infrastructure::database::{
        pool::initialize, repositories::test_support::seed_task_dependencies,
    };
    use chrono::{DateTime, TimeZone, Utc};
    use serde_json::json;
    use sqlx::SqlitePool;
    use tempfile::tempdir;

    #[tokio::test]
    async fn package_binding_is_atomic_project_scoped_restartable_and_cascades() {
        let directory = tempdir().unwrap();
        let database_path = directory.path().join("package-binding.db");
        let pool = initialize(&database_path).await.unwrap();
        seed_task_dependencies(&pool).await;
        let repository = SqliteProductionQueueRepository::new(pool.clone());
        let now = Utc.with_ymd_and_hms(2026, 8, 31, 1, 0, 0).unwrap();
        let batch_id = ProductionBatchId::new();
        let item = fixture_item(
            &batch_id,
            0,
            ProductionBatchItemStatus::Pending,
            None,
            json!({"prompt": "package"}),
        );
        let batch = fixture_batch(&batch_id, ProductionBatchStatus::Ready, now);
        let provenance = ProductionPackageProvenance::new(
            std::path::Path::new("C:/packages/ep01"),
            "a".repeat(64),
            Some("ep01".to_owned()),
            "EP01",
            0,
            2,
            vec!["package-item-1".to_owned()],
        );
        repository
            .insert_with_provenance(&batch, std::slice::from_ref(&item), &provenance)
            .await
            .unwrap();
        let bindings = repository.list_package_bindings("project-1").await.unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].package_item_ids, vec!["package-item-1"]);
        assert_eq!(bindings[0].chunk_index, 0);
        assert_eq!(bindings[0].chunk_count, 2);
        assert!(repository
            .list_package_bindings("other-project")
            .await
            .unwrap()
            .is_empty());

        let second_batch_id = ProductionBatchId::new();
        let second_item = fixture_item(
            &second_batch_id,
            0,
            ProductionBatchItemStatus::Pending,
            None,
            json!({"prompt": "package-2"}),
        );
        let second_provenance = ProductionPackageProvenance::new(
            std::path::Path::new("C:/packages/ep01"),
            "a".repeat(64),
            Some("ep01".to_owned()),
            "EP01",
            1,
            2,
            vec!["package-item-2".to_owned()],
        );
        repository
            .insert_with_provenance(
                &fixture_batch(&second_batch_id, ProductionBatchStatus::Ready, now),
                std::slice::from_ref(&second_item),
                &second_provenance,
            )
            .await
            .unwrap();
        let bindings = repository.list_package_bindings("project-1").await.unwrap();
        assert_eq!(bindings.len(), 2);
        assert!(bindings
            .iter()
            .all(|binding| binding.package_key == provenance.source_package_key));

        pool.close().await;
        let reopened_pool = initialize(&database_path).await.unwrap();
        let reopened = SqliteProductionQueueRepository::new(reopened_pool.clone());
        assert_eq!(
            reopened
                .list_package_bindings("project-1")
                .await
                .unwrap()
                .len(),
            2
        );
        assert!(reopened.delete_batch("project-1", &batch_id).await.unwrap());
        assert_eq!(
            reopened
                .list_package_bindings("project-1")
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(reopened
            .delete_batch("project-1", &second_batch_id)
            .await
            .unwrap());
        assert!(reopened
            .list_package_bindings("project-1")
            .await
            .unwrap()
            .is_empty());
        reopened_pool.close().await;
    }

    #[tokio::test]
    async fn package_binding_insert_rolls_back_batch_when_item_insert_fails() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("package-binding-rollback.db"))
            .await
            .unwrap();
        seed_task_dependencies(&pool).await;
        let repository = SqliteProductionQueueRepository::new(pool.clone());
        let now = Utc.with_ymd_and_hms(2026, 8, 31, 1, 0, 0).unwrap();
        let batch_id = ProductionBatchId::new();
        let mut item = fixture_item(
            &batch_id,
            0,
            ProductionBatchItemStatus::Pending,
            None,
            json!({"prompt": "package"}),
        );
        item.recipe_id = "recipe-missing".to_owned();
        let provenance = ProductionPackageProvenance::new(
            std::path::Path::new("C:/packages/ep01"),
            "a".repeat(64),
            None,
            "EP01",
            0,
            1,
            vec!["package-item-1".to_owned()],
        );
        assert!(repository
            .insert_with_provenance(
                &fixture_batch(&batch_id, ProductionBatchStatus::Ready, now),
                &[item],
                &provenance,
            )
            .await
            .is_err());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_batches WHERE id = ?")
                .bind(batch_id.as_str())
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
        assert!(repository
            .list_package_bindings("project-1")
            .await
            .unwrap()
            .is_empty());
        pool.close().await;
    }

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

    #[tokio::test]
    async fn shot_batch_retry_and_restart_reuse_the_source_frozen_reference_order() {
        let directory = tempdir().unwrap();
        let database_path = directory.path().join("shot-batch-freeze.db");
        let pool = initialize(&database_path).await.unwrap();
        seed_task_dependencies(&pool).await;
        sqlx::query(
            "INSERT INTO shots (id, project_id, ordinal, name, prompt_text, created_at, updated_at)
             VALUES ('sht_freeze', 'project-1', 0, '冻结测试', 'REF2VA', ?, ?)",
        )
        .bind("2026-08-17T00:00:00Z")
        .bind("2026-08-17T00:00:00Z")
        .execute(&pool)
        .await
        .unwrap();

        let repository = SqliteProductionQueueRepository::new(pool.clone());
        let now = Utc.with_ymd_and_hms(2026, 8, 17, 0, 0, 0).unwrap();
        let batch_id = ProductionBatchId::new();
        let source_id = ProductionBatchItemId::new();
        let frozen_values = json!({
            "reference_images": {
                "type": "image_assets",
                "assetIds": ["ast_b", "ast_a", "ast_c"]
            }
        });
        let batch = ProductionBatch {
            id: batch_id.clone(),
            project_id: "project-1".to_owned(),
            name: "REF2VA freeze".to_owned(),
            status: ProductionBatchStatus::Paused,
            continue_on_failure: true,
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
            values_json: frozen_values.clone(),
            status: ProductionBatchItemStatus::Failed,
            task_id: None,
            retry_of_item_id: None,
            error_code: Some("COMFY_TIMEOUT".to_owned()),
            error_message: Some("transient".to_owned()),
            created_at: now,
            updated_at: now,
        };
        repository
            .insert_batch_with_bindings(
                &batch,
                std::slice::from_ref(&source),
                &[ShotBatchBinding {
                    shot_id: "sht_freeze".to_owned(),
                    stage: ShotStage::Video,
                    production_batch_item_id: source_id.as_str().to_owned(),
                }],
            )
            .await
            .unwrap();
        pool.close().await;

        let restarted_pool = initialize(&database_path).await.unwrap();
        let restarted = SqliteProductionQueueRepository::new(restarted_pool.clone());
        let restored = restarted
            .find_detail("project-1", &batch_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(restored.items[0].values_json, frozen_values);

        let retry = ProductionBatchItem {
            id: ProductionBatchItemId::new(),
            batch_id: batch_id.clone(),
            ordinal: 1,
            workflow_version_id: "wrong-workflow".to_owned(),
            recipe_id: "wrong-recipe".to_owned(),
            values_json: json!({
                "reference_images": {
                    "type": "image_assets",
                    "assetIds": ["ast_c", "ast_b", "ast_a"]
                }
            }),
            status: ProductionBatchItemStatus::Pending,
            task_id: None,
            retry_of_item_id: Some(source_id.as_str().to_owned()),
            error_code: None,
            error_message: None,
            created_at: now,
            updated_at: now,
        };
        assert!(restarted
            .append_requeue_item_with_binding(&retry, source_id.as_str(), now)
            .await
            .unwrap());

        let detail = restarted
            .find_detail("project-1", &batch_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(detail.items.len(), 2);
        assert_eq!(detail.items[1].values_json, frozen_values);
        assert_eq!(detail.items[1].workflow_version_id, "workflow-version-1");
        assert_eq!(detail.items[1].recipe_id, "recipe-1");
        assert_eq!(
            detail.items[1].retry_of_item_id.as_deref(),
            Some(source_id.as_str())
        );
        restarted_pool.close().await;
    }

    #[tokio::test]
    async fn cancel_pending_items_is_project_scoped_and_terminal() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("cancel-pending.db"))
            .await
            .unwrap();
        seed_task_dependencies(&pool).await;
        let repository = SqliteProductionQueueRepository::new(pool.clone());
        let now = Utc.with_ymd_and_hms(2026, 8, 8, 13, 30, 0).unwrap();
        let batch_id = ProductionBatchId::new();
        let item_id = ProductionBatchItemId::new();
        repository
            .insert(
                &ProductionBatch {
                    id: batch_id.clone(),
                    project_id: "project-1".to_owned(),
                    name: "Cancel pending test".to_owned(),
                    status: ProductionBatchStatus::Ready,
                    continue_on_failure: false,
                    archived_at: None,
                    created_at: now,
                    updated_at: now,
                },
                &[ProductionBatchItem {
                    id: item_id.clone(),
                    batch_id: batch_id.clone(),
                    ordinal: 0,
                    workflow_version_id: "workflow-version-1".to_owned(),
                    recipe_id: "recipe-1".to_owned(),
                    values_json: json!({"prompt": {"type": "string", "value": "cancel me"}}),
                    status: ProductionBatchItemStatus::Pending,
                    task_id: None,
                    retry_of_item_id: None,
                    error_code: None,
                    error_message: None,
                    created_at: now,
                    updated_at: now,
                }],
            )
            .await
            .unwrap();

        assert_eq!(
            repository
                .cancel_pending_items("project-2", &batch_id, now)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            repository
                .cancel_pending_items("project-1", &batch_id, now)
                .await
                .unwrap(),
            1
        );
        let detail = repository
            .find_detail("project-1", &batch_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(detail.items[0].status, ProductionBatchItemStatus::Cancelled);
        assert!(repository
            .list_non_terminal_items()
            .await
            .unwrap()
            .is_empty());
        pool.close().await;
    }

    #[tokio::test]
    async fn cancel_pending_items_and_complete_commits_item_and_batch_together() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("cancel-pending-atomic.db"))
            .await
            .unwrap();
        seed_task_dependencies(&pool).await;
        let repository = SqliteProductionQueueRepository::new(pool.clone());
        let now = Utc.with_ymd_and_hms(2026, 8, 8, 13, 45, 0).unwrap();
        let batch_id = ProductionBatchId::new();
        repository
            .insert(
                &ProductionBatch {
                    id: batch_id.clone(),
                    project_id: "project-1".to_owned(),
                    name: "Atomic cancellation test".to_owned(),
                    status: ProductionBatchStatus::Ready,
                    continue_on_failure: true,
                    archived_at: None,
                    created_at: now,
                    updated_at: now,
                },
                &[ProductionBatchItem {
                    id: ProductionBatchItemId::new(),
                    batch_id: batch_id.clone(),
                    ordinal: 0,
                    workflow_version_id: "workflow-version-1".to_owned(),
                    recipe_id: "recipe-1".to_owned(),
                    values_json: json!({"prompt": {"type": "string", "value": "cancel atomically"}}),
                    status: ProductionBatchItemStatus::Pending,
                    task_id: None,
                    retry_of_item_id: None,
                    error_code: None,
                    error_message: None,
                    created_at: now,
                    updated_at: now,
                }],
            )
            .await
            .unwrap();

        assert_eq!(
            repository
                .cancel_pending_items_and_complete("project-1", &batch_id, now)
                .await
                .unwrap(),
            1
        );
        let detail = repository
            .find_detail("project-1", &batch_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(detail.batch.status, ProductionBatchStatus::Completed);
        assert_eq!(detail.items[0].status, ProductionBatchItemStatus::Cancelled);
        pool.close().await;
    }

    #[tokio::test]
    async fn global_running_and_active_queries_are_stable_across_projects() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("global-admission.db"))
            .await
            .unwrap();
        seed_task_dependencies(&pool).await;
        sqlx::query(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at)
             VALUES ('project-2', 'Project 2', 'C:/project-2', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let repository = SqliteProductionQueueRepository::new(pool.clone());
        let now = Utc.with_ymd_and_hms(2026, 8, 8, 14, 0, 0).unwrap();
        let first_id = ProductionBatchId::parse("pbt_a_running".to_owned()).unwrap();
        let second_id = ProductionBatchId::parse("pbt_b_running".to_owned()).unwrap();

        for (batch_id, project_id, name) in [
            (second_id.clone(), "project-2", "Second"),
            (first_id.clone(), "project-1", "First"),
        ] {
            let item_id = ProductionBatchItemId::new();
            repository
                .insert(
                    &ProductionBatch {
                        id: batch_id.clone(),
                        project_id: project_id.to_owned(),
                        name: name.to_owned(),
                        status: ProductionBatchStatus::Running,
                        continue_on_failure: false,
                        archived_at: None,
                        created_at: now,
                        updated_at: now,
                    },
                    &[ProductionBatchItem {
                        id: item_id.clone(),
                        batch_id,
                        ordinal: 0,
                        workflow_version_id: "workflow-version-1".to_owned(),
                        recipe_id: "recipe-1".to_owned(),
                        values_json: json!({}),
                        status: ProductionBatchItemStatus::Pending,
                        task_id: None,
                        retry_of_item_id: None,
                        error_code: None,
                        error_message: None,
                        created_at: now,
                        updated_at: now,
                    }],
                )
                .await
                .unwrap();
            if project_id == "project-2" {
                assert!(repository
                    .set_item_dispatching(&item_id, now)
                    .await
                    .unwrap());
            }
        }

        let running = repository.list_running().await.unwrap();
        assert_eq!(
            running
                .iter()
                .map(|batch| batch.id.as_str())
                .collect::<Vec<_>>(),
            vec![first_id.as_str(), second_id.as_str()]
        );
        let active = repository.list_active_items().await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].batch.project_id, "project-2");
        assert_eq!(
            active[0].item.status,
            ProductionBatchItemStatus::Dispatching
        );
        pool.close().await;
    }

    #[tokio::test]
    async fn shot_batch_insert_is_atomic_and_concurrent_task_binding_has_one_winner() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("shot-batch.db"))
            .await
            .unwrap();
        seed_task_dependencies(&pool).await;
        sqlx::query(
            "INSERT INTO shots (id, project_id, ordinal, name, prompt_text, created_at, updated_at)
             VALUES ('sht_batch', 'project-1', 0, '批量镜头', '一只猫', ?, ?)",
        )
        .bind("2026-08-08T14:00:00Z")
        .bind("2026-08-08T14:00:00Z")
        .execute(&pool)
        .await
        .unwrap();
        for task_id in ["tsk_batch_a", "tsk_batch_b"] {
            sqlx::query(
                "INSERT INTO tasks (id, project_id, workflow_id, workflow_version_id, recipe_id, status, progress_mode, created_at)
                 VALUES (?, 'project-1', 'workflow-1', 'workflow-version-1', 'recipe-1', 'CREATED', 'indeterminate', '2026-08-08T14:00:00Z')",
            )
            .bind(task_id)
            .execute(&pool)
            .await
            .unwrap();
        }
        let repository = SqliteProductionQueueRepository::new(pool.clone());
        let now = Utc.with_ymd_and_hms(2026, 8, 8, 14, 0, 0).unwrap();
        let batch_id = ProductionBatchId::new();
        let item_id = ProductionBatchItemId::new();
        let batch = ProductionBatch {
            id: batch_id.clone(),
            project_id: "project-1".to_owned(),
            name: "Shot batch".to_owned(),
            status: ProductionBatchStatus::Ready,
            continue_on_failure: true,
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
            values_json: json!({"prompt": {"type": "string", "value": "一只猫"}}),
            status: ProductionBatchItemStatus::Pending,
            task_id: None,
            retry_of_item_id: None,
            error_code: None,
            error_message: None,
            created_at: now,
            updated_at: now,
        };
        repository
            .insert_batch_with_bindings(
                &batch,
                std::slice::from_ref(&item),
                &[ShotBatchBinding {
                    shot_id: "sht_batch".to_owned(),
                    stage: ShotStage::Image,
                    production_batch_item_id: item_id.as_str().to_owned(),
                }],
            )
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM shot_generation_links WHERE production_batch_item_id = ?"
            )
            .bind(item_id.as_str())
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
        assert!(repository
            .set_item_dispatching(&item_id, now)
            .await
            .unwrap());
        let left = repository.clone();
        let right = repository.clone();
        let (first, second) = tokio::join!(
            left.bind_shot_item_task(item_id.as_str(), "tsk_batch_a", now),
            right.bind_shot_item_task(item_id.as_str(), "tsk_batch_b", now),
        );
        let wins = [first, second]
            .into_iter()
            .filter(|result| matches!(result, Ok(true)))
            .count();
        assert_eq!(wins, 1, "the item must have exactly one task binding");
        let stored: (String, String) = sqlx::query_as(
            "SELECT i.task_id, l.task_id
             FROM production_batch_items i
             JOIN shot_generation_links l ON l.production_batch_item_id = i.id
             WHERE i.id = ?",
        )
        .bind(item_id.as_str())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(stored.0, stored.1);
        pool.close().await;
    }

    #[tokio::test]
    async fn shot_batch_insert_rejects_invalid_binding_without_writing_batch() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("shot-batch-atomic.db"))
            .await
            .unwrap();
        seed_task_dependencies(&pool).await;
        let repository = SqliteProductionQueueRepository::new(pool.clone());
        let now = Utc.with_ymd_and_hms(2026, 8, 8, 15, 0, 0).unwrap();
        let batch_id = ProductionBatchId::new();
        let item_id = ProductionBatchItemId::new();
        let batch = ProductionBatch {
            id: batch_id.clone(),
            project_id: "project-1".to_owned(),
            name: "Atomic Shot batch".to_owned(),
            status: ProductionBatchStatus::Ready,
            continue_on_failure: false,
            archived_at: None,
            created_at: now,
            updated_at: now,
        };
        let item = ProductionBatchItem {
            id: item_id.clone(),
            batch_id,
            ordinal: 0,
            workflow_version_id: "workflow-version-1".to_owned(),
            recipe_id: "recipe-1".to_owned(),
            values_json: json!({}),
            status: ProductionBatchItemStatus::Pending,
            task_id: None,
            retry_of_item_id: None,
            error_code: None,
            error_message: None,
            created_at: now,
            updated_at: now,
        };
        assert!(repository
            .insert_batch_with_bindings(
                &batch,
                std::slice::from_ref(&item),
                &[ShotBatchBinding {
                    shot_id: "sht_missing".to_owned(),
                    stage: ShotStage::Video,
                    production_batch_item_id: item_id.as_str().to_owned(),
                }],
            )
            .await
            .is_err());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_batches WHERE id = ?")
                .bind(batch.id.as_str())
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
        pool.close().await;
    }

    #[tokio::test]
    async fn prepared_batch_bulk_validates_multiple_shot_memberships() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("prepared-bulk-membership.db"))
            .await
            .unwrap();
        seed_task_dependencies(&pool).await;
        let shot_ids = ["sht_prepared_1", "sht_prepared_2", "sht_prepared_3"];
        insert_test_shots(&pool, "project-1", &shot_ids).await;

        let repository = SqliteProductionQueueRepository::new(pool.clone());
        let now = Utc.with_ymd_and_hms(2026, 8, 18, 12, 0, 0).unwrap();
        let batch_id = ProductionBatchId::new();
        let batch = fixture_batch(&batch_id, ProductionBatchStatus::Ready, now);
        let items = (0..shot_ids.len())
            .map(|ordinal| {
                fixture_item(
                    &batch_id,
                    ordinal as u32,
                    ProductionBatchItemStatus::Pending,
                    None,
                    json!({"ordinal": ordinal}),
                )
            })
            .collect::<Vec<_>>();
        let bindings = items
            .iter()
            .zip(shot_ids)
            .map(|(item, shot_id)| ShotBatchBinding {
                shot_id: shot_id.to_owned(),
                stage: ShotStage::Image,
                production_batch_item_id: item.id.as_str().to_owned(),
            })
            .collect::<Vec<_>>();
        let snapshots = items
            .iter()
            .zip(shot_ids)
            .enumerate()
            .map(|(ordinal, (item, shot_id))| {
                fixture_preparation_snapshot(
                    &format!("pps_prepared_{ordinal}"),
                    &batch_id,
                    item,
                    shot_id,
                    ShotStage::Image,
                    now,
                )
            })
            .collect::<Vec<_>>();

        repository
            .insert_prepared_batch_with_bindings(&batch, &items, &bindings, &snapshots)
            .await
            .unwrap();

        assert_eq!(
            count_by_batch(&pool, "production_batches", &batch_id).await,
            1
        );
        assert_eq!(
            count_by_batch(&pool, "production_batch_items", &batch_id).await,
            3
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM shot_generation_links WHERE production_batch_item_id IN (
                    SELECT id FROM production_batch_items WHERE batch_id = ?
                )",
            )
            .bind(batch_id.as_str())
            .fetch_one(&pool)
            .await
            .unwrap(),
            3
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM production_preparation_snapshots WHERE production_batch_id = ?",
            )
            .bind(batch_id.as_str())
            .fetch_one(&pool)
            .await
            .unwrap(),
            3
        );
        pool.close().await;
    }

    #[tokio::test]
    async fn prepared_batch_bulk_rejects_cross_project_shot_membership() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("prepared-bulk-project-scope.db"))
            .await
            .unwrap();
        seed_task_dependencies(&pool).await;
        sqlx::query(
            "INSERT INTO projects (id, name, description, root_path, created_at, updated_at)
             VALUES ('project-2', 'Other Project', NULL, 'C:/other-project', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        insert_test_shots(&pool, "project-1", &["sht_bulk_scope_1"]).await;
        insert_test_shots(&pool, "project-2", &["sht_bulk_scope_2"]).await;

        let repository = SqliteProductionQueueRepository::new(pool.clone());
        let now = Utc.with_ymd_and_hms(2026, 8, 18, 12, 10, 0).unwrap();
        let batch_id = ProductionBatchId::new();
        let batch = fixture_batch(&batch_id, ProductionBatchStatus::Ready, now);
        let shot_ids = ["sht_bulk_scope_1", "sht_bulk_scope_2"];
        let items = (0..shot_ids.len())
            .map(|ordinal| {
                fixture_item(
                    &batch_id,
                    ordinal as u32,
                    ProductionBatchItemStatus::Pending,
                    None,
                    json!({"ordinal": ordinal}),
                )
            })
            .collect::<Vec<_>>();
        let bindings = items
            .iter()
            .zip(shot_ids)
            .map(|(item, shot_id)| ShotBatchBinding {
                shot_id: shot_id.to_owned(),
                stage: ShotStage::Image,
                production_batch_item_id: item.id.as_str().to_owned(),
            })
            .collect::<Vec<_>>();
        let snapshots = items
            .iter()
            .zip(shot_ids)
            .enumerate()
            .map(|(ordinal, (item, shot_id))| {
                fixture_preparation_snapshot(
                    &format!("pps_bulk_scope_{ordinal}"),
                    &batch_id,
                    item,
                    shot_id,
                    ShotStage::Image,
                    now,
                )
            })
            .collect::<Vec<_>>();

        let error = repository
            .insert_prepared_batch_with_bindings(&batch, &items, &bindings, &snapshots)
            .await
            .unwrap_err();
        assert!(matches!(error, RepositoryError::Integrity { .. }));
        assert_eq!(
            count_by_batch(&pool, "production_batches", &batch_id).await,
            0
        );
        assert_eq!(
            count_by_batch(&pool, "production_batch_items", &batch_id).await,
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM production_preparation_snapshots WHERE production_batch_id = ?",
            )
            .bind(batch_id.as_str())
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
        pool.close().await;
    }

    #[tokio::test]
    async fn prepared_batch_bulk_rejects_existing_shot_links_before_insert() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("prepared-bulk-existing-link.db"))
            .await
            .unwrap();
        seed_task_dependencies(&pool).await;
        let shot_ids = ["sht_existing_1", "sht_existing_2", "sht_existing_3"];
        insert_test_shots(&pool, "project-1", &shot_ids).await;

        let repository = SqliteProductionQueueRepository::new(pool.clone());
        let now = Utc.with_ymd_and_hms(2026, 8, 18, 12, 15, 0).unwrap();
        let source_batch_id = ProductionBatchId::new();
        let source_item = fixture_item(
            &source_batch_id,
            0,
            ProductionBatchItemStatus::Pending,
            None,
            json!({"source": true}),
        );
        repository
            .insert_batch_with_bindings(
                &fixture_batch(&source_batch_id, ProductionBatchStatus::Ready, now),
                std::slice::from_ref(&source_item),
                &[ShotBatchBinding {
                    shot_id: shot_ids[0].to_owned(),
                    stage: ShotStage::Image,
                    production_batch_item_id: source_item.id.as_str().to_owned(),
                }],
            )
            .await
            .unwrap();

        let batch_id = ProductionBatchId::new();
        let batch = fixture_batch(&batch_id, ProductionBatchStatus::Ready, now);
        let mut items = (0..shot_ids.len())
            .map(|ordinal| {
                fixture_item(
                    &batch_id,
                    ordinal as u32,
                    ProductionBatchItemStatus::Pending,
                    None,
                    json!({"ordinal": ordinal}),
                )
            })
            .collect::<Vec<_>>();
        items[0].id = source_item.id.clone();
        let bindings = items
            .iter()
            .zip(shot_ids)
            .map(|(item, shot_id)| ShotBatchBinding {
                shot_id: shot_id.to_owned(),
                stage: ShotStage::Image,
                production_batch_item_id: item.id.as_str().to_owned(),
            })
            .collect::<Vec<_>>();
        let snapshots = items
            .iter()
            .zip(shot_ids)
            .enumerate()
            .map(|(ordinal, (item, shot_id))| {
                fixture_preparation_snapshot(
                    &format!("pps_existing_{ordinal}"),
                    &batch_id,
                    item,
                    shot_id,
                    ShotStage::Image,
                    now,
                )
            })
            .collect::<Vec<_>>();

        let error = repository
            .insert_prepared_batch_with_bindings(&batch, &items, &bindings, &snapshots)
            .await
            .unwrap_err();
        assert!(matches!(error, RepositoryError::Integrity { .. }));
        assert_eq!(
            count_by_batch(&pool, "production_batches", &batch_id).await,
            0
        );
        assert_eq!(
            count_by_batch(&pool, "production_batch_items", &batch_id).await,
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM shot_generation_links WHERE production_batch_item_id = ?",
            )
            .bind(source_item.id.as_str())
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM production_preparation_snapshots WHERE production_batch_id = ?",
            )
            .bind(batch_id.as_str())
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
        pool.close().await;
    }

    #[tokio::test]
    async fn prepared_batch_rolls_back_after_late_snapshot_failure() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("prepared-rollback.db"))
            .await
            .unwrap();
        seed_task_dependencies(&pool).await;
        let shot_ids = ["sht_rollback_1", "sht_rollback_2"];
        insert_test_shots(&pool, "project-1", &shot_ids).await;

        let repository = SqliteProductionQueueRepository::new(pool.clone());
        let now = Utc.with_ymd_and_hms(2026, 8, 18, 12, 30, 0).unwrap();
        let batch_id = ProductionBatchId::new();
        let batch = fixture_batch(&batch_id, ProductionBatchStatus::Ready, now);
        let items = (0..shot_ids.len())
            .map(|ordinal| {
                fixture_item(
                    &batch_id,
                    ordinal as u32,
                    ProductionBatchItemStatus::Pending,
                    None,
                    json!({"ordinal": ordinal}),
                )
            })
            .collect::<Vec<_>>();
        let bindings = items
            .iter()
            .zip(shot_ids)
            .map(|(item, shot_id)| ShotBatchBinding {
                shot_id: shot_id.to_owned(),
                stage: ShotStage::Image,
                production_batch_item_id: item.id.as_str().to_owned(),
            })
            .collect::<Vec<_>>();
        let snapshots = items
            .iter()
            .zip(shot_ids)
            .map(|(item, shot_id)| {
                fixture_preparation_snapshot(
                    "pps_duplicate",
                    &batch_id,
                    item,
                    shot_id,
                    ShotStage::Image,
                    now,
                )
            })
            .collect::<Vec<_>>();

        assert!(repository
            .insert_prepared_batch_with_bindings(&batch, &items, &bindings, &snapshots)
            .await
            .is_err());
        assert_eq!(
            count_by_batch(&pool, "production_batches", &batch_id).await,
            0
        );
        assert_eq!(
            count_by_batch(&pool, "production_batch_items", &batch_id).await,
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM shot_generation_links WHERE production_batch_item_id IN (
                    SELECT id FROM production_batch_items WHERE batch_id = ?
                )",
            )
            .bind(batch_id.as_str())
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM production_preparation_snapshots WHERE production_batch_id = ?",
            )
            .bind(batch_id.as_str())
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
        pool.close().await;
    }

    #[tokio::test]
    async fn bulk_requeue_copies_frozen_source_and_shot_binding() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("bulk-requeue-binding.db"))
            .await
            .unwrap();
        seed_task_dependencies(&pool).await;
        sqlx::query(
            "INSERT INTO shots (id, project_id, ordinal, name, prompt_text, created_at, updated_at)
             VALUES ('sht_bulk_retry', 'project-1', 0, '批量恢复', 'REF2VA', ?, ?)",
        )
        .bind("2026-08-17T01:00:00Z")
        .bind("2026-08-17T01:00:00Z")
        .execute(&pool)
        .await
        .unwrap();

        let repository = SqliteProductionQueueRepository::new(pool.clone());
        let now = Utc.with_ymd_and_hms(2026, 8, 17, 1, 0, 0).unwrap();
        let batch_id = ProductionBatchId::new();
        let source_id = ProductionBatchItemId::new();
        let frozen_values = json!({
            "prompt": {"type": "string", "value": "冻结 prompt"},
            "seed": {"type": "seed", "value": 42},
            "reference_images": {"type": "image_assets", "assetIds": ["B", "A", "C"]}
        });
        let source = ProductionBatchItem {
            id: source_id.clone(),
            batch_id: batch_id.clone(),
            ordinal: 0,
            workflow_version_id: "workflow-version-1".to_owned(),
            recipe_id: "recipe-1".to_owned(),
            values_json: frozen_values.clone(),
            status: ProductionBatchItemStatus::Failed,
            task_id: None,
            retry_of_item_id: None,
            error_code: Some("COMFY_TIMEOUT".to_owned()),
            error_message: Some("transient".to_owned()),
            created_at: now,
            updated_at: now,
        };
        repository
            .insert_batch_with_bindings(
                &fixture_batch(&batch_id, ProductionBatchStatus::Paused, now),
                std::slice::from_ref(&source),
                &[ShotBatchBinding {
                    shot_id: "sht_bulk_retry".to_owned(),
                    stage: ShotStage::Video,
                    production_batch_item_id: source_id.as_str().to_owned(),
                }],
            )
            .await
            .unwrap();

        let mut retry = fixture_item(
            &batch_id,
            1,
            ProductionBatchItemStatus::Pending,
            Some(source_id.as_str()),
            json!({"prompt": {"type": "string", "value": "should be ignored"}}),
        );
        retry.workflow_version_id = "wrong-workflow".to_owned();
        retry.recipe_id = "wrong-recipe".to_owned();
        let result = repository
            .append_requeue_items_with_bindings(std::slice::from_ref(&retry), now)
            .await
            .unwrap();

        assert_eq!(result.0, vec![retry.id.as_str().to_owned()]);
        assert!(result.1.is_empty());
        let detail = repository
            .find_detail("project-1", &batch_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(detail.batch.status, ProductionBatchStatus::Paused);
        assert_eq!(detail.items.len(), 2);
        assert_eq!(detail.items[1].workflow_version_id, "workflow-version-1");
        assert_eq!(detail.items[1].recipe_id, "recipe-1");
        assert_eq!(detail.items[1].values_json, frozen_values);
        assert_eq!(
            detail.items[1].retry_of_item_id.as_deref(),
            Some(source_id.as_str())
        );
        let binding: (String, String, Option<String>) = sqlx::query_as(
            "SELECT shot_id, stage, task_id FROM shot_generation_links
             WHERE production_batch_item_id = ?",
        )
        .bind(retry.id.as_str())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(binding.0, "sht_bulk_retry");
        assert_eq!(binding.1, "video");
        assert_eq!(binding.2, None);
        pool.close().await;
    }

    #[tokio::test]
    async fn bulk_requeue_rolls_back_items_and_bindings_when_a_later_source_fails() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("bulk-requeue-rollback.db"))
            .await
            .unwrap();
        seed_task_dependencies(&pool).await;
        let repository = SqliteProductionQueueRepository::new(pool.clone());
        let now = Utc.with_ymd_and_hms(2026, 8, 17, 1, 15, 0).unwrap();
        let batch_id = ProductionBatchId::new();
        let source_id = ProductionBatchItemId::new();
        let mut source = fixture_item(
            &batch_id,
            0,
            ProductionBatchItemStatus::Failed,
            None,
            json!({"prompt": "source"}),
        );
        source.id = source_id.clone();
        repository
            .insert(
                &fixture_batch(&batch_id, ProductionBatchStatus::Ready, now),
                &[source],
            )
            .await
            .unwrap();

        let first = fixture_item(
            &batch_id,
            1,
            ProductionBatchItemStatus::Pending,
            Some(source_id.as_str()),
            json!({"prompt": "candidate"}),
        );
        let second = fixture_item(
            &batch_id,
            2,
            ProductionBatchItemStatus::Pending,
            Some("pbi_missing_source"),
            json!({"prompt": "invalid"}),
        );
        assert!(repository
            .append_requeue_items_with_bindings(&[first, second], now)
            .await
            .is_err());

        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM production_batch_items WHERE batch_id = ?",
            )
            .bind(batch_id.as_str())
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM shot_generation_links WHERE production_batch_item_id IN (
                    SELECT id FROM production_batch_items WHERE batch_id = ?
                )",
            )
            .bind(batch_id.as_str())
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
        let detail = repository
            .find_detail("project-1", &batch_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(detail.batch.status, ProductionBatchStatus::Ready);
        pool.close().await;
    }

    #[tokio::test]
    async fn bulk_requeue_is_idempotent_and_retries_from_the_current_leaf() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("bulk-requeue-idempotent.db"))
            .await
            .unwrap();
        seed_task_dependencies(&pool).await;
        let repository = SqliteProductionQueueRepository::new(pool.clone());
        let now = Utc.with_ymd_and_hms(2026, 8, 17, 1, 30, 0).unwrap();
        let batch_id = ProductionBatchId::new();
        let source_id = ProductionBatchItemId::new();
        repository
            .insert(
                &fixture_batch(&batch_id, ProductionBatchStatus::Ready, now),
                &[ProductionBatchItem {
                    id: source_id.clone(),
                    batch_id: batch_id.clone(),
                    ordinal: 0,
                    workflow_version_id: "workflow-version-1".to_owned(),
                    recipe_id: "recipe-1".to_owned(),
                    values_json: json!({"prompt": "frozen"}),
                    status: ProductionBatchItemStatus::Failed,
                    task_id: None,
                    retry_of_item_id: None,
                    error_code: Some("COMFY_TIMEOUT".to_owned()),
                    error_message: Some("timeout".to_owned()),
                    created_at: now,
                    updated_at: now,
                }],
            )
            .await
            .unwrap();

        let first = fixture_item(
            &batch_id,
            1,
            ProductionBatchItemStatus::Pending,
            Some(source_id.as_str()),
            json!({"prompt": "ignored"}),
        );
        let first_result = repository
            .append_requeue_items_with_bindings(std::slice::from_ref(&first), now)
            .await
            .unwrap();
        assert_eq!(first_result.0, vec![first.id.as_str().to_owned()]);

        let duplicate = fixture_item(
            &batch_id,
            2,
            ProductionBatchItemStatus::Pending,
            Some(source_id.as_str()),
            json!({"prompt": "duplicate"}),
        );
        let duplicate_result = repository
            .append_requeue_items_with_bindings(std::slice::from_ref(&duplicate), now)
            .await
            .unwrap();
        assert!(duplicate_result.0.is_empty());
        assert_eq!(duplicate_result.1, vec![first.id.as_str().to_owned()]);

        let second = fixture_item(
            &batch_id,
            2,
            ProductionBatchItemStatus::Pending,
            Some(first.id.as_str()),
            json!({"prompt": "second round"}),
        );
        let second_result = repository
            .append_requeue_items_with_bindings(std::slice::from_ref(&second), now)
            .await
            .unwrap();
        assert_eq!(second_result.0, vec![second.id.as_str().to_owned()]);
        let detail = repository
            .find_detail("project-1", &batch_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(detail.items.len(), 3);
        assert_eq!(
            detail.items[2].retry_of_item_id.as_deref(),
            Some(first.id.as_str())
        );
        pool.close().await;
    }

    #[tokio::test]
    async fn initial_batch_limit_counts_logical_roots_not_retry_attempts() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("bulk-requeue-boundary.db"))
            .await
            .unwrap();
        seed_task_dependencies(&pool).await;
        let repository = SqliteProductionQueueRepository::new(pool.clone());
        let now = Utc.with_ymd_and_hms(2026, 8, 17, 1, 45, 0).unwrap();
        let batch_id = ProductionBatchId::new();
        let roots = (0..100)
            .map(|ordinal| {
                fixture_item(
                    &batch_id,
                    ordinal,
                    ProductionBatchItemStatus::Failed,
                    None,
                    json!({"ordinal": ordinal}),
                )
            })
            .collect::<Vec<_>>();
        repository
            .insert(
                &fixture_batch(&batch_id, ProductionBatchStatus::Ready, now),
                &roots,
            )
            .await
            .unwrap();
        let retry = fixture_item(
            &batch_id,
            100,
            ProductionBatchItemStatus::Pending,
            Some(roots[0].id.as_str()),
            json!({"retry": true}),
        );
        repository
            .append_requeue_items_with_bindings(std::slice::from_ref(&retry), now)
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM production_batch_items WHERE batch_id = ?",
            )
            .bind(batch_id.as_str())
            .fetch_one(&pool)
            .await
            .unwrap(),
            101
        );

        let overflow_batch_id = ProductionBatchId::new();
        let overflow_items = (0..101)
            .map(|ordinal| {
                fixture_item(
                    &overflow_batch_id,
                    ordinal,
                    ProductionBatchItemStatus::Failed,
                    None,
                    json!({"ordinal": ordinal}),
                )
            })
            .collect::<Vec<_>>();
        let error = repository
            .insert(
                &fixture_batch(&overflow_batch_id, ProductionBatchStatus::Ready, now),
                &overflow_items,
            )
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            RepositoryError::Integrity { message } if message.contains("100")
        ));
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_batches WHERE id = ?")
                .bind(overflow_batch_id.as_str())
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
        pool.close().await;
    }

    fn fixture_batch(
        id: &ProductionBatchId,
        status: ProductionBatchStatus,
        now: DateTime<Utc>,
    ) -> ProductionBatch {
        ProductionBatch {
            id: id.clone(),
            project_id: "project-1".to_owned(),
            name: "DEV-031 fixture".to_owned(),
            status,
            continue_on_failure: true,
            archived_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    async fn insert_test_shots(pool: &SqlitePool, project_id: &str, shot_ids: &[&str]) {
        for (ordinal, shot_id) in shot_ids.iter().enumerate() {
            sqlx::query(
                "INSERT INTO shots (id, project_id, ordinal, name, prompt_text, created_at, updated_at)
                 VALUES (?, ?, ?, ?, 'prompt', '2026-08-18T12:00:00Z', '2026-08-18T12:00:00Z')",
            )
            .bind(shot_id)
            .bind(project_id)
            .bind(ordinal as i64)
            .bind(format!("Shot {ordinal}"))
            .execute(pool)
            .await
            .unwrap();
        }
    }

    async fn count_by_batch(pool: &SqlitePool, table: &str, batch_id: &ProductionBatchId) -> i64 {
        let query = format!(
            "SELECT COUNT(*) FROM {table} WHERE {} = ?",
            if table == "production_batches" {
                "id"
            } else {
                "batch_id"
            }
        );
        sqlx::query_scalar::<_, i64>(&query)
            .bind(batch_id.as_str())
            .fetch_one(pool)
            .await
            .unwrap()
    }

    fn fixture_preparation_snapshot(
        snapshot_id: &str,
        batch_id: &ProductionBatchId,
        item: &ProductionBatchItem,
        shot_id: &str,
        stage: ShotStage,
        now: DateTime<Utc>,
    ) -> PreparationSnapshotRecord {
        let context_hash = format!("context-{shot_id}");
        PreparationSnapshotRecord {
            id: snapshot_id.to_owned(),
            project_id: "project-1".to_owned(),
            shot_id: shot_id.to_owned(),
            stage,
            context_hash: context_hash.clone(),
            production_batch_id: batch_id.as_str().to_owned(),
            production_batch_item_id: item.id.as_str().to_owned(),
            snapshot: PreparationSnapshotV1 {
                schema_version: 1,
                project_id: "project-1".to_owned(),
                shot_id: shot_id.to_owned(),
                stage: stage.as_str().to_owned(),
                context_hash,
                resolved_at: now,
                prepared_at: now,
                structure: Default::default(),
                profiles: Default::default(),
                reference_sets: Vec::new(),
                reference_assets: Vec::new(),
                prompt: PreparationSnapshotPrompt {
                    rendered_text: String::new(),
                    negative_prompt: String::new(),
                    ordered_segments: Vec::new(),
                },
                workflow: PreparationSnapshotWorkflow {
                    workflow_version_id: Some("workflow-version-1".to_owned()),
                    recipe_id: Some("recipe-1".to_owned()),
                    scalar_values: json!({}),
                },
                output_spec: Default::default(),
                stage_input: Default::default(),
                frozen_generation_values: json!({}),
                readiness: PreparationSnapshotReadiness {
                    status: crate::domain::ShotReadinessStatus::Ready,
                    score: 100,
                    gates: Vec::new(),
                    evaluated_at: now,
                },
                comfy_capability_evidence: Default::default(),
            },
            created_at: now,
        }
    }

    fn fixture_item(
        batch_id: &ProductionBatchId,
        ordinal: u32,
        status: ProductionBatchItemStatus,
        retry_of_item_id: Option<&str>,
        values_json: serde_json::Value,
    ) -> ProductionBatchItem {
        ProductionBatchItem {
            id: ProductionBatchItemId::new(),
            batch_id: batch_id.clone(),
            ordinal,
            workflow_version_id: "workflow-version-1".to_owned(),
            recipe_id: "recipe-1".to_owned(),
            values_json,
            status,
            task_id: None,
            retry_of_item_id: retry_of_item_id.map(str::to_owned),
            error_code: None,
            error_message: None,
            created_at: Utc.with_ymd_and_hms(2026, 8, 17, 1, 0, 0).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2026, 8, 17, 1, 0, 0).unwrap(),
        }
    }
}
