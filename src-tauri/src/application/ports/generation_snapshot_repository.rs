use super::RepositoryError;
use crate::domain::{GenerationSnapshot, TaskId};
use async_trait::async_trait;

#[async_trait]
pub trait GenerationSnapshotRepository: Send + Sync {
    async fn insert(&self, snapshot: &GenerationSnapshot) -> Result<(), RepositoryError>;

    async fn find_by_task_id(
        &self,
        task_id: &TaskId,
    ) -> Result<Option<GenerationSnapshot>, RepositoryError>;
}
