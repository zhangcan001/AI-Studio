use crate::{
    app_state::AppState,
    application::h3_local_import_service::{
        H3LocalImportCommitRequest, H3LocalImportError, H3LocalImportInspection, H3LocalImportMode,
        H3LocalImportPair, H3LocalImportResult, H3ProjectFolderInspection, H3ProjectMedia,
        H3ProjectSegmentDraft, H3ProjectSegmentInspection, H3QualityRecipeSelection,
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
    pub project_folder: Option<H3ProjectFolderInspectionView>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct H3ProjectMediaView {
    pub id: String,
    pub display_name: String,
    pub kind: String,
    pub size_bytes: u64,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct H3ProjectSegmentView {
    pub ordinal: usize,
    pub segment_id: String,
    pub folder_name: String,
    pub generation_mode: String,
    pub inferred_mode: String,
    pub mode_source: String,
    pub prompt: Option<String>,
    pub prompt_display_name: Option<String>,
    pub prompt_bytes: Option<usize>,
    pub width: i64,
    pub height: i64,
    pub resolution_source: String,
    pub duration_seconds: i64,
    pub duration_source: String,
    pub first_frame: Option<H3ProjectMediaView>,
    pub last_frame: Option<H3ProjectMediaView>,
    pub reference_images: Vec<H3ProjectMediaView>,
    pub reference_audios: Vec<H3ProjectMediaView>,
    pub reference_videos: Vec<H3ProjectMediaView>,
    pub media: Vec<H3ProjectMediaView>,
    pub status: String,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct H3ProjectFolderInspectionView {
    pub display_root_name: String,
    pub segment_count: usize,
    pub ready_count: usize,
    pub error_count: usize,
    pub segments: Vec<H3ProjectSegmentView>,
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
    pub fl2va_workflow_version_id: Option<String>,
    pub fl2va_recipe_id: Option<String>,
    pub ref2va_workflow_version_id: Option<String>,
    pub ref2va_recipe_id: Option<String>,
    pub quality_profile: Option<String>,
    #[serde(default)]
    pub quality_recipes: Vec<H3QualityRecipeSelectionDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct H3QualityRecipeSelectionDto {
    pub mode: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct H3ProjectSegmentDraftDto {
    pub session_id: String,
    pub segment_id: String,
    pub mode: Option<String>,
    pub prompt: Option<String>,
    pub duration_seconds: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub reference_image_ids: Option<Vec<String>>,
    pub reference_audio_ids: Option<Vec<String>>,
    pub reference_video_ids: Option<Vec<String>>,
    pub first_frame_id: Option<String>,
    pub last_frame_id: Option<String>,
    pub reset_auto_detection: bool,
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
                fl2va_workflow_version_id: request.fl2va_workflow_version_id,
                fl2va_recipe_id: request.fl2va_recipe_id,
                ref2va_workflow_version_id: request.ref2va_workflow_version_id,
                ref2va_recipe_id: request.ref2va_recipe_id,
                quality_profile: request.quality_profile,
                quality_recipes: request
                    .quality_recipes
                    .into_iter()
                    .map(|selection| H3QualityRecipeSelection {
                        mode: selection.mode,
                        workflow_version_id: selection.workflow_version_id,
                        recipe_id: selection.recipe_id,
                    })
                    .collect(),
                mode_recipes: Vec::new(),
            },
        )
        .await
        .map_err(map_local_import_error)?;
    Ok(result_view(result))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn h3_local_import_update_project_segment_draft(
    state: State<'_, AppState>,
    request: H3ProjectSegmentDraftDto,
) -> Result<H3LocalImportInspectionView, AppError> {
    let inspection = state
        .h3_local_import_service
        .update_h3_project_segment_draft(
            &request.session_id,
            H3ProjectSegmentDraft {
                segment_id: request.segment_id,
                mode: request.mode,
                prompt: request.prompt,
                duration_seconds: request.duration_seconds,
                width: request.width,
                height: request.height,
                reference_image_ids: request.reference_image_ids,
                reference_audio_ids: request.reference_audio_ids,
                reference_video_ids: request.reference_video_ids,
                first_frame_id: request.first_frame_id,
                last_frame_id: request.last_frame_id,
                reset_auto_detection: request.reset_auto_detection,
            },
        )
        .await
        .map_err(map_local_import_error)?;
    Ok(inspection_view(&request.session_id, &inspection))
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
        project_folder: inspection.project_folder.as_ref().map(project_folder_view),
        errors: inspection.errors.clone(),
        warnings: inspection.warnings.clone(),
    }
}

fn project_folder_view(project: &H3ProjectFolderInspection) -> H3ProjectFolderInspectionView {
    H3ProjectFolderInspectionView {
        display_root_name: project.display_root_name.clone(),
        segment_count: project.segment_count,
        ready_count: project.ready_count,
        error_count: project.error_count,
        segments: project.segments.iter().map(project_segment_view).collect(),
        errors: project.errors.clone(),
        warnings: project.warnings.clone(),
    }
}

fn project_segment_view(segment: &H3ProjectSegmentInspection) -> H3ProjectSegmentView {
    H3ProjectSegmentView {
        ordinal: segment.ordinal,
        segment_id: segment.segment_id.clone(),
        folder_name: segment.folder_name.clone(),
        generation_mode: segment.generation_mode.clone(),
        inferred_mode: segment.inferred_mode.clone(),
        mode_source: segment.mode_source.clone(),
        prompt: segment.prompt.clone(),
        prompt_display_name: segment.prompt_display_name.clone(),
        prompt_bytes: segment.prompt_bytes,
        width: segment.width,
        height: segment.height,
        resolution_source: segment.resolution_source.clone(),
        duration_seconds: segment.duration_seconds,
        duration_source: segment.duration_source.clone(),
        first_frame: segment.first_frame.as_ref().map(media_view),
        last_frame: segment.last_frame.as_ref().map(media_view),
        reference_images: segment.reference_images.iter().map(media_view).collect(),
        reference_audios: segment.reference_audios.iter().map(media_view).collect(),
        reference_videos: segment.reference_videos.iter().map(media_view).collect(),
        media: segment.all_media.iter().map(media_view).collect(),
        status: segment.status.clone(),
        errors: segment.errors.clone(),
        warnings: segment.warnings.clone(),
    }
}

fn media_view(media: &H3ProjectMedia) -> H3ProjectMediaView {
    H3ProjectMediaView {
        id: media.id.clone(),
        display_name: media.display_name.clone(),
        kind: media.kind.as_str().to_owned(),
        size_bytes: media.size_bytes,
        width: media.width,
        height: media.height,
        duration_ms: media.duration_ms,
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
