use super::RepositoryError;
use crate::domain::{Preset, PresetId};
use async_trait::async_trait;

#[async_trait]
pub trait PresetRepository: Send + Sync {
    async fn list(
        &self,
        project_id: &str,
        workflow_version_id: &str,
        recipe_id: &str,
    ) -> Result<Vec<Preset>, RepositoryError>;

    async fn find_by_id(
        &self,
        project_id: &str,
        preset_id: &PresetId,
    ) -> Result<Option<Preset>, RepositoryError>;

    async fn find_by_name(
        &self,
        project_id: &str,
        workflow_version_id: &str,
        recipe_id: &str,
        name: &str,
    ) -> Result<Option<Preset>, RepositoryError>;

    async fn insert(&self, preset: &Preset) -> Result<(), RepositoryError>;

    async fn update(&self, preset: &Preset) -> Result<Option<Preset>, RepositoryError>;

    async fn delete(&self, project_id: &str, preset_id: &PresetId)
        -> Result<bool, RepositoryError>;
}
