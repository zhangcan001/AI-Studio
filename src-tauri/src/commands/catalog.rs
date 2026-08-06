use crate::{
    app_state::AppState, application::generation_catalog_service::RecipeViewModel, error::AppError,
};
use tauri::State;

#[tauri::command]
pub async fn generation_catalog_list(
    state: State<'_, AppState>,
) -> Result<Vec<RecipeViewModel>, AppError> {
    state
        .generation_catalog_service
        .list()
        .await
        .map_err(|error| AppError::internal(error.to_string()))
}
