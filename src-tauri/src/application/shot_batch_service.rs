use crate::application::generation_input_preparer::{
    GenerationInputPreparer, GenerationInputValue,
};
use crate::application::ports::{
    AssetRepository, AvailableGenerationDefinition, Clock, GenerationDefinitionRepository,
    ProjectRepository, RepositoryError, ShotBatchBinding, ShotBatchRepository, ShotData,
    ShotRepository, TaskRepository,
};
use crate::application::product_runtime_scope::production_runtime_for_stage;
use crate::application::production_queue_service::generation_values_to_json;
use crate::application::shot_service::scalar_values_from_json;
use crate::compiler::{RecipeParser, WorkflowCompiler};
use crate::domain::{
    derive_stage_status, AssetId, AssetType, CompileRequest, InputDefinition, OutputType,
    ProductionBatch, ProductionBatchDetail, ProductionBatchItem, ProductionBatchItemId,
    ProductionBatchStatus, ShotStage, TaskId, TaskStatus, WorkflowDocument,
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
}

pub struct ShotBatchService {
    shot_repository: Arc<dyn ShotRepository>,
    shot_batch_repository: Arc<dyn ShotBatchRepository>,
    task_repository: Arc<dyn TaskRepository>,
    asset_repository: Arc<dyn AssetRepository>,
    definition_repository: Arc<dyn GenerationDefinitionRepository>,
    project_repository: Arc<dyn ProjectRepository>,
    clock: Arc<dyn Clock>,
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
        }
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
            items.push(ProductionBatchItem {
                id: item_id.clone(),
                batch_id: batch_id.clone(),
                ordinal: u32::try_from(ordinal).map_err(|_| {
                    ShotBatchServiceError::InvalidInput("批次序号超出范围".to_owned())
                })?,
                workflow_version_id: shot.workflow_version_id.clone(),
                recipe_id: shot.recipe_id.clone(),
                values_json: generation_values_to_json(&shot.values),
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
        let mut result = Vec::with_capacity(shots.len());
        for data in shots {
            result.push(
                self.inspect_shot(
                    project_id,
                    stage,
                    data,
                    &task_statuses,
                    &missing_tasks,
                    &available_definitions,
                    active_video_task,
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
        if stage == ShotStage::Video {
            let image_inputs = recipe
                .inputs
                .values()
                .filter(|input| {
                    matches!(
                        input,
                        InputDefinition::Image { .. } | InputDefinition::Images { .. }
                    )
                })
                .count();
            if image_inputs != 1 {
                reasons.push("视频 Recipe 必须有且只有一个 Reference Image 输入".to_owned());
            }
            if data.shot.selected_image_asset_id.is_none() {
                reasons.push("请先选择当前项目的关键帧图片".to_owned());
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
                GenerationInputValue::Text(data.shot.prompt_text.clone()),
            );
        }

        if stage == ShotStage::Video {
            if let Some(selected_id) = data.shot.selected_image_asset_id.as_deref() {
                match AssetId::parse(selected_id.to_owned()) {
                    Ok(asset_id) => {
                        if let Err(reason) = self
                            .validate_asset(project_id, &asset_id, AssetType::Image)
                            .await
                        {
                            reasons.push(reason);
                        }
                        if let Some((key, multiple)) = &image_input {
                            values.insert(
                                key.clone(),
                                if *multiple {
                                    GenerationInputValue::ImageAssets(vec![asset_id])
                                } else {
                                    GenerationInputValue::ImageAsset(asset_id)
                                },
                            );
                        }
                    }
                    Err(error) => reasons.push(format!("关键帧素材 ID 无效：{error}")),
                }
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
    use super::{ShotBatchPlanRow, MAX_SHOT_BATCH_ITEMS};
    use crate::application::ports::AvailableGenerationDefinition;
    use crate::application::product_runtime_scope::production_runtime_for_stage;
    use crate::domain::ShotStage;

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
}
