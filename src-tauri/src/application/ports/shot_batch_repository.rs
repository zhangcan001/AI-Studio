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

    async fn has_active_shot_binding(
        &self,
        project_id: &str,
        shot_id: &str,
    ) -> Result<bool, RepositoryError>;
}
