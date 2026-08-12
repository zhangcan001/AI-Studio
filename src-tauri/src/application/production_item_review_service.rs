use crate::application::generation_input_preparer::GenerationInputValue;
use crate::application::h3_local_import_service::is_supported_h3_output_resolution;
use crate::application::ports::{
    AssetRepository, Clock, ProductionItemReviewRecord, ProductionItemReviewRepository,
    ProductionQueueRepository, RepositoryError, TaskRepository,
};
use crate::application::production_queue_service::{
    generation_values_from_json, CreateProductionBatchItem, CreateProductionBatchRequest,
    ProductionQueueError, ProductionQueueService,
};
use crate::domain::{
    Asset, AssetType, ProductionBatchDetail, ProductionBatchItem, ProductionBatchItemStatus,
    ProductionReviewStatus, SeedValue, Task, TaskId, TaskStatus,
};
use std::{collections::BTreeMap, error::Error, fmt, sync::Arc};
use uuid::Uuid;

pub const MAX_REVIEW_NOTE_BYTES: usize = 4 * 1024;

#[derive(Clone, Debug)]
pub struct ProductionReviewItem {
    pub item: ProductionBatchItem,
    pub task: Option<Task>,
    pub output_assets: Vec<Asset>,
    pub review: Option<ProductionItemReviewRecord>,
    pub is_preferred: bool,
}

#[derive(Clone, Debug)]
pub struct ProductionBatchReview {
    pub detail: ProductionBatchDetail,
    pub items: Vec<ProductionReviewItem>,
}

#[derive(Clone, Debug)]
pub struct RegenerateRequest {
    pub project_id: String,
    pub batch_id: String,
    pub item_id: String,
    pub prompt_override: Option<String>,
    pub duration_seconds: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub use_original_seed: bool,
    pub auto_start: bool,
}

#[derive(Clone, Debug)]
pub struct RegenerateResult {
    pub detail: ProductionBatchDetail,
    pub source_item_ids: Vec<String>,
    pub auto_started: bool,
    pub start_warning: Option<String>,
}

pub struct ProductionItemReviewService {
    review_repository: Arc<dyn ProductionItemReviewRepository>,
    production_queue_repository: Arc<dyn ProductionQueueRepository>,
    production_queue_service: Arc<ProductionQueueService>,
    task_repository: Arc<dyn TaskRepository>,
    asset_repository: Arc<dyn AssetRepository>,
    clock: Arc<dyn Clock>,
}

impl ProductionItemReviewService {
    pub fn new(
        review_repository: Arc<dyn ProductionItemReviewRepository>,
        production_queue_repository: Arc<dyn ProductionQueueRepository>,
        production_queue_service: Arc<ProductionQueueService>,
        task_repository: Arc<dyn TaskRepository>,
        asset_repository: Arc<dyn AssetRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            review_repository,
            production_queue_repository,
            production_queue_service,
            task_repository,
            asset_repository,
            clock,
        }
    }

    pub async fn get(
        &self,
        project_id: &str,
        batch_id: &str,
    ) -> Result<ProductionBatchReview, ProductionReviewError> {
        let detail = self
            .production_queue_service
            .get(project_id, batch_id)
            .await?;
        self.build_view(project_id, detail).await
    }

    pub async fn set_status(
        &self,
        project_id: &str,
        batch_id: &str,
        item_id: &str,
        status: ProductionReviewStatus,
    ) -> Result<ProductionBatchReview, ProductionReviewError> {
        let view = self.get(project_id, batch_id).await?;
        let item = view
            .items
            .iter()
            .find(|item| item.item.id.as_str() == item_id)
            .ok_or_else(|| ProductionReviewError::NotFound(item_id.to_owned()))?;
        ensure_reviewable_item(item)?;
        self.review_repository
            .set_status(project_id, item_id, status, self.clock.now())
            .await?;
        self.get(project_id, batch_id).await
    }

    pub async fn set_note(
        &self,
        project_id: &str,
        batch_id: &str,
        item_id: &str,
        note: String,
    ) -> Result<ProductionBatchReview, ProductionReviewError> {
        if note.as_bytes().len() > MAX_REVIEW_NOTE_BYTES {
            return Err(ProductionReviewError::InvalidInput(format!(
                "审片备注不能超过 {} KiB",
                MAX_REVIEW_NOTE_BYTES / 1024
            )));
        }
        let view = self.get(project_id, batch_id).await?;
        let item = view
            .items
            .iter()
            .find(|item| item.item.id.as_str() == item_id)
            .ok_or_else(|| ProductionReviewError::NotFound(item_id.to_owned()))?;
        ensure_reviewable_item(item)?;
        self.review_repository
            .set_note(project_id, item_id, &note, self.clock.now())
            .await?;
        self.get(project_id, batch_id).await
    }

    pub async fn regenerate_item(
        &self,
        request: RegenerateRequest,
    ) -> Result<RegenerateResult, ProductionReviewError> {
        let detail = self
            .production_queue_service
            .get(&request.project_id, &request.batch_id)
            .await?;
        let source = detail
            .items
            .iter()
            .find(|item| item.id.as_str() == request.item_id)
            .cloned()
            .ok_or_else(|| ProductionReviewError::NotFound(request.item_id.clone()))?;
        if source.status != ProductionBatchItemStatus::Succeeded {
            return Err(ProductionReviewError::InvalidState(
                "只有成功的视频结果才能进入审片重生成。".to_owned(),
            ));
        }
        let review = self
            .get(&request.project_id, &request.batch_id)
            .await?
            .items
            .into_iter()
            .find(|item| item.item.id == source.id)
            .ok_or_else(|| ProductionReviewError::NotFound(source.id.as_str().to_owned()))?;
        ensure_reviewable_item(&review)?;
        let values = prepare_regeneration_values(
            &source.values_json,
            &request.prompt_override,
            request.duration_seconds,
            request.width,
            request.height,
            request.use_original_seed,
        )?;
        self.create_regeneration(
            &request.project_id,
            &detail,
            vec![(
                source,
                review.review.expect("reviewable item has review"),
                values,
            )],
            request.auto_start,
        )
        .await
    }

    pub async fn regenerate_marked(
        &self,
        project_id: &str,
        batch_id: &str,
        auto_start: bool,
    ) -> Result<RegenerateResult, ProductionReviewError> {
        let detail = self
            .production_queue_service
            .get(project_id, batch_id)
            .await?;
        let view = self.get(project_id, batch_id).await?;
        let mut selected = Vec::new();
        for item in view.items {
            if item.review.as_ref().map(|review| review.review_status)
                != Some(ProductionReviewStatus::Regenerate)
            {
                continue;
            }
            ensure_reviewable_item(&item)?;
            let values = prepare_regeneration_values(
                &item.item.values_json,
                &None,
                None,
                None,
                None,
                false,
            )?;
            selected.push((
                item.item,
                item.review.expect("reviewable item has review"),
                values,
            ));
        }
        if selected.is_empty() {
            return Err(ProductionReviewError::InvalidState(
                "当前没有标记为待重生成的成功结果。".to_owned(),
            ));
        }
        self.create_regeneration(project_id, &detail, selected, auto_start)
            .await
    }

    async fn create_regeneration(
        &self,
        project_id: &str,
        source_detail: &ProductionBatchDetail,
        selected: Vec<(
            ProductionBatchItem,
            ProductionItemReviewRecord,
            BTreeMap<String, GenerationInputValue>,
        )>,
        auto_start: bool,
    ) -> Result<RegenerateResult, ProductionReviewError> {
        let rework_index = self
            .next_rework_index(project_id, &source_detail.batch.name)
            .await?;
        let items = selected
            .iter()
            .map(|(source, _, values)| CreateProductionBatchItem {
                workflow_version_id: source.workflow_version_id.clone(),
                recipe_id: source.recipe_id.clone(),
                values: values.clone(),
            })
            .collect();
        let detail = self
            .production_queue_service
            .create(CreateProductionBatchRequest {
                project_id: project_id.to_owned(),
                name: format!("{} · 返工 {}", source_detail.batch.name, rework_index),
                continue_on_failure: source_detail.batch.continue_on_failure,
                items,
            })
            .await?;

        for ((source, source_review, _), regenerated) in selected.iter().zip(&detail.items) {
            let lineage = self
                .review_repository
                .list_for_lineage(project_id, &source_review.lineage_key)
                .await?;
            let version = lineage.iter().map(|item| item.version).max().unwrap_or(0) + 1;
            let now = self.clock.now();
            self.review_repository
                .insert(&ProductionItemReviewRecord {
                    id: format!("pri_{}", Uuid::new_v4().simple()),
                    project_id: project_id.to_owned(),
                    production_batch_id: detail.batch.id.as_str().to_owned(),
                    production_batch_item_id: regenerated.id.as_str().to_owned(),
                    task_id: None,
                    result_asset_id: None,
                    review_status: ProductionReviewStatus::Unreviewed,
                    review_note: String::new(),
                    version,
                    lineage_key: source_review.lineage_key.clone(),
                    parent_batch_id: Some(source.batch_id()),
                    parent_item_id: Some(source.id.as_str().to_owned()),
                    created_at: now,
                    updated_at: now,
                })
                .await?;
        }

        let mut auto_started = false;
        let mut start_warning = None;
        if auto_start {
            match self
                .production_queue_service
                .start(project_id, detail.batch.id.as_str())
                .await
            {
                Ok(()) => auto_started = true,
                Err(error) => {
                    start_warning = Some(format!("返工批次已创建，但自动开始失败：{error}"))
                }
            }
        }
        Ok(RegenerateResult {
            detail,
            source_item_ids: selected
                .iter()
                .map(|(item, _, _)| item.id.as_str().to_owned())
                .collect(),
            auto_started,
            start_warning,
        })
    }

    async fn next_rework_index(
        &self,
        project_id: &str,
        source_name: &str,
    ) -> Result<u32, ProductionReviewError> {
        let prefix = format!("{source_name} · 返工 ");
        let batches = self.production_queue_repository.list(project_id).await?;
        Ok(batches
            .iter()
            .filter_map(|batch| batch.name.strip_prefix(&prefix))
            .filter_map(|suffix| suffix.parse::<u32>().ok())
            .max()
            .unwrap_or(0)
            .saturating_add(1))
    }

    async fn build_view(
        &self,
        project_id: &str,
        detail: ProductionBatchDetail,
    ) -> Result<ProductionBatchReview, ProductionReviewError> {
        let mut items = Vec::with_capacity(detail.items.len());
        for item in &detail.items {
            let task = match item.task_id.as_deref() {
                Some(task_id) => {
                    let task_id = TaskId::parse(task_id.to_owned())
                        .map_err(|error| ProductionReviewError::InvalidState(error.to_string()))?;
                    self.task_repository.find_by_id(&task_id).await?
                }
                None => None,
            };
            let output_assets = match task.as_ref() {
                Some(task) if task.status == TaskStatus::Succeeded => {
                    self.asset_repository.list_by_source_task(&task.id).await?
                }
                _ => Vec::new(),
            };
            let review = if item.status == ProductionBatchItemStatus::Succeeded
                && task
                    .as_ref()
                    .is_some_and(|task| task.status == TaskStatus::Succeeded)
            {
                let now = self.clock.now();
                let result_asset_id = output_assets
                    .iter()
                    .find(|asset| asset.asset_type == AssetType::Video)
                    .map(|asset| asset.id.as_str().to_owned());
                Some(
                    self.review_repository
                        .ensure_for_item(&ProductionItemReviewRecord {
                            id: format!("pri_{}", Uuid::new_v4().simple()),
                            project_id: project_id.to_owned(),
                            production_batch_id: detail.batch.id.as_str().to_owned(),
                            production_batch_item_id: item.id.as_str().to_owned(),
                            task_id: item.task_id.clone(),
                            result_asset_id,
                            review_status: ProductionReviewStatus::Unreviewed,
                            review_note: String::new(),
                            version: 1,
                            lineage_key: item.id.as_str().to_owned(),
                            parent_batch_id: None,
                            parent_item_id: None,
                            created_at: now,
                            updated_at: now,
                        })
                        .await?,
                )
            } else {
                self.review_repository
                    .find_for_item(project_id, item.id.as_str())
                    .await?
            };
            let is_preferred = if let Some(review) = &review {
                let lineage = self
                    .review_repository
                    .list_for_lineage(project_id, &review.lineage_key)
                    .await?;
                preferred_review_id(&lineage) == Some(review.id.as_str())
            } else {
                false
            };
            items.push(ProductionReviewItem {
                item: item.clone(),
                task,
                output_assets,
                review,
                is_preferred,
            });
        }
        Ok(ProductionBatchReview { detail, items })
    }
}

fn ensure_reviewable_item(item: &ProductionReviewItem) -> Result<(), ProductionReviewError> {
    if item.item.status != ProductionBatchItemStatus::Succeeded
        || item
            .task
            .as_ref()
            .is_none_or(|task| task.status != TaskStatus::Succeeded)
    {
        return Err(ProductionReviewError::InvalidState(
            "失败或未完成的 Task 不能设置审片状态或备注。".to_owned(),
        ));
    }
    if item.review.is_none() {
        return Err(ProductionReviewError::InvalidState(
            "成功结果尚未生成可审片的版本记录。".to_owned(),
        ));
    }
    Ok(())
}

fn prepare_regeneration_values(
    values_json: &serde_json::Value,
    prompt_override: &Option<String>,
    duration_seconds: Option<i64>,
    width: Option<i64>,
    height: Option<i64>,
    use_original_seed: bool,
) -> Result<BTreeMap<String, GenerationInputValue>, ProductionReviewError> {
    let mut values =
        generation_values_from_json(values_json).map_err(ProductionReviewError::InvalidInput)?;
    if let Some(prompt) = prompt_override {
        let prompt = prompt.trim();
        if prompt.is_empty() {
            return Err(ProductionReviewError::InvalidInput(
                "重生成 Prompt 不能为空。".to_owned(),
            ));
        }
        let key = values
            .keys()
            .find(|key| key.eq_ignore_ascii_case("prompt"))
            .or_else(|| {
                values.keys().find(|key| {
                    key.to_ascii_lowercase().contains("prompt")
                        && matches!(values.get(*key), Some(GenerationInputValue::Text(_)))
                })
            })
            .cloned()
            .ok_or_else(|| {
                ProductionReviewError::InvalidInput("原任务没有可编辑的 Prompt。".to_owned())
            })?;
        values.insert(key, GenerationInputValue::Text(prompt.to_owned()));
    }
    if let Some(duration) = duration_seconds {
        if !(1..=15).contains(&duration) {
            return Err(ProductionReviewError::InvalidInput(
                "H3 时长必须在 1–15 秒范围内。".to_owned(),
            ));
        }
        replace_integer(&mut values, "duration_seconds", duration)?;
    }
    if width.is_some() || height.is_some() {
        let current_width = integer_value(&values, "width")?;
        let current_height = integer_value(&values, "height")?;
        let next_width = width.unwrap_or(current_width);
        let next_height = height.unwrap_or(current_height);
        if !is_supported_h3_output_resolution(next_width, next_height) {
            return Err(ProductionReviewError::InvalidInput(
                "H3 分辨率必须使用现有 14 档合法规格。".to_owned(),
            ));
        }
        replace_integer(&mut values, "width", next_width)?;
        replace_integer(&mut values, "height", next_height)?;
    }
    if !use_original_seed {
        for value in values.values_mut() {
            if matches!(value, GenerationInputValue::Seed(_)) {
                *value = GenerationInputValue::Seed(SeedValue::Random);
            }
        }
    }
    Ok(values)
}

fn integer_value(
    values: &BTreeMap<String, GenerationInputValue>,
    key: &str,
) -> Result<i64, ProductionReviewError> {
    values
        .get(key)
        .and_then(|value| match value {
            GenerationInputValue::Integer(value) => Some(*value),
            _ => None,
        })
        .ok_or_else(|| ProductionReviewError::InvalidInput(format!("原任务缺少 {key} 参数。")))
}

fn replace_integer(
    values: &mut BTreeMap<String, GenerationInputValue>,
    key: &str,
    value: i64,
) -> Result<(), ProductionReviewError> {
    let entry = values
        .get_mut(key)
        .ok_or_else(|| ProductionReviewError::InvalidInput(format!("原任务缺少 {key} 参数。")))?;
    if !matches!(entry, GenerationInputValue::Integer(_)) {
        return Err(ProductionReviewError::InvalidInput(format!(
            "原任务 {key} 参数类型无效。"
        )));
    }
    *entry = GenerationInputValue::Integer(value);
    Ok(())
}

fn preferred_review_id(reviews: &[ProductionItemReviewRecord]) -> Option<&str> {
    reviews
        .iter()
        .filter(|review| review.review_status == ProductionReviewStatus::Starred)
        .max_by_key(|review| review.version)
        .or_else(|| {
            reviews
                .iter()
                .filter(|review| review.review_status == ProductionReviewStatus::Approved)
                .max_by_key(|review| review.version)
        })
        .or_else(|| reviews.iter().max_by_key(|review| review.version))
        .map(|review| review.id.as_str())
}

trait BatchItemExt {
    fn batch_id(&self) -> String;
}

impl BatchItemExt for ProductionBatchItem {
    fn batch_id(&self) -> String {
        self.batch_id.as_str().to_owned()
    }
}

#[derive(Debug)]
pub enum ProductionReviewError {
    InvalidInput(String),
    InvalidState(String),
    NotFound(String),
    Queue(ProductionQueueError),
    Repository(RepositoryError),
}

impl fmt::Display for ProductionReviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(formatter, "INVALID_INPUT: {message}"),
            Self::InvalidState(message) => {
                write!(formatter, "PRODUCTION_REVIEW_INVALID_STATE: {message}")
            }
            Self::NotFound(id) => write!(formatter, "PRODUCTION_REVIEW_NOT_FOUND: {id}"),
            Self::Queue(error) => write!(formatter, "{error}"),
            Self::Repository(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for ProductionReviewError {}

impl From<RepositoryError> for ProductionReviewError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

impl From<ProductionQueueError> for ProductionReviewError {
    fn from(error: ProductionQueueError) -> Self {
        Self::Queue(error)
    }
}

#[cfg(test)]
mod tests {
    use super::prepare_regeneration_values;
    use crate::application::generation_input_preparer::GenerationInputValue;
    use crate::application::production_queue_service::generation_values_to_json;
    use crate::domain::{AssetId, SeedValue};
    use std::collections::BTreeMap;

    fn values() -> serde_json::Value {
        let mut values = BTreeMap::new();
        values.insert(
            "prompt".to_owned(),
            GenerationInputValue::Text("Prompt A".to_owned()),
        );
        values.insert(
            "duration_seconds".to_owned(),
            GenerationInputValue::Integer(10),
        );
        values.insert("width".to_owned(), GenerationInputValue::Integer(1344));
        values.insert("height".to_owned(), GenerationInputValue::Integer(768));
        values.insert(
            "seed".to_owned(),
            GenerationInputValue::Seed(SeedValue::Fixed(42)),
        );
        values.insert(
            "first_frame".to_owned(),
            GenerationInputValue::ImageAsset(AssetId::parse("ast_first").unwrap()),
        );
        values.insert(
            "last_frame".to_owned(),
            GenerationInputValue::ImageAsset(AssetId::parse("ast_last").unwrap()),
        );
        generation_values_to_json(&values)
    }

    #[test]
    fn regeneration_inherits_inputs_and_changes_seed_by_default() {
        let result =
            prepare_regeneration_values(&values(), &None, None, None, None, false).unwrap();
        assert_eq!(
            result["prompt"],
            GenerationInputValue::Text("Prompt A".to_owned())
        );
        assert_eq!(
            result["duration_seconds"],
            GenerationInputValue::Integer(10)
        );
        assert_eq!(result["width"], GenerationInputValue::Integer(1344));
        assert_eq!(
            result["first_frame"],
            GenerationInputValue::ImageAsset(AssetId::parse("ast_first").unwrap())
        );
        assert_eq!(
            result["seed"],
            GenerationInputValue::Seed(SeedValue::Random)
        );
    }

    #[test]
    fn regeneration_can_override_prompt_and_keep_original_seed() {
        let result = prepare_regeneration_values(
            &values(),
            &Some("Prompt B".to_owned()),
            Some(5),
            Some(960),
            Some(544),
            true,
        )
        .unwrap();
        assert_eq!(
            result["prompt"],
            GenerationInputValue::Text("Prompt B".to_owned())
        );
        assert_eq!(result["duration_seconds"], GenerationInputValue::Integer(5));
        assert_eq!(result["width"], GenerationInputValue::Integer(960));
        assert_eq!(result["height"], GenerationInputValue::Integer(544));
        assert_eq!(
            result["seed"],
            GenerationInputValue::Seed(SeedValue::Fixed(42))
        );
    }

    #[test]
    fn regeneration_rejects_unknown_h3_resolution() {
        let error = prepare_regeneration_values(&values(), &None, None, Some(1), Some(1), false)
            .expect_err("unsupported resolution must be rejected");
        assert!(error.to_string().contains("14 档"));
    }
}
