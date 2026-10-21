pub mod artifact;
pub mod asset;
pub mod batch_workflow_preset;
pub mod catalog;
pub mod comfy;
pub mod consistency_assets;
pub mod consistency_bindings;
pub mod diagnostics;
pub mod episode_production;
pub mod external_production_handoff;
pub mod generation;
pub mod h3_local_import;
pub mod model;
pub mod organization;
pub mod preflight;
pub mod preset;
pub mod production_audit;
pub mod production_batch_runbook;
pub mod production_orchestrator;
pub mod production_package;
pub mod production_package_discovery;
pub mod production_package_provenance;
pub mod production_preparation;
pub mod production_queue;
pub mod production_structure;
pub mod project;
pub mod project_command_center;
pub mod prompt_library;
pub mod provenance_lineage;
pub mod recipe_history;
pub mod reference_anchor;
pub mod repair_jobs;
pub mod scene_production;
pub mod series_production;
pub mod settings;
pub mod shot;
pub mod shot_batch;
pub mod shot_bulk;
pub mod shot_readiness;
pub mod task;
pub mod tool;
pub mod workflow_benchmark;
pub mod workflow_library;
pub mod workflow_lifecycle;
pub mod workflow_onboarding;
pub mod workflow_registry;
pub mod workflow_workspace;

use crate::{app_state::AppState, error::AppError};
use serde::Serialize;
use tauri::State;

pub(crate) fn validate_project_id(value: &str) -> Result<(), AppError> {
    crate::domain::validate_project_id(value)
        .map_err(|error| AppError::invalid_input(error.to_string()))
}

pub(crate) fn map_repository_error(error: &crate::application::ports::RepositoryError) -> AppError {
    match error {
        crate::application::ports::RepositoryError::WorkflowVersionConflict { .. } => {
            AppError::workflow_version_conflict(error.to_string())
        }
        crate::application::ports::RepositoryError::RecipeVersionConflict { .. } => {
            AppError::recipe_version_conflict(error.to_string())
        }
        crate::application::ports::RepositoryError::NotFound { .. } => {
            AppError::database(error.to_string())
        }
        _ => AppError::database(error.to_string()),
    }
}

#[derive(Debug, Serialize)]
pub struct AppStatus {
    pub backend: &'static str,
    pub database: &'static str,
    pub version: &'static str,
}

#[tauri::command]
pub fn ping() -> Result<&'static str, AppError> {
    Ok("pong")
}

#[tauri::command]
pub fn get_app_status(state: State<'_, AppState>) -> Result<AppStatus, AppError> {
    if !state.system.data_dirs.root.is_dir() || !state.system.data_dirs.database.is_file() {
        return Err(AppError::internal(
            "application data directory or database is not ready",
        ));
    }

    Ok(AppStatus {
        backend: "ready",
        database: "ready",
        version: env!("CARGO_PKG_VERSION"),
    })
}
pub mod product;
// Inlined (no new backend source files under the frozen Phase2/3 aggregate).
pub mod story_entities {
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

    pub async fn product_story_entity_list_for_shot(
        state: State<'_, AppState>,
        project_id: String,
        shot_id: String,
    ) -> Result<Vec<StoryEntity>, AppError> {
        state
            .shots
            .story_entities
            .list_for_shot(&project_id, &shot_id)
            .await
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct StoryEntityShotBindRequest {
        pub project_id: String,
        pub shot_id: String,
        pub entity_ids: Vec<String>,
    }

    pub async fn product_story_entity_bind_shot(
        state: State<'_, AppState>,
        request: StoryEntityShotBindRequest,
    ) -> Result<Vec<StoryEntity>, AppError> {
        state
            .shots
            .story_entities
            .replace_shot_bindings(&request.project_id, &request.shot_id, request.entity_ids)
            .await
    }
}

pub mod folder_batch_import {
    use crate::application::folder_batch_import_service::{ImportFailure, Preview, PreviewOptions};
    use crate::{app_state::AppState, error::AppError};
    use tauri::{AppHandle, State};
    use tauri_plugin_dialog::DialogExt;
    fn map_error(error: ImportFailure) -> AppError {
        let mut mapped = AppError::invalid_input(error.message);
        mapped.details = Some(serde_json::json!({"code":error.code,"stage":error.stage}));
        mapped
    }
    #[tauri::command(rename_all = "camelCase")]
    pub async fn folder_batch_import_pick(
        app_handle: AppHandle,
        state: State<'_, AppState>,
        project_id: String,
    ) -> Result<Option<Preview>, AppError> {
        super::validate_project_id(&project_id)?;
        let Some(folder) = app_handle.dialog().file().blocking_pick_folder() else {
            return Ok(None);
        };
        let root = folder
            .into_path()
            .map_err(|_| AppError::filesystem("所选目录无法读取"))?;
        state
            .shots
            .folder_import
            .pick(&project_id, root)
            .await
            .map(Some)
            .map_err(map_error)
    }
    #[tauri::command(rename_all = "camelCase")]
    pub async fn folder_batch_import_rescan(
        state: State<'_, AppState>,
        session_id: String,
        options: PreviewOptions,
    ) -> Result<Preview, AppError> {
        state
            .shots
            .folder_import
            .rescan(&session_id, options)
            .await
            .map_err(map_error)
    }
    #[tauri::command(rename_all = "camelCase")]
    pub async fn folder_batch_import_discard(
        state: State<'_, AppState>,
        session_id: String,
    ) -> Result<(), AppError> {
        state
            .shots
            .folder_import
            .discard(&session_id)
            .await
            .map_err(map_error)
    }
}
