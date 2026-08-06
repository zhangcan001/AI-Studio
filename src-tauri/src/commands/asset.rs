use crate::{
    app_state::AppState, application::asset_query_service::AssetQueryError, error::AppError,
};
use tauri::{ipc::Response, State};

#[tauri::command(rename_all = "camelCase")]
pub async fn asset_list_by_task(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<Vec<crate::application::asset_query_service::AssetView>, AppError> {
    state
        .asset_query_service
        .list_by_task(&task_id)
        .await
        .map_err(map_asset_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn asset_read_image(
    state: State<'_, AppState>,
    asset_id: String,
) -> Result<Response, AppError> {
    let asset = state
        .asset_query_service
        .read_image(&asset_id)
        .await
        .map_err(map_asset_error)?;
    Ok(Response::new(asset.bytes))
}

fn map_asset_error(error: AssetQueryError) -> AppError {
    match error {
        AssetQueryError::InvalidTaskId(message) | AssetQueryError::InvalidAssetId(message) => {
            AppError::invalid_input(message)
        }
        AssetQueryError::NotFound(message) => AppError::asset_not_found(message),
        AssetQueryError::NotImage(message) => AppError::invalid_input(message),
        AssetQueryError::Repository(error) => super::map_repository_error(&error),
        AssetQueryError::Read(error) => AppError::asset_read_failed(error.to_string()),
    }
}
