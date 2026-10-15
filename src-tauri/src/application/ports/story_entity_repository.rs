use crate::application::ports::RepositoryError;
use crate::domain::story_entity::{StoryEntity, StoryEntityKind, StoryEntityUpsert};
use async_trait::async_trait;

#[async_trait]
pub trait StoryEntityRepository: Send + Sync {
    async fn list(
        &self,
        project_id: &str,
        kind: Option<StoryEntityKind>,
    ) -> Result<Vec<StoryEntity>, RepositoryError>;
    async fn get(&self, project_id: &str, id: &str)
        -> Result<Option<StoryEntity>, RepositoryError>;
    async fn create(
        &self,
        project_id: &str,
        kind: StoryEntityKind,
        input: StoryEntityUpsert,
    ) -> Result<StoryEntity, RepositoryError>;
    async fn update(
        &self,
        project_id: &str,
        id: &str,
        input: StoryEntityUpsert,
    ) -> Result<StoryEntity, RepositoryError>;
    async fn delete(&self, project_id: &str, id: &str) -> Result<(), RepositoryError>;
}
