use crate::{
    app_state::AppState,
    application::{
        pagination::PageCursor,
        recipe_history_query_service::{RecipeHistoryQueryService, RecipeHistoryView},
    },
    error::AppError,
};
use tauri::State;

#[tauri::command(rename_all = "camelCase")]
pub async fn workflow_recipe_history_get(
    state: State<'_, AppState>,
    workflow_version_id: String,
    recipe_id: String,
    task_cursor: Option<PageCursor>,
    task_limit: Option<u32>,
    project_id: Option<String>,
    statuses: Option<Vec<String>>,
) -> Result<RecipeHistoryView, AppError> {
    state
        .workflow.recipe_history_query
        .get_exact_pair_filtered(
            &workflow_version_id,
            &recipe_id,
            task_cursor,
            task_limit,
            project_id.as_deref(),
            statuses.unwrap_or_default(),
        )
        .await
        .map_err(|error| match error {
            crate::application::recipe_history_query_service::RecipeHistoryQueryError::InvalidIdentity => {
                AppError::invalid_input("workflowVersionId and recipeId are required")
            }
            crate::application::recipe_history_query_service::RecipeHistoryQueryError::NotFound { .. } => {
                AppError::database(error.to_string())
            }
            crate::application::recipe_history_query_service::RecipeHistoryQueryError::Repository(error) => {
                AppError::database(error.to_string())
            }
        })
}

// Keep the service name visible at the command boundary for architecture audits.
#[allow(dead_code)]
fn _recipe_history_service_type(_: &RecipeHistoryQueryService) {}
