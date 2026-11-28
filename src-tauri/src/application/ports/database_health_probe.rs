use crate::error::AppError;
use async_trait::async_trait;

#[async_trait]
pub trait DatabaseHealthProbe: Send + Sync {
    async fn is_healthy(&self) -> bool;

    async fn checkpoint_wal(&self) -> Result<(), AppError> {
        Ok(())
    }
}
