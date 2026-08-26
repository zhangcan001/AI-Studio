use super::RepositoryError;
use crate::domain::consistency::{ReferenceSet, ReferenceSetItem, ReferenceSetPurpose};
use async_trait::async_trait;

/// Persistence boundary for reusable, ordered reference sets.
///
/// The item replacement operation is an atomic persistence contract. A
/// concrete transaction is deliberately deferred to DEV-048.
#[async_trait]
pub trait ReferenceSetRepository: Send + Sync {
    async fn list_reference_sets(
        &self,
        project_id: &str,
        purpose: Option<ReferenceSetPurpose>,
    ) -> Result<Vec<ReferenceSet>, RepositoryError>;

    async fn find_reference_set(
        &self,
        project_id: &str,
        reference_set_id: &str,
    ) -> Result<Option<ReferenceSet>, RepositoryError>;

    async fn insert_reference_set(
        &self,
        reference_set: &ReferenceSet,
    ) -> Result<(), RepositoryError>;

    async fn update_reference_set(
        &self,
        reference_set: &ReferenceSet,
    ) -> Result<bool, RepositoryError>;

    async fn delete_reference_set(
        &self,
        project_id: &str,
        reference_set_id: &str,
    ) -> Result<bool, RepositoryError>;

    async fn list_items(
        &self,
        reference_set_id: &str,
    ) -> Result<Vec<ReferenceSetItem>, RepositoryError>;

    /// Atomically replaces all items belonging to one reference set.
    async fn replace_items(
        &self,
        reference_set_id: &str,
        items: &[ReferenceSetItem],
    ) -> Result<(), RepositoryError>;
}
