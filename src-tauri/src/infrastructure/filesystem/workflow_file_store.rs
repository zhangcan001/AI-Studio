use crate::application::ports::WorkflowFileStore;
use async_trait::async_trait;
use std::path::Path;
use tokio::io::AsyncWriteExt;

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
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let temporary = parent.join(format!(".workflow-{}.partial", uuid::Uuid::new_v4()));
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .await
            .map_err(|error| error.to_string())?;
        let result = async {
            file.write_all(bytes).await?;
            file.sync_all().await
        }
        .await;
        drop(file);
        let result = result.and_then(|()| {
            crate::infrastructure::settings::replace_settings_file(&temporary, path)
        });
        if result.is_err() {
            let _ = tokio::fs::remove_file(&temporary).await;
        }
        result.map_err(|error| error.to_string())
    }
}
