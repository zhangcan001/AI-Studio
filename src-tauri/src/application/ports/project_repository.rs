use super::RepositoryError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub root_path: PathBuf,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[async_trait]
pub trait ProjectRepository: Send + Sync {
    async fn find_by_id(&self, project_id: &str) -> Result<Option<ProjectRecord>, RepositoryError>;

    async fn get_storage_root(&self, project_id: &str) -> Result<Option<PathBuf>, RepositoryError>;

    async fn ensure_default_project(
        &self,
        project_id: &str,
        name: &str,
        root_path: &PathBuf,
        created_at: DateTime<Utc>,
    ) -> Result<ProjectRecord, RepositoryError>;
}
