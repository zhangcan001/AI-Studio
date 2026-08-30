use crate::{
    app_state::AppState,
    application::asset_query_service::AssetSummaryView,
    application::production_item_review_service::{
        ProductionBatchReview, ProductionReviewError, ProductionReviewProductivityView,
        RegenerateRequest,
    },
    domain::ProductionReviewStatus,
    error::AppError,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProductionReviewStatusDto {
    Unreviewed,
    Approved,
    Starred,
    Regenerate,
    Rejected,
}

impl From<ProductionReviewStatusDto> for ProductionReviewStatus {
    fn from(value: ProductionReviewStatusDto) -> Self {
        match value {
            ProductionReviewStatusDto::Unreviewed => Self::Unreviewed,
            ProductionReviewStatusDto::Approved => Self::Approved,
            ProductionReviewStatusDto::Starred => Self::Starred,
            ProductionReviewStatusDto::Regenerate => Self::Regenerate,
            ProductionReviewStatusDto::Rejected => Self::Rejected,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionReviewStatusRequest {
    pub project_id: String,
    pub batch_id: String,
    pub item_id: String,
    pub status: ProductionReviewStatusDto,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionReviewNoteRequest {
    pub project_id: String,
    pub batch_id: String,
    pub item_id: String,
    pub note: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionReviewRegenerateRequest {
    pub project_id: String,
    pub batch_id: String,
    pub item_id: String,
    pub prompt_override: Option<String>,
    pub duration_seconds: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    #[serde(default)]
    pub use_original_seed: bool,
    #[serde(default)]
    pub auto_start: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionReviewBulkRegenerateRequest {
    pub project_id: String,
    pub batch_id: String,
    #[serde(default)]
    pub auto_start: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionReviewOpenAssetRequest {
    pub project_id: String,
    pub batch_id: String,
    pub item_id: String,
    pub asset_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionBatchReviewView {
    pub batch: crate::commands::production_queue::ProductionBatchDetailView,
    pub total: usize,
    pub success_count: usize,
    pub failed_count: usize,
    pub unreviewed_count: usize,
    pub approved_count: usize,
    pub starred_count: usize,
    pub regenerate_count: usize,
    pub rejected_count: usize,
    pub items: Vec<ProductionReviewItemView>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionReviewItemView {
    pub item_id: String,
    pub ordinal: u32,
    pub task_id: Option<String>,
    pub task_status: String,
    pub production_item_status: String,
    pub review_status: String,
    pub review_note: String,
    pub version: Option<i64>,
    pub lineage_key: Option<String>,
    pub parent_batch_id: Option<String>,
    pub parent_item_id: Option<String>,
    pub preferred: bool,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub prompt_text: Option<String>,
    pub seed: Option<String>,
    pub duration_seconds: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub quality_profile: String,
    pub created_at: String,
    pub finished_at: Option<String>,
    pub output_assets: Vec<AssetSummaryView>,
    pub shot_id: Option<String>,
    pub stage: Option<String>,
    pub selected_asset_id: Option<String>,
    pub reviewable: bool,
    pub candidate_assets: Vec<ProductionReviewCandidateAssetView>,
    pub context: ProductionReviewContextView,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionReviewCandidateAssetView {
    pub asset_id: String,
    pub asset_type: String,
    pub name: String,
    pub mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_path: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub thumbnail_available: bool,
    pub task_id: Option<String>,
    pub selected: bool,
    pub review_result: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionReviewContextView {
    pub shot_id: Option<String>,
    pub stage: Option<String>,
    pub context_hash: Option<String>,
    pub snapshot_available: bool,
    pub prompt_text: Option<String>,
    pub negative_prompt: Option<String>,
    pub workflow_version_id: Option<String>,
    pub recipe_id: Option<String>,
    pub reference_sets: Vec<ProductionReviewReferenceSetView>,
    pub reference_assets: Vec<ProductionReviewReferenceAssetView>,
    pub output_spec: Value,
    pub stage_input: Value,
    pub readiness_status: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionReviewReferenceSetView {
    pub id: String,
    pub name: String,
    pub role: String,
    pub ordinal: i64,
    pub required: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionReviewReferenceAssetView {
    pub id: String,
    pub name: String,
    pub sha256: String,
    pub role: String,
    pub ordinal: i64,
    pub source_reference_set_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionReviewRegenerateView {
    pub batch: crate::commands::production_queue::ProductionBatchDetailView,
    pub source_item_ids: Vec<String>,
    pub selected_count: usize,
    pub auto_started: bool,
    pub start_warning: Option<String>,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_item_review_get(
    state: State<'_, AppState>,
    project_id: String,
    batch_id: String,
) -> Result<ProductionBatchReviewView, AppError> {
    super::validate_project_id(&project_id)?;
    state
        .production_item_review_service
        .get(&project_id, &batch_id)
        .await
        .map_err(map_review_error)
        .map(Into::into)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_item_review_productivity_get(
    state: State<'_, AppState>,
    project_id: String,
    batch_id: String,
) -> Result<ProductionBatchReviewView, AppError> {
    super::validate_project_id(&project_id)?;
    state
        .production_item_review_service
        .get_productivity_view(&project_id, &batch_id)
        .await
        .map_err(map_review_error)
        .map(Into::into)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_item_review_reveal_asset(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ProductionReviewOpenAssetRequest,
) -> Result<(), AppError> {
    super::validate_project_id(&request.project_id)?;
    let path = state
        .production_item_review_service
        .resolve_candidate_asset_path(
            &request.project_id,
            &request.batch_id,
            &request.item_id,
            &request.asset_id,
        )
        .await
        .map_err(map_review_error)?;
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|error| AppError::filesystem(format!("failed to reveal review asset: {error}")))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_item_review_open_output_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ProductionReviewOpenAssetRequest,
) -> Result<(), AppError> {
    super::validate_project_id(&request.project_id)?;
    let path = state
        .production_item_review_service
        .resolve_candidate_asset_path(
            &request.project_id,
            &request.batch_id,
            &request.item_id,
            &request.asset_id,
        )
        .await
        .map_err(map_review_error)?;
    let folder = path
        .parent()
        .map(|path| path.to_path_buf())
        .ok_or_else(|| AppError::filesystem("review asset has no output folder"))?;
    app.opener()
        .open_path(folder.to_string_lossy().into_owned(), None::<&str>)
        .map_err(|error| {
            AppError::filesystem(format!("failed to open review output folder: {error}"))
        })
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_item_review_set_status(
    state: State<'_, AppState>,
    request: ProductionReviewStatusRequest,
) -> Result<ProductionBatchReviewView, AppError> {
    super::validate_project_id(&request.project_id)?;
    state
        .production_item_review_service
        .set_status(
            &request.project_id,
            &request.batch_id,
            &request.item_id,
            request.status.into(),
        )
        .await
        .map(Into::into)
        .map_err(map_review_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_item_review_set_note(
    state: State<'_, AppState>,
    request: ProductionReviewNoteRequest,
) -> Result<ProductionBatchReviewView, AppError> {
    super::validate_project_id(&request.project_id)?;
    state
        .production_item_review_service
        .set_note(
            &request.project_id,
            &request.batch_id,
            &request.item_id,
            request.note,
        )
        .await
        .map(Into::into)
        .map_err(map_review_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_item_review_regenerate(
    state: State<'_, AppState>,
    request: ProductionReviewRegenerateRequest,
) -> Result<ProductionReviewRegenerateView, AppError> {
    super::validate_project_id(&request.project_id)?;
    state
        .production_item_review_service
        .regenerate_item(RegenerateRequest {
            project_id: request.project_id,
            batch_id: request.batch_id,
            item_id: request.item_id,
            prompt_override: request.prompt_override,
            duration_seconds: request.duration_seconds,
            width: request.width,
            height: request.height,
            use_original_seed: request.use_original_seed,
            // DEV-053 regeneration creates a READY batch. Starting remains a
            // separate, explicit queue action even if an older client sends
            // autoStart=true.
            auto_start: false,
        })
        .await
        .map(Into::into)
        .map_err(map_review_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_item_review_regenerate_marked(
    state: State<'_, AppState>,
    request: ProductionReviewBulkRegenerateRequest,
) -> Result<ProductionReviewRegenerateView, AppError> {
    super::validate_project_id(&request.project_id)?;
    state
        .production_item_review_service
        // Keep the legacy request field for wire compatibility, but never
        // start a regeneration batch from the review command.
        .regenerate_marked(&request.project_id, &request.batch_id, false)
        .await
        .map(Into::into)
        .map_err(map_review_error)
}

impl From<ProductionBatchReview> for ProductionBatchReviewView {
    fn from(value: ProductionBatchReview) -> Self {
        let batch = value.detail.clone().into();
        let mut view = Self {
            batch,
            total: value.items.len(),
            success_count: 0,
            failed_count: 0,
            unreviewed_count: 0,
            approved_count: 0,
            starred_count: 0,
            regenerate_count: 0,
            rejected_count: 0,
            items: value.items.into_iter().map(Into::into).collect(),
        };
        for item in &view.items {
            if item.production_item_status == "SUCCEEDED" && item.task_status == "SUCCEEDED" {
                view.success_count += 1;
            } else if item.production_item_status == "FAILED" || item.task_status == "FAILED" {
                view.failed_count += 1;
            }
            match item.review_status.as_str() {
                "UNREVIEWED" => view.unreviewed_count += 1,
                "APPROVED" => view.approved_count += 1,
                "STARRED" => view.starred_count += 1,
                "REGENERATE" => view.regenerate_count += 1,
                "REJECTED" => view.rejected_count += 1,
                _ => {}
            }
        }
        view
    }
}

impl From<ProductionReviewProductivityView> for ProductionBatchReviewView {
    fn from(value: ProductionReviewProductivityView) -> Self {
        let detail = value.detail;
        let batch = detail.clone().into();
        let items = value
            .items
            .into_iter()
            .map(|item| {
                let detail_item = detail
                    .items
                    .iter()
                    .find(|candidate| candidate.id.as_str() == item.item_id);
                let production_item_status = detail_item
                    .map(|candidate| candidate.status.as_str().to_owned())
                    .unwrap_or_else(|| "UNKNOWN".to_owned());
                let task_status = item.task_status.as_deref().unwrap_or_else(|| {
                    if item.reviewable {
                        "SUCCEEDED"
                    } else if production_item_status == "FAILED" {
                        "FAILED"
                    } else if item.task_id.is_some() {
                        "UNKNOWN"
                    } else {
                        "NOT_STARTED"
                    }
                });
                let review_status = item.review_status.clone().unwrap_or_else(|| {
                    if production_item_status == "FAILED" {
                        "FAILED".to_owned()
                    } else {
                        "IN_PROGRESS".to_owned()
                    }
                });
                let context = productivity_context(&item);
                let prompt_text = context.prompt_text.clone();
                let width = context.output_spec.get("width").and_then(Value::as_i64);
                let height = context.output_spec.get("height").and_then(Value::as_i64);
                let duration_seconds = context
                    .output_spec
                    .get("durationSeconds")
                    .and_then(Value::as_f64)
                    .map(|value| value.round() as i64);
                let quality_profile = if item
                    .workflow_version_id
                    .to_ascii_lowercase()
                    .contains("quality")
                    || item.recipe_id.to_ascii_lowercase().contains("quality")
                {
                    "QUALITY"
                } else {
                    "FAST"
                };
                let created_at = detail_item
                    .map(|candidate| candidate.created_at.to_rfc3339())
                    .unwrap_or_else(|| detail.batch.created_at.to_rfc3339());
                let output_assets = item
                    .candidate_assets
                    .iter()
                    .map(|candidate| asset_summary_from_candidate(candidate, &created_at))
                    .collect();
                let selected_asset_id = item
                    .candidate_assets
                    .iter()
                    .find(|candidate| candidate.selected)
                    .map(|candidate| candidate.asset_id.clone());
                ProductionReviewItemView {
                    item_id: item.item_id,
                    ordinal: item.ordinal,
                    task_id: item.task_id,
                    task_status: task_status.to_owned(),
                    production_item_status,
                    review_status,
                    review_note: item.review_note,
                    version: item.version,
                    lineage_key: None,
                    parent_batch_id: None,
                    parent_item_id: None,
                    preferred: item.preferred,
                    workflow_version_id: item.workflow_version_id,
                    recipe_id: item.recipe_id,
                    prompt_text,
                    seed: None,
                    duration_seconds,
                    width,
                    height,
                    quality_profile: quality_profile.to_owned(),
                    created_at,
                    finished_at: None,
                    output_assets,
                    shot_id: item.shot_id,
                    stage: item.stage,
                    selected_asset_id,
                    reviewable: item.reviewable,
                    candidate_assets: item.candidate_assets.into_iter().map(Into::into).collect(),
                    context,
                }
            })
            .collect();
        Self {
            batch,
            total: value.total,
            success_count: value.success_count,
            failed_count: value.failed_count,
            unreviewed_count: value.unreviewed_count,
            approved_count: value.approved_count,
            starred_count: value.starred_count,
            regenerate_count: value.regenerate_count,
            rejected_count: value.rejected_count,
            items,
        }
    }
}

impl From<crate::application::production_item_review_service::ProductionReviewCandidateAsset>
    for ProductionReviewCandidateAssetView
{
    fn from(
        value: crate::application::production_item_review_service::ProductionReviewCandidateAsset,
    ) -> Self {
        Self {
            asset_id: value.asset_id,
            asset_type: value.asset_type,
            name: value.name,
            mime_type: value.mime_type,
            local_path: value.local_path,
            width: (value.width > 0).then_some(value.width),
            height: (value.height > 0).then_some(value.height),
            thumbnail_available: value.thumbnail_available,
            task_id: value.task_id,
            selected: value.selected,
            review_result: value.review_result,
        }
    }
}

fn productivity_context(
    item: &crate::application::production_item_review_service::ProductionReviewProductivityItem,
) -> ProductionReviewContextView {
    ProductionReviewContextView {
        shot_id: item.shot_id.clone(),
        stage: item.stage.clone(),
        context_hash: item.frozen_context.context_hash.clone(),
        snapshot_available: item.frozen_context.snapshot_available,
        prompt_text: item.frozen_context.prompt_text.clone(),
        negative_prompt: item.frozen_context.negative_prompt.clone(),
        workflow_version_id: item
            .frozen_context
            .workflow_version_id
            .clone()
            .or_else(|| Some(item.workflow_version_id.clone())),
        recipe_id: item
            .frozen_context
            .recipe_id
            .clone()
            .or_else(|| Some(item.recipe_id.clone())),
        reference_sets: item
            .frozen_context
            .reference_sets
            .iter()
            .map(|reference_set| ProductionReviewReferenceSetView {
                id: reference_set.reference_set_id.clone(),
                name: reference_set.reference_set_id.clone(),
                role: reference_set.role.clone(),
                ordinal: reference_set.ordinal,
                required: reference_set.required,
            })
            .collect(),
        reference_assets: item
            .frozen_context
            .reference_assets
            .iter()
            .map(|asset| ProductionReviewReferenceAssetView {
                id: asset.asset_id.clone(),
                name: asset.asset_id.clone(),
                sha256: asset.sha256.clone(),
                role: asset.role.clone(),
                ordinal: asset.ordinal,
                source_reference_set_id: asset.source_reference_set_id.clone(),
            })
            .collect(),
        output_spec: item.frozen_context.output_spec.clone(),
        stage_input: item.frozen_context.stage_input.clone(),
        readiness_status: item.frozen_context.readiness_status.clone(),
    }
}

fn asset_summary_from_candidate(
    candidate: &crate::application::production_item_review_service::ProductionReviewCandidateAsset,
    created_at: &str,
) -> AssetSummaryView {
    AssetSummaryView {
        id: candidate.asset_id.clone(),
        asset_type: candidate.asset_type.clone(),
        category: if candidate.asset_type == "video" {
            "generated_video".to_owned()
        } else {
            "generated_image".to_owned()
        },
        name: candidate.name.clone(),
        original_name: candidate.name.clone(),
        mime_type: candidate.mime_type.clone(),
        width: (candidate.width > 0).then_some(candidate.width),
        height: (candidate.height > 0).then_some(candidate.height),
        duration_ms: None,
        file_size: 0,
        created_at: chrono::DateTime::parse_from_rfc3339(created_at)
            .map(|value| value.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now()),
        source_task_id: candidate.task_id.clone(),
        thumbnail_available: candidate.thumbnail_available,
        is_favorite: false,
        tags: Vec::new(),
    }
}

impl From<crate::application::production_item_review_service::ProductionReviewItem>
    for ProductionReviewItemView
{
    fn from(
        value: crate::application::production_item_review_service::ProductionReviewItem,
    ) -> Self {
        let task_status = value
            .task
            .as_ref()
            .map(|task| task.status.as_str().to_owned())
            .unwrap_or_else(|| "NOT_STARTED".to_owned());
        let review_status_value = value.review.as_ref().map(|review| review.review_status);
        let review_result_asset_id = value
            .review
            .as_ref()
            .and_then(|review| review.result_asset_id.clone());
        let (review_status, review_note, version, lineage_key, parent_batch_id, parent_item_id) =
            value
                .review
                .as_ref()
                .map(|review| {
                    (
                        review.review_status.as_str().to_owned(),
                        review.review_note.clone(),
                        Some(review.version),
                        Some(review.lineage_key.clone()),
                        review.parent_batch_id.clone(),
                        review.parent_item_id.clone(),
                    )
                })
                .unwrap_or_else(|| {
                    let failed = value.production_item_status_is_failed();
                    (
                        if failed { "FAILED" } else { "IN_PROGRESS" }.to_owned(),
                        String::new(),
                        None,
                        None,
                        None,
                        None,
                    )
                });
        let prompt_text = value_string(&value.item.values_json, |key| {
            key.to_ascii_lowercase().contains("prompt")
        });
        let duration_seconds = value_integer(&value.item.values_json, "duration_seconds");
        let width = value_integer(&value.item.values_json, "width");
        let height = value_integer(&value.item.values_json, "height");
        let seed = value
            .item
            .values_json
            .as_object()
            .and_then(|object| {
                object
                    .values()
                    .find(|value| value.get("type").and_then(Value::as_str) == Some("seed_fixed"))
            })
            .and_then(|value| value.get("value").and_then(Value::as_str))
            .map(ToOwned::to_owned);
        let workflow_version_id = value.item.workflow_version_id.clone();
        let recipe_id = value.item.recipe_id.clone();
        let quality_profile = if workflow_version_id.to_ascii_lowercase().contains("quality")
            || recipe_id.to_ascii_lowercase().contains("quality")
        {
            "QUALITY".to_owned()
        } else {
            "FAST".to_owned()
        };
        let reviewable = value.item.status.as_str() == "SUCCEEDED"
            && value
                .task
                .as_ref()
                .is_some_and(|task| task.status.as_str() == "SUCCEEDED")
            && review_status_value.is_some();
        let candidate_assets = value
            .output_assets
            .iter()
            .map(|asset| ProductionReviewCandidateAssetView {
                asset_id: asset.id.as_str().to_owned(),
                asset_type: asset.asset_type.as_str().to_owned(),
                name: asset.name.clone(),
                mime_type: asset.mime_type.clone(),
                local_path: Some(asset.storage_path.clone()),
                width: (asset.width > 0).then_some(asset.width),
                height: (asset.height > 0).then_some(asset.height),
                thumbnail_available: asset.thumbnail_path.is_some(),
                task_id: value.item.task_id.clone(),
                selected: false,
                review_result: (review_result_asset_id.as_deref() == Some(asset.id.as_str()))
                    .then(|| review_status.clone()),
            })
            .collect();
        let context = legacy_context(&value.item, None, None, reviewable, &prompt_text);
        Self {
            item_id: value.item.id.as_str().to_owned(),
            ordinal: value.item.ordinal,
            task_id: value.item.task_id,
            task_status,
            production_item_status: value.item.status.as_str().to_owned(),
            review_status,
            review_note,
            version,
            lineage_key,
            parent_batch_id,
            parent_item_id,
            preferred: value.is_preferred,
            workflow_version_id,
            recipe_id,
            prompt_text,
            seed,
            duration_seconds,
            width,
            height,
            quality_profile,
            created_at: value.item.created_at.to_rfc3339(),
            finished_at: value
                .task
                .and_then(|task| task.finished_at.map(|value| value.to_rfc3339())),
            output_assets: value
                .output_assets
                .into_iter()
                .map(AssetSummaryView::from)
                .collect(),
            shot_id: None,
            stage: None,
            selected_asset_id: None,
            reviewable,
            candidate_assets,
            context,
        }
    }
}

fn legacy_context(
    item: &crate::domain::ProductionBatchItem,
    shot_id: Option<String>,
    stage: Option<String>,
    _reviewable: bool,
    prompt_text: &Option<String>,
) -> ProductionReviewContextView {
    ProductionReviewContextView {
        shot_id,
        stage,
        context_hash: None,
        snapshot_available: false,
        prompt_text: prompt_text.clone(),
        negative_prompt: value_string(&item.values_json, |key| {
            key.to_ascii_lowercase().contains("negative")
        }),
        workflow_version_id: Some(item.workflow_version_id.clone()),
        recipe_id: Some(item.recipe_id.clone()),
        reference_sets: Vec::new(),
        reference_assets: Vec::new(),
        output_spec: serde_json::json!({
            "width": value_integer(&item.values_json, "width"),
            "height": value_integer(&item.values_json, "height"),
            "count": value_integer(&item.values_json, "count"),
            "durationSeconds": value_integer(&item.values_json, "duration_seconds")
                .map(|value| value as f64),
        }),
        stage_input: Value::Null,
        readiness_status: None,
    }
}

trait ProductionReviewItemStatusExt {
    fn production_item_status_is_failed(&self) -> bool;
}

impl ProductionReviewItemStatusExt
    for crate::application::production_item_review_service::ProductionReviewItem
{
    fn production_item_status_is_failed(&self) -> bool {
        self.item.status.as_str() == "FAILED"
            || self
                .task
                .as_ref()
                .is_some_and(|task| task.status.as_str() == "FAILED")
    }
}

impl From<crate::application::production_item_review_service::RegenerateResult>
    for ProductionReviewRegenerateView
{
    fn from(value: crate::application::production_item_review_service::RegenerateResult) -> Self {
        let selected_count = value.source_item_ids.len();
        Self {
            batch: value.detail.into(),
            source_item_ids: value.source_item_ids,
            selected_count,
            auto_started: value.auto_started,
            start_warning: value.start_warning,
        }
    }
}

fn value_string(values: &Value, predicate: impl Fn(&str) -> bool) -> Option<String> {
    let object = values.as_object()?;
    object
        .iter()
        .find(|(key, value)| {
            key.eq_ignore_ascii_case("prompt")
                && predicate(key)
                && value.get("type").and_then(Value::as_str) == Some("string")
        })
        .or_else(|| {
            object.iter().find(|(key, value)| {
                predicate(key) && value.get("type").and_then(Value::as_str) == Some("string")
            })
        })
        .and_then(|(_, value)| value.get("value").and_then(Value::as_str))
        .map(ToOwned::to_owned)
}

fn value_integer(values: &Value, key: &str) -> Option<i64> {
    values
        .get(key)
        .and_then(|value| value.get("value"))
        .and_then(Value::as_i64)
}

fn map_review_error(error: ProductionReviewError) -> AppError {
    match error {
        ProductionReviewError::Queue(error) => super::production_queue::map_queue_error(error),
        ProductionReviewError::InvalidInput(message)
        | ProductionReviewError::InvalidState(message)
        | ProductionReviewError::NotFound(message) => AppError::invalid_input(message),
        ProductionReviewError::Repository(error) => super::map_repository_error(&error),
    }
}
