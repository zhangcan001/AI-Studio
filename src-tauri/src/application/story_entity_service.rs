use crate::application::ports::{RepositoryError, StoryEntityRepository};
use crate::domain::story_entity::{StoryEntity, StoryEntityKind, StoryEntityUpsert};
use crate::error::AppError;
use std::sync::Arc;

pub struct StoryEntityService {
    repo: Arc<dyn StoryEntityRepository>,
}

impl StoryEntityService {
    pub fn new(repo: Arc<dyn StoryEntityRepository>) -> Self {
        Self { repo }
    }

    pub async fn list(
        &self,
        project_id: &str,
        kind: Option<StoryEntityKind>,
    ) -> Result<Vec<StoryEntity>, AppError> {
        self.repo.list(project_id, kind).await.map_err(map_repo)
    }

    pub async fn get(&self, project_id: &str, id: &str) -> Result<StoryEntity, AppError> {
        match self.repo.get(project_id, id).await.map_err(map_repo)? {
            Some(entity) => Ok(entity),
            None => Err(AppError::invalid_input(format!(
                "story entity not found: {id}"
            ))),
        }
    }

    pub async fn create(
        &self,
        project_id: &str,
        kind: StoryEntityKind,
        input: StoryEntityUpsert,
    ) -> Result<StoryEntity, AppError> {
        self.repo
            .create(project_id, kind, input)
            .await
            .map_err(map_repo)
    }

    pub async fn update(
        &self,
        project_id: &str,
        id: &str,
        input: StoryEntityUpsert,
    ) -> Result<StoryEntity, AppError> {
        self.repo
            .update(project_id, id, input)
            .await
            .map_err(map_repo)
    }

    pub async fn delete(&self, project_id: &str, id: &str) -> Result<(), AppError> {
        self.repo.delete(project_id, id).await.map_err(map_repo)
    }

    pub async fn list_for_shot(
        &self,
        project_id: &str,
        shot_id: &str,
    ) -> Result<Vec<StoryEntity>, AppError> {
        self.repo
            .list_for_shot(project_id, shot_id)
            .await
            .map_err(map_repo)
    }

    pub async fn replace_shot_bindings(
        &self,
        project_id: &str,
        shot_id: &str,
        entity_ids: Vec<String>,
    ) -> Result<Vec<StoryEntity>, AppError> {
        self.repo
            .replace_shot_bindings(project_id, shot_id, &entity_ids)
            .await
            .map_err(map_repo)
    }
}

fn map_repo(error: RepositoryError) -> AppError {
    match error {
        RepositoryError::NotFound { entity, id } => {
            AppError::invalid_input(format!("{entity} not found: {id}"))
        }
        RepositoryError::Integrity { message } => AppError::invalid_input(message),
        RepositoryError::Database { message } => AppError::database(message),
        RepositoryError::Serialization { context, message } => {
            AppError::database(format!("{context}: {message}"))
        }
        other => AppError::database(format!("{other:?}")),
    }
}
