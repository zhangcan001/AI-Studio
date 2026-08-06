use crate::application::ports::{
    WorkflowPackageBytes, WorkflowPackageStore, WorkflowPackageStoreError,
};
use async_trait::async_trait;
use std::{
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct FileSystemWorkflowPackageStore {
    library_root: PathBuf,
    staging_root: PathBuf,
}

impl FileSystemWorkflowPackageStore {
    pub fn new(library_root: PathBuf, staging_root: PathBuf) -> Self {
        Self {
            library_root,
            staging_root,
        }
    }

    fn staging_path(&self, staging_id: &str) -> Result<PathBuf, WorkflowPackageStoreError> {
        validate_staging_id(staging_id)?;
        Ok(self.staging_root.join(staging_id))
    }

    fn runtime_path(&self, package_name: &str) -> Result<PathBuf, WorkflowPackageStoreError> {
        validate_package_name(package_name)?;
        Ok(self.library_root.join(package_name))
    }

    fn write_package(
        path: &Path,
        package: &WorkflowPackageBytes,
    ) -> Result<(), WorkflowPackageStoreError> {
        if path.exists() {
            return Err(store_error("staging package already exists"));
        }
        fs::create_dir_all(path).map_err(io_error)?;
        let result = (|| {
            fs::write(path.join("manifest.yaml"), &package.manifest_yaml).map_err(io_error)?;
            fs::write(path.join("recipe.yaml"), &package.recipe_yaml).map_err(io_error)?;
            fs::write(path.join("workflow_api.json"), &package.workflow_api_json)
                .map_err(io_error)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(path);
        }
        result
    }

    fn read_package(path: &Path) -> Result<WorkflowPackageBytes, WorkflowPackageStoreError> {
        if !path.is_dir() {
            return Err(store_error("workflow package is unavailable"));
        }
        Ok(WorkflowPackageBytes::new(
            fs::read(path.join("manifest.yaml")).map_err(io_error)?,
            fs::read(path.join("recipe.yaml")).map_err(io_error)?,
            fs::read(path.join("workflow_api.json")).map_err(io_error)?,
        ))
    }
}

#[async_trait]
impl WorkflowPackageStore for FileSystemWorkflowPackageStore {
    async fn stage(
        &self,
        staging_id: &str,
        package: &WorkflowPackageBytes,
    ) -> Result<(), WorkflowPackageStoreError> {
        Self::write_package(&self.staging_path(staging_id)?, package)
    }

    async fn read_staging(
        &self,
        staging_id: &str,
    ) -> Result<WorkflowPackageBytes, WorkflowPackageStoreError> {
        Self::read_package(&self.staging_path(staging_id)?)
    }

    async fn publish_atomic(
        &self,
        staging_id: &str,
        package_name: &str,
    ) -> Result<(), WorkflowPackageStoreError> {
        let staging = self.staging_path(staging_id)?;
        let runtime = self.runtime_path(package_name)?;
        if runtime.exists() {
            return Err(store_error("workflow package already exists"));
        }
        fs::rename(staging, runtime).map_err(io_error)
    }

    async fn remove_staging(&self, staging_id: &str) -> Result<(), WorkflowPackageStoreError> {
        let path = self.staging_path(staging_id)?;
        if path.exists() {
            fs::remove_dir_all(path).map_err(io_error)?;
        }
        Ok(())
    }

    async fn remove_published(&self, package_name: &str) -> Result<(), WorkflowPackageStoreError> {
        let path = self.runtime_path(package_name)?;
        if path.exists() {
            fs::remove_dir_all(path).map_err(io_error)?;
        }
        Ok(())
    }

    async fn read_runtime(
        &self,
        package_name: &str,
    ) -> Result<WorkflowPackageBytes, WorkflowPackageStoreError> {
        Self::read_package(&self.runtime_path(package_name)?)
    }

    async fn list_staging_ids(&self) -> Result<Vec<String>, WorkflowPackageStoreError> {
        let entries = fs::read_dir(&self.staging_root).map_err(io_error)?;
        let mut result = Vec::new();
        for entry in entries {
            let entry = entry.map_err(io_error)?;
            if entry.path().is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if validate_staging_id(&name).is_ok() {
                    result.push(name);
                }
            }
        }
        result.sort();
        Ok(result)
    }
}

fn validate_staging_id(value: &str) -> Result<(), WorkflowPackageStoreError> {
    let Some(uuid) = value.strip_prefix("onb_") else {
        return Err(store_error("invalid staging identifier"));
    };
    Uuid::parse_str(uuid).map_err(|_| store_error("invalid staging identifier"))?;
    Ok(())
}

fn validate_package_name(value: &str) -> Result<(), WorkflowPackageStoreError> {
    if value.is_empty()
        || value.len() > 160
        || !value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
    {
        return Err(store_error("invalid workflow package identifier"));
    }
    Ok(())
}

fn io_error(error: std::io::Error) -> WorkflowPackageStoreError {
    store_error(error.to_string())
}

fn store_error(message: impl Into<String>) -> WorkflowPackageStoreError {
    WorkflowPackageStoreError {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::FileSystemWorkflowPackageStore;
    use crate::application::ports::{WorkflowPackageBytes, WorkflowPackageStore};
    use tempfile::tempdir;
    use uuid::Uuid;

    #[tokio::test]
    async fn stages_reads_and_atomically_publishes_without_exposing_paths() {
        let directory = tempdir().unwrap();
        let store = FileSystemWorkflowPackageStore::new(
            directory.path().join("library"),
            directory.path().join("staging"),
        );
        std::fs::create_dir_all(directory.path().join("library")).unwrap();
        std::fs::create_dir_all(directory.path().join("staging")).unwrap();
        let staging_id = format!("onb_{}", Uuid::new_v4());
        let package =
            WorkflowPackageBytes::new(b"manifest".to_vec(), b"recipe".to_vec(), b"{}".to_vec());
        store.stage(&staging_id, &package).await.unwrap();
        assert_eq!(store.read_staging(&staging_id).await.unwrap(), package);
        store
            .publish_atomic(&staging_id, "sample_1_0_0_deadbeef")
            .await
            .unwrap();
        assert_eq!(
            store.read_runtime("sample_1_0_0_deadbeef").await.unwrap(),
            package
        );
        assert!(store.list_staging_ids().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn rejects_path_like_identifiers() {
        let directory = tempdir().unwrap();
        let store = FileSystemWorkflowPackageStore::new(
            directory.path().join("library"),
            directory.path().join("staging"),
        );
        assert!(store.read_runtime("../escape").await.is_err());
        assert!(store.read_staging("onb_../escape").await.is_err());
    }
}
