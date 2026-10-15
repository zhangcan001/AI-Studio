use crate::app_state::AppState;
use crate::domain::story_entity::{StoryEntity, StoryEntityKind, StoryEntityUpsert};
use crate::error::AppError;
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryEntityCreateRequest {
    pub project_id: String,
    pub kind: String,
    pub name: String,
    pub description: String,
    pub cover_asset_id: Option<String>,
    pub image_asset_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryEntityUpdateRequest {
    pub project_id: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub cover_asset_id: Option<String>,
    pub image_asset_ids: Vec<String>,
}

pub async fn product_story_entity_list(
    state: State<'_, AppState>,
    project_id: String,
    kind: Option<String>,
) -> Result<Vec<StoryEntity>, AppError> {
    let kind = match kind.as_deref() {
        None | Some("") => None,
        Some(value) => Some(StoryEntityKind::parse(value).map_err(AppError::invalid_input)?),
    };
    state.shots.story_entities.list(&project_id, kind).await
}

pub async fn product_story_entity_get(
    state: State<'_, AppState>,
    project_id: String,
    id: String,
) -> Result<StoryEntity, AppError> {
    state.shots.story_entities.get(&project_id, &id).await
}

pub async fn product_story_entity_create(
    state: State<'_, AppState>,
    request: StoryEntityCreateRequest,
) -> Result<StoryEntity, AppError> {
    let kind = StoryEntityKind::parse(&request.kind).map_err(AppError::invalid_input)?;
    state
        .shots
        .story_entities
        .create(
            &request.project_id,
            kind,
            StoryEntityUpsert {
                name: request.name,
                description: request.description,
                cover_asset_id: request.cover_asset_id,
                image_asset_ids: request.image_asset_ids,
            },
        )
        .await
}

pub async fn product_story_entity_update(
    state: State<'_, AppState>,
    request: StoryEntityUpdateRequest,
) -> Result<StoryEntity, AppError> {
    state
        .shots
        .story_entities
        .update(
            &request.project_id,
            &request.id,
            StoryEntityUpsert {
                name: request.name,
                description: request.description,
                cover_asset_id: request.cover_asset_id,
                image_asset_ids: request.image_asset_ids,
            },
        )
        .await
}

pub async fn product_story_entity_delete(
    state: State<'_, AppState>,
    project_id: String,
    id: String,
) -> Result<(), AppError> {
    state.shots.story_entities.delete(&project_id, &id).await
}
