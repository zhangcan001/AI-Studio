use crate::application::ports::{
    WorkflowPackageBytes, WorkflowPackageQuarantineResult, WorkflowPackageStore,
    WorkflowPackageStoreError, WorkflowPurgeOperationEntry, WorkflowPurgeOperationRecord,
};
use async_trait::async_trait;
use std::{
    fs::{self, OpenOptions},
    io::Write,
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

    fn quarantine_path(
        &self,
        operation_id: &str,
        package_name: &str,
    ) -> Result<PathBuf, WorkflowPackageStoreError> {
        validate_operation_id(operation_id)?;
        validate_package_name(package_name)?;
        Ok(self
            .library_root
            .join(".purge")
            .join(operation_id)
            .join(package_name))
    }

    fn quarantine_root(&self, operation_id: &str) -> Result<PathBuf, WorkflowPackageStoreError> {
        validate_operation_id(operation_id)?;
        Ok(self.library_root.join(".purge").join(operation_id))
    }

    fn operation_json_path(
        &self,
        operation_id: &str,
    ) -> Result<PathBuf, WorkflowPackageStoreError> {
        Ok(self.quarantine_root(operation_id)?.join("operation.json"))
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

    async fn prepare_purge_operation(
        &self,
        operation: &WorkflowPurgeOperationRecord,
    ) -> Result<(), WorkflowPackageStoreError> {
        validate_operation_record(operation)?;
        let serialized = serde_json::to_vec(operation)
            .map_err(|error| store_error(format!("serialize purge operation: {error}")))?;
        let purge_root = self.library_root.join(".purge");
        fs::create_dir_all(&purge_root).map_err(io_error)?;
        let operation_root = self.quarantine_root(&operation.operation_id)?;
        fs::create_dir(&operation_root).map_err(io_error)?;
        let result = (|| {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(self.operation_json_path(&operation.operation_id)?)
                .map_err(io_error)?;
            file.write_all(&serialized).map_err(io_error)?;
            file.sync_all().map_err(io_error)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(operation_root);
        }
        result
    }

    async fn list_purge_operations(
        &self,
    ) -> Result<Vec<WorkflowPurgeOperationEntry>, WorkflowPackageStoreError> {
        let purge_root = self.library_root.join(".purge");
        let entries = match fs::read_dir(&purge_root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(io_error(error)),
        };
        let mut operations = Vec::new();
        for entry in entries {
            let entry = entry.map_err(io_error)?;
            let operation_id = entry.file_name().to_string_lossy().to_string();
            let path = entry.path();
            if !path.is_dir() {
                operations.push(WorkflowPurgeOperationEntry::Malformed {
                    operation_id,
                    message: "purge entry is not a directory".to_owned(),
                });
                continue;
            }
            if let Err(error) = validate_operation_id(&operation_id) {
                operations.push(WorkflowPurgeOperationEntry::Malformed {
                    operation_id,
                    message: error.message,
                });
                continue;
            }
            match fs::read(self.operation_json_path(&operation_id)?) {
                Ok(bytes) => match serde_json::from_slice::<WorkflowPurgeOperationRecord>(&bytes) {
                    Ok(record) => match validate_operation_record(&record) {
                        Ok(()) if record.operation_id == operation_id => {
                            operations.push(WorkflowPurgeOperationEntry::Journal(record));
                        }
                        Ok(()) => operations.push(WorkflowPurgeOperationEntry::Malformed {
                            operation_id,
                            message: "purge journal operationId does not match directory"
                                .to_owned(),
                        }),
                        Err(error) => operations.push(WorkflowPurgeOperationEntry::Malformed {
                            operation_id,
                            message: error.message,
                        }),
                    },
                    Err(error) => operations.push(WorkflowPurgeOperationEntry::Malformed {
                        operation_id,
                        message: format!("invalid purge journal: {error}"),
                    }),
                },
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    operations.push(WorkflowPurgeOperationEntry::Legacy { operation_id });
                }
                Err(error) => operations.push(WorkflowPurgeOperationEntry::Malformed {
                    operation_id,
                    message: format!("read purge journal: {error}"),
                }),
            }
        }
        operations.sort_by(|left, right| left.operation_id().cmp(right.operation_id()));
        Ok(operations)
    }

    async fn list_quarantined_packages(
        &self,
        operation_id: &str,
    ) -> Result<Vec<String>, WorkflowPackageStoreError> {
        let root = self.quarantine_root(operation_id)?;
        let entries = match fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(io_error(error)),
        };
        let mut packages = Vec::new();
        for entry in entries {
            let entry = entry.map_err(io_error)?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "operation.json" {
                continue;
            }
            let metadata = fs::symlink_metadata(entry.path()).map_err(io_error)?;
            if !metadata.is_dir() {
                return Err(store_error(format!(
                    "unexpected purge entry for {operation_id}: {name}"
                )));
            }
            validate_package_name(&name)?;
            packages.push(name);
        }
        packages.sort();
        Ok(packages)
    }

    async fn read_quarantined(
        &self,
        operation_id: &str,
        package_name: &str,
    ) -> Result<WorkflowPackageBytes, WorkflowPackageStoreError> {
        Self::read_package(&self.quarantine_path(operation_id, package_name)?)
    }

    async fn quarantine_published(
        &self,
        operation_id: &str,
        package_name: &str,
    ) -> Result<WorkflowPackageQuarantineResult, WorkflowPackageStoreError> {
        let source = self.runtime_path(package_name)?;
        let target = self.quarantine_path(operation_id, package_name)?;
        match fs::metadata(&target) {
            Ok(_) => {
                return Err(store_error(
                    "workflow package quarantine target already exists",
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(error)),
        }
        match fs::metadata(&source) {
            Ok(metadata) if !metadata.is_dir() => {
                return Err(store_error("workflow package is unavailable"));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(WorkflowPackageQuarantineResult::AlreadyMissing);
            }
            Err(error) => return Err(io_error(error)),
        }
        fs::create_dir_all(self.quarantine_root(operation_id)?).map_err(io_error)?;
        match fs::rename(&source, &target) {
            Ok(()) => Ok(WorkflowPackageQuarantineResult::Quarantined),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                classify_rename_not_found(&source)
            }
            Err(error) => Err(io_error(error)),
        }
    }

    async fn restore_quarantined(
        &self,
        operation_id: &str,
        package_name: &str,
    ) -> Result<(), WorkflowPackageStoreError> {
        let source = self.quarantine_path(operation_id, package_name)?;
        let target = self.runtime_path(package_name)?;
        if !source.is_dir() {
            return Err(store_error("workflow package quarantine is unavailable"));
        }
        if target.exists() {
            return Err(store_error(
                "workflow package restore target already exists",
            ));
        }
        fs::create_dir_all(&self.library_root).map_err(io_error)?;
        fs::rename(source, target).map_err(io_error)
    }

    async fn remove_quarantine(&self, operation_id: &str) -> Result<(), WorkflowPackageStoreError> {
        let path = self.quarantine_root(operation_id)?;
        match fs::metadata(&path) {
            Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path).map_err(io_error)?,
            Ok(_) => return Err(store_error("workflow package quarantine is unavailable")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(error)),
        }
        Ok(())
    }

    async fn list_published(&self) -> Result<Vec<String>, WorkflowPackageStoreError> {
        let entries = match fs::read_dir(&self.library_root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(io_error(error)),
        };
        let mut result = Vec::new();
        for entry in entries {
            let entry = entry.map_err(io_error)?;
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.path().is_dir()
                && !name.starts_with('.')
                && validate_package_name(&name).is_ok()
            {
                result.push(name);
            }
        }
        result.sort();
        Ok(result)
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

fn validate_operation_id(value: &str) -> Result<(), WorkflowPackageStoreError> {
    if !value.starts_with("purge_")
        || value.is_empty()
        || value.len() > 160
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    {
        return Err(store_error("invalid workflow package operation identifier"));
    }
    Ok(())
}

fn validate_workflow_id(value: &str) -> Result<(), WorkflowPackageStoreError> {
    if value.trim().is_empty() || value.len() > 160 {
        return Err(store_error("invalid purge workflow identifier"));
    }
    Ok(())
}

fn validate_operation_record(
    operation: &WorkflowPurgeOperationRecord,
) -> Result<(), WorkflowPackageStoreError> {
    if operation.schema_version != 1 {
        return Err(store_error("unsupported purge journal schema version"));
    }
    validate_operation_id(&operation.operation_id)?;
    validate_workflow_id(&operation.workflow_id)?;
    if operation.created_at.trim().is_empty() {
        return Err(store_error("purge journal createdAt must not be empty"));
    }
    let mut package_names = std::collections::BTreeSet::new();
    for package_name in &operation.package_names {
        validate_package_name(package_name)?;
        if !package_names.insert(package_name) {
            return Err(store_error("purge journal packageNames must be unique"));
        }
    }
    Ok(())
}

fn classify_rename_not_found(
    source: &Path,
) -> Result<WorkflowPackageQuarantineResult, WorkflowPackageStoreError> {
    match fs::metadata(source) {
        Ok(metadata) if metadata.is_dir() => Err(store_error(
            "workflow package rename failed after NotFound; source still exists",
        )),
        Ok(_) => Err(store_error("workflow package is unavailable")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(WorkflowPackageQuarantineResult::AlreadyMissing)
        }
        Err(error) => Err(io_error(error)),
    }
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
    use super::{classify_rename_not_found, FileSystemWorkflowPackageStore};
    use crate::application::ports::{
        WorkflowPackageBytes, WorkflowPackageQuarantineResult, WorkflowPackageStore,
        WorkflowPurgeOperationEntry, WorkflowPurgeOperationRecord,
    };
    use std::fs;
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

    #[tokio::test]
    async fn purge_journal_is_persisted_before_any_package_move() {
        let directory = tempdir().unwrap();
        let library = directory.path().join("library");
        let staging = directory.path().join("staging");
        fs::create_dir_all(&library).unwrap();
        fs::create_dir_all(&staging).unwrap();
        let store = FileSystemWorkflowPackageStore::new(library.clone(), staging);
        let operation = WorkflowPurgeOperationRecord {
            schema_version: 1,
            operation_id: format!("purge_{}", Uuid::new_v4()),
            workflow_id: "wfl_journal_test".to_owned(),
            package_names: vec!["journal_package".to_owned()],
            created_at: "2026-09-05T00:00:00Z".to_owned(),
        };

        store.prepare_purge_operation(&operation).await.unwrap();

        assert!(library
            .join(".purge")
            .join(&operation.operation_id)
            .join("operation.json")
            .is_file());
        assert_eq!(
            store.list_purge_operations().await.unwrap(),
            vec![WorkflowPurgeOperationEntry::Journal(operation)]
        );
    }

    #[test]
    fn rename_not_found_rechecks_the_source() {
        let directory = tempdir().unwrap();
        let missing = directory.path().join("missing");
        assert_eq!(
            classify_rename_not_found(&missing).unwrap(),
            WorkflowPackageQuarantineResult::AlreadyMissing
        );

        let present = directory.path().join("present");
        fs::create_dir_all(&present).unwrap();
        assert!(classify_rename_not_found(&present).is_err());
    }
}
