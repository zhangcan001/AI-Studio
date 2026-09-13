use super::RepositoryError;
use crate::domain::{Model, ModelId, ModelVersion, ModelVersionId};
use async_trait::async_trait;

#[async_trait]
pub trait ModelRepository: Send + Sync {
    async fn list_models(&self) -> Result<Vec<Model>, RepositoryError>;

    async fn find_model(&self, model_id: &ModelId) -> Result<Option<Model>, RepositoryError>;

    async fn create_model(&self, model: &Model) -> Result<(), RepositoryError>;

    async fn update_model(&self, model: &Model) -> Result<Option<Model>, RepositoryError>;

    async fn delete_model(&self, model_id: &ModelId) -> Result<bool, RepositoryError>;

    async fn list_versions(&self, model_id: &ModelId)
        -> Result<Vec<ModelVersion>, RepositoryError>;

    async fn current_version(
        &self,
        model_id: &ModelId,
    ) -> Result<Option<ModelVersion>, RepositoryError>;

    async fn find_version_by_id(
        &self,
        version_id: &ModelVersionId,
    ) -> Result<Option<ModelVersion>, RepositoryError>;

    async fn create_version(&self, version: &ModelVersion) -> Result<(), RepositoryError>;
}
