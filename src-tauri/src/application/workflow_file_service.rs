//! Selected Workflow file use cases extracted verbatim from the transport adapters.
//! Dialog cancellation stays in commands; limits and operation-specific errors stay here.
use crate::{
    application::{
        ports::WorkflowFileStore, workflow_lifecycle_service::MAX_WORKFLOW_ARCHIVE_BYTES,
        workflow_onboarding_service::MAX_WORKFLOW_IMPORT_BYTES,
    },
    error::AppError,
};
use std::{path::Path, sync::Arc};
pub struct WorkflowFileService {
    store: Arc<dyn WorkflowFileStore>,
}
impl WorkflowFileService {
    pub fn new(store: Arc<dyn WorkflowFileStore>) -> Self {
        Self { store }
    }
    pub async fn read_json(&self, path: &Path) -> Result<(Vec<u8>, String), AppError> {
        let is_json = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("json"));
        if !is_json {
            return Err(AppError::workflow_onboarding(
                "WORKFLOW_FILE_TYPE: select a .json ComfyUI workflow",
            ));
        }
        let original_filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|n| !n.trim().is_empty())
            .unwrap_or("workflow.json")
            .to_owned();
        let file_size =
            self.store.size(path).await.map_err(|_| {
                AppError::filesystem("selected workflow file could not be inspected")
            })?;
        if file_size > MAX_WORKFLOW_IMPORT_BYTES {
            return Err(AppError::workflow_onboarding(format!("WORKFLOW_FILE_TOO_LARGE: workflow import is {file_size} bytes; maximum is {MAX_WORKFLOW_IMPORT_BYTES} bytes")));
        }
        let bytes = self
            .store
            .read(path)
            .await
            .map_err(|_| AppError::filesystem("selected workflow file could not be read"))?;
        Ok((bytes, original_filename))
    }
    pub async fn read_archive(&self, path: &Path) -> Result<Vec<u8>, AppError> {
        let is_zip = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("zip"));
        if !is_zip {
            return Err(AppError::workflow_onboarding(
                "PACKAGE_ARCHIVE_INVALID: select a .zip AI Studio workflow package",
            ));
        }
        let size =
            self.store.size(path).await.map_err(|_| {
                AppError::filesystem("workflow package source could not be inspected")
            })?;
        if size > MAX_WORKFLOW_ARCHIVE_BYTES as u64 {
            return Err(AppError::workflow_onboarding(
                "PACKAGE_ARCHIVE_TOO_LARGE: archive exceeds the 64 MiB compressed limit",
            ));
        }
        self.store.read(path).await.map_err(|e| {
            AppError::filesystem(format!("workflow package source could not be read: {e}"))
        })
    }
    pub async fn write_archive(&self, path: &Path, bytes: &[u8]) -> Result<(), AppError> {
        self.store
            .write(path, bytes)
            .await
            .map_err(|e| AppError::filesystem(format!("workflow package export failed: {e}")))
    }
}

#[cfg(test)]
mod phase8_tests {
    use super::*;
    use crate::infrastructure::filesystem::FileSystemWorkflowFileStore;
    use tempfile::tempdir;
    fn service() -> WorkflowFileService {
        WorkflowFileService::new(Arc::new(FileSystemWorkflowFileStore))
    }
    #[tokio::test]
    async fn phase8_target1_json_policy_uses_real_selected_file() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("Selected.JSON");
        let bytes = br#"{"1":{"class_type":"SaveImage"}}"#;
        tokio::fs::write(&p, bytes).await.unwrap();
        let (actual, name) = service().read_json(&p).await.unwrap();
        assert_eq!(actual, bytes);
        assert_eq!(name, "Selected.JSON");
        let e = service()
            .read_json(&p.with_extension("txt"))
            .await
            .unwrap_err();
        assert_eq!(
            e.message,
            "WORKFLOW_FILE_TYPE: select a .json ComfyUI workflow"
        );
        let large = dir.path().join("large.json");
        let file = std::fs::File::create(&large).unwrap();
        file.set_len(MAX_WORKFLOW_IMPORT_BYTES + 1).unwrap();
        let e = service().read_json(&large).await.unwrap_err();
        assert!(e.message.starts_with("WORKFLOW_FILE_TOO_LARGE:"));
        assert_eq!(
            serde_json::to_value(e).unwrap()["code"],
            "WORKFLOW_ONBOARDING_ERROR"
        );
    }
    #[tokio::test]
    async fn phase8_target2_archive_policy_uses_real_selected_file() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("backup.ZIP");
        let bytes = b"opaque zip bytes; validation stays in lifecycle";
        tokio::fs::write(&p, bytes).await.unwrap();
        assert_eq!(service().read_archive(&p).await.unwrap(), bytes);
        assert_eq!(
            service()
                .read_archive(&p.with_extension("json"))
                .await
                .unwrap_err()
                .message,
            "PACKAGE_ARCHIVE_INVALID: select a .zip AI Studio workflow package"
        );
        let large = dir.path().join("large.zip");
        std::fs::File::create(&large)
            .unwrap()
            .set_len(MAX_WORKFLOW_ARCHIVE_BYTES as u64 + 1)
            .unwrap();
        assert_eq!(
            service().read_archive(&large).await.unwrap_err().message,
            "PACKAGE_ARCHIVE_TOO_LARGE: archive exceeds the 64 MiB compressed limit"
        );
    }
    #[tokio::test]
    async fn phase8_target3_export_writes_exact_bytes_without_new_authority() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("export.zip");
        let bytes = b"exact immutable package bytes";
        service().write_archive(&p, bytes).await.unwrap();
        assert_eq!(tokio::fs::read(&p).await.unwrap(), bytes);
    }
    #[tokio::test]
    async fn phase8_target4_filesystem_errors_keep_transport_envelope() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("missing.json");
        let e = service().read_json(&missing).await.unwrap_err();
        assert_eq!(e.message, "selected workflow file could not be inspected");
        assert_eq!(
            serde_json::to_value(&e).unwrap()["code"],
            "FILESYSTEM_ERROR"
        );
        assert_eq!(
            service()
                .read_archive(&dir.path().join("missing.zip"))
                .await
                .unwrap_err()
                .message,
            "workflow package source could not be inspected"
        );
        let folder = dir.path().join("directory.json");
        tokio::fs::create_dir(&folder).await.unwrap();
        assert_eq!(
            service().read_json(&folder).await.unwrap_err().message,
            "selected workflow file could not be read"
        );
        assert!(service()
            .write_archive(dir.path(), b"not a file")
            .await
            .unwrap_err()
            .message
            .starts_with("workflow package export failed:"));
    }
}
