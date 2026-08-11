use crate::{
    app_state::AppState,
    application::h3_local_import_service::{
        H3LocalImportCommitRequest, H3LocalImportError, H3LocalImportInspection, H3LocalImportMode,
        H3LocalImportPair, H3LocalImportResult,
    },
    domain::SeedValue,
    error::AppError,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct H3LocalImportPairView {
    pub ordinal: usize,
    pub image_display_name: String,
    pub prompt_display_name: String,
    pub prompt_preview: Option<String>,
    pub prompt_bytes: Option<usize>,
    pub status: String,
    pub last_image_display_name: Option<String>,
    pub video_display_names: Vec<String>,
    pub audio_display_names: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct H3LocalImportInspectionView {
    pub session_id: String,
    pub display_root_name: String,
    pub mode: String,
    pub detected_manifest: bool,
    pub image_count: usize,
    pub prompt_count: usize,
    pub ready_count: usize,
    pub error_count: usize,
    pub pairs: Vec<H3LocalImportPairView>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct H3LocalImportCommitRequestDto {
    pub session_id: String,
    pub batch_name: Option<String>,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub width: i64,
    pub height: i64,
    pub duration_seconds: i64,
    pub seed: Option<String>,
    pub auto_start: bool,
    pub generation_mode: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct H3LocalImportResultView {
    pub batch_id: String,
    pub batch_name: String,
    pub item_count: usize,
    pub imported_asset_count: usize,
    pub auto_started: bool,
    pub warnings: Vec<String>,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn h3_local_import_pick_directory(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    mode: String,
) -> Result<Option<H3LocalImportInspectionView>, AppError> {
    super::validate_project_id(&project_id)?;
    let mode = H3LocalImportMode::parse(&mode).map_err(map_local_import_error)?;
    let Some(folder) = app_handle.dialog().file().blocking_pick_folder() else {
        return Ok(None);
    };
    let root_path = folder
        .into_path()
        .map_err(|_| AppError::filesystem("所选任务目录无法读取"))?;
    let (session_id, inspection) = state
        .h3_local_import_service
        .pick(&project_id, root_path, mode)
        .await
        .map_err(map_local_import_error)?;
    Ok(Some(inspection_view(&session_id, &inspection)))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn h3_local_import_rescan(
    state: State<'_, AppState>,
    session_id: String,
    mode: String,
) -> Result<H3LocalImportInspectionView, AppError> {
    let mode = H3LocalImportMode::parse(&mode).map_err(map_local_import_error)?;
    let inspection = state
        .h3_local_import_service
        .rescan(&session_id, mode)
        .await
        .map_err(map_local_import_error)?;
    Ok(inspection_view(&session_id, &inspection))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn h3_local_import_commit(
    state: State<'_, AppState>,
    request: H3LocalImportCommitRequestDto,
) -> Result<H3LocalImportResultView, AppError> {
    let seed = match request.seed.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(value) => Some(
            value
                .parse::<u64>()
                .map_err(|_| AppError::invalid_input("随机种子必须是十进制整数"))
                .map(SeedValue::Fixed)?,
        ),
    };
    let result = state
        .h3_local_import_service
        .commit(
            &request.session_id,
            H3LocalImportCommitRequest {
                batch_name: request.batch_name,
                workflow_version_id: request.workflow_version_id,
                recipe_id: request.recipe_id,
                width: request.width,
                height: request.height,
                duration_seconds: request.duration_seconds,
                seed,
                auto_start: request.auto_start,
                generation_mode: request.generation_mode,
            },
        )
        .await
        .map_err(map_local_import_error)?;
    Ok(result_view(result))
}

fn inspection_view(
    session_id: &str,
    inspection: &H3LocalImportInspection,
) -> H3LocalImportInspectionView {
    H3LocalImportInspectionView {
        session_id: session_id.to_owned(),
        display_root_name: inspection.display_root_name.clone(),
        mode: inspection.mode.as_str().to_owned(),
        detected_manifest: inspection.detected_manifest,
        image_count: inspection.image_count,
        prompt_count: inspection.prompt_count,
        ready_count: inspection.ready_count,
        error_count: inspection.error_count,
        pairs: inspection.pairs.iter().map(pair_view).collect(),
        errors: inspection.errors.clone(),
        warnings: inspection.warnings.clone(),
    }
}

fn pair_view(pair: &H3LocalImportPair) -> H3LocalImportPairView {
    H3LocalImportPairView {
        ordinal: pair.ordinal,
        image_display_name: pair.image_display_name.clone(),
        prompt_display_name: pair.prompt_display_name.clone(),
        prompt_preview: pair.prompt_preview.clone(),
        prompt_bytes: pair.prompt_bytes,
        status: pair.status.as_str().to_owned(),
        last_image_display_name: pair.last_image_display_name.clone(),
        video_display_names: pair.video_display_names.clone(),
        audio_display_names: pair.audio_display_names.clone(),
    }
}

fn result_view(result: H3LocalImportResult) -> H3LocalImportResultView {
    H3LocalImportResultView {
        batch_id: result.batch_id,
        batch_name: result.batch_name,
        item_count: result.item_count,
        imported_asset_count: result.imported_asset_count,
        auto_started: result.auto_started,
        warnings: result.warnings,
    }
}

fn map_local_import_error(error: H3LocalImportError) -> AppError {
    match error {
        H3LocalImportError::FilesystemBoundary(message) => AppError::filesystem_boundary(message),
        H3LocalImportError::Filesystem(message) => AppError::filesystem(message),
        H3LocalImportError::Queue(message) => AppError::invalid_input(message),
        H3LocalImportError::AssetImport(message) => AppError::invalid_input(message),
        H3LocalImportError::Prompt(message) => AppError::invalid_input(message),
        H3LocalImportError::InvalidInput(message) | H3LocalImportError::Inspection(message) => {
            AppError::invalid_input(message)
        }
        H3LocalImportError::SessionNotFound => {
            AppError::invalid_input("本地导入会话不存在，请重新选择目录")
        }
        H3LocalImportError::SessionExpired => {
            AppError::invalid_input("本地导入会话已过期，请重新选择目录")
        }
    }
}
