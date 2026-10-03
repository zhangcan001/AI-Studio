use async_trait::async_trait;
use std::path::Path;

/// Selected-file I/O only. Format/limit/error policy belongs to the application use case.
#[async_trait]
pub trait WorkflowFileStore: Send + Sync {
    async fn size(&self, path: &Path) -> Result<u64, String>;
    async fn read(&self, path: &Path) -> Result<Vec<u8>, String>;
    async fn write(&self, path: &Path, bytes: &[u8]) -> Result<(), String>;
}
