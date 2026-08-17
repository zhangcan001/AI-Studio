use crate::application::generation_input_preparer::{
    GenerationInputPreparer, GenerationInputValue,
};
use crate::application::ordered_reference_binding::{
    ref2va_image_bounds, validate_ordered_reference_ids,
};
use crate::application::ports::{
    AssetRepository, AvailableGenerationDefinition, Clock, GenerationDefinitionRepository,
    ProjectRepository, RepositoryError, ShotBatchBinding, ShotBatchRepository, ShotBulkRepository,
    ShotData, ShotRepository, TaskRepository,
};
use crate::application::product_runtime_scope::production_runtime_for_stage;
use crate::application::production_queue_service::{
    freeze_random_seed_values, generation_values_to_json,
};
use crate::application::shot_service::scalar_values_from_json;
use crate::compiler::{RecipeParser, WorkflowCompiler};
use crate::domain::{
    derive_stage_status, AssetId, AssetType, CompileRequest, InputDefinition, OutputType,
    ProductionBatch, ProductionBatchDetail, ProductionBatchItem, ProductionBatchItemId,
    ProductionBatchStatus, Recipe, ShotStage, TaskId, TaskStatus, WorkflowDocument,
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    error::Error,
    fmt,
    sync::Arc,
};

pub const MAX_SHOT_BATCH_ITEMS: usize = 100;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotBatchPlanView {
    pub project_id: String,
    pub stage: String,
    pub max_items: usize,
    pub eligible_count: usize,
    pub rows: Vec<ShotBatchPlanRow>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotBatchPlanRow {
    pub shot_id: String,
    pub ordinal: i64,
    pub name: String,
    pub stage: String,
    pub workflow_version_id: Option<String>,
    pub recipe_id: Option<String>,
    pub recipe_name: Option<String>,
    pub current_status: String,
    pub selected_image_asset_id: Option<String>,
    pub selected_video_asset_id: Option<String>,
    pub video_mode: Option<String>,
    pub reference_count: usize,
    pub reference_min: Option<usize>,
    pub reference_max: Option<usize>,
    pub eligible: bool,
    pub blocking_reasons: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateShotBatchRequest {
    pub project_id: String,
    pub stage: ShotStage,
    pub shot_ids: Vec<String>,
}

#[derive(Clone, Debug)]
struct PlannedShot {
    row: ShotBatchPlanRow,
    workflow_version_id: String,
    recipe_id: String,
    values: BTreeMap<String, GenerationInputValue>,
    recipe: Option<Recipe>,
}

pub struct ShotBatchService {
    shot_repository: Arc<dyn ShotRepository>,
    shot_batch_repository: Arc<dyn ShotBatchRepository>,
    task_repository: Arc<dyn TaskRepository>,
    asset_repository: Arc<dyn AssetRepository>,
    definition_repository: Arc<dyn GenerationDefinitionRepository>,
    project_repository: Arc<dyn ProjectRepository>,
    clock: Arc<dyn Clock>,
    stage_prompt_repository: Option<Arc<dyn ShotBulkRepository>>,
}

impl ShotBatchService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        shot_repository: Arc<dyn ShotRepository>,
        shot_batch_repository: Arc<dyn ShotBatchRepository>,
        task_repository: Arc<dyn TaskRepository>,
        asset_repository: Arc<dyn AssetRepository>,
        definition_repository: Arc<dyn GenerationDefinitionRepository>,
        project_repository: Arc<dyn ProjectRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            shot_repository,
            shot_batch_repository,
            task_repository,
            asset_repository,
            definition_repository,
            project_repository,
            clock,
            stage_prompt_repository: None,
        }
    }

    pub fn with_stage_prompt_repository(mut self, repository: Arc<dyn ShotBulkRepository>) -> Self {
        self.stage_prompt_repository = Some(repository);
        self
    }

    pub async fn plan(
        &self,
        project_id: &str,
        stage: ShotStage,
    ) -> Result<ShotBatchPlanView, ShotBatchServiceError> {
        validate_project(project_id)?;
        let planned = self.build_plan(project_id, stage).await?;
        let eligible_count = planned.iter().filter(|item| item.row.eligible).count();
        Ok(ShotBatchPlanView {
            project_id: project_id.to_owned(),
            stage: stage.as_str().to_owned(),
            max_items: MAX_SHOT_BATCH_ITEMS,
            eligible_count,
            rows: planned.into_iter().map(|item| item.row).collect(),
        })
    }

    pub async fn create(
        &self,
        request: CreateShotBatchRequest,
    ) -> Result<ProductionBatchDetail, ShotBatchServiceError> {
        validate_project(&request.project_id)?;
        if request.shot_ids.is_empty() || request.shot_ids.len() > MAX_SHOT_BATCH_ITEMS {
            return Err(ShotBatchServiceError::InvalidInput(format!(
                "批量生产每次必须选择 1–{} 个镜头",
                MAX_SHOT_BATCH_ITEMS
            )));
        }
        let selected_ids = request.shot_ids.iter().cloned().collect::<HashSet<_>>();
        if selected_ids.len() != request.shot_ids.len() {
            return Err(ShotBatchServiceError::InvalidInput(
                "批量生产不能重复选择同一个镜头".to_owned(),
            ));
        }
        let planned = self.build_plan(&request.project_id, request.stage).await?;
        let by_id = planned
            .into_iter()
            .map(|item| (item.row.shot_id.clone(), item))
            .collect::<HashMap<_, _>>();
        let mut selected = Vec::with_capacity(request.shot_ids.len());
        for shot_id in &request.shot_ids {
            let Some(item) = by_id.get(shot_id) else {
                return Err(ShotBatchServiceError::InvalidInput(format!(
                    "镜头 {shot_id} 不属于当前项目"
                )));
            };
            if !item.row.eligible {
                return Err(ShotBatchServiceError::InvalidInput(format!(
                    "镜头「{}」暂时不能加入批次：{}",
                    item.row.name,
                    item.row.blocking_reasons.join("；")
                )));
            }
            selected.push(item.clone());
        }
        selected.sort_by_key(|item| item.row.ordinal);

        let project = self
            .project_repository
            .find_by_id(&request.project_id)
            .await?
            .ok_or_else(|| ShotBatchServiceError::NotFound(request.project_id.clone()))?;
        let now = self.clock.now();
        let label = match request.stage {
            ShotStage::Image => "镜头关键帧",
            ShotStage::Video => "镜头视频",
        };
        let batch_id = crate::domain::ProductionBatchId::new();
        let batch = ProductionBatch {
            id: batch_id.clone(),
            project_id: request.project_id.clone(),
            name: format!(
                "{label} · {} · {}",
                project.name,
                now.format("%Y-%m-%d %H:%M:%S")
            ),
            status: ProductionBatchStatus::Ready,
            // H3 is intentionally strict-sequential but should still finish
            // the remaining selected Shots after one item fails.
            continue_on_failure: request.stage == ShotStage::Video,
            archived_at: None,
            created_at: now,
            updated_at: now,
        };
        let mut items = Vec::with_capacity(selected.len());
        let mut bindings = Vec::with_capacity(selected.len());
        for (ordinal, shot) in selected.iter().enumerate() {
            let item_id = ProductionBatchItemId::new();
            let recipe = shot.recipe.as_ref().ok_or_else(|| {
                ShotBatchServiceError::InvalidInput(format!(
                    "镜头「{}」缺少有效 Recipe",
                    shot.row.name
                ))
            })?;
            let values = freeze_shot_batch_values(request.stage, shot.values.clone(), recipe)
                .map_err(ShotBatchServiceError::InvalidInput)?;
            items.push(ProductionBatchItem {
                id: item_id.clone(),
                batch_id: batch_id.clone(),
                ordinal: u32::try_from(ordinal).map_err(|_| {
                    ShotBatchServiceError::InvalidInput("批次序号超出范围".to_owned())
                })?,
                workflow_version_id: shot.workflow_version_id.clone(),
                recipe_id: shot.recipe_id.clone(),
                values_json: generation_values_to_json(&values),
                status: crate::domain::ProductionBatchItemStatus::Pending,
                task_id: None,
                retry_of_item_id: None,
                error_code: None,
                error_message: None,
                created_at: now,
                updated_at: now,
            });
            bindings.push(ShotBatchBinding {
                shot_id: shot.row.shot_id.clone(),
                stage: request.stage,
                production_batch_item_id: item_id.as_str().to_owned(),
            });
        }
        self.shot_batch_repository
            .insert_batch_with_bindings(&batch, &items, &bindings)
            .await?;
        Ok(ProductionBatchDetail { batch, items })
    }

    async fn build_plan(
        &self,
        project_id: &str,
        stage: ShotStage,
    ) -> Result<Vec<PlannedShot>, ShotBatchServiceError> {
        let shots = self.shot_repository.list(project_id).await?;
        let available_definitions = self
            .definition_repository
            .list_available()
            .await?
            .into_iter()
            .map(|definition| {
                (
                    (
                        definition.workflow_version_id.clone(),
                        definition.recipe_id.clone(),
                    ),
                    definition,
                )
            })
            .collect::<HashMap<_, _>>();
        let mut task_statuses = HashMap::<String, TaskStatus>::new();
        let mut missing_tasks = HashSet::<String>::new();
        for data in &shots {
            for link in &data.generation_links {
                let Some(task_id) = link.task_id.as_deref() else {
                    continue;
                };
                if task_statuses.contains_key(task_id) || missing_tasks.contains(task_id) {
                    continue;
                }
                let parsed = TaskId::parse(task_id.to_owned())
                    .map_err(|error| ShotBatchServiceError::InvalidInput(error.to_string()))?;
                if let Some(task) = self.task_repository.find_by_id(&parsed).await? {
                    if task.project_id == project_id {
                        task_statuses.insert(task_id.to_owned(), task.status);
                    } else {
                        missing_tasks.insert(task_id.to_owned());
                    }
                } else {
                    missing_tasks.insert(task_id.to_owned());
                }
            }
        }
        let active_video_task = stage == ShotStage::Video
            && shots.iter().any(|data| {
                data.generation_links.iter().any(|link| {
                    link.stage == ShotStage::Video
                        && link
                            .task_id
                            .as_ref()
                            .and_then(|id| task_statuses.get(id))
                            .is_some_and(|status| !status.is_terminal())
                })
            });
        let stage_prompts = if let Some(repository) = &self.stage_prompt_repository {
            repository
                .list_bulk_data(project_id)
                .await?
                .into_iter()
                .map(|item| {
                    let prompt = item
                        .stage_prompts
                        .into_iter()
                        .find(|prompt| prompt.stage == stage)
                        .map(|prompt| prompt.prompt_text)
                        .unwrap_or(item.shot.prompt_text);
                    (item.shot.id, prompt)
                })
                .collect::<HashMap<_, _>>()
        } else {
            HashMap::new()
        };
        let mut result = Vec::with_capacity(shots.len());
        for data in shots {
            let stage_prompt = stage_prompts.get(&data.shot.id).map(String::as_str);
            result.push(
                self.inspect_shot(
                    project_id,
                    stage,
                    data,
                    &task_statuses,
                    &missing_tasks,
                    &available_definitions,
                    active_video_task,
                    stage_prompt,
                )
                .await?,
            );
        }
        Ok(result)
    }

    async fn inspect_shot(
        &self,
        project_id: &str,
        stage: ShotStage,
        data: ShotData,
        task_statuses: &HashMap<String, TaskStatus>,
        missing_tasks: &HashSet<String>,
        available_definitions: &HashMap<(String, String), AvailableGenerationDefinition>,
        active_video_task: bool,
        stage_prompt: Option<&str>,
    ) -> Result<PlannedShot, ShotBatchServiceError> {
        let config = data
            .stage_configs
            .iter()
            .find(|config| config.stage == stage);
        let selected = match stage {
            ShotStage::Image => data.shot.selected_image_asset_id.is_some(),
            ShotStage::Video => data.shot.selected_video_asset_id.is_some(),
        };
        let latest_task_status = data
            .generation_links
            .iter()
            .filter(|link| link.stage == stage)
            .filter_map(|link| link.task_id.as_ref().and_then(|id| task_statuses.get(id)))
            .copied()
            .next();
        let current_status =
            derive_stage_status(stage, config.is_some(), selected, latest_task_status);
        let has_active_task = data.generation_links.iter().any(|link| {
            link.stage == stage
                && link
                    .task_id
                    .as_ref()
                    .and_then(|id| task_statuses.get(id))
                    .is_some_and(|status| !status.is_terminal())
        });
        let mut reasons = Vec::new();
        if data.generation_links.iter().any(|link| {
            link.stage == stage
                && link
                    .task_id
                    .as_ref()
                    .is_some_and(|id| missing_tasks.contains(id))
        }) {
            reasons.push("关联任务记录缺失，需要先检查任务历史".to_owned());
        }
        if has_active_task {
            reasons.push("当前阶段已有任务运行中".to_owned());
        }
        if stage == ShotStage::Video && active_video_task {
            reasons.push("视频阶段严格串行：当前项目已有视频任务运行中".to_owned());
        }

        let mut row = ShotBatchPlanRow {
            shot_id: data.shot.id.clone(),
            ordinal: data.shot.ordinal,
            name: data.shot.name.clone(),
            stage: stage.as_str().to_owned(),
            workflow_version_id: config.map(|value| value.workflow_version_id.clone()),
            recipe_id: config.map(|value| value.recipe_id.clone()),
            recipe_name: None,
            current_status: current_status.as_str().to_owned(),
            selected_image_asset_id: data.shot.selected_image_asset_id.clone(),
            selected_video_asset_id: data.shot.selected_video_asset_id.clone(),
            video_mode: None,
            reference_count: 0,
            reference_min: None,
            reference_max: None,
            eligible: false,
            blocking_reasons: Vec::new(),
        };
        let Some(config) = config else {
            reasons.push("尚未配置当前阶段工作流".to_owned());
            row.blocking_reasons = reasons;
            return Ok(PlannedShot {
                row,
                workflow_version_id: String::new(),
                recipe_id: String::new(),
                values: BTreeMap::new(),
                recipe: None,
            });
        };
        let available_definition = available_definitions
            .get(&(config.workflow_version_id.clone(), config.recipe_id.clone()));
        if let Some(available_definition) = available_definition {
            if production_runtime_for_stage(stage, &available_definition.workflow_id).is_none() {
                reasons.push(match stage {
                    ShotStage::Image => "批量关键帧当前只支持 Kera2 运行时".to_owned(),
                    ShotStage::Video => {
                        "批量视频当前只支持 MiniMax H3 参考图生视频运行时".to_owned()
                    }
                });
            }
        } else {
            reasons.push("当前工作流版本或 Recipe 未达到可用状态".to_owned());
        }
        let Some(definition) = self
            .definition_repository
            .find(&config.workflow_version_id, &config.recipe_id)
            .await?
        else {
            reasons.push("当前工作流版本或 Recipe 已不可用".to_owned());
            row.blocking_reasons = reasons;
            return Ok(PlannedShot {
                row,
                workflow_version_id: config.workflow_version_id.clone(),
                recipe_id: config.recipe_id.clone(),
                values: BTreeMap::new(),
                recipe: None,
            });
        };
        let recipe = match RecipeParser::parse(&definition.recipe_yaml) {
            Ok(recipe) => recipe,
            Err(error) => {
                reasons.push(format!("Recipe 无法解析：{error}"));
                row.blocking_reasons = reasons;
                return Ok(PlannedShot {
                    row,
                    workflow_version_id: config.workflow_version_id.clone(),
                    recipe_id: config.recipe_id.clone(),
                    values: BTreeMap::new(),
                    recipe: None,
                });
            }
        };
        row.recipe_name = Some(recipe.name.clone());
        let expected_output = match stage {
            ShotStage::Image => OutputType::Image,
            ShotStage::Video => OutputType::Video,
        };
        if !recipe
            .outputs
            .iter()
            .any(|output| output.output_type == expected_output)
        {
            reasons.push(format!("Recipe 没有 {} 输出", stage.as_str()));
        }
        let image_input = recipe.inputs.iter().find_map(|(key, input)| match input {
            InputDefinition::Image { .. } => Some((key.clone(), false)),
            InputDefinition::Images { .. } => Some((key.clone(), true)),
            _ => None,
        });
        let ref2va_bounds = if stage == ShotStage::Video {
            match ref2va_image_bounds(&definition.workflow_id, &recipe) {
                Ok(bounds) => bounds,
                Err(error) => {
                    reasons.push(error);
                    None
                }
            }
        } else {
            None
        };
        if stage == ShotStage::Video {
            row.video_mode = Some(if ref2va_bounds.is_some() {
                "REF2VA".to_owned()
            } else {
                "I2V".to_owned()
            });
            row.reference_count = if ref2va_bounds.is_some() {
                data.reference_assets
                    .iter()
                    .filter(|reference| reference.stage == stage)
                    .count()
            } else {
                usize::from(data.shot.selected_image_asset_id.is_some())
            };
            if let Some((min_items, max_items)) = ref2va_bounds {
                row.reference_min = Some(min_items);
                row.reference_max = Some(max_items);
            }
            match (image_input.as_ref(), ref2va_bounds) {
                (Some((_, true)), Some(_)) | (Some((_, false)), None) => {}
                (Some((_, true)), None) => reasons.push(
                    "I2V Recipe 必须使用单个 image 输入；多图输入仅支持 H3 REF2VA".to_owned(),
                ),
                (Some((_, false)), Some(_)) => {
                    reasons.push("REF2VA Recipe 必须使用 plural reference_images 输入".to_owned())
                }
                (None, _) => reasons.push("当前 Recipe 没有可用的图片输入".to_owned()),
            }
        }

        let mut values = match scalar_values_from_json(&config.scalar_values) {
            Ok(values) => values,
            Err(error) => {
                reasons.push(error.to_string());
                BTreeMap::new()
            }
        };
        if let Some(prompt_key) = recipe.inputs.iter().find_map(|(key, input)| {
            matches!(input, InputDefinition::TextArea { .. }).then_some(key)
        }) {
            values.insert(
                prompt_key.clone(),
                GenerationInputValue::Text(
                    stage_prompt.unwrap_or(&data.shot.prompt_text).to_owned(),
                ),
            );
        }

        if stage == ShotStage::Video {
            match (image_input.as_ref(), ref2va_bounds) {
                (Some((key, true)), Some(bounds)) => {
                    match ordered_reference_asset_ids(&data, stage) {
                        Ok(references) => {
                            if let Err(error) =
                                validate_ordered_reference_ids(&references, Some(bounds))
                            {
                                reasons.push(error);
                            }
                            for asset_id in &references {
                                if let Err(reason) = self
                                    .validate_asset(project_id, asset_id, AssetType::Image)
                                    .await
                                {
                                    reasons.push(reason);
                                }
                            }
                            values
                                .insert(key.clone(), GenerationInputValue::ImageAssets(references));
                        }
                        Err(error) => reasons.push(error),
                    }
                }
                (Some((key, false)), None) => {
                    if let Some(selected_id) = data.shot.selected_image_asset_id.as_deref() {
                        match AssetId::parse(selected_id.to_owned()) {
                            Ok(asset_id) => {
                                if let Err(reason) = self
                                    .validate_asset(project_id, &asset_id, AssetType::Image)
                                    .await
                                {
                                    reasons.push(reason);
                                }
                                values.insert(
                                    key.clone(),
                                    GenerationInputValue::ImageAsset(asset_id),
                                );
                            }
                            Err(error) => reasons.push(format!("关键帧素材 ID 无效：{error}")),
                        }
                    } else {
                        reasons.push("I2V 请先选择当前项目的关键帧图片".to_owned());
                    }
                }
                _ => {}
            }
        } else {
            let references = data
                .reference_assets
                .iter()
                .filter(|reference| reference.stage == stage)
                .map(|reference| AssetId::parse(reference.asset_id.clone()))
                .collect::<Result<Vec<_>, _>>();
            match references {
                Ok(references) => {
                    for asset_id in &references {
                        if let Err(reason) = self
                            .validate_asset(project_id, asset_id, AssetType::Image)
                            .await
                        {
                            reasons.push(reason);
                        }
                    }
                    if !references.is_empty() {
                        match image_input {
                            Some((key, true)) => {
                                values.insert(key, GenerationInputValue::ImageAssets(references));
                            }
                            Some((key, false)) if references.len() == 1 => {
                                values.insert(
                                    key,
                                    GenerationInputValue::ImageAsset(references[0].clone()),
                                );
                            }
                            Some((_key, false)) => {
                                reasons.push("单图片输入只能绑定一个 Reference Asset".to_owned())
                            }
                            None => reasons.push("当前 Recipe 没有可用的图片输入".to_owned()),
                        }
                    }
                }
                Err(error) => reasons.push(format!("Reference 素材 ID 无效：{error}")),
            }
        }

        if reasons.is_empty() {
            match WorkflowDocument::parse(definition.workflow_json.clone()) {
                Ok(workflow) => {
                    let request =
                        CompileRequest::new(GenerationInputPreparer::preflight_values(&values));
                    if let Err(error) = WorkflowCompiler.compile(&workflow, &recipe, &request) {
                        reasons.push(format!("输入检查失败：{error}"));
                    }
                }
                Err(error) => reasons.push(format!("工作流无法解析：{error}")),
            }
        }
        row.eligible = reasons.is_empty();
        row.blocking_reasons = reasons;
        Ok(PlannedShot {
            row,
            workflow_version_id: config.workflow_version_id.clone(),
            recipe_id: config.recipe_id.clone(),
            values,
            recipe: Some(recipe),
        })
    }

    async fn validate_asset(
        &self,
        project_id: &str,
        asset_id: &AssetId,
        expected: AssetType,
    ) -> Result<(), String> {
        let asset = self
            .asset_repository
            .find_by_id(asset_id)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("素材 {} 不存在", asset_id.as_str()))?;
        if asset.project_id != project_id || asset.asset_type != expected {
            return Err(format!(
                "素材 {} 必须属于当前项目且为图片素材",
                asset_id.as_str()
            ));
        }
        Ok(())
    }
}

fn ordered_reference_asset_ids(data: &ShotData, stage: ShotStage) -> Result<Vec<AssetId>, String> {
    let mut references = data
        .reference_assets
        .iter()
        .filter(|reference| reference.stage == stage)
        .collect::<Vec<_>>();
    references.sort_by(|left, right| {
        left.ordinal
            .cmp(&right.ordinal)
            .then_with(|| left.asset_id.cmp(&right.asset_id))
    });

    let mut seen = HashSet::new();
    references
        .into_iter()
        .map(|reference| {
            let asset_id = AssetId::parse(reference.asset_id.clone())
                .map_err(|error| format!("Reference 素材 ID 无效：{error}"))?;
            if !seen.insert(asset_id.clone()) {
                return Err("参考图重复".to_owned());
            }
            Ok(asset_id)
        })
        .collect()
}

/// A batch item stores its complete input map in `values_json`. For plural
/// image inputs that map is also the durable reference manifest: vector order
/// is the user-visible reference order and must not be rebuilt from the Shot
/// when the queue later runs or retries the item.
fn freeze_shot_batch_values(
    stage: ShotStage,
    values: BTreeMap<String, GenerationInputValue>,
    recipe: &Recipe,
) -> Result<BTreeMap<String, GenerationInputValue>, String> {
    let values = freeze_random_seed_values(values, recipe)?;
    if stage != ShotStage::Video {
        return Ok(values);
    }

    for (key, definition) in &recipe.inputs {
        let InputDefinition::Images {
            min_items,
            max_items,
            ..
        } = definition
        else {
            continue;
        };
        let Some(GenerationInputValue::ImageAssets(asset_ids)) = values.get(key) else {
            continue;
        };
        if asset_ids.len() < *min_items || asset_ids.len() > *max_items {
            return Err(format!(
                "REF2VA 参考图数量必须在 {min_items}–{max_items} 张之间，当前为 {} 张",
                asset_ids.len()
            ));
        }
        let mut seen = HashSet::new();
        for asset_id in asset_ids {
            if !seen.insert(asset_id.as_str()) {
                return Err(format!("REF2VA 参考图重复：{}", asset_id.as_str()));
            }
        }
    }
    Ok(values)
}

fn validate_project(project_id: &str) -> Result<(), ShotBatchServiceError> {
    crate::domain::validate_project_id(project_id)
        .map_err(|error| ShotBatchServiceError::InvalidInput(error.to_string()))
}

#[derive(Debug)]
pub enum ShotBatchServiceError {
    InvalidInput(String),
    NotFound(String),
    Repository(RepositoryError),
}

impl fmt::Display for ShotBatchServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(formatter, "INVALID_INPUT: {message}"),
            Self::NotFound(id) => write!(formatter, "SHOT_BATCH_NOT_FOUND: {id}"),
            Self::Repository(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for ShotBatchServiceError {}

impl From<RepositoryError> for ShotBatchServiceError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ordered_reference_asset_ids, CreateShotBatchRequest, ShotBatchPlanRow, ShotBatchService,
        MAX_SHOT_BATCH_ITEMS,
    };
    use crate::application::ports::{
        AvailableGenerationDefinition, ShotData, ShotRecord, ShotReferenceAssetRecord,
        ShotRepository, ShotStageConfigRecord,
    };
    use crate::application::product_runtime_scope::production_runtime_for_stage;
    use crate::domain::ShotStage;
    use crate::infrastructure::database::{
        initialize,
        repositories::{
            test_support, SqliteAssetRepository, SqliteGenerationDefinitionRepository,
            SqliteProductionQueueRepository, SqliteProjectRepository, SqliteShotRepository,
            SqliteTaskRepository,
        },
    };
    use crate::infrastructure::time::SystemClock;
    use chrono::Utc;
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::tempdir;

    const REF2VA_TEST_RECIPE_YAML: &str = r#"
schema_version: 1
id: ref2va_test
name: REF2VA Test
workflow:
  file: workflow_api.json
inputs:
  prompt:
    type: textarea
    label: Prompt
    required: true
    default: ""
  reference_images:
    type: images
    label: Reference Images
    required: true
    min_items: 3
    max_items: 3
bindings:
  - source: prompt
    target:
      node: "6"
      input: text
  - source: reference_images
    target:
      node: "10"
      input: image
outputs:
  - id: generated_video
    type: video
    node: "9"
    required: true
"#;

    const REF2VA_TEST_WORKFLOW_JSON: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/simple_i2i/workflow_api.json"
    ));

    const I2V_TEST_RECIPE_YAML: &str = r#"
schema_version: 1
id: i2v_test
name: I2V Test
workflow:
  file: workflow_api.json
inputs:
  prompt:
    type: textarea
    label: Prompt
    required: true
    default: ""
  first_frame:
    type: image
    label: First frame
    required: false
bindings:
  - source: prompt
    target:
      node: "6"
      input: text
  - source: first_frame
    target:
      node: "10"
      input: image
outputs:
  - id: generated_video
    type: video
    node: "9"
    required: true
"#;

    fn shot_data_with_references(
        selected_image_asset_id: Option<&str>,
        references: &[(&str, i64)],
    ) -> ShotData {
        let now = Utc::now();
        ShotData {
            shot: ShotRecord {
                id: "sht_test".to_owned(),
                project_id: "prj_default".to_owned(),
                ordinal: 0,
                name: "Test Shot".to_owned(),
                prompt_text: "test prompt".to_owned(),
                prompt_entry_id: None,
                prompt_version_id: None,
                selected_image_asset_id: selected_image_asset_id.map(str::to_owned),
                selected_video_asset_id: None,
                created_at: now,
                updated_at: now,
            },
            stage_configs: Vec::new(),
            reference_assets: references
                .iter()
                .map(|(asset_id, ordinal)| ShotReferenceAssetRecord {
                    shot_id: "sht_test".to_owned(),
                    stage: ShotStage::Video,
                    asset_id: (*asset_id).to_owned(),
                    ordinal: *ordinal,
                })
                .collect(),
            generation_links: Vec::new(),
        }
    }

    async fn insert_image_asset(pool: &sqlx::SqlitePool, asset_id: &str) {
        sqlx::query(
            "INSERT INTO assets
             (id, project_id, type, category, name, original_name, storage_path, sha256,
              mime_type, width, height, file_size, metadata_json, created_at, updated_at)
             VALUES (?, 'prj_default', 'image', 'source_image', ?, ?, ?, 'sha', 'image/png',
                     1, 1, 1, '{}', '2026-08-17T00:00:00Z', '2026-08-17T00:00:00Z')",
        )
        .bind(asset_id)
        .bind(asset_id)
        .bind(asset_id)
        .bind(format!("assets/{asset_id}.png"))
        .execute(pool)
        .await
        .expect("image asset fixture should insert");
    }

    async fn insert_video_definition(
        pool: &sqlx::SqlitePool,
        workflow_id: &str,
        workflow_version_id: &str,
        recipe_id: &str,
        recipe_yaml: &str,
    ) {
        let now = "2026-08-17T00:00:00Z";
        sqlx::query(
            "INSERT INTO workflows
             (id, name, category, mode, current_version_id, created_at, updated_at)
             VALUES (?, ?, 'video', 'video', ?, ?, ?)",
        )
        .bind(workflow_id)
        .bind(workflow_id)
        .bind(workflow_version_id)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("video workflow fixture should insert");
        sqlx::query(
            "INSERT INTO workflow_versions
             (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
             VALUES (?, ?, '1', ?, 'sha', ?)",
        )
        .bind(workflow_version_id)
        .bind(workflow_id)
        .bind(REF2VA_TEST_WORKFLOW_JSON)
        .bind(now)
        .execute(pool)
        .await
        .expect("video workflow version fixture should insert");
        sqlx::query(
            "INSERT INTO recipes
             (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at)
             VALUES (?, ?, '1', 1, ?, 'sha', ?)",
        )
        .bind(recipe_id)
        .bind(workflow_version_id)
        .bind(recipe_yaml)
        .bind(now)
        .execute(pool)
        .await
        .expect("video recipe fixture should insert");
    }

    #[test]
    fn planner_limits_match_pack_contract() {
        assert_eq!(MAX_SHOT_BATCH_ITEMS, 100);
        let row = ShotBatchPlanRow {
            shot_id: "sht_1".to_owned(),
            ordinal: 0,
            name: "镜头 01".to_owned(),
            stage: "video".to_owned(),
            workflow_version_id: None,
            recipe_id: None,
            recipe_name: None,
            current_status: "DRAFT".to_owned(),
            selected_image_asset_id: None,
            selected_video_asset_id: None,
            video_mode: Some("I2V".to_owned()),
            reference_count: 0,
            reference_min: None,
            reference_max: None,
            eligible: false,
            blocking_reasons: vec!["尚未配置当前阶段工作流".to_owned()],
        };
        assert!(!row.eligible);
        assert_eq!(row.blocking_reasons.len(), 1);
    }

    #[test]
    fn planner_scope_uses_exact_ids_and_ignores_display_names() {
        let definition = |workflow_id: &str, name: &str, category: &str, mode: &str| {
            AvailableGenerationDefinition {
                workflow_id: workflow_id.to_owned(),
                workflow_version_id: "wfv".to_owned(),
                recipe_id: "recipe".to_owned(),
                recipe_version: "1.0.0".to_owned(),
                name: name.to_owned(),
                category: category.to_owned(),
                mode: mode.to_owned(),
                recipe_yaml: String::new(),
            }
        };
        assert!(production_runtime_for_stage(
            ShotStage::Image,
            &definition(
                "wfl_kera2_t2i_local_v2",
                "Kera2 Test Fake Name",
                "unrelated",
                "not-a-mode"
            )
            .workflow_id
        )
        .is_some());
        assert!(production_runtime_for_stage(
            ShotStage::Video,
            &definition(
                "wfl_minimax_h3_reference_video",
                "MiniMax H3 Reference Video Clone",
                "unrelated",
                "not-a-mode"
            )
            .workflow_id
        )
        .is_some());
        assert!(production_runtime_for_stage(
            ShotStage::Image,
            &definition("wfl_other", "Kera2 Test Fake", "image", "text_to_image").workflow_id
        )
        .is_none());
        assert!(production_runtime_for_stage(
            ShotStage::Video,
            &definition(
                "wfl_fake",
                "MiniMax H3 Reference Video Clone",
                "video",
                "reference_to_video"
            )
            .workflow_id
        )
        .is_none());
    }

    #[test]
    fn ref2va_references_are_sorted_by_persisted_ordinal_and_reject_duplicates() {
        let data =
            shot_data_with_references(Some("ast_a"), &[("ast_c", 2), ("ast_a", 1), ("ast_b", 0)]);
        let ordered = ordered_reference_asset_ids(&data, ShotStage::Video).unwrap();
        assert_eq!(
            ordered
                .iter()
                .map(|asset| asset.as_str())
                .collect::<Vec<_>>(),
            ["ast_b", "ast_a", "ast_c"]
        );

        let duplicate = shot_data_with_references(None, &[("ast_b", 0), ("ast_b", 1)]);
        assert_eq!(
            ordered_reference_asset_ids(&duplicate, ShotStage::Video).unwrap_err(),
            "参考图重复"
        );
    }

    #[tokio::test]
    async fn create_freezes_ref2va_order_and_new_batch_reads_updated_shot_order() {
        let directory = tempdir().expect("temporary directory should exist");
        let pool = initialize(&directory.path().join("shot-batch.db"))
            .await
            .expect("database should initialize");
        test_support::seed_task_dependencies(&pool).await;
        sqlx::query("UPDATE projects SET id = 'prj_default' WHERE id = 'project-1'")
            .execute(&pool)
            .await
            .expect("test project id should update");

        let now = "2026-08-17T00:00:00Z";
        sqlx::query(
            "INSERT INTO workflows
             (id, name, category, mode, current_version_id, created_at, updated_at)
             VALUES ('wfl_minimax_h3_reference_video_quality', 'H3 REF2VA', 'video', 'video',
                     'wfv-dev026-ref2va', ?, ?)",
        )
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .expect("REF2VA workflow fixture should insert");
        sqlx::query(
            "INSERT INTO workflow_versions
             (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
             VALUES ('wfv-dev026-ref2va', 'wfl_minimax_h3_reference_video_quality', '1', ?, 'sha', ?)" ,
        )
        .bind(REF2VA_TEST_WORKFLOW_JSON)
        .bind(now)
        .execute(&pool)
        .await
        .expect("REF2VA workflow version fixture should insert");
        sqlx::query(
            "INSERT INTO recipes
             (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at)
             VALUES ('rcp-dev026-ref2va', 'wfv-dev026-ref2va', '1', 1, ?, 'sha', ?)" ,
        )
        .bind(REF2VA_TEST_RECIPE_YAML)
        .bind(now)
        .execute(&pool)
        .await
        .expect("REF2VA recipe fixture should insert");

        for asset_id in ["ast_b", "ast_a", "ast_c"] {
            insert_image_asset(&pool, asset_id).await;
        }
        let shot_repository = Arc::new(SqliteShotRepository::new(pool.clone()));
        shot_repository
            .insert(&ShotRecord {
                id: "sht_dev026".to_owned(),
                project_id: "prj_default".to_owned(),
                ordinal: 0,
                name: "DEV-026 REF2VA".to_owned(),
                prompt_text: "shot prompt".to_owned(),
                prompt_entry_id: None,
                prompt_version_id: None,
                selected_image_asset_id: Some("ast_a".to_owned()),
                selected_video_asset_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .await
            .expect("Shot fixture should insert");
        shot_repository
            .upsert_stage_config(
                "prj_default",
                &ShotStageConfigRecord {
                    shot_id: "sht_dev026".to_owned(),
                    stage: ShotStage::Video,
                    workflow_version_id: "wfv-dev026-ref2va".to_owned(),
                    recipe_id: "rcp-dev026-ref2va".to_owned(),
                    scalar_values: json!({}),
                    updated_at: Utc::now(),
                },
            )
            .await
            .expect("Shot video config should persist");
        shot_repository
            .replace_reference_assets(
                "prj_default",
                "sht_dev026",
                ShotStage::Video,
                &["ast_b".to_owned(), "ast_a".to_owned(), "ast_c".to_owned()],
            )
            .await
            .expect("initial reference order should persist");

        let service = ShotBatchService::new(
            shot_repository.clone(),
            Arc::new(SqliteProductionQueueRepository::new(pool.clone())),
            Arc::new(SqliteTaskRepository::new(pool.clone())),
            Arc::new(SqliteAssetRepository::new(pool.clone())),
            Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone())),
            Arc::new(SqliteProjectRepository::new(pool.clone())),
            Arc::new(SystemClock),
        );
        let old_batch = service
            .create(CreateShotBatchRequest {
                project_id: "prj_default".to_owned(),
                stage: ShotStage::Video,
                shot_ids: vec!["sht_dev026".to_owned()],
            })
            .await
            .expect("initial Shot batch should be created");
        assert_eq!(
            old_batch.items[0].values_json["reference_images"]["assetIds"],
            json!(["ast_b", "ast_a", "ast_c"])
        );

        shot_repository
            .replace_reference_assets(
                "prj_default",
                "sht_dev026",
                ShotStage::Video,
                &["ast_c".to_owned(), "ast_b".to_owned(), "ast_a".to_owned()],
            )
            .await
            .expect("updated reference order should persist");
        let new_batch = service
            .create(CreateShotBatchRequest {
                project_id: "prj_default".to_owned(),
                stage: ShotStage::Video,
                shot_ids: vec!["sht_dev026".to_owned()],
            })
            .await
            .expect("new Shot batch should be created");

        assert_eq!(
            old_batch.items[0].values_json["reference_images"]["assetIds"],
            json!(["ast_b", "ast_a", "ast_c"])
        );
        assert_eq!(
            new_batch.items[0].values_json["reference_images"]["assetIds"],
            json!(["ast_c", "ast_b", "ast_a"])
        );
        pool.close().await;
    }

    #[tokio::test]
    async fn no_gpu_three_shot_video_batch_freezes_modes_order_retry_and_restart() {
        use crate::application::ports::{ProductionQueueRepository, ShotBatchRepository};
        use crate::domain::{
            ProductionBatchItem, ProductionBatchItemId, ProductionBatchItemStatus,
        };

        let directory = tempdir().expect("temporary directory should exist");
        let database_path = directory.path().join("shot-batch-three-shot.db");
        let pool = initialize(&database_path)
            .await
            .expect("database should initialize");
        test_support::seed_task_dependencies(&pool).await;
        sqlx::query("UPDATE projects SET id = 'prj_default' WHERE id = 'project-1'")
            .execute(&pool)
            .await
            .expect("test project id should update");

        insert_video_definition(
            &pool,
            "wfl_minimax_h3_fl2va_i2v_quality",
            "wfv-dev026-i2v",
            "rcp-dev026-i2v",
            I2V_TEST_RECIPE_YAML,
        )
        .await;
        insert_video_definition(
            &pool,
            "wfl_minimax_h3_reference_video_quality",
            "wfv-dev026-ref2va",
            "rcp-dev026-ref2va",
            REF2VA_TEST_RECIPE_YAML,
        )
        .await;
        for asset_id in ["ast_a", "ast_b", "ast_c", "ast_d", "ast_e", "ast_f"] {
            insert_image_asset(&pool, asset_id).await;
        }

        let shot_repository = Arc::new(SqliteShotRepository::new(pool.clone()));
        let now = Utc::now();
        for (shot_id, ordinal, selected_image_asset_id, workflow_version_id, recipe_id) in [
            (
                "sht_dev026_01",
                0,
                Some("ast_a"),
                "wfv-dev026-i2v",
                "rcp-dev026-i2v",
            ),
            (
                "sht_dev026_02",
                1,
                None,
                "wfv-dev026-ref2va",
                "rcp-dev026-ref2va",
            ),
            (
                "sht_dev026_03",
                2,
                None,
                "wfv-dev026-ref2va",
                "rcp-dev026-ref2va",
            ),
        ] {
            shot_repository
                .insert(&ShotRecord {
                    id: shot_id.to_owned(),
                    project_id: "prj_default".to_owned(),
                    ordinal,
                    name: format!("DEV-026 Shot {}", ordinal + 1),
                    prompt_text: format!("shot {}", ordinal + 1),
                    prompt_entry_id: None,
                    prompt_version_id: None,
                    selected_image_asset_id: selected_image_asset_id.map(str::to_owned),
                    selected_video_asset_id: None,
                    created_at: now,
                    updated_at: now,
                })
                .await
                .expect("Shot fixture should insert");
            shot_repository
                .upsert_stage_config(
                    "prj_default",
                    &ShotStageConfigRecord {
                        shot_id: shot_id.to_owned(),
                        stage: ShotStage::Video,
                        workflow_version_id: workflow_version_id.to_owned(),
                        recipe_id: recipe_id.to_owned(),
                        scalar_values: json!({}),
                        updated_at: now,
                    },
                )
                .await
                .expect("Shot video config should persist");
        }
        shot_repository
            .replace_reference_assets(
                "prj_default",
                "sht_dev026_02",
                ShotStage::Video,
                &["ast_b".to_owned(), "ast_a".to_owned(), "ast_c".to_owned()],
            )
            .await
            .expect("Shot02 reference order should persist");
        shot_repository
            .replace_reference_assets(
                "prj_default",
                "sht_dev026_03",
                ShotStage::Video,
                &["ast_d".to_owned(), "ast_e".to_owned(), "ast_f".to_owned()],
            )
            .await
            .expect("Shot03 reference order should persist");

        let queue_repository = Arc::new(SqliteProductionQueueRepository::new(pool.clone()));
        let service = ShotBatchService::new(
            shot_repository.clone(),
            queue_repository.clone(),
            Arc::new(SqliteTaskRepository::new(pool.clone())),
            Arc::new(SqliteAssetRepository::new(pool.clone())),
            Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone())),
            Arc::new(SqliteProjectRepository::new(pool.clone())),
            Arc::new(SystemClock),
        );
        let plan = service
            .plan("prj_default", ShotStage::Video)
            .await
            .expect("three-shot plan should build");
        assert_eq!(plan.eligible_count, 3);
        assert_eq!(plan.rows[0].video_mode.as_deref(), Some("I2V"));
        assert_eq!(plan.rows[0].reference_count, 1);
        assert_eq!(plan.rows[1].video_mode.as_deref(), Some("REF2VA"));
        assert_eq!(plan.rows[1].reference_count, 3);
        assert_eq!(plan.rows[1].reference_min, Some(3));
        assert_eq!(plan.rows[2].reference_count, 3);

        let old_batch = service
            .create(CreateShotBatchRequest {
                project_id: "prj_default".to_owned(),
                stage: ShotStage::Video,
                shot_ids: vec![
                    "sht_dev026_01".to_owned(),
                    "sht_dev026_02".to_owned(),
                    "sht_dev026_03".to_owned(),
                ],
            })
            .await
            .expect("three-shot batch should be created");
        assert_eq!(old_batch.items.len(), 3);
        assert_eq!(
            old_batch.items[0].values_json["first_frame"]["assetId"],
            json!("ast_a")
        );
        assert_eq!(
            old_batch.items[1].values_json["reference_images"]["assetIds"],
            json!(["ast_b", "ast_a", "ast_c"])
        );
        assert_eq!(
            old_batch.items[2].values_json["reference_images"]["assetIds"],
            json!(["ast_d", "ast_e", "ast_f"])
        );

        shot_repository
            .replace_reference_assets(
                "prj_default",
                "sht_dev026_02",
                ShotStage::Video,
                &["ast_c".to_owned(), "ast_b".to_owned(), "ast_a".to_owned()],
            )
            .await
            .expect("Shot02 updated order should persist");
        let new_batch = service
            .create(CreateShotBatchRequest {
                project_id: "prj_default".to_owned(),
                stage: ShotStage::Video,
                shot_ids: vec!["sht_dev026_02".to_owned()],
            })
            .await
            .expect("new Shot02 batch should be created");
        assert_eq!(
            old_batch.items[1].values_json["reference_images"]["assetIds"],
            json!(["ast_b", "ast_a", "ast_c"])
        );
        assert_eq!(
            new_batch.items[0].values_json["reference_images"]["assetIds"],
            json!(["ast_c", "ast_b", "ast_a"])
        );

        assert!(queue_repository
            .set_item_dispatching(&old_batch.items[1].id, now)
            .await
            .expect("source Shot02 item should enter dispatching"));
        assert!(queue_repository
            .finish_item(
                &old_batch.items[1].id,
                ProductionBatchItemStatus::Failed,
                Some("COMFY_TIMEOUT"),
                Some("deterministic no-GPU failure"),
                now,
            )
            .await
            .expect("source Shot02 item should fail"));
        let retry_id = ProductionBatchItemId::new();
        queue_repository
            .append_requeue_item_with_binding(
                &ProductionBatchItem {
                    id: retry_id.clone(),
                    batch_id: old_batch.batch.id.clone(),
                    ordinal: 3,
                    workflow_version_id: "wrong-workflow".to_owned(),
                    recipe_id: "wrong-recipe".to_owned(),
                    values_json: json!({
                        "reference_images": {
                            "type": "image_assets",
                            "assetIds": ["ast_a", "ast_b", "ast_c"]
                        }
                    }),
                    status: ProductionBatchItemStatus::Pending,
                    task_id: None,
                    retry_of_item_id: Some(old_batch.items[1].id.as_str().to_owned()),
                    error_code: None,
                    error_message: None,
                    created_at: now,
                    updated_at: now,
                },
                old_batch.items[1].id.as_str(),
                now,
            )
            .await
            .expect("Shot02 retry should be appended");

        let before_restart = queue_repository
            .find_detail("prj_default", &old_batch.batch.id)
            .await
            .expect("old batch should be readable")
            .expect("old batch should exist");
        let retry = before_restart
            .items
            .iter()
            .find(|item| item.id == retry_id)
            .expect("retry item should exist");
        assert_eq!(
            retry.values_json["reference_images"]["assetIds"],
            json!(["ast_b", "ast_a", "ast_c"])
        );
        assert_eq!(
            retry.retry_of_item_id.as_deref(),
            Some(old_batch.items[1].id.as_str())
        );
        assert_eq!(
            before_restart.items[1].status,
            ProductionBatchItemStatus::Failed
        );
        pool.close().await;

        let restarted_pool = initialize(&database_path)
            .await
            .expect("database should restart");
        let restarted_queue = SqliteProductionQueueRepository::new(restarted_pool.clone());
        let restored_old = restarted_queue
            .find_detail("prj_default", &old_batch.batch.id)
            .await
            .expect("old batch should restore")
            .expect("old batch should remain present");
        let restored_new = restarted_queue
            .find_detail("prj_default", &new_batch.batch.id)
            .await
            .expect("new batch should restore")
            .expect("new batch should remain present");
        assert_eq!(
            restored_old.items[1].values_json["reference_images"]["assetIds"],
            json!(["ast_b", "ast_a", "ast_c"])
        );
        assert_eq!(
            restored_old
                .items
                .iter()
                .find(|item| item.id == retry_id)
                .expect("retry should restore")
                .values_json["reference_images"]["assetIds"],
            json!(["ast_b", "ast_a", "ast_c"])
        );
        assert_eq!(
            restored_new.items[0].values_json["reference_images"]["assetIds"],
            json!(["ast_c", "ast_b", "ast_a"])
        );
        restarted_pool.close().await;
    }
}
