use async_trait::async_trait;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectDirectoryStoreError {
    InvalidProjectId(String),
    Create { path: PathBuf, message: String },
    Remove { path: PathBuf, message: String },
}

impl std::fmt::Display for ProjectDirectoryStoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidProjectId(message) => write!(formatter, "invalid project id: {message}"),
            Self::Create { path, message } => {
                write!(formatter, "failed to create {}: {message}", path.display())
            }
            Self::Remove { path, message } => {
                write!(formatter, "failed to remove {}: {message}", path.display())
            }
        }
    }
}

impl std::error::Error for ProjectDirectoryStoreError {}

#[async_trait]
pub trait ProjectDirectoryStore: Send + Sync {
    async fn create_project_root(
        &self,
        project_id: &str,
    ) -> Result<PathBuf, ProjectDirectoryStoreError>;

    async fn remove_new_project_root(
        &self,
        project_id: &str,
    ) -> Result<(), ProjectDirectoryStoreError>;
}
