use crate::domain::{
    ProductionBatch, ProductionBatchDetail, ProductionBatchId, ProductionBatchItem,
    ProductionBatchItemId, ProductionBatchItemStatus, ProductionBatchStatus,
    ProductionPackageBatchBinding, ProductionPackageProvenance,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::RepositoryError;

#[derive(Clone, Debug, PartialEq)]
pub struct ActiveProductionItem {
    pub batch: ProductionBatch,
    pub item: ProductionBatchItem,
}

#[async_trait]
pub trait ProductionQueueRepository: Send + Sync {
    async fn insert(
        &self,
        batch: &ProductionBatch,
        items: &[ProductionBatchItem],
    ) -> Result<(), RepositoryError>;

    /// Inserts a package-backed batch and its durable source binding as one
    /// unit. Lightweight repositories can retain the legacy insert behavior;
    /// SQLite overrides this with a real transaction.
    async fn insert_with_provenance(
        &self,
        batch: &ProductionBatch,
        items: &[ProductionBatchItem],
        provenance: &ProductionPackageProvenance,
    ) -> Result<(), RepositoryError> {
        let _ = provenance;
        self.insert(batch, items).await
    }

    /// Lists package bindings belonging to one project. Implementations must
    /// keep the project predicate in the repository query.
    async fn list_package_bindings(
        &self,
        _project_id: &str,
    ) -> Result<Vec<ProductionPackageBatchBinding>, RepositoryError> {
        Ok(Vec::new())
    }

    async fn list(&self, project_id: &str) -> Result<Vec<ProductionBatch>, RepositoryError>;

    async fn list_running(&self) -> Result<Vec<ProductionBatch>, RepositoryError>;

    async fn list_active_items(&self) -> Result<Vec<ActiveProductionItem>, RepositoryError>;

    async fn list_non_terminal_items(&self) -> Result<Vec<ActiveProductionItem>, RepositoryError> {
        self.list_active_items().await
    }

    async fn find_detail(
        &self,
        project_id: &str,
        batch_id: &ProductionBatchId,
    ) -> Result<Option<ProductionBatchDetail>, RepositoryError>;

    async fn set_batch_status(
        &self,
        project_id: &str,
        batch_id: &ProductionBatchId,
        status: ProductionBatchStatus,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError>;

    async fn set_item_dispatching(
        &self,
        item_id: &ProductionBatchItemId,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError>;

    async fn cancel_pending_items(
        &self,
        project_id: &str,
        batch_id: &ProductionBatchId,
        updated_at: DateTime<Utc>,
    ) -> Result<u64, RepositoryError>;

    /// Cancels pending items and completes their batch in one repository transaction when
    /// supported by the concrete repository. The default keeps lightweight test repositories
    /// source-compatible; SQLite overrides it with a real transaction.
    async fn cancel_pending_items_and_complete(
        &self,
        project_id: &str,
        batch_id: &ProductionBatchId,
        updated_at: DateTime<Utc>,
    ) -> Result<u64, RepositoryError> {
        let cancelled = self
            .cancel_pending_items(project_id, batch_id, updated_at)
            .await?;
        if cancelled > 0 {
            self.set_batch_status(
                project_id,
                batch_id,
                ProductionBatchStatus::Completed,
                updated_at,
            )
            .await?;
        }
        Ok(cancelled)
    }

    async fn link_item_task(
        &self,
        item_id: &ProductionBatchItemId,
        task_id: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError>;

    async fn finish_item(
        &self,
        item_id: &ProductionBatchItemId,
        status: ProductionBatchItemStatus,
        error_code: Option<&str>,
        error_message: Option<&str>,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError>;

    async fn set_item_skipped(
        &self,
        item_id: &ProductionBatchItemId,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError>;

    async fn append_requeue_item(
        &self,
        item: &ProductionBatchItem,
        updated_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError>;

    async fn set_archived_at(
        &self,
        project_id: &str,
        batch_id: &ProductionBatchId,
        archived_at: Option<DateTime<Utc>>,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError>;

    async fn delete_batch(
        &self,
        project_id: &str,
        batch_id: &ProductionBatchId,
    ) -> Result<bool, RepositoryError>;

    async fn recover_uncertain_dispatches(
        &self,
        updated_at: DateTime<Utc>,
    ) -> Result<Vec<ProductionBatchId>, RepositoryError>;
}
