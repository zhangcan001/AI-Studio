use crate::application::ports::{Clock, ProjectRecord, ProjectRepository, RepositoryError};
use std::sync::Arc;
use std::{error::Error, fmt, path::Path};

pub const DEFAULT_PROJECT_ID: &str = "prj_default";
pub const DEFAULT_PROJECT_NAME: &str = "Default Project";

pub struct DefaultProjectBootstrap {
    project_repository: Arc<dyn ProjectRepository>,
    clock: Arc<dyn Clock>,
}

impl DefaultProjectBootstrap {
    pub fn new(project_repository: Arc<dyn ProjectRepository>, clock: Arc<dyn Clock>) -> Self {
        Self {
            project_repository,
            clock,
        }
    }

    pub async fn ensure_default_project(
        &self,
        projects_root: &Path,
    ) -> Result<ProjectRecord, DefaultProjectBootstrapError> {
        let expected_root = projects_root.join(DEFAULT_PROJECT_ID);
        let project = self
            .project_repository
            .ensure_default_project(
                DEFAULT_PROJECT_ID,
                DEFAULT_PROJECT_NAME,
                &expected_root,
                self.clock.now(),
            )
            .await
            .map_err(DefaultProjectBootstrapError::Repository)?;

        std::fs::create_dir_all(project.root_path.join("assets")).map_err(|error| {
            DefaultProjectBootstrapError::Filesystem {
                path: project.root_path.join("assets"),
                message: error.to_string(),
            }
        })?;
        Ok(project)
    }
}

#[derive(Debug)]
pub enum DefaultProjectBootstrapError {
    Repository(RepositoryError),
    Filesystem {
        path: std::path::PathBuf,
        message: String,
    },
}

impl fmt::Display for DefaultProjectBootstrapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Repository(error) => write!(formatter, "{error}"),
            Self::Filesystem { path, message } => {
                write!(formatter, "failed to create {}: {message}", path.display())
            }
        }
    }
}

impl Error for DefaultProjectBootstrapError {}
