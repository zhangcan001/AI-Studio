use super::RepositoryError;
use async_trait::async_trait;
use std::path::PathBuf;

#[async_trait]
pub trait ProjectRepository: Send + Sync {
    async fn get_storage_root(&self, project_id: &str) -> Result<Option<PathBuf>, RepositoryError>;
}
