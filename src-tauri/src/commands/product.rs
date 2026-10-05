use crate::{
    app_state::AppState,
    application::product::{
        creation_facade::{
            CreationAccepted, CreationContext, CreationReadiness, CreationShot,
            CreationShotSummary, CreationShotUpdate, CreationSubmission, GeneratorOption,
            ProductCreationFacade,
        },
        error::ProductError,
        project_facade::{
            GeneratorBindingSetRequest, GeneratorBindingSummary, ProductProjectFacade,
            ProjectOverview,
        },
        run_facade::{
            ProductRun, ProductRunFacade, RunDetailServices, RunList, RunListFilter, RunRef,
            RunResult, RunResultReviewRequest, RunRetryRequest,
        },
    },
};
use tauri::State;

use crate::application::product::library_facade::{
    LibraryCreateIntent, LibraryDeletionInspection, LibraryDetail, LibraryEditRequest, LibraryList,
    LibraryOperations, LibraryQuery, LibraryRelation, LibraryServices, LibraryVersions,
    ResourceRef,
};

fn library(state: &AppState) -> LibraryServices<'_> {
    LibraryServices {
        assets: &state.assets.library,
        asset_detail: &state.assets.query,
        prompts: &state.catalog.prompt_library,
        profiles: &state.shots.consistency_profile,
        reference_sets: &state.shots.reference_set,
    }
}

fn library_operations(state: &AppState) -> LibraryOperations<'_> {
    LibraryOperations {
        library: library(state),
        usage: &state.assets.usage,
        deletion: &state.assets.deletion,
        data: &state.assets.data,
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn product_library_image_get(
    state: State<'_, AppState>,
    project_id: String,
    resource: ResourceRef,
) -> Result<Vec<u8>, ProductError> {
    library(&state).image_get(&project_id, &resource).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn product_library_relations_get(
    state: State<'_, AppState>,
    project_id: String,
    resource: ResourceRef,
) -> Result<Vec<LibraryRelation>, ProductError> {
    library_operations(&state)
        .relations_get(&project_id, &resource)
        .await
}
#[tauri::command(rename_all = "camelCase")]
pub async fn product_library_versions_get(
    state: State<'_, AppState>,
    project_id: String,
    resource: ResourceRef,
) -> Result<LibraryVersions, ProductError> {
    library_operations(&state)
        .versions_get(&project_id, &resource)
        .await
}
#[tauri::command(rename_all = "camelCase")]
pub async fn product_library_use_in_creation(
    state: State<'_, AppState>,
    project_id: String,
    resource: ResourceRef,
) -> Result<LibraryCreateIntent, ProductError> {
    library_operations(&state)
        .use_in_creation(&project_id, &resource)
        .await
}
#[tauri::command(rename_all = "camelCase")]
pub async fn product_library_deletion_inspect(
    state: State<'_, AppState>,
    project_id: String,
    resource: ResourceRef,
) -> Result<LibraryDeletionInspection, ProductError> {
    library_operations(&state)
        .deletion_inspect(&project_id, &resource)
        .await
}
#[tauri::command(rename_all = "camelCase")]
pub async fn product_library_delete(
    state: State<'_, AppState>,
    project_id: String,
    resource: ResourceRef,
    confirmed: bool,
) -> Result<(), ProductError> {
    library_operations(&state)
        .delete(&project_id, &resource, confirmed)
        .await
}
#[tauri::command(rename_all = "camelCase")]
pub async fn product_library_resource_edit(
    state: State<'_, AppState>,
    project_id: String,
    request: LibraryEditRequest,
) -> Result<LibraryDetail, ProductError> {
    library_operations(&state)
        .resource_edit(&project_id, request)
        .await
}

/// Read-only adapter to the existing Advanced Asset organization authority.
#[tauri::command(rename_all = "camelCase")]
pub async fn product_library_tags_list(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<crate::application::ports::AssetTag>, ProductError> {
    state
        .organization
        .organization
        .list_tags(&project_id)
        .await
        .map_err(ProductError::internal)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn product_library_list(
    state: State<'_, AppState>,
    project_id: String,
    query: LibraryQuery,
) -> Result<LibraryList, ProductError> {
    library(&state).list(&project_id, query).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn product_library_get(
    state: State<'_, AppState>,
    project_id: String,
    resource: ResourceRef,
) -> Result<LibraryDetail, ProductError> {
    library(&state).get(&project_id, &resource).await
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreationSubmissionDto {
    prompt_id: Option<String>,
    prompt_version_id: Option<String>,
    project_id: String,
    shot_id: String,
    stage: String,
    selection_ref: String,
    values: std::collections::BTreeMap<String, super::generation::InputValueDto>,
    submission_idempotency_key: String,
}

impl CreationSubmissionDto {
    fn into_application(self) -> Result<CreationSubmission, ProductError> {
        let values = self
            .values
            .into_iter()
            .map(|(key, value)| {
                value
                    .into_application(&key)
                    .map(|value| (key.clone(), value))
                    .map_err(|error| {
                        let mut result = ProductError::new(
                            "INVALID_INPUT",
                            "输入格式无效。",
                            Some("EDIT_INPUT"),
                        );
                        result.details.field = Some(key);
                        result.details.technical_details = Some(error.to_string());
                        result
                    })
            })
            .collect::<Result<_, _>>()?;
        Ok(CreationSubmission {
            prompt_id: self.prompt_id,
            prompt_version_id: self.prompt_version_id,
            project_id: self.project_id,
            shot_id: self.shot_id,
            stage: self.stage,
            selection_ref: self.selection_ref,
            values,
            submission_idempotency_key: self.submission_idempotency_key,
        })
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn product_creation_readiness_get(
    state: State<'_, AppState>,
    request: CreationSubmissionDto,
) -> Result<CreationReadiness, ProductError> {
    Ok(creation(&state)
        .readiness_get(
            &state.production.queue,
            &state.catalog.prompt_library,
            request.into_application()?,
        )
        .await)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn product_creation_generate(
    state: State<'_, AppState>,
    request: CreationSubmissionDto,
) -> Result<CreationAccepted, ProductError> {
    let admission = state.production.admission.clone();
    creation(&state)
        .generate(
            &state.production.queue,
            &state.catalog.prompt_library,
            request.into_application()?,
            move |project_id, batch_id| async move {
                admission
                    .start(&project_id, &batch_id)
                    .await
                    .map_err(|error| {
                        let mut result = ProductError::new(
                            "RUNTIME_BLOCKED",
                            "生成已加入队列，但暂时无法启动。",
                            Some("OPEN_RUN"),
                        );
                        result.details.technical_details = Some(error.to_string());
                        result
                    })
            },
        )
        .await
}

fn runs(state: &AppState) -> ProductRunFacade {
    ProductRunFacade::new(
        state.production.queue.clone(),
        state.production.orchestrator.clone(),
        state.tasks.query.clone(),
    )
}

fn run_details(state: &AppState) -> RunDetailServices<'_> {
    RunDetailServices {
        history: &state.tasks.history,
        assets: &state.assets.query,
        artifacts: &state.assets.artifact,
        shots: &state.shots.shot,
    }
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
    let facade = creation(&state);
    let mut context = facade
        .get(
            &state.projects.command_center,
            &state.assets.query,
            &project_id,
            shot_id.as_deref(),
            &stage,
        )
        .await?;
    facade
        .project_prompt_choices(&mut context, &state.catalog.prompt_library)
        .await?;
    Ok(context)
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
    let mut overview = ProductProjectFacade::new(state.projects.command_center.clone())
        .get_overview(&project_id)
        .await?;
    overview.generator_bindings = crate::application::product::project_facade::binding_summaries(
        state
            .projects
            .workflow_binding
            .get(&project_id)
            .await
            .map_err(ProductError::binding)?,
    )?;
    Ok(overview)
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
    runs(&state)
        .get_detail(&project_id, run_ref, &run_details(&state))
        .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn product_run_results_get(
    state: State<'_, AppState>,
    project_id: String,
    run_ref: RunRef,
) -> Result<Vec<RunResult>, ProductError> {
    runs(&state)
        .results_get(&project_id, run_ref, &run_details(&state))
        .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn product_run_result_review(
    state: State<'_, AppState>,
    project_id: String,
    request: RunResultReviewRequest,
) -> Result<(), ProductError> {
    runs(&state)
        .result_review(&project_id, request, &run_details(&state))
        .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn product_run_retry(
    state: State<'_, AppState>,
    project_id: String,
    request: RunRetryRequest,
) -> Result<ProductRun, ProductError> {
    runs(&state).retry(&project_id, request).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn product_run_list(
    state: State<'_, AppState>,
    project_id: String,
    filter: RunListFilter,
    cursor: Option<String>,
) -> Result<RunList, ProductError> {
    runs(&state)
        .list(&project_id, filter, cursor.as_deref())
        .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn product_run_start(
    state: State<'_, AppState>,
    project_id: String,
    run_ref: RunRef,
) -> Result<ProductRun, ProductError> {
    let admission = state.production.admission.clone();
    runs(&state)
        .start(&project_id, run_ref, move |project, batch| async move {
            admission.start(&project, &batch).await.map_err(|error| {
                let mut result = ProductError::new(
                    "RUN_START_BLOCKED",
                    "当前无法启动，请检查运行环境和输入。",
                    Some("REVIEW_RUN"),
                );
                result.details.technical_details = Some(error.to_string());
                result
            })
        })
        .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn product_run_pause(
    state: State<'_, AppState>,
    project_id: String,
    run_ref: RunRef,
) -> Result<ProductRun, ProductError> {
    runs(&state).pause(&project_id, run_ref).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn product_run_cancel(
    state: State<'_, AppState>,
    project_id: String,
    run_ref: RunRef,
) -> Result<ProductRun, ProductError> {
    runs(&state)
        .cancel(&project_id, run_ref, &state.tasks.cancellation)
        .await
}
