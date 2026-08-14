use crate::application::generation_input_preparer::GenerationInputValue;
use crate::application::generation_service::{
    CreateGenerationRequest, GenerationService, GenerationServiceError, ReferenceManifest,
};
use crate::application::ports::{
    AssetRepository, Clock, GenerationDefinitionRepository, PromptLibraryRepository,
    RepositoryError, ShotBatchRepository, ShotData, ShotRecord, ShotRepository,
    ShotStageConfigRecord, TaskRepository, TaskUpdatePayload,
};
use crate::application::prompt_library_service::canonical_prompt_text;
use crate::application::task_query_service::TaskQueryService;
use crate::compiler::RecipeParser;
use crate::domain::{
    canonical_shot_name, derive_stage_status, validate_project_id, AssetType, InputDefinition,
    OutputType, SeedValue, ShotStage, ShotViewStatus, TaskId,
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
        }
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
        let recipe = RecipeParser::parse(&definition.recipe_yaml)
            .map_err(|error| ShotServiceError::InvalidInput(error.to_string()))?;
        let expected_output = match request.stage {
            ShotStage::Image => OutputType::Image,
            ShotStage::Video => OutputType::Video,
        };
        if !recipe
            .outputs
            .iter()
            .any(|output| output.output_type == expected_output)
        {
            return Err(ShotServiceError::InvalidInput(format!(
                "{} 阶段需要兼容的 {} 输出",
                request.stage.as_str(),
                expected_output_label(expected_output)
            )));
        }
        if request.stage == ShotStage::Video {
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
                return Err(ShotServiceError::InvalidInput(
                    "视频阶段必须有且只有一个图片输入用于 Reference Image".to_owned(),
                ));
            }
        }
        let scalar_values = scalar_values_to_json(&recipe.inputs, &request.values)?;
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
        let config = data
            .stage_configs
            .iter()
            .find(|config| config.stage == request.stage)
            .ok_or_else(|| ShotServiceError::InvalidInput("请先配置当前阶段工作流".to_owned()))?;
        if request.stage == ShotStage::Video {
            let selected = data
                .shot
                .selected_image_asset_id
                .as_deref()
                .ok_or_else(|| ShotServiceError::InvalidInput("请先选择关键帧图片".to_owned()))?;
            let asset = self
                .asset_repository
                .find_by_id(&crate::domain::AssetId::parse(selected.to_owned()).map_err(
                    |error| ShotServiceError::InvalidInput(format!("关键帧素材 ID 无效：{error}")),
                )?)
                .await?
                .ok_or_else(|| ShotServiceError::InvalidInput("关键帧素材不存在".to_owned()))?;
            if asset.project_id != request.project_id || asset.asset_type != AssetType::Image {
                return Err(ShotServiceError::InvalidInput(
                    "关键帧必须是当前项目的图片素材".to_owned(),
                ));
            }
            self.ensure_no_active_video_tasks(&request.project_id)
                .await?;
        }

        let definition = self
            .definition_repository
            .find(&config.workflow_version_id, &config.recipe_id)
            .await?
            .ok_or_else(|| ShotServiceError::InvalidInput("当前阶段工作流已不可用".to_owned()))?;
        let recipe = RecipeParser::parse(&definition.recipe_yaml)
            .map_err(|error| ShotServiceError::InvalidInput(error.to_string()))?;
        let _ = scalar_values_to_json(&recipe.inputs, &request.values)?;
        let mut values = scalar_values_from_json(&config.scalar_values)?;
        values.extend(request.values);
        let mut reference_manifest = None;
        if let Some(prompt_key) = recipe.inputs.iter().find_map(|(key, input)| {
            matches!(input, InputDefinition::TextArea { .. }).then_some(key)
        }) {
            values.insert(
                prompt_key.clone(),
                GenerationInputValue::Text(data.shot.prompt_text.clone()),
            );
        }
        let image_input = recipe.inputs.iter().find_map(|(key, input)| match input {
            InputDefinition::Image { .. } => Some((key, false)),
            InputDefinition::Images { .. } => Some((key, true)),
            _ => None,
        });
        if request.stage == ShotStage::Video {
            let (key, multiple) = image_input.ok_or_else(|| {
                ShotServiceError::InvalidInput("视频 Recipe 没有可用的图片输入".to_owned())
            })?;
            let selected = data
                .shot
                .selected_image_asset_id
                .as_ref()
                .expect("video stage selected image is validated above");
            let selected = crate::domain::AssetId::parse(selected.clone())
                .map_err(|error| ShotServiceError::InvalidInput(error.to_string()))?;
            let mut references = vec![selected.clone()];
            for reference in data
                .reference_assets
                .iter()
                .filter(|reference| reference.stage == request.stage)
            {
                let asset_id = crate::domain::AssetId::parse(reference.asset_id.clone())
                    .map_err(|error| ShotServiceError::InvalidInput(error.to_string()))?;
                if references.contains(&asset_id) {
                    continue;
                }
                let asset = self
                    .asset_repository
                    .find_by_id(&asset_id)
                    .await?
                    .ok_or_else(|| {
                        ShotServiceError::InvalidInput("Reference 素材不存在".to_owned())
                    })?;
                if asset.project_id != request.project_id || asset.asset_type != AssetType::Image {
                    return Err(ShotServiceError::InvalidInput(
                        "Reference 必须是当前项目的图片素材".to_owned(),
                    ));
                }
                references.push(asset_id);
            }
            values.insert(
                key.clone(),
                if multiple {
                    reference_manifest = Some(ReferenceManifest {
                        input_key: key.clone(),
                        asset_ids: references.clone(),
                    });
                    GenerationInputValue::ImageAssets(references)
                } else {
                    GenerationInputValue::ImageAsset(selected)
                },
            );
        } else {
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
                let (key, multiple) = image_input.ok_or_else(|| {
                    ShotServiceError::InvalidInput("当前阶段 Recipe 没有可用的图片输入".to_owned())
                })?;
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
            workflow_version_id: config.workflow_version_id.clone(),
            recipe_id: config.recipe_id.clone(),
            values,
            reference_manifest,
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

    async fn views(&self, data: Vec<ShotData>) -> Result<Vec<ShotView>, ShotServiceError> {
        let mut result = Vec::with_capacity(data.len());
        for item in data {
            result.push(self.view(item).await?);
        }
        Ok(result)
    }

    async fn view(&self, data: ShotData) -> Result<ShotView, ShotServiceError> {
        let mut task_statuses = HashMap::new();
        let mut generation_links = Vec::with_capacity(data.generation_links.len());
        for link in &data.generation_links {
            let task = if let Some(task_id) = &link.task_id {
                let task_id = TaskId::parse(task_id.clone())
                    .map_err(|error| ShotServiceError::InvalidInput(error.to_string()))?;
                let task = self
                    .task_repository
                    .find_by_id(&task_id)
                    .await?
                    .ok_or_else(|| ShotServiceError::NotFound(task_id.to_string()))?;
                if task.project_id != data.shot.project_id {
                    return Err(ShotServiceError::InvalidInput(
                        "Shot generation link 跨项目".to_owned(),
                    ));
                }
                task_statuses.insert(task.id.to_string(), task.status);
                Some(
                    self.task_query_service
                        .view(task)
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

fn expected_output_label(output: OutputType) -> &'static str {
    match output {
        OutputType::Image => "image",
        OutputType::Video => "video",
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
