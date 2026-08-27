use super::RepositoryError;
use crate::domain::{
    PreparationSnapshotRecord, PreparedShotBatchRecord, ProductionBatch, ProductionBatchItem,
    ShotStage,
};
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

/// The small, batch-scoped Shot projection needed by review productivity.
/// Keeping selected asset ids here avoids loading each Shot while building a
/// review board.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductionBatchShotLink {
    pub production_batch_item_id: String,
    pub shot_id: String,
    pub stage: ShotStage,
    pub selected_image_asset_id: Option<String>,
    pub selected_video_asset_id: Option<String>,
}

#[async_trait]
pub trait ShotBatchRepository: Send + Sync {
    async fn insert_batch_with_bindings(
        &self,
        batch: &ProductionBatch,
        items: &[ProductionBatchItem],
        bindings: &[ShotBatchBinding],
    ) -> Result<(), RepositoryError>;

    /// Atomically inserts a preparation batch, its Shot links, and the
    /// immutable preparation snapshots that prove the values were generated
    /// from the same live context/readiness pass.
    ///
    /// The default keeps small in-memory repositories source-compatible while
    /// failing closed if a caller attempts to persist snapshots without a
    /// transaction-capable implementation.
    async fn insert_prepared_batch_with_bindings(
        &self,
        batch: &ProductionBatch,
        items: &[ProductionBatchItem],
        bindings: &[ShotBatchBinding],
        snapshots: &[PreparationSnapshotRecord],
    ) -> Result<(), RepositoryError> {
        let _ = (batch, items, bindings, snapshots);
        Err(RepositoryError::database(
            "prepared Shot batch persistence is not supported by this repository",
        ))
    }

    /// Returns non-terminal preparation records for a project/stage/Shot set.
    /// Implementations must keep the query project-scoped and set-based.
    async fn list_prepared_shot_records(
        &self,
        _project_id: &str,
        _stage: ShotStage,
        _shot_ids: &[String],
    ) -> Result<Vec<PreparedShotBatchRecord>, RepositoryError> {
        Ok(Vec::new())
    }

    /// Reads one immutable preparation snapshot by its queue item identity.
    async fn find_preparation_snapshot(
        &self,
        _project_id: &str,
        _production_batch_item_id: &str,
    ) -> Result<Option<PreparationSnapshotRecord>, RepositoryError> {
        Ok(None)
    }

    /// Loads all preparation snapshots belonging to a batch in one query.
    async fn list_preparation_snapshots_for_batch(
        &self,
        _project_id: &str,
        _production_batch_id: &str,
    ) -> Result<Vec<PreparationSnapshotRecord>, RepositoryError> {
        Ok(Vec::new())
    }

    /// Loads the item → Shot/stage relationship and selected Shot assets in a
    /// single batch-scoped query.
    async fn list_shot_links_for_batch(
        &self,
        _project_id: &str,
        _production_batch_id: &str,
    ) -> Result<Vec<ProductionBatchShotLink>, RepositoryError> {
        Ok(Vec::new())
    }

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
