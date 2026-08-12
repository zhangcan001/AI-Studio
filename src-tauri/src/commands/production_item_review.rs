use crate::{
    app_state::AppState,
    application::asset_query_service::AssetSummaryView,
    application::production_item_review_service::{
        ProductionBatchReview, ProductionReviewError, RegenerateRequest,
    },
    domain::ProductionReviewStatus,
    error::AppError,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

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
        .map(Into::into)
        .map_err(map_review_error)
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
            auto_start: request.auto_start,
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
        .regenerate_marked(&request.project_id, &request.batch_id, request.auto_start)
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
        }
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
