use crate::application::ports::{
    Clock, ProjectDirectoryStore, ProjectDirectoryStoreError, ProjectRecord, ProjectRepository,
    RepositoryError,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::{error::Error, fmt, sync::Arc};
use uuid::Uuid;

pub const MAX_PROJECT_NAME_CHARS: usize = 80;
pub const MAX_PROJECT_DESCRIPTION_CHARS: usize = 500;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectView {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct ProjectService {
    project_repository: Arc<dyn ProjectRepository>,
    directory_store: Arc<dyn ProjectDirectoryStore>,
    clock: Arc<dyn Clock>,
}

impl ProjectService {
    pub fn new(
        project_repository: Arc<dyn ProjectRepository>,
        directory_store: Arc<dyn ProjectDirectoryStore>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            project_repository,
            directory_store,
            clock,
        }
    }

    pub async fn list(&self) -> Result<Vec<ProjectView>, ProjectServiceError> {
        Ok(self
            .project_repository
            .list()
            .await?
            .into_iter()
            .map(ProjectView::from)
            .collect())
    }

    pub async fn create(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<ProjectView, ProjectServiceError> {
        let (name, description) = normalize_metadata(name, description)?;
        let project_id = format!("prj_{}", Uuid::new_v4());
        let root_path = self
            .directory_store
            .create_project_root(&project_id)
            .await?;
        let created_at = self.clock.now();
        let project = ProjectRecord {
            id: project_id.clone(),
            name,
            description,
            root_path,
            created_at,
            updated_at: created_at,
        };

        if let Err(repository_error) = self.project_repository.insert(&project).await {
            if let Err(cleanup_error) = self
                .directory_store
                .remove_new_project_root(&project_id)
                .await
            {
                return Err(ProjectServiceError::Compensation {
                    repository: repository_error,
                    cleanup: cleanup_error,
                });
            }
            return Err(ProjectServiceError::Repository(repository_error));
        }

        Ok(ProjectView::from(project))
    }

    pub async fn update(
        &self,
        project_id: &str,
        name: &str,
        description: Option<&str>,
    ) -> Result<ProjectView, ProjectServiceError> {
        validate_project_id(project_id)?;
        let (name, description) = normalize_metadata(name, description)?;
        let project = self
            .project_repository
            .update_metadata(project_id, &name, description.as_deref(), self.clock.now())
            .await?
            .ok_or_else(|| ProjectServiceError::NotFound(project_id.to_owned()))?;
        Ok(ProjectView::from(project))
    }
}

fn normalize_metadata(
    name: &str,
    description: Option<&str>,
) -> Result<(String, Option<String>), ProjectServiceError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ProjectServiceError::InvalidName(
            "PROJECT_NAME_REQUIRED: project name must not be empty".to_owned(),
        ));
    }
    if name.chars().count() > MAX_PROJECT_NAME_CHARS {
        return Err(ProjectServiceError::InvalidName(format!(
            "PROJECT_NAME_TOO_LONG: project name must be at most {MAX_PROJECT_NAME_CHARS} characters"
        )));
    }

    let description = description.map(str::trim).filter(|value| !value.is_empty());
    if let Some(description) = description {
        if description.chars().count() > MAX_PROJECT_DESCRIPTION_CHARS {
            return Err(ProjectServiceError::InvalidDescription(format!(
                "PROJECT_DESCRIPTION_TOO_LONG: project description must be at most {MAX_PROJECT_DESCRIPTION_CHARS} characters"
            )));
        }
    }

    Ok((name.to_owned(), description.map(str::to_owned)))
}

pub fn validate_project_id(project_id: &str) -> Result<(), ProjectServiceError> {
    if project_id.trim().is_empty()
        || project_id.contains('/')
        || project_id.contains('\\')
        || project_id.contains(':')
        || project_id.contains("..")
    {
        return Err(ProjectServiceError::InvalidProjectId(format!(
            "INVALID_PROJECT_ID: project id {project_id:?} is not valid"
        )));
    }
    Ok(())
}

impl From<ProjectRecord> for ProjectView {
    fn from(project: ProjectRecord) -> Self {
        Self {
            id: project.id,
            name: project.name,
            description: project.description,
            created_at: project.created_at,
            updated_at: project.updated_at,
        }
    }
}

#[derive(Debug)]
pub enum ProjectServiceError {
    InvalidName(String),
    InvalidDescription(String),
    InvalidProjectId(String),
    NotFound(String),
    Repository(RepositoryError),
    Directory(ProjectDirectoryStoreError),
    Compensation {
        repository: RepositoryError,
        cleanup: ProjectDirectoryStoreError,
    },
}

impl fmt::Display for ProjectServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(message)
            | Self::InvalidDescription(message)
            | Self::InvalidProjectId(message) => formatter.write_str(message),
            Self::NotFound(project_id) => write!(
                formatter,
                "PROJECT_NOT_FOUND: project {project_id} was not found"
            ),
            Self::Repository(error) => write!(formatter, "{error}"),
            Self::Directory(error) => write!(formatter, "{error}"),
            Self::Compensation {
                repository,
                cleanup,
            } => write!(
                formatter,
                "project insert failed: {repository}; directory compensation failed: {cleanup}"
            ),
        }
    }
}

impl Error for ProjectServiceError {}

impl From<RepositoryError> for ProjectServiceError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

impl From<ProjectDirectoryStoreError> for ProjectServiceError {
    fn from(error: ProjectDirectoryStoreError) -> Self {
        Self::Directory(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_metadata, ProjectService, ProjectServiceError};
    use crate::application::ports::{
        Clock, ProjectDirectoryStore, ProjectDirectoryStoreError, ProjectRecord, ProjectRepository,
        RepositoryError,
    };
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use std::{path::PathBuf, sync::Arc};
    use tempfile::tempdir;

    #[derive(Clone, Copy)]
    struct FixedClock;

    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            Utc::now()
        }
    }

    #[derive(Clone)]
    struct FailingRepository;

    #[async_trait]
    impl ProjectRepository for FailingRepository {
        async fn list(&self) -> Result<Vec<ProjectRecord>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn find_by_id(
            &self,
            _project_id: &str,
        ) -> Result<Option<ProjectRecord>, RepositoryError> {
            Ok(None)
        }

        async fn insert(&self, _project: &ProjectRecord) -> Result<(), RepositoryError> {
            Err(RepositoryError::database("forced project insert failure"))
        }

        async fn update_metadata(
            &self,
            _project_id: &str,
            _name: &str,
            _description: Option<&str>,
            _updated_at: DateTime<Utc>,
        ) -> Result<Option<ProjectRecord>, RepositoryError> {
            Ok(None)
        }

        async fn get_storage_root(
            &self,
            _project_id: &str,
        ) -> Result<Option<PathBuf>, RepositoryError> {
            Ok(None)
        }

        async fn ensure_default_project(
            &self,
            _project_id: &str,
            _name: &str,
            _root_path: &PathBuf,
            _created_at: DateTime<Utc>,
        ) -> Result<ProjectRecord, RepositoryError> {
            unreachable!()
        }
    }

    #[derive(Clone)]
    struct RecordingDirectoryStore {
        root: PathBuf,
    }

    #[async_trait]
    impl ProjectDirectoryStore for RecordingDirectoryStore {
        async fn create_project_root(
            &self,
            project_id: &str,
        ) -> Result<PathBuf, ProjectDirectoryStoreError> {
            let path = self.root.join(project_id);
            tokio::fs::create_dir(&path).await.map_err(|error| {
                ProjectDirectoryStoreError::Create {
                    path: path.clone(),
                    message: error.to_string(),
                }
            })?;
            Ok(path)
        }

        async fn remove_new_project_root(
            &self,
            project_id: &str,
        ) -> Result<(), ProjectDirectoryStoreError> {
            tokio::fs::remove_dir(self.root.join(project_id))
                .await
                .map_err(|error| ProjectDirectoryStoreError::Remove {
                    path: self.root.join(project_id),
                    message: error.to_string(),
                })
        }
    }

    #[test]
    fn normalizes_and_validates_unicode_metadata() {
        let (name, description) = normalize_metadata("  项目  ", Some("  描述  ")).unwrap();
        assert_eq!(name, "项目");
        assert_eq!(description.as_deref(), Some("描述"));
        assert_eq!(
            normalize_metadata(" ", None)
                .unwrap_err()
                .to_string()
                .split(':')
                .next(),
            Some("PROJECT_NAME_REQUIRED")
        );
        assert!(matches!(
            normalize_metadata(&"名".repeat(81), None),
            Err(ProjectServiceError::InvalidName(_))
        ));
        assert!(matches!(
            normalize_metadata("name", Some(&"描".repeat(501))),
            Err(ProjectServiceError::InvalidDescription(_))
        ));
    }

    #[tokio::test]
    async fn removes_new_directory_when_database_insert_fails() {
        let directory = tempdir().unwrap();
        let service = ProjectService::new(
            Arc::new(FailingRepository),
            Arc::new(RecordingDirectoryStore {
                root: directory.path().to_owned(),
            }),
            Arc::new(FixedClock),
        );

        assert!(service.create("Transient", None).await.is_err());
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }
}
