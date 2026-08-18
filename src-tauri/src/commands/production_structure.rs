use crate::{
    app_state::AppState,
    application::production_structure_service::{
        CreateEpisodeRequest, CreateSceneRequest, CreateSeriesRequest, ProductionEpisodeView,
        ProductionSceneView, ProductionSeriesView, ProductionStructureError,
        ProductionStructureTreeView, UpdateEpisodeRequest, UpdateSceneRequest, UpdateSeriesRequest,
    },
    error::AppError,
};
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionStructureNameRequest {
    pub project_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionSeriesUpdateRequest {
    pub project_id: String,
    pub series_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionEpisodeCreateRequest {
    pub project_id: String,
    pub series_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionEpisodeUpdateRequest {
    pub project_id: String,
    pub episode_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionSceneCreateRequest {
    pub project_id: String,
    pub episode_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionSceneUpdateRequest {
    pub project_id: String,
    pub scene_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionStructureReorderRequest {
    pub project_id: String,
    pub parent_id: Option<String>,
    pub ordered_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionSceneAssignShotsRequest {
    pub project_id: String,
    pub scene_id: String,
    pub shot_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionSceneUnassignShotsRequest {
    pub project_id: String,
    pub shot_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionSceneReorderShotsRequest {
    pub scene_id: String,
    pub ordered_shot_ids: Vec<String>,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_structure_tree(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<ProductionStructureTreeView, AppError> {
    state
        .production_structure_service
        .tree(&project_id)
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_series_create(
    state: State<'_, AppState>,
    request: ProductionStructureNameRequest,
) -> Result<ProductionSeriesView, AppError> {
    state
        .production_structure_service
        .create_series(CreateSeriesRequest {
            project_id: request.project_id,
            name: request.name,
            description: request.description,
        })
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_series_update(
    state: State<'_, AppState>,
    request: ProductionSeriesUpdateRequest,
) -> Result<ProductionSeriesView, AppError> {
    state
        .production_structure_service
        .update_series(UpdateSeriesRequest {
            project_id: request.project_id,
            series_id: request.series_id,
            name: request.name,
            description: request.description,
        })
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_series_delete(
    state: State<'_, AppState>,
    project_id: String,
    series_id: String,
) -> Result<(), AppError> {
    state
        .production_structure_service
        .delete_series(&project_id, &series_id)
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_series_reorder(
    state: State<'_, AppState>,
    request: ProductionStructureReorderRequest,
) -> Result<(), AppError> {
    state
        .production_structure_service
        .reorder_series(&request.project_id, &request.ordered_ids)
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_episode_create(
    state: State<'_, AppState>,
    request: ProductionEpisodeCreateRequest,
) -> Result<ProductionEpisodeView, AppError> {
    state
        .production_structure_service
        .create_episode(CreateEpisodeRequest {
            project_id: request.project_id,
            series_id: request.series_id,
            name: request.name,
            description: request.description,
        })
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_episode_update(
    state: State<'_, AppState>,
    request: ProductionEpisodeUpdateRequest,
) -> Result<ProductionEpisodeView, AppError> {
    state
        .production_structure_service
        .update_episode(UpdateEpisodeRequest {
            project_id: request.project_id,
            episode_id: request.episode_id,
            name: request.name,
            description: request.description,
        })
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_episode_delete(
    state: State<'_, AppState>,
    project_id: String,
    episode_id: String,
) -> Result<(), AppError> {
    state
        .production_structure_service
        .delete_episode(&project_id, &episode_id)
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_episode_reorder(
    state: State<'_, AppState>,
    request: ProductionStructureReorderRequest,
) -> Result<(), AppError> {
    let series_id = request
        .parent_id
        .ok_or_else(|| AppError::invalid_input("series id is required"))?;
    state
        .production_structure_service
        .reorder_episodes(&request.project_id, &series_id, &request.ordered_ids)
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_scene_create(
    state: State<'_, AppState>,
    request: ProductionSceneCreateRequest,
) -> Result<ProductionSceneView, AppError> {
    state
        .production_structure_service
        .create_scene(CreateSceneRequest {
            project_id: request.project_id,
            episode_id: request.episode_id,
            name: request.name,
            description: request.description,
        })
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_scene_update(
    state: State<'_, AppState>,
    request: ProductionSceneUpdateRequest,
) -> Result<ProductionSceneView, AppError> {
    state
        .production_structure_service
        .update_scene(UpdateSceneRequest {
            project_id: request.project_id,
            scene_id: request.scene_id,
            name: request.name,
            description: request.description,
        })
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_scene_delete(
    state: State<'_, AppState>,
    project_id: String,
    scene_id: String,
) -> Result<(), AppError> {
    state
        .production_structure_service
        .delete_scene(&project_id, &scene_id)
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_scene_reorder(
    state: State<'_, AppState>,
    request: ProductionStructureReorderRequest,
) -> Result<(), AppError> {
    let episode_id = request
        .parent_id
        .ok_or_else(|| AppError::invalid_input("episode id is required"))?;
    state
        .production_structure_service
        .reorder_scenes(&request.project_id, &episode_id, &request.ordered_ids)
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_scene_assign_shots(
    state: State<'_, AppState>,
    request: ProductionSceneAssignShotsRequest,
) -> Result<(), AppError> {
    state
        .production_structure_service
        .assign_shots(&request.project_id, &request.scene_id, &request.shot_ids)
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_scene_unassign_shots(
    state: State<'_, AppState>,
    request: ProductionSceneUnassignShotsRequest,
) -> Result<(), AppError> {
    state
        .production_structure_service
        .unassign_shots(&request.project_id, &request.shot_ids)
        .await
        .map_err(map_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_scene_reorder_shots(
    state: State<'_, AppState>,
    request: ProductionSceneReorderShotsRequest,
) -> Result<(), AppError> {
    state
        .production_structure_service
        .reorder_scene_shots(&request.scene_id, &request.ordered_shot_ids)
        .await
        .map_err(map_error)
}

fn map_error(error: ProductionStructureError) -> AppError {
    match error {
        ProductionStructureError::InvalidInput(message) => AppError::invalid_input(message),
        ProductionStructureError::NotFound(message) => AppError::database(message),
        ProductionStructureError::Repository(
            crate::application::ports::RepositoryError::Integrity { message },
        ) if message.contains("PRODUCTION_STRUCTURE_PROJECT_MISMATCH") => {
            AppError::invalid_input(message)
        }
        ProductionStructureError::Repository(repository) => {
            super::map_repository_error(&repository)
        }
    }
}
