use crate::application::generation_input_preparer::GenerationInputValue;
use crate::application::generation_service::{
    CreateGenerationRequest, GenerationService, GenerationServiceError, ReferenceManifest,
};
use crate::application::ordered_reference_binding::{
    reference_manifest, validate_ordered_reference_ids,
};
use crate::application::ports::{
    AssetRepository, Clock, GenerationDefinitionRepository, GenerationSnapshotRepository,
    PromptLibraryRepository, RepositoryError, ShotBatchRepository, ShotBulkRepository, ShotData,
    ShotRecord, ShotReferenceAssetRecord, ShotRepository, ShotStageConfigRecord,
    ShotStagePromptRecord, TaskRepository, TaskUpdatePayload,
};
use crate::application::prompt_library_service::canonical_prompt_text;
use crate::application::shot_workflow_compatibility::{
    classify_shot_recipe, ShotVideoInputMode, ShotWorkflowCompatibility,
};
use crate::application::task_query_service::TaskQueryService;
use crate::compiler::RecipeParser;
use crate::domain::{
    canonical_shot_name, derive_stage_status, validate_project_id, AssetId, AssetType,
    InputDefinition, Recipe, SeedValue, ShotStage, ShotViewStatus, TaskId, TaskStatus,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
    fmt,
    sync::Arc,
};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotStageConfigView {
    pub stage: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub scalar_values: Value,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotStagePromptView {
    pub stage: String,
    pub prompt_text: String,
    pub prompt_entry_id: Option<String>,
    pub prompt_version_id: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotReferenceAssetView {
    pub stage: String,
    pub asset_id: String,
    pub ordinal: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotGenerationLinkView {
    pub id: String,
    pub stage: String,
    pub task_id: Option<String>,
    pub production_batch_item_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub task: Option<TaskUpdatePayload>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotView {
    pub id: String,
    pub project_id: String,
    pub ordinal: i64,
    pub name: String,
    pub prompt_text: String,
    pub prompt_entry_id: Option<String>,
    pub prompt_version_id: Option<String>,
    pub stage_prompts: Vec<ShotStagePromptView>,
    pub selected_image_asset_id: Option<String>,
    pub selected_video_asset_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub status: String,
    pub image_status: String,
    pub video_status: String,
    pub stage_configs: Vec<ShotStageConfigView>,
    pub reference_assets: Vec<ShotReferenceAssetView>,
    pub generation_links: Vec<ShotGenerationLinkView>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShotUpdateRequest {
    pub project_id: String,
    pub shot_id: String,
    pub name: String,
    pub prompt_text: String,
    pub prompt_entry_id: Option<String>,
    pub prompt_version_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShotStageConfigRequest {
    pub project_id: String,
    pub shot_id: String,
    pub stage: ShotStage,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub values: BTreeMap<String, GenerationInputValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShotGenerationRequest {
    pub project_id: String,
    pub shot_id: String,
    pub stage: ShotStage,
    pub values: BTreeMap<String, GenerationInputValue>,
    pub production_batch_item_id: Option<String>,
    pub retry_task_id: Option<String>,
}

pub struct ShotService {
    repository: Arc<dyn ShotRepository>,
    task_repository: Arc<dyn TaskRepository>,
    asset_repository: Arc<dyn AssetRepository>,
    definition_repository: Arc<dyn GenerationDefinitionRepository>,
    prompt_repository: Arc<dyn PromptLibraryRepository>,
    task_query_service: Arc<TaskQueryService>,
    generation_service: Arc<GenerationService>,
    shot_batch_repository: Arc<dyn ShotBatchRepository>,
    clock: Arc<dyn Clock>,
    stage_prompt_repository: Option<Arc<dyn ShotBulkRepository>>,
    generation_snapshot_repository: Option<Arc<dyn GenerationSnapshotRepository>>,
}

impl ShotService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        repository: Arc<dyn ShotRepository>,
        task_repository: Arc<dyn TaskRepository>,
        asset_repository: Arc<dyn AssetRepository>,
        definition_repository: Arc<dyn GenerationDefinitionRepository>,
        prompt_repository: Arc<dyn PromptLibraryRepository>,
        task_query_service: Arc<TaskQueryService>,
        generation_service: Arc<GenerationService>,
        shot_batch_repository: Arc<dyn ShotBatchRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repository,
            task_repository,
            asset_repository,
            definition_repository,
            prompt_repository,
            task_query_service,
            generation_service,
            shot_batch_repository,
            clock,
            stage_prompt_repository: None,
            generation_snapshot_repository: None,
        }
    }

    pub fn with_stage_prompt_repository(mut self, repository: Arc<dyn ShotBulkRepository>) -> Self {
        self.stage_prompt_repository = Some(repository);
        self
    }

    pub fn with_generation_snapshot_repository(
        mut self,
        repository: Arc<dyn GenerationSnapshotRepository>,
    ) -> Self {
        self.generation_snapshot_repository = Some(repository);
        self
    }

    pub async fn list(&self, project_id: &str) -> Result<Vec<ShotView>, ShotServiceError> {
        validate_project(project_id)?;
        let data = self.repository.list(project_id).await?;
        self.views(data).await
    }

    pub async fn get(&self, project_id: &str, shot_id: &str) -> Result<ShotView, ShotServiceError> {
        validate_project(project_id)?;
        let data = self
            .repository
            .find(project_id, shot_id)
            .await?
            .ok_or_else(|| ShotServiceError::NotFound(shot_id.to_owned()))?;
        Ok(self.view(data).await?)
    }

    pub async fn create(&self, project_id: &str) -> Result<ShotView, ShotServiceError> {
        validate_project(project_id)?;
        let existing = self.repository.list(project_id).await?;
        let ordinal = i64::try_from(existing.len())
            .map_err(|_| ShotServiceError::InvalidInput("镜头序号超出范围".to_owned()))?;
        let now = self.clock.now();
        let shot = ShotRecord {
            id: format!("sht_{}", Uuid::new_v4()),
            project_id: project_id.to_owned(),
            ordinal,
            name: canonical_shot_name(&format!("镜头 {:02}", ordinal + 1))?,
            prompt_text: String::new(),
            prompt_entry_id: None,
            prompt_version_id: None,
            selected_image_asset_id: None,
            selected_video_asset_id: None,
            created_at: now,
            updated_at: now,
        };
        self.repository.insert(&shot).await?;
        self.get(project_id, &shot.id).await
    }

    pub async fn update(&self, request: ShotUpdateRequest) -> Result<ShotView, ShotServiceError> {
        validate_project(&request.project_id)?;
        let mut shot = self
            .repository
            .find(&request.project_id, &request.shot_id)
            .await?
            .ok_or_else(|| ShotServiceError::NotFound(request.shot_id.clone()))?
            .shot;
        let previous_prompt = shot.prompt_text.clone();
        let previous_prompt_entry_id = shot.prompt_entry_id.clone();
        let previous_prompt_version_id = shot.prompt_version_id.clone();
        shot.name = canonical_shot_name(&request.name)?;
        shot.prompt_text =
            canonical_prompt_text(&request.prompt_text).map_err(ShotServiceError::InvalidInput)?;
        self.validate_prompt_provenance(
            &request.project_id,
            request.prompt_entry_id.as_deref(),
            request.prompt_version_id.as_deref(),
        )
        .await?;
        shot.prompt_entry_id = request.prompt_entry_id;
        shot.prompt_version_id = request.prompt_version_id;
        shot.updated_at = self.clock.now();
        if !self.repository.update(&shot).await? {
            return Err(ShotServiceError::NotFound(request.shot_id));
        }
        if let Some(stage_prompt_repository) = &self.stage_prompt_repository {
            let inherited_updates = stage_prompt_repository
                .find_bulk_data(&shot.project_id, &shot.id)
                .await?
                .into_iter()
                .flat_map(|data| data.stage_prompts)
                .filter(|prompt| {
                    prompt.prompt_text == previous_prompt
                        && prompt.prompt_entry_id == previous_prompt_entry_id
                        && prompt.prompt_version_id == previous_prompt_version_id
                })
                .map(|prompt| ShotStagePromptRecord {
                    shot_id: prompt.shot_id,
                    stage: prompt.stage,
                    prompt_text: shot.prompt_text.clone(),
                    prompt_entry_id: shot.prompt_entry_id.clone(),
                    prompt_version_id: shot.prompt_version_id.clone(),
                    updated_at: shot.updated_at,
                })
                .collect::<Vec<_>>();
            if !inherited_updates.is_empty() {
                stage_prompt_repository
                    .update_stage_prompts_atomic(&shot.project_id, &inherited_updates)
                    .await?;
            }
        }
        self.get(&shot.project_id, &shot.id).await
    }

    pub async fn delete(&self, project_id: &str, shot_id: &str) -> Result<(), ShotServiceError> {
        validate_project(project_id)?;
        if self
            .shot_batch_repository
            .has_active_shot_binding(project_id, shot_id)
            .await?
        {
            return Err(ShotServiceError::InvalidInput(
                "该镜头已绑定待处理或运行中的生产队列，必须等待队列项进入终态后才能删除".to_owned(),
            ));
        }
        if !self.repository.delete(project_id, shot_id).await? {
            return Err(ShotServiceError::NotFound(shot_id.to_owned()));
        }
        Ok(())
    }

    pub async fn reorder(
        &self,
        project_id: &str,
        ordered_ids: Vec<String>,
    ) -> Result<Vec<ShotView>, ShotServiceError> {
        validate_project(project_id)?;
        self.repository
            .reorder(project_id, &ordered_ids, self.clock.now())
            .await?;
        self.list(project_id).await
    }

    pub async fn set_stage_config(
        &self,
        request: ShotStageConfigRequest,
    ) -> Result<ShotView, ShotServiceError> {
        validate_project(&request.project_id)?;
        self.repository
            .find(&request.project_id, &request.shot_id)
            .await?
            .ok_or_else(|| ShotServiceError::NotFound(request.shot_id.clone()))?;
        let definition = self
            .definition_repository
            .find(&request.workflow_version_id, &request.recipe_id)
            .await?
            .ok_or_else(|| {
                ShotServiceError::InvalidInput("所选工作流版本或 Recipe 当前不可用".to_owned())
            })?;
        let scalar_values =
            validate_stage_config_values(request.stage, &definition, &request.values)?;
        self.repository
            .upsert_stage_config(
                &request.project_id,
                &ShotStageConfigRecord {
                    shot_id: request.shot_id.clone(),
                    stage: request.stage,
                    workflow_version_id: request.workflow_version_id,
                    recipe_id: request.recipe_id,
                    scalar_values,
                    updated_at: self.clock.now(),
                },
            )
            .await?;
        self.get(&request.project_id, &request.shot_id).await
    }

    pub async fn replace_references(
        &self,
        project_id: &str,
        shot_id: &str,
        stage: ShotStage,
        asset_ids: Vec<String>,
    ) -> Result<ShotView, ShotServiceError> {
        validate_project(project_id)?;
        if asset_ids.len() > 20 {
            return Err(ShotServiceError::InvalidInput(
                "一个阶段最多保留 20 个 Reference Asset".to_owned(),
            ));
        }
        let parsed_asset_ids = asset_ids
            .iter()
            .map(|asset_id| {
                AssetId::parse(asset_id.clone()).map_err(|error| {
                    ShotServiceError::InvalidInput(format!("参考图素材 ID 无效：{error}"))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        validate_ordered_reference_ids(&parsed_asset_ids, None)
            .map_err(ShotServiceError::InvalidInput)?;
        for asset_id in &parsed_asset_ids {
            self.validate_image_asset(project_id, asset_id, "参考图")
                .await?;
        }
        self.repository
            .replace_reference_assets(project_id, shot_id, stage, &asset_ids)
            .await?;
        self.get(project_id, shot_id).await
    }

    pub async fn select_result(
        &self,
        project_id: &str,
        shot_id: &str,
        stage: ShotStage,
        asset_id: &str,
        from_linked_task: bool,
    ) -> Result<ShotView, ShotServiceError> {
        validate_project(project_id)?;
        let data = self
            .repository
            .find(project_id, shot_id)
            .await?
            .ok_or_else(|| ShotServiceError::NotFound(shot_id.to_owned()))?;
        let asset_id = asset_id.trim();
        let asset =
            self.asset_repository
                .find_by_id(&crate::domain::AssetId::parse(asset_id.to_owned()).map_err(
                    |error| ShotServiceError::InvalidInput(format!("素材 ID 无效：{error}")),
                )?)
                .await?
                .ok_or_else(|| ShotServiceError::InvalidInput("素材不存在".to_owned()))?;
        let expected_type = match stage {
            ShotStage::Image => AssetType::Image,
            ShotStage::Video => AssetType::Video,
        };
        if asset.project_id != project_id || asset.asset_type != expected_type {
            return Err(ShotServiceError::InvalidInput(
                "素材必须属于当前项目且媒体类型与阶段匹配".to_owned(),
            ));
        }
        if from_linked_task {
            let source_task_id = asset.source_task_id.as_ref().map(|id| id.as_str());
            if source_task_id.is_none()
                || !data
                    .generation_links
                    .iter()
                    .any(|link| link.stage == stage && link.task_id.as_deref() == source_task_id)
            {
                return Err(ShotServiceError::InvalidInput(
                    "候选素材不是本镜头该阶段任务的输出".to_owned(),
                ));
            }
        }
        match stage {
            ShotStage::Image => {
                self.repository
                    .select_image(project_id, shot_id, asset_id)
                    .await?
            }
            ShotStage::Video => {
                self.repository
                    .select_video(project_id, shot_id, asset_id)
                    .await?
            }
        }
        self.get(project_id, shot_id).await
    }

    pub async fn generate(
        &self,
        request: ShotGenerationRequest,
    ) -> Result<TaskUpdatePayload, ShotServiceError> {
        validate_project(&request.project_id)?;
        let data = self
            .repository
            .find(&request.project_id, &request.shot_id)
            .await?
            .ok_or_else(|| ShotServiceError::NotFound(request.shot_id.clone()))?;
        let retry_task = if let Some(retry_task_id) = request.retry_task_id.as_deref() {
            let task_id = TaskId::parse(retry_task_id.to_owned())
                .map_err(|error| ShotServiceError::InvalidInput(error.to_string()))?;
            let task = self
                .task_repository
                .find_by_id(&task_id)
                .await?
                .ok_or_else(|| ShotServiceError::NotFound(retry_task_id.to_owned()))?;
            if task.project_id != request.project_id
                || task.status != TaskStatus::Failed
                || !data.generation_links.iter().any(|link| {
                    link.stage == request.stage && link.task_id.as_deref() == Some(retry_task_id)
                })
            {
                return Err(ShotServiceError::InvalidInput(
                    "只能从当前 Shot 的失败任务创建重试".to_owned(),
                ));
            }
            let snapshot_repository =
                self.generation_snapshot_repository
                    .as_ref()
                    .ok_or_else(|| {
                        ShotServiceError::InvalidInput("当前运行时不支持快照重试".to_owned())
                    })?;
            let snapshot = snapshot_repository
                .find_by_task_id(&task_id)
                .await?
                .ok_or_else(|| {
                    ShotServiceError::InvalidInput(
                        "失败任务缺少不可变生成快照，无法安全重试".to_owned(),
                    )
                })?;
            Some((task, snapshot))
        } else {
            None
        };
        let config = data
            .stage_configs
            .iter()
            .find(|config| config.stage == request.stage);
        let (workflow_version_id, recipe_id) = if let Some((task, _)) = retry_task.as_ref() {
            (task.workflow_version_id.clone(), task.recipe_id.clone())
        } else {
            let config = config.ok_or_else(|| {
                ShotServiceError::InvalidInput("请先配置当前阶段工作流".to_owned())
            })?;
            (config.workflow_version_id.clone(), config.recipe_id.clone())
        };

        let definition = self
            .definition_repository
            .find(&workflow_version_id, &recipe_id)
            .await?
            .ok_or_else(|| ShotServiceError::InvalidInput("当前阶段工作流已不可用".to_owned()))?;
        let recipe = RecipeParser::parse(&definition.recipe_yaml)
            .map_err(|error| ShotServiceError::InvalidInput(error.to_string()))?;
        let compatibility = if request.stage == ShotStage::Video {
            validate_video_recipe(&definition.workflow_id, &recipe)?
        } else {
            classify_shot_recipe(request.stage, &definition.workflow_id, &recipe)
                .map_err(ShotServiceError::InvalidInput)?
        };
        let input_mode = compatibility.input_mode;
        let frozen_values = retry_task
            .as_ref()
            .map(|(_, snapshot)| input_values_from_snapshot(&snapshot.user_inputs_json, &recipe))
            .transpose()?;
        let is_frozen_retry = frozen_values.is_some();
        let mut values = if let Some(values) = frozen_values {
            values
        } else {
            let config = config.ok_or_else(|| {
                ShotServiceError::InvalidInput("请先配置当前阶段工作流".to_owned())
            })?;
            let _ = scalar_values_to_json(&recipe.inputs, &request.values)?;
            let mut values = scalar_values_from_json(&config.scalar_values)?;
            values.extend(request.values);
            values
        };
        let mut reference_manifest = None;
        if !is_frozen_retry {
            if let Some(prompt_key) = recipe.inputs.iter().find_map(|(key, input)| {
                matches!(input, InputDefinition::TextArea { .. }).then_some(key)
            }) {
                let prompt = self
                    .stage_prompt_text(
                        &request.project_id,
                        &request.shot_id,
                        request.stage,
                        &data.shot,
                    )
                    .await?;
                values.insert(prompt_key.clone(), GenerationInputValue::Text(prompt));
            }
        }
        if request.stage == ShotStage::Video {
            let (selected, references) = if is_frozen_retry {
                match &input_mode {
                    ShotVideoInputMode::TextOnly => (None, Vec::new()),
                    ShotVideoInputMode::SingleImage { key } => match values.get(key).cloned() {
                        Some(GenerationInputValue::ImageAsset(id)) => (Some(id), Vec::new()),
                        _ => {
                            return Err(ShotServiceError::InvalidInput(
                                "失败任务快照缺少有效的视频输入，无法安全重试".to_owned(),
                            ));
                        }
                    },
                    ShotVideoInputMode::ReferenceImages { key, .. } => {
                        match values.get(key).cloned() {
                            Some(GenerationInputValue::ImageAssets(ids)) => (None, ids),
                            _ => {
                                return Err(ShotServiceError::InvalidInput(
                                    "失败任务快照缺少有效的视频输入，无法安全重试".to_owned(),
                                ));
                            }
                        }
                    }
                }
            } else {
                match &input_mode {
                    ShotVideoInputMode::TextOnly => (None, Vec::new()),
                    ShotVideoInputMode::SingleImage { .. } => {
                        let selected =
                            data.shot.selected_image_asset_id.as_ref().ok_or_else(|| {
                                ShotServiceError::InvalidInput("请先选择关键帧图片".to_owned())
                            })?;
                        let selected = AssetId::parse(selected.clone()).map_err(|error| {
                            ShotServiceError::InvalidInput(format!("关键帧素材 ID 无效：{error}"))
                        })?;
                        self.validate_image_asset(&request.project_id, &selected, "关键帧")
                            .await?;
                        (Some(selected), Vec::new())
                    }
                    ShotVideoInputMode::ReferenceImages {
                        min_items,
                        max_items,
                        ..
                    } => {
                        let references =
                            ordered_reference_asset_ids(&data.reference_assets, request.stage)?;
                        validate_ordered_reference_ids(&references, Some((*min_items, *max_items)))
                            .map_err(ShotServiceError::InvalidInput)?;
                        for asset_id in &references {
                            self.validate_image_asset(&request.project_id, asset_id, "参考图")
                                .await?;
                        }
                        (None, references)
                    }
                }
            };
            if !matches!(&input_mode, ShotVideoInputMode::TextOnly) {
                let bounds = match &input_mode {
                    ShotVideoInputMode::ReferenceImages {
                        min_items,
                        max_items,
                        ..
                    } => Some((*min_items, *max_items)),
                    _ => None,
                };
                let (key, image_value, manifest) =
                    build_video_input(&recipe, selected, references, bounds)?;
                values.insert(key, image_value);
                reference_manifest = manifest;
            }
            self.ensure_no_active_video_tasks(&request.project_id)
                .await?;
        } else if !is_frozen_retry {
            let references = data
                .reference_assets
                .iter()
                .filter(|reference| reference.stage == request.stage)
                .map(|reference| {
                    crate::domain::AssetId::parse(reference.asset_id.clone())
                        .map_err(|error| ShotServiceError::InvalidInput(error.to_string()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if !references.is_empty() {
                let (key, multiple) = match &input_mode {
                    ShotVideoInputMode::SingleImage { key } => (key, false),
                    ShotVideoInputMode::ReferenceImages { key, .. } => (key, true),
                    ShotVideoInputMode::TextOnly => {
                        return Err(ShotServiceError::InvalidInput(
                            "当前阶段 Recipe 没有可用的图片输入".to_owned(),
                        ));
                    }
                };
                if !multiple && references.len() != 1 {
                    return Err(ShotServiceError::InvalidInput(
                        "单图片输入只能绑定一个 Reference Asset".to_owned(),
                    ));
                }
                values.insert(
                    key.clone(),
                    if multiple {
                        GenerationInputValue::ImageAssets(references)
                    } else {
                        GenerationInputValue::ImageAsset(references[0].clone())
                    },
                );
            }
        }

        let generation_request = CreateGenerationRequest {
            project_id: request.project_id.clone(),
            workflow_version_id,
            recipe_id,
            values,
            reference_manifest,
            submission_idempotency_key: None,
            submission_attempt: None,
            parent_task_id: None,
        };
        let repository = Arc::clone(&self.repository);
        let project_id = request.project_id.clone();
        let shot_id = request.shot_id.clone();
        let stage = request.stage;
        let item_id = request.production_batch_item_id.clone();
        let created_at = self.clock.now();
        let task = self
            .generation_service
            .start_generation_with_task_hook(generation_request, move |task| {
                let repository = Arc::clone(&repository);
                let project_id = project_id.clone();
                let shot_id = shot_id.clone();
                let item_id = item_id.clone();
                let task_id = task.id.to_string();
                async move {
                    repository
                        .link_generation(
                            &project_id,
                            &shot_id,
                            stage,
                            &task_id,
                            item_id.as_deref(),
                            created_at,
                        )
                        .await
                        .map(|_| ())
                }
            })
            .await
            .map_err(ShotServiceError::Generation)?;
        self.task_query_service
            .view(task)
            .await
            .map_err(|error| ShotServiceError::TaskView(error.to_string()))
    }

    async fn validate_image_asset(
        &self,
        project_id: &str,
        asset_id: &AssetId,
        label: &str,
    ) -> Result<(), ShotServiceError> {
        let asset = self
            .asset_repository
            .find_by_id(asset_id)
            .await?
            .ok_or_else(|| {
                ShotServiceError::InvalidInput(format!("{label}不存在：{}", asset_id.as_str()))
            })?;
        if asset.project_id != project_id {
            return Err(ShotServiceError::InvalidInput(format!(
                "{label}必须属于当前项目：{}",
                asset_id.as_str()
            )));
        }
        if asset.asset_type != AssetType::Image {
            return Err(ShotServiceError::InvalidInput(format!(
                "{label}必须是图片素材：{}",
                asset_id.as_str()
            )));
        }
        Ok(())
    }

    async fn ensure_no_active_video_tasks(&self, project_id: &str) -> Result<(), ShotServiceError> {
        let shots = self.repository.list(project_id).await?;
        for data in shots {
            for link in data
                .generation_links
                .iter()
                .filter(|link| link.stage == ShotStage::Video)
            {
                let Some(task_id) = link.task_id.as_deref() else {
                    continue;
                };
                let task_id = TaskId::parse(task_id.to_owned())
                    .map_err(|error| ShotServiceError::InvalidInput(error.to_string()))?;
                if let Some(task) = self.task_repository.find_by_id(&task_id).await? {
                    if !task.status.is_terminal() {
                        return Err(ShotServiceError::InvalidInput(
                            "当前已有一个视频生成任务运行中，请等待它结束后再生成".to_owned(),
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    async fn validate_prompt_provenance(
        &self,
        project_id: &str,
        entry_id: Option<&str>,
        version_id: Option<&str>,
    ) -> Result<(), ShotServiceError> {
        if entry_id.is_none() != version_id.is_none() {
            return Err(ShotServiceError::InvalidInput(
                "Prompt provenance 必须同时包含 entry 和 version，或同时为空".to_owned(),
            ));
        }
        let (Some(entry_id), Some(version_id)) = (entry_id, version_id) else {
            return Ok(());
        };
        self.prompt_repository
            .find_by_id(project_id, entry_id)
            .await?
            .ok_or_else(|| {
                ShotServiceError::InvalidInput("Prompt Library entry 不属于当前项目".to_owned())
            })?;
        let versions = self
            .prompt_repository
            .list_versions(project_id, entry_id)
            .await?;
        if !versions.iter().any(|version| version.id == version_id) {
            return Err(ShotServiceError::InvalidInput(
                "Prompt Library version 不属于该 entry 或当前项目".to_owned(),
            ));
        }
        Ok(())
    }

    async fn stage_prompt_text(
        &self,
        project_id: &str,
        shot_id: &str,
        stage: ShotStage,
        shot: &ShotRecord,
    ) -> Result<String, ShotServiceError> {
        let Some(repository) = &self.stage_prompt_repository else {
            return Ok(shot.prompt_text.clone());
        };
        let data = repository.find_bulk_data(project_id, shot_id).await?;
        Ok(data
            .and_then(|item| {
                item.stage_prompts
                    .into_iter()
                    .find(|prompt| prompt.stage == stage)
                    .map(|prompt| prompt.prompt_text)
            })
            .unwrap_or_else(|| shot.prompt_text.clone()))
    }

    async fn views(&self, data: Vec<ShotData>) -> Result<Vec<ShotView>, ShotServiceError> {
        let task_ids = data
            .iter()
            .flat_map(|item| item.generation_links.iter())
            .filter_map(|link| link.task_id.as_deref())
            .map(|task_id| {
                TaskId::parse(task_id.to_owned())
                    .map_err(|error| ShotServiceError::InvalidInput(error.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let tasks = self.task_repository.find_many_by_ids(&task_ids).await?;
        let tasks_by_id = tasks
            .into_iter()
            .map(|task| (task.id.to_string(), task))
            .collect::<HashMap<_, _>>();
        let mut result = Vec::with_capacity(data.len());
        for item in data {
            result.push(self.view_with_tasks(item, &tasks_by_id).await?);
        }
        Ok(result)
    }

    async fn view(&self, data: ShotData) -> Result<ShotView, ShotServiceError> {
        let task_ids = data
            .generation_links
            .iter()
            .filter_map(|link| link.task_id.as_deref())
            .map(|task_id| {
                TaskId::parse(task_id.to_owned())
                    .map_err(|error| ShotServiceError::InvalidInput(error.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let tasks = self.task_repository.find_many_by_ids(&task_ids).await?;
        let tasks_by_id = tasks
            .into_iter()
            .map(|task| (task.id.to_string(), task))
            .collect::<HashMap<_, _>>();
        self.view_with_tasks(data, &tasks_by_id).await
    }

    async fn view_with_tasks(
        &self,
        data: ShotData,
        tasks_by_id: &HashMap<String, crate::domain::Task>,
    ) -> Result<ShotView, ShotServiceError> {
        let stage_prompts = if self.stage_prompt_repository.is_some() {
            data.stage_prompts
        } else {
            Vec::new()
        };
        let mut task_statuses = HashMap::new();
        let mut generation_links = Vec::with_capacity(data.generation_links.len());
        for link in &data.generation_links {
            let task = if let Some(task_id) = &link.task_id {
                let task = tasks_by_id
                    .get(task_id)
                    .ok_or_else(|| ShotServiceError::NotFound(task_id.to_owned()))?;
                if task.project_id != data.shot.project_id {
                    return Err(ShotServiceError::InvalidInput(
                        "Shot generation link 跨项目".to_owned(),
                    ));
                }
                task_statuses.insert(task.id.to_string(), task.status);
                Some(
                    self.task_query_service
                        .view(task.clone())
                        .await
                        .map_err(|error| ShotServiceError::TaskView(error.to_string()))?,
                )
            } else {
                None
            };
            generation_links.push(ShotGenerationLinkView {
                id: link.id.clone(),
                stage: link.stage.as_str().to_owned(),
                task_id: link.task_id.clone(),
                production_batch_item_id: link.production_batch_item_id.clone(),
                created_at: link.created_at,
                task,
            });
        }

        let latest_status = |stage: ShotStage| {
            data.generation_links
                .iter()
                .filter(|link| link.stage == stage)
                .filter_map(|link| link.task_id.as_ref().and_then(|id| task_statuses.get(id)))
                .next()
                .copied()
        };
        let has_stage = |stage: ShotStage| {
            data.stage_configs
                .iter()
                .any(|config| config.stage == stage)
        };
        let image_status = derive_stage_status(
            ShotStage::Image,
            has_stage(ShotStage::Image),
            data.shot.selected_image_asset_id.is_some(),
            latest_status(ShotStage::Image),
        );
        let video_status = derive_stage_status(
            ShotStage::Video,
            has_stage(ShotStage::Video),
            data.shot.selected_video_asset_id.is_some(),
            latest_status(ShotStage::Video),
        );
        let status = overall_status(image_status, video_status, has_stage(ShotStage::Video));
        Ok(ShotView {
            id: data.shot.id,
            project_id: data.shot.project_id,
            ordinal: data.shot.ordinal,
            name: data.shot.name,
            prompt_text: data.shot.prompt_text,
            prompt_entry_id: data.shot.prompt_entry_id,
            prompt_version_id: data.shot.prompt_version_id,
            stage_prompts: stage_prompts
                .into_iter()
                .map(|prompt| ShotStagePromptView {
                    stage: prompt.stage.as_str().to_owned(),
                    prompt_text: prompt.prompt_text,
                    prompt_entry_id: prompt.prompt_entry_id,
                    prompt_version_id: prompt.prompt_version_id,
                    updated_at: prompt.updated_at,
                })
                .collect(),
            selected_image_asset_id: data.shot.selected_image_asset_id,
            selected_video_asset_id: data.shot.selected_video_asset_id,
            created_at: data.shot.created_at,
            updated_at: data.shot.updated_at,
            status: status.as_str().to_owned(),
            image_status: image_status.as_str().to_owned(),
            video_status: video_status.as_str().to_owned(),
            stage_configs: data
                .stage_configs
                .into_iter()
                .map(|config| ShotStageConfigView {
                    stage: config.stage.as_str().to_owned(),
                    workflow_version_id: config.workflow_version_id,
                    recipe_id: config.recipe_id,
                    scalar_values: config.scalar_values,
                    updated_at: config.updated_at,
                })
                .collect(),
            reference_assets: data
                .reference_assets
                .into_iter()
                .map(|reference| ShotReferenceAssetView {
                    stage: reference.stage.as_str().to_owned(),
                    asset_id: reference.asset_id,
                    ordinal: reference.ordinal,
                })
                .collect(),
            generation_links,
        })
    }
}

/// Validate a stage configuration using the same runtime, output, Recipe and
/// scalar rules as the normal ShotService path, without writing it.
///
/// Bulk configuration uses this preflight for every selected Shot before its
/// repository transaction begins.  Keeping the helper here prevents a second
/// REF2VA/I2V decision tree from drifting away from the single-shot path.
pub(crate) fn validate_stage_config_values(
    stage: ShotStage,
    definition: &crate::application::ports::GenerationDefinition,
    values: &BTreeMap<String, GenerationInputValue>,
) -> Result<Value, ShotServiceError> {
    let recipe = RecipeParser::parse(&definition.recipe_yaml)
        .map_err(|error| ShotServiceError::InvalidInput(error.to_string()))?;
    classify_shot_recipe(stage, &definition.workflow_id, &recipe)
        .map_err(ShotServiceError::InvalidInput)?;
    scalar_values_to_json(&recipe.inputs, values)
}

fn overall_status(
    image_status: ShotViewStatus,
    video_status: ShotViewStatus,
    has_video_stage: bool,
) -> ShotViewStatus {
    if has_video_stage {
        if video_status == ShotViewStatus::Completed {
            return ShotViewStatus::Completed;
        }
        if matches!(video_status, ShotViewStatus::GeneratingVideo) {
            return video_status;
        }
        if matches!(
            video_status,
            ShotViewStatus::VideoReview | ShotViewStatus::Failed
        ) {
            return video_status;
        }
    }
    if !has_video_stage && image_status == ShotViewStatus::ImageSelected {
        return ShotViewStatus::Completed;
    }
    match image_status {
        ShotViewStatus::GeneratingImage | ShotViewStatus::ImageReview | ShotViewStatus::Failed => {
            image_status
        }
        _ => {
            if image_status == ShotViewStatus::ImageSelected {
                ShotViewStatus::ImageSelected
            } else if image_status == ShotViewStatus::Ready || video_status == ShotViewStatus::Ready
            {
                ShotViewStatus::Ready
            } else {
                ShotViewStatus::Draft
            }
        }
    }
}

fn video_image_input(recipe: &Recipe) -> Result<(&str, &InputDefinition), ShotServiceError> {
    let mut inputs = recipe.inputs.iter().filter(|(_, input)| {
        matches!(
            input,
            InputDefinition::Image { .. } | InputDefinition::Images { .. }
        )
    });
    let Some((key, input)) = inputs.next() else {
        return Err(ShotServiceError::InvalidInput(
            "视频 Recipe 必须声明一个 image 或 images 输入".to_owned(),
        ));
    };
    if inputs.next().is_some() {
        return Err(ShotServiceError::InvalidInput(
            "视频 Recipe 只能声明一个 image 或 images 输入".to_owned(),
        ));
    }
    Ok((key.as_str(), input))
}

fn ordered_reference_asset_ids(
    references: &[ShotReferenceAssetRecord],
    stage: ShotStage,
) -> Result<Vec<AssetId>, ShotServiceError> {
    let mut references = references
        .iter()
        .filter(|reference| reference.stage == stage)
        .collect::<Vec<_>>();
    references.sort_by(|left, right| {
        left.ordinal
            .cmp(&right.ordinal)
            .then_with(|| left.asset_id.cmp(&right.asset_id))
    });
    references
        .into_iter()
        .map(|reference| {
            AssetId::parse(reference.asset_id.clone()).map_err(|error| {
                ShotServiceError::InvalidInput(format!("参考图素材 ID 无效：{error}"))
            })
        })
        .collect()
}

fn input_values_from_snapshot(
    snapshot: &Value,
    recipe: &Recipe,
) -> Result<BTreeMap<String, GenerationInputValue>, ShotServiceError> {
    let values = snapshot.as_object().ok_or_else(|| {
        ShotServiceError::InvalidInput("失败任务快照的输入必须是 JSON object".to_owned())
    })?;
    let mut restored = BTreeMap::new();
    for (key, value) in values {
        let input = recipe.inputs.get(key).ok_or_else(|| {
            ShotServiceError::InvalidInput(format!("失败任务快照包含未知 Recipe 参数 {key}"))
        })?;
        let restored_value = match input {
            InputDefinition::TextArea { .. } => value
                .as_str()
                .map(|value| GenerationInputValue::Text(value.to_owned()))
                .ok_or_else(|| ShotServiceError::InvalidInput(format!("快照参数 {key} 无效")))?,
            InputDefinition::Integer { .. } => {
                GenerationInputValue::Integer(value.as_i64().ok_or_else(|| {
                    ShotServiceError::InvalidInput(format!("快照参数 {key} 无效"))
                })?)
            }
            InputDefinition::Number { .. } => {
                let value = value.as_f64().ok_or_else(|| {
                    ShotServiceError::InvalidInput(format!("快照参数 {key} 无效"))
                })?;
                if !value.is_finite() {
                    return Err(ShotServiceError::InvalidInput(format!(
                        "快照参数 {key} 无效"
                    )));
                }
                GenerationInputValue::Number(value)
            }
            InputDefinition::Seed { .. } => match value {
                Value::String(value) if value == "random" => {
                    GenerationInputValue::Seed(SeedValue::Random)
                }
                Value::Number(value) => {
                    GenerationInputValue::Seed(SeedValue::Fixed(value.as_u64().ok_or_else(
                        || ShotServiceError::InvalidInput(format!("快照参数 {key} 无效")),
                    )?))
                }
                _ => {
                    return Err(ShotServiceError::InvalidInput(format!(
                        "快照参数 {key} 无效"
                    )))
                }
            },
            InputDefinition::Image { .. } => {
                GenerationInputValue::ImageAsset(snapshot_asset_id(value, "image_asset", key)?)
            }
            InputDefinition::Images { .. } => {
                GenerationInputValue::ImageAssets(snapshot_asset_ids(value, "image_assets", key)?)
            }
            InputDefinition::Video { .. } => {
                GenerationInputValue::VideoAsset(snapshot_asset_id(value, "video_asset", key)?)
            }
            InputDefinition::Audio { .. } => {
                GenerationInputValue::AudioAsset(snapshot_asset_id(value, "audio_asset", key)?)
            }
            InputDefinition::Videos { .. } => {
                GenerationInputValue::VideoAssets(snapshot_asset_ids(value, "video_assets", key)?)
            }
            InputDefinition::Audios { .. } => {
                GenerationInputValue::AudioAssets(snapshot_asset_ids(value, "audio_assets", key)?)
            }
        };
        restored.insert(key.clone(), restored_value);
    }
    Ok(restored)
}

fn snapshot_asset_id(
    value: &Value,
    expected_type: &str,
    key: &str,
) -> Result<AssetId, ShotServiceError> {
    let actual_type = value.get("type").and_then(Value::as_str);
    if actual_type != Some(expected_type) {
        return Err(ShotServiceError::InvalidInput(format!(
            "快照参数 {key} 的素材类型无效"
        )));
    }
    let asset_id = value
        .get("assetId")
        .and_then(Value::as_str)
        .ok_or_else(|| ShotServiceError::InvalidInput(format!("快照参数 {key} 缺少素材 ID")))?;
    AssetId::parse(asset_id.to_owned()).map_err(|error| {
        ShotServiceError::InvalidInput(format!("快照参数 {key} 的素材 ID 无效：{error}"))
    })
}

fn snapshot_asset_ids(
    value: &Value,
    expected_type: &str,
    key: &str,
) -> Result<Vec<AssetId>, ShotServiceError> {
    let actual_type = value.get("type").and_then(Value::as_str);
    if actual_type != Some(expected_type) {
        return Err(ShotServiceError::InvalidInput(format!(
            "快照参数 {key} 的素材类型无效"
        )));
    }
    value
        .get("assetIds")
        .and_then(Value::as_array)
        .ok_or_else(|| ShotServiceError::InvalidInput(format!("快照参数 {key} 缺少素材 ID")))?
        .iter()
        .map(|value| {
            let asset_id = value.as_str().ok_or_else(|| {
                ShotServiceError::InvalidInput(format!("快照参数 {key} 的素材 ID 无效"))
            })?;
            AssetId::parse(asset_id.to_owned()).map_err(|error| {
                ShotServiceError::InvalidInput(format!("快照参数 {key} 的素材 ID 无效：{error}"))
            })
        })
        .collect()
}

fn validate_video_recipe(
    workflow_id: &str,
    recipe: &Recipe,
) -> Result<ShotWorkflowCompatibility, ShotServiceError> {
    classify_shot_recipe(ShotStage::Video, workflow_id, recipe)
        .map_err(ShotServiceError::InvalidInput)
}

fn build_video_input(
    recipe: &Recipe,
    selected_image_asset_id: Option<AssetId>,
    references: Vec<AssetId>,
    ref2va_bounds: Option<(usize, usize)>,
) -> Result<(String, GenerationInputValue, Option<ReferenceManifest>), ShotServiceError> {
    let (key, input) = video_image_input(recipe)?;
    match input {
        InputDefinition::Image { .. } => {
            let selected = selected_image_asset_id
                .ok_or_else(|| ShotServiceError::InvalidInput("请先选择关键帧图片".to_owned()))?;
            Ok((
                key.to_owned(),
                GenerationInputValue::ImageAsset(selected),
                None,
            ))
        }
        InputDefinition::Images { .. } => {
            let bounds = ref2va_bounds.ok_or_else(|| {
                ShotServiceError::InvalidInput(
                    "视频多图输入缺少有效的 Recipe reference bounds".to_owned(),
                )
            })?;
            validate_ordered_reference_ids(&references, Some(bounds))
                .map_err(ShotServiceError::InvalidInput)?;
            Ok((
                key.to_owned(),
                GenerationInputValue::ImageAssets(references.clone()),
                Some(reference_manifest(key, &references)),
            ))
        }
        _ => unreachable!("video_image_input only returns image inputs"),
    }
}

pub(crate) fn scalar_values_to_json(
    inputs: &std::collections::BTreeMap<String, InputDefinition>,
    values: &BTreeMap<String, GenerationInputValue>,
) -> Result<Value, ShotServiceError> {
    let mut result = Map::new();
    for (key, value) in values {
        let Some(input) = inputs.get(key) else {
            return Err(ShotServiceError::InvalidInput(format!(
                "参数 {key} 不属于当前 Recipe"
            )));
        };
        let json_value = match (input, value) {
            (InputDefinition::Integer { .. }, GenerationInputValue::Integer(value)) => {
                json!({"type": "integer", "value": value})
            }
            (InputDefinition::Number { .. }, GenerationInputValue::Number(value)) => {
                json!({"type": "number", "value": value})
            }
            (InputDefinition::Seed { .. }, GenerationInputValue::Seed(SeedValue::Random)) => {
                json!({"type": "seed_random"})
            }
            (InputDefinition::Seed { .. }, GenerationInputValue::Seed(SeedValue::Fixed(value))) => {
                json!({"type": "seed_fixed", "value": value.to_string()})
            }
            (_, GenerationInputValue::Text(_)) => {
                return Err(ShotServiceError::InvalidInput(
                    "Shot scalar 参数不保存 prompt；prompt 单独由 Shot 管理".to_owned(),
                ))
            }
            _ => {
                return Err(ShotServiceError::InvalidInput(format!(
                    "参数 {key} 必须是 Recipe 的 integer、number 或 seed"
                )))
            }
        };
        result.insert(key.clone(), json_value);
    }
    Ok(Value::Object(result))
}

pub(crate) fn scalar_values_from_json(
    value: &Value,
) -> Result<BTreeMap<String, GenerationInputValue>, ShotServiceError> {
    let Some(values) = value.as_object() else {
        return Err(ShotServiceError::InvalidInput(
            "Shot scalar 参数格式无效".to_owned(),
        ));
    };
    let mut result = BTreeMap::new();
    for (key, value) in values {
        match value.get("type").and_then(Value::as_str) {
            Some("integer") => result.insert(
                key.clone(),
                GenerationInputValue::Integer(
                    value.get("value").and_then(Value::as_i64).ok_or_else(|| {
                        ShotServiceError::InvalidInput(format!("参数 {key} 无效"))
                    })?,
                ),
            ),
            Some("number") => {
                let value = value
                    .get("value")
                    .and_then(Value::as_f64)
                    .ok_or_else(|| ShotServiceError::InvalidInput(format!("参数 {key} 无效")))?;
                if !value.is_finite() {
                    return Err(ShotServiceError::InvalidInput(format!("参数 {key} 无效")));
                }
                result.insert(key.clone(), GenerationInputValue::Number(value))
            }
            Some("seed_random") => {
                result.insert(key.clone(), GenerationInputValue::Seed(SeedValue::Random))
            }
            Some("seed_fixed") => result.insert(
                key.clone(),
                GenerationInputValue::Seed(SeedValue::Fixed(
                    value
                        .get("value")
                        .and_then(Value::as_str)
                        .ok_or_else(|| ShotServiceError::InvalidInput(format!("参数 {key} 无效")))?
                        .parse::<u64>()
                        .map_err(|_| {
                            ShotServiceError::InvalidInput(format!("参数 {key} 的 seed 无效"))
                        })?,
                )),
            ),
            _ => {
                return Err(ShotServiceError::InvalidInput(format!(
                    "参数 {key} 不是允许的 scalar 类型"
                )))
            }
        };
    }
    Ok(result)
}

fn validate_project(project_id: &str) -> Result<(), ShotServiceError> {
    validate_project_id(project_id)
        .map_err(|error| ShotServiceError::InvalidInput(error.to_string()))
}

#[derive(Debug)]
pub enum ShotServiceError {
    InvalidInput(String),
    NotFound(String),
    Repository(RepositoryError),
    Generation(GenerationServiceError),
    TaskView(String),
}

impl fmt::Display for ShotServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(formatter, "INVALID_INPUT: {message}"),
            Self::NotFound(id) => write!(formatter, "SHOT_NOT_FOUND: {id}"),
            Self::Repository(error) => write!(formatter, "{error}"),
            Self::Generation(error) => write!(formatter, "{error}"),
            Self::TaskView(message) => write!(formatter, "TASK_VIEW_ERROR: {message}"),
        }
    }
}

impl Error for ShotServiceError {}

impl From<RepositoryError> for ShotServiceError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

impl From<crate::domain::ShotDomainError> for ShotServiceError {
    fn from(error: crate::domain::ShotDomainError) -> Self {
        Self::InvalidInput(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_video_input, input_values_from_snapshot, ordered_reference_asset_ids,
        ShotServiceError,
    };
    use crate::application::generation_input_preparer::GenerationInputValue;
    use crate::application::generation_service::ReferenceManifest;
    use crate::application::ports::ShotReferenceAssetRecord;
    use crate::domain::{
        AssetId, Binding, InputDefinition, OutputDefinition, OutputType, Recipe, SeedDefault,
        ShotStage, WorkflowRef,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    fn asset_id(value: &str) -> AssetId {
        AssetId::parse(format!("ast_{value}")).expect("test asset id")
    }

    #[test]
    fn retry_snapshot_restores_prompt_seed_and_reference_order() {
        let recipe = Recipe {
            schema_version: 1,
            id: "rcp_retry".to_owned(),
            name: "Retry".to_owned(),
            workflow: WorkflowRef {
                file: "workflow_api.json".to_owned(),
            },
            inputs: BTreeMap::from([
                (
                    "prompt".to_owned(),
                    InputDefinition::TextArea {
                        label: "Prompt".to_owned(),
                        required: true,
                        default: None,
                    },
                ),
                (
                    "seed".to_owned(),
                    InputDefinition::Seed {
                        label: "Seed".to_owned(),
                        default: SeedDefault::Random,
                        min: Some(0),
                        max: Some(1000),
                    },
                ),
                (
                    "reference_images".to_owned(),
                    InputDefinition::Images {
                        label: "References".to_owned(),
                        required: true,
                        min_items: 2,
                        max_items: 3,
                    },
                ),
            ]),
            bindings: Vec::new(),
            outputs: Vec::new(),
        };
        let values = input_values_from_snapshot(
            &json!({
                "prompt": "P1",
                "seed": 777,
                "reference_images": {
                    "type": "image_assets",
                    "assetIds": ["ast_b", "ast_a", "ast_c"]
                }
            }),
            &recipe,
        )
        .expect("retry snapshot should restore typed values");

        assert_eq!(
            values["prompt"],
            GenerationInputValue::Text("P1".to_owned())
        );
        assert_eq!(
            values["seed"],
            GenerationInputValue::Seed(crate::domain::SeedValue::Fixed(777))
        );
        assert_eq!(
            values["reference_images"],
            GenerationInputValue::ImageAssets(vec![asset_id("b"), asset_id("a"), asset_id("c")])
        );
    }

    fn video_recipe(input: InputDefinition) -> Recipe {
        Recipe {
            schema_version: 1,
            id: "rcp_test_video".to_owned(),
            name: "Test video".to_owned(),
            workflow: WorkflowRef {
                file: "workflow_api.json".to_owned(),
            },
            inputs: BTreeMap::from([("reference_images".to_owned(), input)]),
            bindings: Vec::<Binding>::new(),
            outputs: vec![OutputDefinition {
                id: "generated_video".to_owned(),
                output_type: OutputType::Video,
                node: "1".to_owned(),
                required: true,
            }],
        }
    }

    #[test]
    fn video_input_keeps_selected_image_for_i2v() {
        let selected = asset_id("keyframe");
        let (key, value, manifest) = build_video_input(
            &video_recipe(InputDefinition::Image {
                label: "Keyframe".to_owned(),
                required: true,
            }),
            Some(selected.clone()),
            Vec::new(),
            None,
        )
        .expect("I2V input should be valid");

        assert_eq!(key, "reference_images");
        assert_eq!(value, GenerationInputValue::ImageAsset(selected));
        assert_eq!(manifest, None);
    }

    #[test]
    fn video_input_preserves_persisted_ref2va_order_and_manifest() {
        let persisted = vec![
            ShotReferenceAssetRecord {
                shot_id: "sht_test".to_owned(),
                stage: ShotStage::Video,
                asset_id: "ast_c".to_owned(),
                ordinal: 2,
            },
            ShotReferenceAssetRecord {
                shot_id: "sht_test".to_owned(),
                stage: ShotStage::Video,
                asset_id: "ast_b".to_owned(),
                ordinal: 0,
            },
            ShotReferenceAssetRecord {
                shot_id: "sht_test".to_owned(),
                stage: ShotStage::Video,
                asset_id: "ast_a".to_owned(),
                ordinal: 1,
            },
        ];
        let ordered = ordered_reference_asset_ids(&persisted, ShotStage::Video)
            .expect("persisted reference order should be readable");
        let (key, value, manifest) = build_video_input(
            &video_recipe(InputDefinition::Images {
                label: "References".to_owned(),
                required: true,
                min_items: 2,
                max_items: 3,
            }),
            None,
            ordered.clone(),
            Some((2, 3)),
        )
        .expect("REF2VA input should be valid without a selected keyframe");

        assert_eq!(key, "reference_images");
        assert_eq!(value, GenerationInputValue::ImageAssets(ordered.clone()));
        assert_eq!(
            manifest,
            Some(ReferenceManifest {
                input_key: "reference_images".to_owned(),
                asset_ids: ordered,
            })
        );
    }

    #[test]
    fn ref2va_validation_errors_name_the_recipe_limit_and_duplicate() {
        let recipe = video_recipe(InputDefinition::Images {
            label: "References".to_owned(),
            required: true,
            min_items: 2,
            max_items: 3,
        });
        let error =
            build_video_input(&recipe, None, vec![asset_id("a")], Some((2, 3))).unwrap_err();
        assert!(matches!(
            error,
            ShotServiceError::InvalidInput(message) if message.contains("至少需要 2")
        ));

        let error = build_video_input(
            &recipe,
            None,
            vec![asset_id("a"), asset_id("b"), asset_id("c"), asset_id("d")],
            Some((2, 3)),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ShotServiceError::InvalidInput(message) if message.contains("最多允许 3")
        ));

        let error = build_video_input(
            &recipe,
            None,
            vec![asset_id("a"), asset_id("a")],
            Some((2, 3)),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ShotServiceError::InvalidInput(message) if message.contains("参考图重复")
        ));
    }
}
