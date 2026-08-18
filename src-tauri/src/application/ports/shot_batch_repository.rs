use super::RepositoryError;
use crate::domain::{ProductionBatch, ProductionBatchItem, ShotStage};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// The durable relationship between one normal production queue item and one
/// Shot.  The relationship is created before a Task exists; the runner fills
/// in the Task id immediately before execution starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShotBatchBinding {
    pub shot_id: String,
    pub stage: ShotStage,
    pub production_batch_item_id: String,
}

/// A non-terminal Shot binding already owned by the production queue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveShotBatchBinding {
    pub shot_id: String,
    pub stage: ShotStage,
    pub production_batch_id: String,
    pub production_batch_item_id: String,
}

#[async_trait]
pub trait ShotBatchRepository: Send + Sync {
    async fn insert_batch_with_bindings(
        &self,
        batch: &ProductionBatch,
        items: &[ProductionBatchItem],
        bindings: &[ShotBatchBinding],
    ) -> Result<(), RepositoryError>;

    /// Atomically binds a newly-created Task to a Shot item. Returns `true`
    /// when the item has a Shot placeholder link, and `false` for a generic
    /// production item which the caller should link through the normal queue
    /// repository method.
    async fn bind_shot_item_task(
        &self,
        item_id: &str,
        task_id: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError>;

    /// Requeues a Shot-bound item while preserving the failed Task/link and
    /// creating a new placeholder link for the retry item.
    async fn append_requeue_item_with_binding(
        &self,
        item: &ProductionBatchItem,
        source_item_id: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError>;

    /// Appends retry attempts for one batch in one transaction. The returned
    /// tuple contains newly-created item ids followed by ids already prepared
    /// for an idempotent source retry.
    async fn append_requeue_items_with_bindings(
        &self,
        items: &[ProductionBatchItem],
        updated_at: DateTime<Utc>,
    ) -> Result<(Vec<String>, Vec<String>), RepositoryError>;

    async fn has_active_shot_binding(
        &self,
        project_id: &str,
        shot_id: &str,
    ) -> Result<bool, RepositoryError>;

    /// Returns active bindings for the requested shots in one set-based query.
    async fn list_active_shot_bindings(
        &self,
        project_id: &str,
        stage: ShotStage,
        shot_ids: &[String],
    ) -> Result<Vec<ActiveShotBatchBinding>, RepositoryError>;
}
