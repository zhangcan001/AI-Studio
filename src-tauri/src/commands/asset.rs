use crate::{
    app_state::AppState,
    application::{
        asset_library_service::{AssetLibraryError, AssetLibraryPageView},
        asset_query_service::{AssetQueryError, AssetView},
        source_asset_import_service::SourceAssetImportError,
        source_asset_import_service::MAX_SOURCE_IMAGE_BYTES,
    },
    application::{pagination::PageCursor, ports::AssetCategoryFilter},
    error::AppError,
};
use tauri::{ipc::Response, AppHandle, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command(rename_all = "camelCase")]
pub async fn asset_list_by_task(
    state: State<'_, AppState>,
    project_id: String,
    task_id: String,
) -> Result<Vec<crate::application::asset_query_service::AssetView>, AppError> {
    super::validate_project_id(&project_id)?;
    let task_exists = state
        .task_query_service
        .get(&project_id, &task_id)
        .await
        .map_err(map_task_query_error)?
        .is_some();
    if !task_exists {
        return Err(AppError::task_not_found(format!(
            "task {task_id} was not found"
        )));
    }
    state
        .asset_query_service
        .list_by_task(&project_id, &task_id)
        .await
        .map_err(map_asset_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn asset_list_recent(
    state: State<'_, AppState>,
    project_id: String,
    limit: Option<u32>,
) -> Result<Vec<AssetView>, AppError> {
    super::validate_project_id(&project_id)?;
    state
        .asset_query_service
        .list_recent(&project_id, limit.unwrap_or(100))
        .await
        .map_err(map_asset_error)
}

#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AssetCategoryFilterDto {
    All,
    SourceImage,
    GeneratedImage,
    GeneratedVideo,
}

impl From<AssetCategoryFilterDto> for AssetCategoryFilter {
    fn from(value: AssetCategoryFilterDto) -> Self {
        match value {
            AssetCategoryFilterDto::All => Self::All,
            AssetCategoryFilterDto::SourceImage => Self::SourceImage,
            AssetCategoryFilterDto::GeneratedImage => Self::GeneratedImage,
            AssetCategoryFilterDto::GeneratedVideo => Self::GeneratedVideo,
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn asset_library_page(
    state: State<'_, AppState>,
    project_id: String,
    category: AssetCategoryFilterDto,
    cursor: Option<PageCursor>,
    limit: Option<u32>,
) -> Result<AssetLibraryPageView, AppError> {
    super::validate_project_id(&project_id)?;
    state
        .asset_library_service
        .list_page(&project_id, category.into(), cursor, limit.unwrap_or(30))
        .await
        .map_err(map_asset_library_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn asset_get(
    state: State<'_, AppState>,
    project_id: String,
    asset_id: String,
) -> Result<AssetView, AppError> {
    super::validate_project_id(&project_id)?;
    state
        .asset_query_service
        .get(&project_id, &asset_id)
        .await
        .map_err(map_asset_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn asset_pick_and_import_image(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Option<AssetView>, AppError> {
    super::validate_project_id(&project_id)?;
    let Some(file) = app_handle
        .dialog()
        .file()
        .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = file.into_path().map_err(|error| {
        AppError::filesystem(format!("selected image path is unavailable: {error}"))
    })?;
    let original_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| AppError::invalid_input("selected image has no usable file name"))?
        .to_owned();
    let file_size = tokio::fs::metadata(&path)
        .await
        .map_err(|error| {
            AppError::filesystem(format!("selected image could not be inspected: {error}"))
        })?
        .len();
    if file_size > MAX_SOURCE_IMAGE_BYTES {
        return Err(AppError::invalid_input(format!(
            "SOURCE_IMAGE_TOO_LARGE: image is {file_size} bytes; maximum is {MAX_SOURCE_IMAGE_BYTES}"
        )));
    }
    let bytes = tokio::fs::read(&path).await.map_err(|error| {
        AppError::filesystem(format!("selected image could not be read: {error}"))
    })?;
    let asset = state
        .source_asset_import_service
        .import_bytes(&project_id, &original_name, &bytes)
        .await
        .map_err(map_source_import_error)?;
    Ok(Some(AssetView::from(asset)))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn asset_read_image(
    state: State<'_, AppState>,
    project_id: String,
    asset_id: String,
) -> Result<Response, AppError> {
    super::validate_project_id(&project_id)?;
    let asset = state
        .asset_query_service
        .read_image(&project_id, &asset_id)
        .await
        .map_err(map_asset_error)?;
    Ok(Response::new(asset.bytes))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn asset_read_thumbnail(
    state: State<'_, AppState>,
    project_id: String,
    asset_id: String,
) -> Result<Response, AppError> {
    super::validate_project_id(&project_id)?;
    let asset = state
        .asset_query_service
        .read_thumbnail(&project_id, &asset_id)
        .await
        .map_err(map_asset_error)?;
    Ok(Response::new(asset.bytes))
}

fn map_task_query_error(error: crate::application::task_query_service::TaskQueryError) -> AppError {
    match error {
        crate::application::task_query_service::TaskQueryError::InvalidProjectId(message)
        | crate::application::task_query_service::TaskQueryError::InvalidTaskId(message) => {
            AppError::invalid_input(message)
        }
        crate::application::task_query_service::TaskQueryError::Repository(error) => {
            super::map_repository_error(&error)
        }
    }
}

fn map_asset_error(error: AssetQueryError) -> AppError {
    match error {
        AssetQueryError::InvalidTaskId(message)
        | AssetQueryError::InvalidProjectId(message)
        | AssetQueryError::InvalidAssetId(message) => AppError::invalid_input(message),
        AssetQueryError::NotFound(message) => AppError::asset_not_found(message),
        AssetQueryError::NotImage(message) => AppError::invalid_input(message),
        AssetQueryError::ThumbnailNotAvailable(message) => AppError::asset_read_failed(message),
        AssetQueryError::Repository(error) => super::map_repository_error(&error),
        AssetQueryError::Read(error) => AppError::asset_read_failed(error.to_string()),
    }
}

fn map_asset_library_error(error: AssetLibraryError) -> AppError {
    match error {
        AssetLibraryError::InvalidProjectId => {
            AppError::invalid_input("INVALID_PROJECT_ID: project id must not be empty")
        }
        AssetLibraryError::Repository(error) => super::map_repository_error(&error),
    }
}

fn map_source_import_error(error: SourceAssetImportError) -> AppError {
    let code = error.code();
    match error {
        SourceAssetImportError::InvalidSourceImage { message } => {
            AppError::invalid_input(format!("{code}: {message}"))
        }
        SourceAssetImportError::SourceImageTooLarge {
            max_bytes,
            actual_bytes,
        } => AppError::invalid_input(format!(
            "{code}: image is {actual_bytes} bytes; maximum is {max_bytes}"
        )),
        SourceAssetImportError::ProjectStorageMissing { project_id } => AppError::database(
            format!("ASSET_PERSISTENCE_ERROR: project {project_id} has no storage root"),
        ),
        SourceAssetImportError::AssetPersistence { message } => AppError::database(message),
    }
}
