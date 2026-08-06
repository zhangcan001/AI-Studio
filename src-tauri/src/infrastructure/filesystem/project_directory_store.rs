use crate::application::ports::{ProjectDirectoryStore, ProjectDirectoryStoreError};
use async_trait::async_trait;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct FileSystemProjectDirectoryStore {
    projects_root: PathBuf,
}

impl FileSystemProjectDirectoryStore {
    pub fn new(projects_root: PathBuf) -> Self {
        Self { projects_root }
    }

    fn validate_project_id(project_id: &str) -> Result<(), ProjectDirectoryStoreError> {
        if project_id.trim().is_empty()
            || project_id.contains('/')
            || project_id.contains('\\')
            || project_id.contains(':')
            || project_id.contains("..")
        {
            return Err(ProjectDirectoryStoreError::InvalidProjectId(
                project_id.to_owned(),
            ));
        }
        Ok(())
    }

    fn project_path(&self, project_id: &str) -> Result<PathBuf, ProjectDirectoryStoreError> {
        Self::validate_project_id(project_id)?;
        let root = self.projects_root.join(project_id);
        if !root.starts_with(&self.projects_root) {
            return Err(ProjectDirectoryStoreError::InvalidProjectId(
                project_id.to_owned(),
            ));
        }
        Ok(root)
    }
}

#[async_trait]
impl ProjectDirectoryStore for FileSystemProjectDirectoryStore {
    async fn create_project_root(
        &self,
        project_id: &str,
    ) -> Result<PathBuf, ProjectDirectoryStoreError> {
        let path = self.project_path(project_id)?;
        tokio::fs::create_dir(&path)
            .await
            .map_err(|error| ProjectDirectoryStoreError::Create {
                path: path.clone(),
                message: error.to_string(),
            })?;
        Ok(path)
    }

    async fn remove_new_project_root(
        &self,
        project_id: &str,
    ) -> Result<(), ProjectDirectoryStoreError> {
        let path = self.project_path(project_id)?;
        tokio::fs::remove_dir(&path)
            .await
            .map_err(|error| ProjectDirectoryStoreError::Remove {
                path,
                message: error.to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::FileSystemProjectDirectoryStore;
    use crate::application::ports::ProjectDirectoryStore;
    use tempfile::tempdir;

    #[tokio::test]
    async fn creates_and_removes_only_the_requested_empty_project_root() {
        let directory = tempdir().unwrap();
        let projects_root = directory.path().join("projects");
        tokio::fs::create_dir_all(&projects_root).await.unwrap();
        tokio::fs::create_dir(projects_root.join("prj_existing"))
            .await
            .unwrap();
        let store = FileSystemProjectDirectoryStore::new(projects_root.clone());

        let created = store.create_project_root("prj_new").await.unwrap();
        assert_eq!(created, projects_root.join("prj_new"));
        store.remove_new_project_root("prj_new").await.unwrap();
        assert!(!projects_root.join("prj_new").exists());
        assert!(projects_root.join("prj_existing").exists());
    }

    #[tokio::test]
    async fn rejects_path_like_project_ids() {
        let directory = tempdir().unwrap();
        let store = FileSystemProjectDirectoryStore::new(directory.path().to_owned());
        for project_id in ["", "prj/a", "prj\\a", "prj:a", "prj..a"] {
            assert!(store.create_project_root(project_id).await.is_err());
        }
    }
}
