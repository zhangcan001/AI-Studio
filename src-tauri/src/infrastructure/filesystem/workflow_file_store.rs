use crate::application::ports::WorkflowFileStore;
use async_trait::async_trait;
use std::path::Path;

pub struct FileSystemWorkflowFileStore;
#[async_trait]
impl WorkflowFileStore for FileSystemWorkflowFileStore {
    async fn size(&self, path: &Path) -> Result<u64, String> {
        tokio::fs::metadata(path)
            .await
            .map(|m| m.len())
            .map_err(|e| e.to_string())
    }
    async fn read(&self, path: &Path) -> Result<Vec<u8>, String> {
        tokio::fs::read(path).await.map_err(|e| e.to_string())
    }
    async fn write(&self, path: &Path, bytes: &[u8]) -> Result<(), String> {
        tokio::fs::write(path, bytes)
            .await
            .map_err(|e| e.to_string())
    }
}
