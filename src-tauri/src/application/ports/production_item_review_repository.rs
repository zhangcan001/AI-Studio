use crate::domain::ProductionReviewStatus;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::RepositoryError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductionItemReviewRecord {
    pub id: String,
    pub project_id: String,
    pub production_batch_id: String,
    pub production_batch_item_id: String,
    pub task_id: Option<String>,
    pub result_asset_id: Option<String>,
    pub review_status: ProductionReviewStatus,
    pub review_note: String,
    pub version: i64,
    pub lineage_key: String,
    pub parent_batch_id: Option<String>,
    pub parent_item_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductionReviewInboxItem {
    pub project_id: String,
    pub batch_id: String,
    pub batch_name: String,
    pub batch_status: String,
    pub item_id: String,
    pub ordinal: i64,
    pub item_status: String,
    pub task_id: Option<String>,
    pub task_status: Option<String>,
    pub shot_id: Option<String>,
    pub stage: Option<String>,
    pub asset_id: Option<String>,
    pub asset_name: Option<String>,
    pub asset_type: Option<String>,
    pub asset_mime_type: Option<String>,
    pub selected_asset_id: Option<String>,
    pub review_status: ProductionReviewStatus,
    pub review_note: String,
    pub version: i64,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub prompt_summary: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProductionReviewInboxPage {
    pub items: Vec<ProductionReviewInboxItem>,
    pub total: usize,
    pub unreviewed_count: usize,
    pub regenerate_count: usize,
}

#[async_trait]
pub trait ProductionItemReviewRepository: Send + Sync {
    /// Reads a bounded project-level review projection. SQLite implements this
    /// as one set-based query; the default keeps small test repositories source
    /// compatible without creating a second review store.
    async fn list_project_inbox(
        &self,
        _project_id: &str,
        _limit: usize,
        _offset: usize,
    ) -> Result<ProductionReviewInboxPage, RepositoryError> {
        Ok(ProductionReviewInboxPage::default())
    }

    async fn list_for_batch(
        &self,
        project_id: &str,
        production_batch_id: &str,
    ) -> Result<Vec<ProductionItemReviewRecord>, RepositoryError>;

    async fn list_for_lineage(
        &self,
        project_id: &str,
        lineage_key: &str,
    ) -> Result<Vec<ProductionItemReviewRecord>, RepositoryError>;

    /// Loads all review lineage rows for a batch of lineage keys. SQLite
    /// overrides this with one project-scoped `IN` query.
    async fn list_for_lineages(
        &self,
        project_id: &str,
        lineage_keys: &[String],
    ) -> Result<Vec<ProductionItemReviewRecord>, RepositoryError> {
        let mut records = Vec::new();
        for lineage_key in lineage_keys {
            records.extend(self.list_for_lineage(project_id, lineage_key).await?);
        }
        Ok(records)
    }

    async fn find_for_item(
        &self,
        project_id: &str,
        production_batch_item_id: &str,
    ) -> Result<Option<ProductionItemReviewRecord>, RepositoryError>;

    /// Inserts the first review row or refreshes only the task/result references.
    /// Review status, note and version are immutable under this operation.
    async fn ensure_for_item(
        &self,
        record: &ProductionItemReviewRecord,
    ) -> Result<ProductionItemReviewRecord, RepositoryError>;

    /// Ensures the missing review rows for one batch in one repository call.
    /// The default preserves source compatibility for small test repositories;
    /// production SQLite uses a single transaction and batch readback.
    async fn ensure_for_items(
        &self,
        records: &[ProductionItemReviewRecord],
    ) -> Result<Vec<ProductionItemReviewRecord>, RepositoryError> {
        let mut ensured = Vec::with_capacity(records.len());
        for record in records {
            ensured.push(self.ensure_for_item(record).await?);
        }
        Ok(ensured)
    }

    async fn insert(&self, record: &ProductionItemReviewRecord) -> Result<(), RepositoryError>;

    async fn set_status(
        &self,
        project_id: &str,
        production_batch_item_id: &str,
        status: ProductionReviewStatus,
        updated_at: DateTime<Utc>,
    ) -> Result<ProductionItemReviewRecord, RepositoryError>;

    async fn set_note(
        &self,
        project_id: &str,
        production_batch_item_id: &str,
        note: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<ProductionItemReviewRecord, RepositoryError>;
}
