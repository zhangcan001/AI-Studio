use crate::application::generation_input_preparer::GenerationInputValue;
use crate::application::h3_local_import_service::is_supported_h3_output_resolution;
use crate::application::ports::{
    AssetRepository, Clock, ProductionItemReviewRecord, ProductionItemReviewRepository,
    ProductionQueueRepository, RepositoryError, ShotBatchRepository, TaskRepository,
};
use crate::application::production_queue_service::{
    generation_values_from_json, CreateProductionBatchItem, CreateProductionBatchRequest,
    ProductionQueueError, ProductionQueueService,
};
use crate::domain::{
    Asset, AssetId, AssetType, PreparationSnapshotRecord, ProductionBatchDetail,
    ProductionBatchItem, ProductionBatchItemStatus, ProductionReviewStatus, SeedValue, ShotStage,
    Task, TaskId, TaskStatus,
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    error::Error,
    fmt,
    path::PathBuf,
    sync::Arc,
};
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewReferenceSetSummary {
    pub reference_set_id: String,
    pub role: String,
    pub ordinal: i64,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewReferenceAssetSummary {
    pub asset_id: String,
    pub sha256: String,
    pub role: String,
    pub ordinal: i64,
    pub source_reference_set_id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProductionReviewFrozenContext {
    pub snapshot_available: bool,
    pub context_hash: Option<String>,
    pub prompt_text: Option<String>,
    pub negative_prompt: Option<String>,
    pub workflow_version_id: Option<String>,
    pub recipe_id: Option<String>,
    pub reference_sets: Vec<ReviewReferenceSetSummary>,
    pub reference_assets: Vec<ReviewReferenceAssetSummary>,
    pub output_spec: Value,
    pub stage_input: Value,
    pub readiness_status: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProductionReviewCandidateAsset {
    pub asset_id: String,
    pub asset_type: String,
    pub name: String,
    pub mime_type: String,
    pub local_path: Option<String>,
    pub width: u32,
    pub height: u32,
    pub thumbnail_available: bool,
    pub task_id: Option<String>,
    pub selected: bool,
    pub review_result: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProductionReviewProductivityItem {
    pub item_id: String,
    pub ordinal: u32,
    pub task_id: Option<String>,
    pub production_item_status: String,
    pub task_status: Option<String>,
    pub review_status: Option<String>,
    pub review_note: String,
    pub version: Option<i64>,
    pub preferred: bool,
    pub shot_id: Option<String>,
    pub stage: Option<String>,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub candidate_assets: Vec<ProductionReviewCandidateAsset>,
    pub frozen_context: ProductionReviewFrozenContext,
    pub reviewable: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProductionReviewProductivityView {
    pub detail: ProductionBatchDetail,
    pub total: usize,
    pub success_count: usize,
    pub failed_count: usize,
    pub unreviewed_count: usize,
    pub approved_count: usize,
    pub starred_count: usize,
    pub regenerate_count: usize,
    pub rejected_count: usize,
    pub items: Vec<ProductionReviewProductivityItem>,
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
    shot_batch_repository: Option<Arc<dyn ShotBatchRepository>>,
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
            shot_batch_repository: None,
            clock,
        }
    }

    pub fn new_with_shot_batch_repository(
        review_repository: Arc<dyn ProductionItemReviewRepository>,
        production_queue_repository: Arc<dyn ProductionQueueRepository>,
        production_queue_service: Arc<ProductionQueueService>,
        task_repository: Arc<dyn TaskRepository>,
        asset_repository: Arc<dyn AssetRepository>,
        shot_batch_repository: Arc<dyn ShotBatchRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            review_repository,
            production_queue_repository,
            production_queue_service,
            task_repository,
            asset_repository,
            shot_batch_repository: Some(shot_batch_repository),
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

    pub async fn get_productivity_view(
        &self,
        project_id: &str,
        batch_id: &str,
    ) -> Result<ProductionReviewProductivityView, ProductionReviewError> {
        let detail = self
            .production_queue_service
            .get(project_id, batch_id)
            .await?;
        self.build_productivity_view(project_id, detail).await
    }

    /// Explicit facade name for callers building a review productivity board.
    pub async fn get_productivity(
        &self,
        project_id: &str,
        batch_id: &str,
    ) -> Result<ProductionReviewProductivityView, ProductionReviewError> {
        self.get_productivity_view(project_id, batch_id).await
    }

    /// Resolve an output candidate from the database before allowing a desktop
    /// open action. The caller supplies identifiers only; the path is never
    /// accepted from the client.
    pub async fn resolve_candidate_asset_path(
        &self,
        project_id: &str,
        batch_id: &str,
        item_id: &str,
        asset_id: &str,
    ) -> Result<PathBuf, ProductionReviewError> {
        let detail = self
            .production_queue_service
            .get(project_id, batch_id)
            .await?;
        let item = detail
            .items
            .iter()
            .find(|item| item.id.as_str() == item_id)
            .ok_or_else(|| ProductionReviewError::NotFound(item_id.to_owned()))?;
        let task_id = item
            .task_id
            .as_deref()
            .ok_or_else(|| ProductionReviewError::NotFound(asset_id.to_owned()))
            .and_then(|task_id| {
                TaskId::parse(task_id.to_owned())
                    .map_err(|error| ProductionReviewError::InvalidState(error.to_string()))
            })?;
        let asset_id = AssetId::parse(asset_id.to_owned())
            .map_err(|error| ProductionReviewError::InvalidInput(error.to_string()))?;
        let asset = self
            .asset_repository
            .find_by_id(&asset_id)
            .await?
            .ok_or_else(|| ProductionReviewError::NotFound(asset_id.as_str().to_owned()))?;
        if asset.project_id != project_id || asset.source_task_id.as_ref() != Some(&task_id) {
            return Err(ProductionReviewError::NotFound(
                asset_id.as_str().to_owned(),
            ));
        }
        let path = PathBuf::from(asset.storage_path);
        if !path.is_absolute() {
            return Err(ProductionReviewError::InvalidState(
                "Asset storage path must be absolute before it can be opened.".to_owned(),
            ));
        }
        Ok(path)
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
        let (items, _) = self.build_review_items(project_id, &detail).await?;
        Ok(ProductionBatchReview { detail, items })
    }

    async fn build_review_items(
        &self,
        project_id: &str,
        detail: &ProductionBatchDetail,
    ) -> Result<
        (
            Vec<ProductionReviewItem>,
            Vec<ProductionReviewProductivityItem>,
        ),
        ProductionReviewError,
    > {
        let mut task_ids = Vec::new();
        let mut seen_task_ids = HashSet::new();
        for item in &detail.items {
            if let Some(task_id) = &item.task_id {
                let task_id = TaskId::parse(task_id.clone())
                    .map_err(|error| ProductionReviewError::InvalidState(error.to_string()))?;
                if seen_task_ids.insert(task_id.as_str().to_owned()) {
                    task_ids.push(task_id);
                }
            }
        }
        let tasks = self
            .task_repository
            .find_many_by_ids(&task_ids)
            .await?
            .into_iter()
            .map(|task| (task.id.as_str().to_owned(), task))
            .collect::<HashMap<_, _>>();
        let assets = self
            .asset_repository
            .list_by_source_tasks(&task_ids)
            .await?;
        let assets_by_task =
            assets
                .into_iter()
                .fold(HashMap::<String, Vec<Asset>>::new(), |mut map, asset| {
                    if let Some(task_id) = &asset.source_task_id {
                        map.entry(task_id.as_str().to_owned())
                            .or_default()
                            .push(asset);
                    }
                    map
                });

        let mut reviews = self
            .review_repository
            .list_for_batch(project_id, detail.batch.id.as_str())
            .await?;
        let mut reviews_by_item = reviews
            .iter()
            .cloned()
            .map(|review| (review.production_batch_item_id.clone(), review))
            .collect::<HashMap<_, _>>();
        let now = self.clock.now();
        let missing_reviews = detail
            .items
            .iter()
            .filter(|item| {
                item.status == ProductionBatchItemStatus::Succeeded
                    && item
                        .task_id
                        .as_deref()
                        .and_then(|task_id| tasks.get(task_id))
                        .is_some_and(|task| task.status == TaskStatus::Succeeded)
                    && !reviews_by_item.contains_key(item.id.as_str())
            })
            .map(|item| {
                let result_asset_id = item
                    .task_id
                    .as_deref()
                    .and_then(|task_id| assets_by_task.get(task_id))
                    .into_iter()
                    .flatten()
                    .find(|asset| asset.asset_type == AssetType::Video)
                    .map(|asset| asset.id.as_str().to_owned());
                ProductionItemReviewRecord {
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
                }
            })
            .collect::<Vec<_>>();
        if !missing_reviews.is_empty() {
            let ensured = self
                .review_repository
                .ensure_for_items(&missing_reviews)
                .await?;
            for review in ensured {
                reviews_by_item.insert(review.production_batch_item_id.clone(), review.clone());
                reviews.push(review);
            }
        }

        let lineage_keys = reviews
            .iter()
            .map(|review| review.lineage_key.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let lineage_reviews = self
            .review_repository
            .list_for_lineages(project_id, &lineage_keys)
            .await?;
        let mut preferred_by_lineage = HashMap::new();
        for lineage_key in &lineage_keys {
            let lineage = lineage_reviews
                .iter()
                .chain(reviews.iter())
                .filter(|review| &review.lineage_key == lineage_key)
                .cloned()
                .collect::<Vec<_>>();
            if let Some(preferred) = preferred_review_id(&lineage) {
                preferred_by_lineage.insert(lineage_key.clone(), preferred.to_owned());
            }
        }

        let (snapshots, shot_links) = if let Some(repository) = &self.shot_batch_repository {
            tokio::try_join!(
                repository
                    .list_preparation_snapshots_for_batch(project_id, detail.batch.id.as_str(),),
                repository.list_shot_links_for_batch(project_id, detail.batch.id.as_str()),
            )?
        } else {
            (Vec::new(), Vec::new())
        };
        let snapshots_by_item = snapshots
            .into_iter()
            .map(|snapshot| (snapshot.production_batch_item_id.clone(), snapshot))
            .collect::<HashMap<_, _>>();
        let links_by_item = shot_links
            .into_iter()
            .map(|link| (link.production_batch_item_id.clone(), link))
            .collect::<HashMap<_, _>>();

        let mut old_items = Vec::with_capacity(detail.items.len());
        let mut productivity_items = Vec::with_capacity(detail.items.len());
        for item in &detail.items {
            let task = item
                .task_id
                .as_deref()
                .and_then(|task_id| tasks.get(task_id).cloned());
            let output_assets = task
                .as_ref()
                .filter(|task| task.status == TaskStatus::Succeeded)
                .and_then(|task| assets_by_task.get(task.id.as_str()))
                .cloned()
                .unwrap_or_default();
            let review = reviews_by_item.get(item.id.as_str()).cloned();
            let reviewable = item.status == ProductionBatchItemStatus::Succeeded
                && task
                    .as_ref()
                    .is_some_and(|task| task.status == TaskStatus::Succeeded)
                && review.is_some();
            let preferred = review
                .as_ref()
                .and_then(|review| preferred_by_lineage.get(&review.lineage_key))
                .is_some_and(|id| review.as_ref().is_some_and(|review| id == &review.id));
            let link = links_by_item.get(item.id.as_str());
            let snapshot = snapshots_by_item.get(item.id.as_str());
            let mut frozen_context = snapshot
                .map(snapshot_context)
                .unwrap_or_else(|| legacy_context(item));
            if frozen_context.workflow_version_id.is_none() {
                frozen_context.workflow_version_id = Some(item.workflow_version_id.clone());
            }
            if frozen_context.recipe_id.is_none() {
                frozen_context.recipe_id = Some(item.recipe_id.clone());
            }
            let selected_asset_id = match link {
                Some(link) => match link.stage {
                    ShotStage::Image => link.selected_image_asset_id.as_deref(),
                    ShotStage::Video => link.selected_video_asset_id.as_deref(),
                },
                None => review
                    .as_ref()
                    .and_then(|review| review.result_asset_id.as_deref()),
            };
            let candidate_assets = output_assets
                .iter()
                .map(|asset| ProductionReviewCandidateAsset {
                    asset_id: asset.id.as_str().to_owned(),
                    asset_type: asset.asset_type.as_str().to_owned(),
                    name: asset.name.clone(),
                    mime_type: asset.mime_type.clone(),
                    local_path: Some(asset.storage_path.clone()),
                    width: asset.width,
                    height: asset.height,
                    thumbnail_available: asset.thumbnail_path.is_some(),
                    task_id: asset
                        .source_task_id
                        .as_ref()
                        .map(|task_id| task_id.as_str().to_owned()),
                    selected: selected_asset_id == Some(asset.id.as_str()),
                    review_result: review
                        .as_ref()
                        .filter(|review| {
                            review.result_asset_id.as_deref() == Some(asset.id.as_str())
                        })
                        .map(|review| review.review_status.as_str().to_owned()),
                })
                .collect();
            old_items.push(ProductionReviewItem {
                item: item.clone(),
                task: task.clone(),
                output_assets,
                review: review.clone(),
                is_preferred: preferred,
            });
            productivity_items.push(ProductionReviewProductivityItem {
                item_id: item.id.as_str().to_owned(),
                ordinal: item.ordinal,
                task_id: item.task_id.clone(),
                production_item_status: item.status.as_str().to_owned(),
                task_status: task.as_ref().map(|task| task.status.as_str().to_owned()),
                review_status: if reviewable {
                    review
                        .as_ref()
                        .map(|review| review.review_status.as_str().to_owned())
                } else {
                    None
                },
                review_note: review
                    .as_ref()
                    .map(|review| review.review_note.clone())
                    .unwrap_or_default(),
                version: review
                    .as_ref()
                    .filter(|_| reviewable)
                    .map(|review| review.version),
                preferred,
                shot_id: link
                    .map(|link| link.shot_id.clone())
                    .or_else(|| snapshot.map(|snapshot| snapshot.shot_id.clone())),
                stage: link
                    .map(|link| link.stage.as_str().to_owned())
                    .or_else(|| snapshot.map(|snapshot| snapshot.stage.as_str().to_owned())),
                workflow_version_id: frozen_context
                    .workflow_version_id
                    .clone()
                    .unwrap_or_else(|| item.workflow_version_id.clone()),
                recipe_id: frozen_context
                    .recipe_id
                    .clone()
                    .unwrap_or_else(|| item.recipe_id.clone()),
                candidate_assets,
                frozen_context,
                reviewable,
            });
        }
        Ok((old_items, productivity_items))
    }

    async fn build_productivity_view(
        &self,
        project_id: &str,
        detail: ProductionBatchDetail,
    ) -> Result<ProductionReviewProductivityView, ProductionReviewError> {
        let (_, items) = self.build_review_items(project_id, &detail).await?;
        let mut view = ProductionReviewProductivityView {
            detail,
            total: items.len(),
            success_count: 0,
            failed_count: 0,
            unreviewed_count: 0,
            approved_count: 0,
            starred_count: 0,
            regenerate_count: 0,
            rejected_count: 0,
            items,
        };
        for item in &view.items {
            if item.production_item_status == ProductionBatchItemStatus::Succeeded.as_str()
                && item.task_status.as_deref() == Some(TaskStatus::Succeeded.as_str())
            {
                view.success_count += 1;
            } else if item.production_item_status == ProductionBatchItemStatus::Failed.as_str()
                || item.task_status.as_deref() == Some(TaskStatus::Failed.as_str())
            {
                view.failed_count += 1;
            }
            match item.review_status.as_deref() {
                Some("UNREVIEWED") => view.unreviewed_count += 1,
                Some("APPROVED") => view.approved_count += 1,
                Some("STARRED") => view.starred_count += 1,
                Some("REGENERATE") => view.regenerate_count += 1,
                Some("REJECTED") => view.rejected_count += 1,
                _ => {}
            }
        }
        Ok(view)
    }
}

fn snapshot_context(record: &PreparationSnapshotRecord) -> ProductionReviewFrozenContext {
    let snapshot = &record.snapshot;
    ProductionReviewFrozenContext {
        snapshot_available: true,
        context_hash: Some(record.context_hash.clone()),
        prompt_text: Some(snapshot.prompt.rendered_text.clone()),
        negative_prompt: Some(snapshot.prompt.negative_prompt.clone()),
        workflow_version_id: snapshot.workflow.workflow_version_id.clone(),
        recipe_id: snapshot.workflow.recipe_id.clone(),
        reference_sets: snapshot
            .reference_sets
            .iter()
            .map(|reference_set| ReviewReferenceSetSummary {
                reference_set_id: reference_set.reference_set_id.clone(),
                role: reference_set.role.as_str().to_owned(),
                ordinal: reference_set.ordinal,
                required: reference_set.required,
            })
            .collect(),
        reference_assets: snapshot
            .reference_assets
            .iter()
            .map(|asset| ReviewReferenceAssetSummary {
                asset_id: asset.asset_id.clone(),
                sha256: asset.sha256.clone(),
                role: asset.role.as_str().to_owned(),
                ordinal: asset.ordinal,
                source_reference_set_id: asset.source_reference_set_id.clone(),
            })
            .collect(),
        output_spec: json!({
            "width": snapshot.output_spec.width,
            "height": snapshot.output_spec.height,
            "count": snapshot.output_spec.count,
            "durationSeconds": snapshot.output_spec.duration_seconds,
        }),
        stage_input: json!({
            "selectedImageAssetId": snapshot.stage_input.selected_image_asset_id,
            "selectedImageSha256": snapshot.stage_input.selected_image_sha256,
        }),
        readiness_status: Some(snapshot.readiness.status.as_str().to_owned()),
    }
}

fn legacy_context(item: &ProductionBatchItem) -> ProductionReviewFrozenContext {
    let values = &item.values_json;
    let prompt_text = legacy_string_value(values, |key| key.eq_ignore_ascii_case("prompt"))
        .or_else(|| legacy_string_value(values, |key| key.to_ascii_lowercase().contains("prompt")));
    let negative_prompt = legacy_string_value(values, |key| {
        let key = key.to_ascii_lowercase();
        key == "negative_prompt" || key == "negativeprompt"
    });
    let selected_image_asset_id = legacy_string_value(values, |key| {
        matches!(
            key.to_ascii_lowercase().as_str(),
            "selected_image_asset_id" | "selectedimageassetid" | "image_asset_id"
        )
    });
    let reference_assets = values
        .get("reference_images")
        .or_else(|| values.get("referenceImages"))
        .and_then(|value| value.get("value").or(Some(value)))
        .and_then(|value| value.get("assetIds"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .enumerate()
        .map(|(ordinal, asset_id)| ReviewReferenceAssetSummary {
            asset_id: asset_id.to_owned(),
            sha256: String::new(),
            role: "REFERENCE".to_owned(),
            ordinal: ordinal as i64,
            source_reference_set_id: String::new(),
        })
        .collect();
    ProductionReviewFrozenContext {
        snapshot_available: false,
        context_hash: None,
        prompt_text,
        negative_prompt,
        workflow_version_id: Some(item.workflow_version_id.clone()),
        recipe_id: Some(item.recipe_id.clone()),
        reference_sets: Vec::new(),
        reference_assets,
        output_spec: json!({
            "width": legacy_integer_value(values, "width"),
            "height": legacy_integer_value(values, "height"),
            "durationSeconds": legacy_integer_value(values, "duration_seconds"),
        }),
        stage_input: json!({
            "selectedImageAssetId": selected_image_asset_id,
        }),
        readiness_status: None,
    }
}

fn legacy_string_value(values: &Value, predicate: impl Fn(&str) -> bool) -> Option<String> {
    values.as_object()?.iter().find_map(|(key, value)| {
        if !predicate(key) {
            return None;
        }
        value
            .get("value")
            .and_then(Value::as_str)
            .or_else(|| value.as_str())
            .map(ToOwned::to_owned)
    })
}

fn legacy_integer_value(values: &Value, key: &str) -> Option<i64> {
    values
        .get(key)
        .and_then(|value| value.get("value").or(Some(value)))
        .and_then(Value::as_i64)
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
