use crate::application::ports::{
    AssetRepository, Clock, ExternalProductionHandoffAssetReference,
    ExternalProductionHandoffEntityMapping, ExternalProductionHandoffEpisode,
    ExternalProductionHandoffImportPlan, ExternalProductionHandoffImportResult,
    ExternalProductionHandoffPrompt, ExternalProductionHandoffRecord,
    ExternalProductionHandoffRepository, ExternalProductionHandoffScene,
    ExternalProductionHandoffSceneAssignment, ExternalProductionHandoffSeries,
    ExternalProductionHandoffShot, ExternalProductionHandoffStageConfig,
    GenerationDefinitionRepository, ProjectRepository, RepositoryError,
};
use crate::domain::{
    canonical_shot_name, validate_project_id, AssetId, ProductionEpisodeId, ProductionSceneId,
    ProductionSeriesId,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
    sync::Arc,
};
use uuid::Uuid;

pub const MAX_HANDOFF_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_HANDOFF_SHOTS: usize = 500;
const MAX_SERIES: usize = 100;
const MAX_EPISODES_PER_SERIES: usize = 100;
const MAX_SCENES_PER_EPISODE: usize = 200;
const MAX_NAME_CHARS: usize = 100;
const MAX_DESCRIPTION_CHARS: usize = 1000;
const MAX_PROMPT_BYTES: usize = 64 * 1024;
const MAX_ASSET_REFS_PER_SHOT: usize = 20;
const MAX_EXTERNAL_ID_CHARS: usize = 200;
const MAX_SOURCE_FIELD_CHARS: usize = 200;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExternalProductionHandoffV1 {
    pub schema_version: u32,
    pub project_id: String,
    pub source: ExternalProductionHandoffSource,
    pub series: Vec<ExternalProductionHandoffSeriesInput>,
    #[serde(default)]
    pub limits: Option<ExternalProductionHandoffLimits>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExternalProductionHandoffSource {
    pub agent: String,
    #[serde(default)]
    pub revision: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExternalProductionHandoffLimits {
    #[serde(default)]
    pub max_shots: Option<usize>,
    #[serde(default)]
    pub max_series: Option<usize>,
    #[serde(default)]
    pub max_episodes: Option<usize>,
    #[serde(default)]
    pub max_scenes: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExternalProductionHandoffSeriesInput {
    pub external_id: String,
    pub name: String,
    pub description: String,
    pub ordinal: u32,
    pub episodes: Vec<ExternalProductionHandoffEpisodeInput>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExternalProductionHandoffEpisodeInput {
    pub external_id: String,
    pub name: String,
    pub description: String,
    pub ordinal: u32,
    pub scenes: Vec<ExternalProductionHandoffSceneInput>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExternalProductionHandoffSceneInput {
    pub external_id: String,
    pub name: String,
    pub description: String,
    pub ordinal: u32,
    pub shots: Vec<ExternalProductionHandoffShotInput>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExternalProductionHandoffShotInput {
    pub external_id: String,
    pub name: String,
    pub ordinal: u32,
    pub description: String,
    #[serde(default)]
    pub image_prompt: Option<String>,
    #[serde(default)]
    pub video_prompt: Option<String>,
    #[serde(default)]
    pub asset_refs: Vec<ExternalProductionHandoffAssetRefInput>,
    #[serde(default)]
    pub stages: ExternalProductionHandoffStages,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExternalProductionHandoffStages {
    #[serde(default)]
    pub image: Option<ExternalProductionHandoffStageInput>,
    #[serde(default)]
    pub video: Option<ExternalProductionHandoffStageInput>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExternalProductionHandoffStageInput {
    pub workflow_version_id: String,
    pub recipe_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExternalProductionHandoffAssetRefInput {
    pub asset_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalProductionHandoffIssue {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalProductionHandoffReplay {
    pub status: String,
    pub handoff_id: Option<String>,
    pub mappings: Vec<ExternalProductionHandoffEntityMappingView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalProductionHandoffEntityMappingView {
    pub entity_kind: String,
    pub external_id: String,
    pub formal_entity_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalProductionHandoffPreview {
    pub project_id: String,
    pub document_sha256: String,
    pub series_count: usize,
    pub episode_count: usize,
    pub scene_count: usize,
    pub shot_count: usize,
    pub normalized: ExternalProductionHandoffV1,
    pub errors: Vec<ExternalProductionHandoffIssue>,
    pub warnings: Vec<ExternalProductionHandoffIssue>,
    pub replay: ExternalProductionHandoffReplay,
    pub write_plan: ExternalProductionHandoffWritePlan,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalProductionHandoffWritePlan {
    pub creates_series: usize,
    pub creates_episodes: usize,
    pub creates_scenes: usize,
    pub creates_shots: usize,
    pub writes_prompts: usize,
    pub writes_asset_references: usize,
    pub writes_stage_configs: usize,
    pub creates_provenance_mappings: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalProductionHandoffConfirmResult {
    pub project_id: String,
    pub handoff_id: String,
    pub document_sha256: String,
    pub replayed: bool,
    pub series_count: usize,
    pub episode_count: usize,
    pub scene_count: usize,
    pub shot_count: usize,
    pub mappings: Vec<ExternalProductionHandoffEntityMappingView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalProductionHandoffHistoryItem {
    pub id: String,
    pub project_id: String,
    pub schema_version: u32,
    pub source_agent: String,
    pub source_revision: Option<String>,
    pub document_sha256: String,
    pub imported_at: DateTime<Utc>,
}

#[derive(Debug)]
pub enum ExternalProductionHandoffError {
    Contract { code: &'static str, message: String },
    Repository(RepositoryError),
}

impl ExternalProductionHandoffError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Contract { code, .. } => code,
            Self::Repository(_) => "HANDOFF_TRANSACTION_FAILED",
        }
    }
}

impl fmt::Display for ExternalProductionHandoffError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract { code, message } => write!(formatter, "{code}: {message}"),
            Self::Repository(error) => write!(formatter, "HANDOFF_TRANSACTION_FAILED: {error}"),
        }
    }
}

impl Error for ExternalProductionHandoffError {}

impl From<RepositoryError> for ExternalProductionHandoffError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

pub struct ExternalProductionHandoffService {
    repository: Arc<dyn ExternalProductionHandoffRepository>,
    project_repository: Arc<dyn ProjectRepository>,
    asset_repository: Arc<dyn AssetRepository>,
    definition_repository: Arc<dyn GenerationDefinitionRepository>,
    clock: Arc<dyn Clock>,
}

impl ExternalProductionHandoffService {
    pub fn new(
        repository: Arc<dyn ExternalProductionHandoffRepository>,
        project_repository: Arc<dyn ProjectRepository>,
        asset_repository: Arc<dyn AssetRepository>,
        definition_repository: Arc<dyn GenerationDefinitionRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repository,
            project_repository,
            asset_repository,
            definition_repository,
            clock,
        }
    }

    pub async fn preview(
        &self,
        project_id: &str,
        content: &str,
    ) -> Result<ExternalProductionHandoffPreview, ExternalProductionHandoffError> {
        validate_project_id(project_id)
            .map_err(|error| contract("HANDOFF_PROJECT_MISMATCH", error.to_string()))?;
        let (document, canonical, sha256) = parse_document(content)?;
        let mut errors = Vec::new();
        let plan = self
            .build_plan(project_id, &document, &sha256, &mut errors)
            .await?;
        let replay = self.replay_state(project_id, &document, &sha256).await?;
        if replay.status == "SOURCE_REVISION_CONFLICT" {
            errors.push(issue(
                "HANDOFF_SOURCE_REVISION_CONFLICT",
                "同一 source.agent + source.revision 已对应不同文档。",
                None,
            ));
        }
        let _ = canonical;
        Ok(preview_from_plan(document, plan, sha256, errors, replay))
    }

    pub async fn confirm(
        &self,
        project_id: &str,
        content: &str,
        expected_document_sha256: &str,
    ) -> Result<ExternalProductionHandoffConfirmResult, ExternalProductionHandoffError> {
        let preview = self.preview(project_id, content).await?;
        if !expected_document_sha256.eq_ignore_ascii_case(&preview.document_sha256) {
            return Err(contract(
                "HANDOFF_PREVIEW_STALE",
                "确认文档与预览摘要不一致，请重新预览。",
            ));
        }
        if preview.replay.status == "SOURCE_REVISION_CONFLICT" {
            return Err(contract(
                "HANDOFF_SOURCE_REVISION_CONFLICT",
                "同一 source.agent + source.revision 已对应不同文档。",
            ));
        }
        if preview.replay.status == "ALREADY_IMPORTED" {
            let handoff_id = preview.replay.handoff_id.clone().ok_or_else(|| {
                contract("HANDOFF_ALREADY_IMPORTED", "已导入记录缺少 handoffId。")
            })?;
            return Ok(ExternalProductionHandoffConfirmResult {
                project_id: project_id.to_owned(),
                handoff_id,
                document_sha256: preview.document_sha256,
                replayed: true,
                series_count: preview.series_count,
                episode_count: preview.episode_count,
                scene_count: preview.scene_count,
                shot_count: preview.shot_count,
                mappings: preview.replay.mappings,
            });
        }
        if let Some(error) = preview.errors.first() {
            return Err(contract_owned(&error.code, error.message.clone()));
        }
        let (document, _, sha256) = parse_document(content)?;
        let mut errors = Vec::new();
        let plan = self
            .build_plan(project_id, &document, &sha256, &mut errors)
            .await?;
        if let Some(error) = errors.first() {
            return Err(contract_owned(&error.code, error.message.clone()));
        }
        let result = self.repository.import_atomic(&plan).await?;
        Ok(confirm_result(
            result,
            project_id,
            plan.series.len(),
            plan.episodes.len(),
            plan.scenes.len(),
            plan.shots.len(),
        ))
    }

    pub async fn list(
        &self,
        project_id: &str,
    ) -> Result<Vec<ExternalProductionHandoffHistoryItem>, ExternalProductionHandoffError> {
        validate_project_id(project_id)
            .map_err(|error| contract("HANDOFF_PROJECT_MISMATCH", error.to_string()))?;
        Ok(self
            .repository
            .list(project_id)
            .await?
            .into_iter()
            .map(history_item)
            .collect())
    }

    pub async fn mappings(
        &self,
        project_id: &str,
        handoff_id: &str,
    ) -> Result<Vec<ExternalProductionHandoffEntityMappingView>, ExternalProductionHandoffError>
    {
        validate_project_id(project_id)
            .map_err(|error| contract("HANDOFF_PROJECT_MISMATCH", error.to_string()))?;
        Ok(self
            .repository
            .mappings(project_id, handoff_id)
            .await?
            .into_iter()
            .map(mapping_view)
            .collect())
    }

    async fn replay_state(
        &self,
        project_id: &str,
        document: &ExternalProductionHandoffV1,
        sha256: &str,
    ) -> Result<ExternalProductionHandoffReplay, ExternalProductionHandoffError> {
        if let Some(existing) = self
            .repository
            .find_by_document_hash(project_id, sha256)
            .await?
        {
            return Ok(ExternalProductionHandoffReplay {
                status: "ALREADY_IMPORTED".to_owned(),
                handoff_id: Some(existing.handoff.id),
                mappings: existing.mappings.into_iter().map(mapping_view).collect(),
            });
        }
        if let Some(revision) = document.source.revision.as_deref() {
            if self
                .repository
                .find_by_source_revision(project_id, &document.source.agent, revision)
                .await?
                .is_some()
            {
                return Ok(ExternalProductionHandoffReplay {
                    status: "SOURCE_REVISION_CONFLICT".to_owned(),
                    handoff_id: None,
                    mappings: Vec::new(),
                });
            }
        }
        Ok(ExternalProductionHandoffReplay {
            status: "NEW".to_owned(),
            handoff_id: None,
            mappings: Vec::new(),
        })
    }

    async fn build_plan(
        &self,
        project_id: &str,
        document: &ExternalProductionHandoffV1,
        sha256: &str,
        errors: &mut Vec<ExternalProductionHandoffIssue>,
    ) -> Result<ExternalProductionHandoffImportPlan, ExternalProductionHandoffError> {
        if document.schema_version != 1 {
            errors.push(issue(
                "HANDOFF_SCHEMA_UNSUPPORTED",
                "只支持 schemaVersion=1。",
                Some("schemaVersion"),
            ));
        }
        if document.project_id != project_id {
            errors.push(issue(
                "HANDOFF_PROJECT_MISMATCH",
                "文档 projectId 必须与当前项目一致。",
                Some("projectId"),
            ));
        }
        validate_text(
            &document.source.agent,
            MAX_SOURCE_FIELD_CHARS,
            "source.agent",
            errors,
        );
        if let Some(revision) = &document.source.revision {
            validate_text(revision, MAX_SOURCE_FIELD_CHARS, "source.revision", errors);
        }
        let _project = self
            .project_repository
            .find_by_id(project_id)
            .await?
            .ok_or_else(|| contract("HANDOFF_PROJECT_MISMATCH", "目标项目不存在。"))?;
        if document.series.len() > MAX_SERIES {
            errors.push(issue(
                "HANDOFF_TOO_LARGE",
                "Series 数量超过限制。",
                Some("series"),
            ));
        }
        if let Some(limit) = document.limits.as_ref().and_then(|value| value.max_series) {
            if limit > MAX_SERIES {
                errors.push(issue(
                    "HANDOFF_TOO_LARGE",
                    "limits.maxSeries 超过服务器限制。",
                    Some("limits.maxSeries"),
                ));
            }
        }
        let handoff_id = format!("hnd_{}", Uuid::new_v4().simple());
        let now = self.clock.now();
        let mut series = Vec::new();
        let mut episodes = Vec::new();
        let mut scenes = Vec::new();
        let mut shots = Vec::new();
        let mut prompts: Vec<ExternalProductionHandoffPrompt> = Vec::new();
        let mut stage_configs: Vec<ExternalProductionHandoffStageConfig> = Vec::new();
        let mut references: Vec<ExternalProductionHandoffAssetReference> = Vec::new();
        let mut assignments: Vec<ExternalProductionHandoffSceneAssignment> = Vec::new();
        let mut mappings: Vec<ExternalProductionHandoffEntityMapping> = Vec::new();
        let mut total_shots = 0usize;
        let mut total_episodes = 0usize;
        let mut total_scenes = 0usize;
        let mut formal_shot_ordinal = 0i64;
        let mut series_ids = HashSet::new();
        for (series_index, input_series) in document.series.iter().enumerate() {
            validate_identity(
                input_series.external_id.as_str(),
                &format!("series[{series_index}].externalId"),
                &mut series_ids,
                errors,
            );
            validate_metadata(
                &input_series.name,
                &input_series.description,
                &format!("series[{series_index}]"),
                errors,
            );
            validate_ordinal(
                input_series.ordinal,
                &format!("series[{series_index}].ordinal"),
                errors,
            );
            let series_id = ProductionSeriesId::new().to_string();
            series.push(ExternalProductionHandoffSeries {
                id: series_id.clone(),
                project_id: project_id.to_owned(),
                ordinal: input_series.ordinal.saturating_sub(1),
                name: input_series.name.clone(),
                description: input_series.description.clone(),
                created_at: now,
                updated_at: now,
            });
            mappings.push(mapping(
                &handoff_id,
                "series",
                &input_series.external_id,
                &series_id,
            ));
            let mut episode_ids = HashSet::new();
            let mut episode_ordinals = HashSet::new();
            if input_series.episodes.len() > MAX_EPISODES_PER_SERIES {
                errors.push(issue(
                    "HANDOFF_TOO_LARGE",
                    "Episode 数量超过限制。",
                    Some(&format!("series[{series_index}].episodes")),
                ));
            }
            for (episode_index, input_episode) in input_series.episodes.iter().enumerate() {
                total_episodes += 1;
                validate_identity(
                    &input_episode.external_id,
                    &format!("series[{series_index}].episodes[{episode_index}].externalId"),
                    &mut episode_ids,
                    errors,
                );
                if !episode_ordinals.insert(input_episode.ordinal) {
                    errors.push(issue(
                        "HANDOFF_DUPLICATE_ORDINAL",
                        "同一 Series 内 Episode ordinal 不能重复。",
                        Some(&format!("series[{series_index}].episodes")),
                    ));
                }
                validate_metadata(
                    &input_episode.name,
                    &input_episode.description,
                    &format!("series[{series_index}].episodes[{episode_index}]"),
                    errors,
                );
                validate_ordinal(
                    input_episode.ordinal,
                    &format!("series[{series_index}].episodes[{episode_index}].ordinal"),
                    errors,
                );
                let episode_id = ProductionEpisodeId::new().to_string();
                episodes.push(ExternalProductionHandoffEpisode {
                    id: episode_id.clone(),
                    series_id: series_id.clone(),
                    ordinal: input_episode.ordinal.saturating_sub(1),
                    name: input_episode.name.clone(),
                    description: input_episode.description.clone(),
                    created_at: now,
                    updated_at: now,
                });
                mappings.push(mapping(
                    &handoff_id,
                    "episode",
                    &input_episode.external_id,
                    &episode_id,
                ));
                let mut scene_ids = HashSet::new();
                let mut scene_ordinals = HashSet::new();
                if input_episode.scenes.len() > MAX_SCENES_PER_EPISODE {
                    errors.push(issue(
                        "HANDOFF_TOO_LARGE",
                        "Scene 数量超过限制。",
                        Some(&format!(
                            "series[{series_index}].episodes[{episode_index}].scenes"
                        )),
                    ));
                }
                for (scene_index, input_scene) in input_episode.scenes.iter().enumerate() {
                    total_scenes += 1;
                    validate_identity(&input_scene.external_id, &format!("series[{series_index}].episodes[{episode_index}].scenes[{scene_index}].externalId"), &mut scene_ids, errors);
                    if !scene_ordinals.insert(input_scene.ordinal) {
                        errors.push(issue(
                            "HANDOFF_DUPLICATE_ORDINAL",
                            "同一 Episode 内 Scene ordinal 不能重复。",
                            Some(&format!(
                                "series[{series_index}].episodes[{episode_index}].scenes"
                            )),
                        ));
                    }
                    validate_metadata(&input_scene.name, &input_scene.description, &format!("series[{series_index}].episodes[{episode_index}].scenes[{scene_index}]"), errors);
                    validate_ordinal(input_scene.ordinal, &format!("series[{series_index}].episodes[{episode_index}].scenes[{scene_index}].ordinal"), errors);
                    let scene_id = ProductionSceneId::new().to_string();
                    scenes.push(ExternalProductionHandoffScene {
                        id: scene_id.clone(),
                        episode_id: episode_id.clone(),
                        ordinal: input_scene.ordinal.saturating_sub(1),
                        name: input_scene.name.clone(),
                        description: input_scene.description.clone(),
                        created_at: now,
                        updated_at: now,
                    });
                    mappings.push(mapping(
                        &handoff_id,
                        "scene",
                        &input_scene.external_id,
                        &scene_id,
                    ));
                    let mut shot_ids = HashSet::new();
                    let mut shot_ordinals = HashSet::new();
                    for (shot_index, input_shot) in input_scene.shots.iter().enumerate() {
                        total_shots += 1;
                        if total_shots > MAX_HANDOFF_SHOTS {
                            errors.push(issue(
                                "HANDOFF_TOO_LARGE",
                                "Shot 数量超过 500。",
                                Some("series"),
                            ));
                        }
                        validate_identity(&input_shot.external_id, &format!("series[{series_index}].episodes[{episode_index}].scenes[{scene_index}].shots[{shot_index}].externalId"), &mut shot_ids, errors);
                        if !shot_ordinals.insert(input_shot.ordinal) {
                            errors.push(issue("HANDOFF_DUPLICATE_ORDINAL", "同一 Scene 内 Shot ordinal 不能重复。", Some(&format!("series[{series_index}].episodes[{episode_index}].scenes[{scene_index}].shots"))));
                        }
                        validate_ordinal(input_shot.ordinal, &format!("series[{series_index}].episodes[{episode_index}].scenes[{scene_index}].shots[{shot_index}].ordinal"), errors);
                        validate_metadata(&input_shot.name, &input_shot.description, &format!("series[{series_index}].episodes[{episode_index}].scenes[{scene_index}].shots[{shot_index}]"), errors);
                        if let Err(error) = canonical_shot_name(&input_shot.name) {
                            errors.push(issue(
                                "HANDOFF_SCHEMA_UNSUPPORTED",
                                error.to_string(),
                                Some("name"),
                            ));
                        }
                        for (stage, prompt) in [
                            ("image", input_shot.image_prompt.as_ref()),
                            ("video", input_shot.video_prompt.as_ref()),
                        ] {
                            if let Some(prompt) = prompt {
                                if prompt.len() > MAX_PROMPT_BYTES {
                                    errors.push(issue(
                                        "HANDOFF_TOO_LARGE",
                                        "Prompt 超过 64 KiB。",
                                        None,
                                    ));
                                }
                                prompts.push(ExternalProductionHandoffPrompt {
                                    shot_id: String::new(),
                                    stage: stage.to_owned(),
                                    prompt_text: prompt.clone(),
                                    updated_at: now,
                                });
                            }
                        }
                        if input_shot.asset_refs.len() > MAX_ASSET_REFS_PER_SHOT {
                            errors.push(issue(
                                "HANDOFF_TOO_LARGE",
                                "单个 Shot 的 assetRefs 数量超过限制。",
                                None,
                            ));
                        }
                        let shot_id = format!("sht_{}", Uuid::new_v4().simple());
                        shots.push(ExternalProductionHandoffShot {
                            id: shot_id.clone(),
                            project_id: project_id.to_owned(),
                            ordinal: formal_shot_ordinal,
                            name: input_shot.name.clone(),
                            description: input_shot.description.clone(),
                            created_at: now,
                            updated_at: now,
                        });
                        formal_shot_ordinal += 1;
                        mappings.push(mapping(
                            &handoff_id,
                            "shot",
                            &input_shot.external_id,
                            &shot_id,
                        ));
                        assignments.push(ExternalProductionHandoffSceneAssignment {
                            shot_id: shot_id.clone(),
                            scene_id: scene_id.clone(),
                            ordinal: input_shot.ordinal.saturating_sub(1) as i64,
                            created_at: now,
                            updated_at: now,
                        });
                        for asset_ref in &input_shot.asset_refs {
                            references.push(ExternalProductionHandoffAssetReference {
                                shot_id: shot_id.clone(),
                                stage: "image".to_owned(),
                                asset_id: asset_ref.asset_id.clone(),
                                ordinal: references
                                    .iter()
                                    .filter(|item| item.shot_id == shot_id && item.stage == "image")
                                    .count() as i64,
                            });
                            references.push(ExternalProductionHandoffAssetReference {
                                shot_id: shot_id.clone(),
                                stage: "video".to_owned(),
                                asset_id: asset_ref.asset_id.clone(),
                                ordinal: references
                                    .iter()
                                    .filter(|item| item.shot_id == shot_id && item.stage == "video")
                                    .count() as i64,
                            });
                        }
                        for (stage, config) in [
                            ("image", input_shot.stages.image.as_ref()),
                            ("video", input_shot.stages.video.as_ref()),
                        ] {
                            if let Some(config) = config {
                                if config.workflow_version_id.trim().is_empty()
                                    || config.recipe_id.trim().is_empty()
                                {
                                    errors.push(issue(
                                        "HANDOFF_WORKFLOW_UNAVAILABLE",
                                        "Workflow/Recipe 标识不能为空。",
                                        None,
                                    ));
                                } else if self
                                    .definition_repository
                                    .find_active(&config.workflow_version_id, &config.recipe_id)
                                    .await?
                                    .is_none()
                                {
                                    let code = if self
                                        .definition_repository
                                        .workflow_version_is_active(&config.workflow_version_id)
                                        .await?
                                    {
                                        "HANDOFF_RECIPE_UNAVAILABLE"
                                    } else {
                                        "HANDOFF_WORKFLOW_UNAVAILABLE"
                                    };
                                    errors.push(issue(
                                        code,
                                        "精确 workflowVersionId + recipeId 不可用或已归档。",
                                        None,
                                    ));
                                }
                                stage_configs.push(ExternalProductionHandoffStageConfig {
                                    shot_id: shot_id.clone(),
                                    stage: stage.to_owned(),
                                    workflow_version_id: config.workflow_version_id.clone(),
                                    recipe_id: config.recipe_id.clone(),
                                    scalar_values: Value::Object(Default::default()),
                                    updated_at: now,
                                });
                            }
                        }
                        for prompt in prompts.iter_mut().rev().take(2) {
                            if prompt.shot_id.is_empty() {
                                prompt.shot_id = shot_id.clone();
                            }
                        }
                    }
                }
            }
        }
        let mut asset_ids = Vec::new();
        for asset_id in references
            .iter()
            .map(|reference| reference.asset_id.clone())
            .collect::<HashSet<_>>()
        {
            match AssetId::parse(asset_id) {
                Ok(value) => asset_ids.push(value),
                Err(error) => errors.push(issue(
                    "HANDOFF_ASSET_NOT_FOUND",
                    error.to_string(),
                    Some("assetRefs"),
                )),
            }
        }
        let assets = self.asset_repository.find_many_by_ids(&asset_ids).await?;
        let by_id = assets
            .into_iter()
            .map(|asset| (asset.id.as_str().to_owned(), asset))
            .collect::<HashMap<_, _>>();
        for reference in &references {
            match by_id.get(&reference.asset_id) {
                None => errors.push(issue(
                    "HANDOFF_ASSET_NOT_FOUND",
                    "Asset 不存在。",
                    Some("assetRefs"),
                )),
                Some(asset) if asset.project_id != project_id => errors.push(issue(
                    "HANDOFF_ASSET_PROJECT_MISMATCH",
                    "Asset 不属于当前项目。",
                    Some("assetRefs"),
                )),
                Some(asset) if asset.asset_type != crate::domain::AssetType::Image => {
                    errors.push(issue(
                        "HANDOFF_ASSET_NOT_FOUND",
                        "Shot assetRefs 只支持 image Asset。",
                        Some("assetRefs"),
                    ))
                }
                Some(_) => {}
            }
        }
        if total_shots == 0 {
            errors.push(issue(
                "HANDOFF_SCHEMA_UNSUPPORTED",
                "至少需要一个 Shot。",
                Some("series"),
            ));
        }
        if let Some(limit) = document.limits.as_ref().and_then(|value| value.max_shots) {
            if limit > MAX_HANDOFF_SHOTS || total_shots > limit {
                errors.push(issue(
                    "HANDOFF_TOO_LARGE",
                    "Shot 数量超过 limits.maxShots 或服务器限制。",
                    Some("limits.maxShots"),
                ));
            }
        }
        if let Some(limit) = document.limits.as_ref().and_then(|value| value.max_series) {
            if limit > MAX_SERIES || document.series.len() > limit {
                errors.push(issue(
                    "HANDOFF_TOO_LARGE",
                    "Series 数量超过 limits.maxSeries 或服务器限制。",
                    Some("limits.maxSeries"),
                ));
            }
        }
        if let Some(limit) = document
            .limits
            .as_ref()
            .and_then(|value| value.max_episodes)
        {
            if limit > MAX_EPISODES_PER_SERIES || total_episodes > limit {
                errors.push(issue(
                    "HANDOFF_TOO_LARGE",
                    "Episode 数量超过 limits.maxEpisodes 或服务器限制。",
                    Some("limits.maxEpisodes"),
                ));
            }
        }
        if let Some(limit) = document.limits.as_ref().and_then(|value| value.max_scenes) {
            if limit > MAX_SCENES_PER_EPISODE || total_scenes > limit {
                errors.push(issue(
                    "HANDOFF_TOO_LARGE",
                    "Scene 数量超过 limits.maxScenes 或服务器限制。",
                    Some("limits.maxScenes"),
                ));
            }
        }
        let handoff = ExternalProductionHandoffRecord {
            id: handoff_id.clone(),
            project_id: project_id.to_owned(),
            schema_version: 1,
            source_agent: document.source.agent.clone(),
            source_revision: document.source.revision.clone(),
            document_sha256: sha256.to_owned(),
            imported_at: now,
        };
        Ok(ExternalProductionHandoffImportPlan {
            handoff,
            series,
            episodes,
            scenes,
            shots,
            prompts,
            stage_configs,
            asset_references: references,
            assignments,
            mappings,
        })
    }
}

fn parse_document(
    content: &str,
) -> Result<(ExternalProductionHandoffV1, String, String), ExternalProductionHandoffError> {
    if content.as_bytes().len() > MAX_HANDOFF_DOCUMENT_BYTES {
        return Err(contract("HANDOFF_TOO_LARGE", "handoff JSON 超过 4 MiB。"));
    }
    let document =
        serde_json::from_str::<ExternalProductionHandoffV1>(content).map_err(|error| {
            let code = if error.to_string().contains("unknown field") {
                "HANDOFF_UNKNOWN_FIELD"
            } else {
                "HANDOFF_SCHEMA_UNSUPPORTED"
            };
            contract(code, format!("handoff JSON 无效：{error}"))
        })?;
    let canonical_bytes = serde_json::to_vec(&document)
        .map_err(|error| contract("HANDOFF_SCHEMA_UNSUPPORTED", error.to_string()))?;
    let sha256 = format!("{:x}", Sha256::digest(&canonical_bytes));
    Ok((
        document,
        String::from_utf8(canonical_bytes).unwrap_or_default(),
        sha256,
    ))
}

fn preview_from_plan(
    document: ExternalProductionHandoffV1,
    plan: ExternalProductionHandoffImportPlan,
    sha256: String,
    errors: Vec<ExternalProductionHandoffIssue>,
    replay: ExternalProductionHandoffReplay,
) -> ExternalProductionHandoffPreview {
    ExternalProductionHandoffPreview {
        project_id: plan.handoff.project_id,
        document_sha256: sha256,
        series_count: plan.series.len(),
        episode_count: plan.episodes.len(),
        scene_count: plan.scenes.len(),
        shot_count: plan.shots.len(),
        normalized: document,
        errors,
        warnings: Vec::new(),
        replay,
        write_plan: ExternalProductionHandoffWritePlan {
            creates_series: plan.series.len(),
            creates_episodes: plan.episodes.len(),
            creates_scenes: plan.scenes.len(),
            creates_shots: plan.shots.len(),
            writes_prompts: plan.prompts.len(),
            writes_asset_references: plan.asset_references.len(),
            writes_stage_configs: plan.stage_configs.len(),
            creates_provenance_mappings: plan.mappings.len(),
        },
    }
}

fn confirm_result(
    result: ExternalProductionHandoffImportResult,
    project_id: &str,
    series_count: usize,
    episode_count: usize,
    scene_count: usize,
    shot_count: usize,
) -> ExternalProductionHandoffConfirmResult {
    ExternalProductionHandoffConfirmResult {
        project_id: project_id.to_owned(),
        handoff_id: result.handoff.id,
        document_sha256: result.handoff.document_sha256,
        replayed: result.replayed,
        series_count,
        episode_count,
        scene_count,
        shot_count,
        mappings: result.mappings.into_iter().map(mapping_view).collect(),
    }
}

fn history_item(value: ExternalProductionHandoffRecord) -> ExternalProductionHandoffHistoryItem {
    ExternalProductionHandoffHistoryItem {
        id: value.id,
        project_id: value.project_id,
        schema_version: value.schema_version,
        source_agent: value.source_agent,
        source_revision: value.source_revision,
        document_sha256: value.document_sha256,
        imported_at: value.imported_at,
    }
}
fn mapping_view(
    value: ExternalProductionHandoffEntityMapping,
) -> ExternalProductionHandoffEntityMappingView {
    ExternalProductionHandoffEntityMappingView {
        entity_kind: value.entity_kind,
        external_id: value.external_id,
        formal_entity_id: value.formal_entity_id,
    }
}
fn mapping(
    handoff_id: &str,
    entity_kind: &str,
    external_id: &str,
    formal_entity_id: &str,
) -> ExternalProductionHandoffEntityMapping {
    ExternalProductionHandoffEntityMapping {
        handoff_id: handoff_id.to_owned(),
        entity_kind: entity_kind.to_owned(),
        external_id: external_id.to_owned(),
        formal_entity_id: formal_entity_id.to_owned(),
    }
}
fn issue(
    code: &str,
    message: impl Into<String>,
    path: Option<&str>,
) -> ExternalProductionHandoffIssue {
    ExternalProductionHandoffIssue {
        severity: "error".to_owned(),
        code: code.to_owned(),
        message: message.into(),
        path: path.map(str::to_owned),
    }
}
fn contract(code: &'static str, message: impl Into<String>) -> ExternalProductionHandoffError {
    ExternalProductionHandoffError::Contract {
        code,
        message: message.into(),
    }
}
fn contract_owned(code: &str, message: String) -> ExternalProductionHandoffError {
    let code = match code {
        "HANDOFF_UNKNOWN_FIELD" => "HANDOFF_UNKNOWN_FIELD",
        "HANDOFF_PROJECT_MISMATCH" => "HANDOFF_PROJECT_MISMATCH",
        "HANDOFF_TOO_LARGE" => "HANDOFF_TOO_LARGE",
        "HANDOFF_DUPLICATE_EXTERNAL_ID" => "HANDOFF_DUPLICATE_EXTERNAL_ID",
        "HANDOFF_DUPLICATE_ORDINAL" => "HANDOFF_DUPLICATE_ORDINAL",
        "HANDOFF_ASSET_NOT_FOUND" => "HANDOFF_ASSET_NOT_FOUND",
        "HANDOFF_ASSET_PROJECT_MISMATCH" => "HANDOFF_ASSET_PROJECT_MISMATCH",
        "HANDOFF_WORKFLOW_UNAVAILABLE" => "HANDOFF_WORKFLOW_UNAVAILABLE",
        "HANDOFF_RECIPE_UNAVAILABLE" => "HANDOFF_RECIPE_UNAVAILABLE",
        "HANDOFF_PREVIEW_STALE" => "HANDOFF_PREVIEW_STALE",
        "HANDOFF_ALREADY_IMPORTED" => "HANDOFF_ALREADY_IMPORTED",
        "HANDOFF_SOURCE_REVISION_CONFLICT" => "HANDOFF_SOURCE_REVISION_CONFLICT",
        _ => "HANDOFF_SCHEMA_UNSUPPORTED",
    };
    contract(code, message)
}
fn validate_text(
    value: &str,
    max_chars: usize,
    path: &str,
    errors: &mut Vec<ExternalProductionHandoffIssue>,
) {
    if value.trim().is_empty() || value.chars().count() > max_chars || value.contains(['\r', '\n'])
    {
        errors.push(issue(
            "HANDOFF_SCHEMA_UNSUPPORTED",
            format!("{path} 文本无效。"),
            Some(path),
        ));
    }
}
fn validate_metadata(
    name: &str,
    description: &str,
    path: &str,
    errors: &mut Vec<ExternalProductionHandoffIssue>,
) {
    validate_text(name, MAX_NAME_CHARS, &format!("{path}.name"), errors);
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        errors.push(issue(
            "HANDOFF_TOO_LARGE",
            format!("{path}.description 超过限制。"),
            Some(path),
        ));
    }
}
fn validate_identity(
    value: &str,
    path: &str,
    seen: &mut HashSet<String>,
    errors: &mut Vec<ExternalProductionHandoffIssue>,
) {
    if value.trim().is_empty()
        || value.chars().count() > MAX_EXTERNAL_ID_CHARS
        || !seen.insert(value.to_owned())
    {
        errors.push(issue(
            "HANDOFF_DUPLICATE_EXTERNAL_ID",
            format!("{path} externalId 重复或无效。"),
            Some(path),
        ));
    }
}
fn validate_ordinal(value: u32, path: &str, errors: &mut Vec<ExternalProductionHandoffIssue>) {
    if value == 0 {
        errors.push(issue(
            "HANDOFF_DUPLICATE_ORDINAL",
            format!("{path} 必须为正整数。"),
            Some(path),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::parse_document;

    #[test]
    fn canonical_hash_ignores_json_whitespace() {
        let first = r#"{"schemaVersion":1,"projectId":"prj_default","source":{"agent":"agent"},"series":[]}"#;
        let second = "{\n  \"schemaVersion\": 1, \"projectId\": \"prj_default\", \"source\": { \"agent\": \"agent\" }, \"series\": []\n}";
        assert_eq!(
            parse_document(first).unwrap().2,
            parse_document(second).unwrap().2
        );
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let content = r#"{"schemaVersion":1,"projectId":"prj_default","source":{"agent":"agent"},"series":[],"unexpectedMagic":true}"#;
        assert_eq!(
            parse_document(content).unwrap_err().code(),
            "HANDOFF_UNKNOWN_FIELD"
        );
    }
}
