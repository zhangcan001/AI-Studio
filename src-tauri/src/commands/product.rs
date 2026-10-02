use crate::{
    app_state::AppState,
    application::product::{
        creation_facade::{
            CreationContext, CreationShot, CreationShotSummary, CreationShotUpdate,
            GeneratorOption, ProductCreationFacade,
        },
        error::ProductError,
        project_facade::{
            GeneratorBindingSetRequest, GeneratorBindingSummary, ProductProjectFacade,
            ProjectOverview,
        },
        run_facade::{ProductRun, ProductRunFacade, RunRef, RunRetryRequest},
    },
};
use tauri::State;

fn runs(state: &AppState) -> ProductRunFacade {
    ProductRunFacade::new(
        state.production.queue.clone(),
        state.production.orchestrator.clone(),
        state.tasks.query.clone(),
    )
}

fn creation(state: &AppState) -> ProductCreationFacade {
    ProductCreationFacade::new(
        state.catalog.generation.clone(),
        state.projects.workflow_binding.clone(),
        state.shots.shot.clone(),
        state.workflow.registry.clone(),
    )
}

#[tauri::command(rename_all = "camelCase")]
pub async fn product_creation_get(
    state: State<'_, AppState>,
    project_id: String,
    shot_id: Option<String>,
    stage: String,
) -> Result<CreationContext, ProductError> {
    creation(&state)
        .get(
            &state.projects.command_center,
            &state.assets.query,
            &project_id,
            shot_id.as_deref(),
            &stage,
        )
        .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn product_creation_shot_create(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<CreationShotSummary, ProductError> {
    creation(&state).create_shot(&project_id).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn product_creation_shot_update(
    state: State<'_, AppState>,
    project_id: String,
    request: CreationShotUpdate,
) -> Result<CreationShotSummary, ProductError> {
    creation(&state).update_shot(&project_id, request).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn product_creation_shot_delete(
    state: State<'_, AppState>,
    project_id: String,
    shot_id: String,
) -> Result<(), ProductError> {
    creation(&state).delete_shot(&project_id, &shot_id).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn product_creation_references_set(
    state: State<'_, AppState>,
    project_id: String,
    shot_id: String,
    stage: String,
    asset_ids: Vec<String>,
) -> Result<CreationShot, ProductError> {
    creation(&state)
        .references_set(&project_id, &shot_id, &stage, asset_ids)
        .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn product_creation_result_select(
    state: State<'_, AppState>,
    project_id: String,
    shot_id: String,
    stage: String,
    asset_id: String,
) -> Result<CreationShot, ProductError> {
    creation(&state)
        .select_result(
            &state.assets.query,
            &project_id,
            &shot_id,
            &stage,
            &asset_id,
        )
        .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn product_project_overview(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<ProjectOverview, ProductError> {
    ProductProjectFacade::new(state.projects.command_center.clone())
        .get_overview(&project_id)
        .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn product_generators_list(
    state: State<'_, AppState>,
    project_id: String,
    shot_id: Option<String>,
    stage: String,
) -> Result<Vec<GeneratorOption>, ProductError> {
    ProductCreationFacade::new(
        state.catalog.generation.clone(),
        state.projects.workflow_binding.clone(),
        state.shots.shot.clone(),
        state.workflow.registry.clone(),
    )
    .generators_list(&project_id, shot_id.as_deref(), &stage)
    .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn product_generator_binding_set(
    state: State<'_, AppState>,
    project_id: String,
    request: GeneratorBindingSetRequest,
) -> Result<Vec<GeneratorBindingSummary>, ProductError> {
    ProductProjectFacade::set_generator_binding(
        &state.projects.workflow_binding,
        &project_id,
        request,
    )
    .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn product_run_get(
    state: State<'_, AppState>,
    project_id: String,
    run_ref: RunRef,
) -> Result<ProductRun, ProductError> {
    runs(&state).get(&project_id, run_ref).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn product_run_retry(
    state: State<'_, AppState>,
    project_id: String,
    request: RunRetryRequest,
) -> Result<ProductRun, ProductError> {
    runs(&state).retry(&project_id, request).await
}
