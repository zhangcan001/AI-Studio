use crate::application::production_structure_service::{
    ProductionStructureError, ProductionStructureService,
};
use crate::application::scene_production_service::{
    SceneProductionError, SceneProductionPlan, SceneProductionService,
};
use crate::domain::ShotStage;
use serde::Serialize;
use std::{collections::HashSet, error::Error, fmt, sync::Arc};

pub const MAX_EPISODE_PREPARE_SCENES: usize = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum EpisodeSceneClassification {
    Done,
    Prepared,
    Ready,
    Partial,
    Blocked,
    Empty,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeProductionScenePlan {
    pub scene_id: String,
    pub scene_name: String,
    pub scene_ordinal: u32,
    pub total: usize,
    pub done: usize,
    pub prepared: usize,
    pub eligible: usize,
    pub blocked: usize,
    pub can_prepare: bool,
    pub classification: EpisodeSceneClassification,
    pub existing_batch_ids: Vec<String>,
    pub blocking_reasons: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeProductionPlan {
    pub project_id: String,
    pub series_id: String,
    pub series_name: String,
    pub episode_id: String,
    pub episode_name: String,
    pub episode_ordinal: u32,
    pub stage: String,
    pub scene_total: usize,
    pub shot_total: usize,
    pub done: usize,
    pub prepared: usize,
    pub eligible: usize,
    pub blocked: usize,
    pub ready_scene_count: usize,
    pub blocked_scene_count: usize,
    pub fully_done_scene_count: usize,
    pub can_prepare_all: bool,
    pub scenes: Vec<EpisodeProductionScenePlan>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EpisodePrepareStatus {
    Success,
    Noop,
    Partial,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EpisodeScenePrepareStatus {
    Created,
    AlreadyPrepared,
    SkippedDone,
    SkippedEmpty,
    SkippedBlocked,
    Noop,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeProductionScenePrepareResult {
    pub scene_id: String,
    pub scene_name: String,
    pub status: EpisodeScenePrepareStatus,
    pub created: bool,
    pub created_count: usize,
    pub batch_id: Option<String>,
    pub existing_batch_ids: Vec<String>,
    pub blocking_reasons: Vec<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeProductionPrepareResult {
    pub project_id: String,
    pub episode_id: String,
    pub stage: String,
    pub status: EpisodePrepareStatus,
    pub requested_scenes: usize,
    pub created_batches: usize,
    pub created_items: usize,
    pub already_prepared_scenes: Vec<String>,
    pub skipped_done_scenes: Vec<String>,
    pub skipped_empty_scenes: Vec<String>,
    pub skipped_blocked_scenes: Vec<String>,
    pub results: Vec<EpisodeProductionScenePrepareResult>,
}

pub struct EpisodeProductionService {
    structure_service: Arc<ProductionStructureService>,
    scene_production_service: Arc<SceneProductionService>,
}

impl EpisodeProductionService {
    pub fn new(
        structure_service: Arc<ProductionStructureService>,
        scene_production_service: Arc<SceneProductionService>,
    ) -> Self {
        Self {
            structure_service,
            scene_production_service,
        }
    }

    pub async fn plan(
        &self,
        project_id: &str,
        episode_id: &str,
        stage: ShotStage,
    ) -> Result<EpisodeProductionPlan, EpisodeProductionError> {
        let scope = self.episode_scope(project_id, episode_id).await?;
        self.plan_scope(&scope, stage).await
    }

    pub async fn prepare(
        &self,
        project_id: &str,
        episode_id: &str,
        stage: ShotStage,
        scene_ids: &[String],
        allow_partial: bool,
    ) -> Result<EpisodeProductionPrepareResult, EpisodeProductionError> {
        let scope = self.episode_scope(project_id, episode_id).await?;
        let selected_indices = select_scenes(&scope.scenes, scene_ids)?;
        let plan = self.plan_scope(&scope, stage).await?;

        if !allow_partial
            && selected_indices
                .iter()
                .any(|index| plan.scenes[*index].blocked > 0)
        {
            return Err(EpisodeProductionError::Blocked(plan));
        }

        let mut result = EpisodeProductionPrepareResult {
            project_id: project_id.to_owned(),
            episode_id: episode_id.to_owned(),
            stage: stage.as_str().to_owned(),
            status: EpisodePrepareStatus::Noop,
            requested_scenes: selected_indices.len(),
            created_batches: 0,
            created_items: 0,
            already_prepared_scenes: Vec::new(),
            skipped_done_scenes: Vec::new(),
            skipped_empty_scenes: Vec::new(),
            skipped_blocked_scenes: Vec::new(),
            results: Vec::with_capacity(selected_indices.len()),
        };

        for index in selected_indices {
            let scene = &scope.scenes[index];
            let scene_plan = &plan.scenes[index];
            let base = |status| EpisodeProductionScenePrepareResult {
                scene_id: scene.scene_id.clone(),
                scene_name: scene.scene_name.clone(),
                status,
                created: false,
                created_count: 0,
                batch_id: None,
                existing_batch_ids: scene_plan.existing_batch_ids.clone(),
                blocking_reasons: scene_plan.blocking_reasons.clone(),
                error: None,
            };

            match scene_plan.classification {
                EpisodeSceneClassification::Done => {
                    result.skipped_done_scenes.push(scene.scene_id.clone());
                    result
                        .results
                        .push(base(EpisodeScenePrepareStatus::SkippedDone));
                }
                EpisodeSceneClassification::Empty => {
                    result.skipped_empty_scenes.push(scene.scene_id.clone());
                    result
                        .results
                        .push(base(EpisodeScenePrepareStatus::SkippedEmpty));
                }
                EpisodeSceneClassification::Prepared => {
                    result.already_prepared_scenes.push(scene.scene_id.clone());
                    result
                        .results
                        .push(base(EpisodeScenePrepareStatus::AlreadyPrepared));
                }
                EpisodeSceneClassification::Blocked if scene_plan.eligible == 0 => {
                    result.skipped_blocked_scenes.push(scene.scene_id.clone());
                    result
                        .results
                        .push(base(EpisodeScenePrepareStatus::SkippedBlocked));
                }
                EpisodeSceneClassification::Ready | EpisodeSceneClassification::Partial => {
                    match self
                        .scene_production_service
                        .prepare_scope(
                            project_id,
                            &scene.scene_id,
                            &scene.scene_name,
                            &scene.shot_ids,
                            stage,
                            true,
                        )
                        .await
                    {
                        Ok(prepared) if prepared.created => {
                            result.created_batches += 1;
                            result.created_items += prepared.created_count;
                            result.status = EpisodePrepareStatus::Success;
                            result.results.push(EpisodeProductionScenePrepareResult {
                                scene_id: scene.scene_id.clone(),
                                scene_name: scene.scene_name.clone(),
                                status: EpisodeScenePrepareStatus::Created,
                                created: true,
                                created_count: prepared.created_count,
                                batch_id: prepared.existing_batch_ids.first().cloned(),
                                existing_batch_ids: prepared.existing_batch_ids,
                                blocking_reasons: scene_plan.blocking_reasons.clone(),
                                error: None,
                            });
                        }
                        Ok(prepared) if prepared.already_prepared => {
                            result.already_prepared_scenes.push(scene.scene_id.clone());
                            result.results.push(EpisodeProductionScenePrepareResult {
                                scene_id: scene.scene_id.clone(),
                                scene_name: scene.scene_name.clone(),
                                status: EpisodeScenePrepareStatus::AlreadyPrepared,
                                created: false,
                                created_count: 0,
                                batch_id: prepared.existing_batch_ids.first().cloned(),
                                existing_batch_ids: prepared.existing_batch_ids,
                                blocking_reasons: Vec::new(),
                                error: None,
                            });
                        }
                        Ok(prepared) => {
                            result.results.push(EpisodeProductionScenePrepareResult {
                                scene_id: scene.scene_id.clone(),
                                scene_name: scene.scene_name.clone(),
                                status: EpisodeScenePrepareStatus::Noop,
                                created: false,
                                created_count: 0,
                                batch_id: prepared.existing_batch_ids.first().cloned(),
                                existing_batch_ids: prepared.existing_batch_ids,
                                blocking_reasons: Vec::new(),
                                error: None,
                            });
                        }
                        Err(error) => {
                            result.status = EpisodePrepareStatus::Partial;
                            result.results.push(EpisodeProductionScenePrepareResult {
                                scene_id: scene.scene_id.clone(),
                                scene_name: scene.scene_name.clone(),
                                status: EpisodeScenePrepareStatus::Failed,
                                created: false,
                                created_count: 0,
                                batch_id: None,
                                existing_batch_ids: scene_plan.existing_batch_ids.clone(),
                                blocking_reasons: scene_plan.blocking_reasons.clone(),
                                error: Some(error.to_string()),
                            });
                            return Err(EpisodeProductionError::Partial(result));
                        }
                    }
                }
                EpisodeSceneClassification::Blocked => {
                    // Strict mode returned above. Partial mode only reaches here
                    // for a scene whose eligible subset can still be prepared.
                    unreachable!("blocked scene with eligible shots is partial");
                }
            }
        }

        result.status = if allow_partial && selected_scene_has_blockers(&plan, &result.results) {
            EpisodePrepareStatus::Partial
        } else if result.created_batches == 0 {
            EpisodePrepareStatus::Noop
        } else {
            EpisodePrepareStatus::Success
        };
        Ok(result)
    }

    async fn episode_scope(
        &self,
        project_id: &str,
        episode_id: &str,
    ) -> Result<EpisodeScope, EpisodeProductionError> {
        let tree = self.structure_service.tree(project_id).await?;
        let Some((series, episode)) = tree.series.iter().find_map(|series| {
            series
                .episodes
                .iter()
                .find(|episode| episode.episode.id == episode_id)
                .map(|episode| (series, episode))
        }) else {
            return Err(EpisodeProductionError::EpisodeNotFound(
                episode_id.to_owned(),
            ));
        };
        if series.series.project_id != project_id {
            return Err(EpisodeProductionError::EpisodeProjectMismatch {
                episode_id: episode_id.to_owned(),
                project_id: project_id.to_owned(),
            });
        }

        let mut scenes = episode
            .scenes
            .iter()
            .map(|scene| EpisodeSceneScope {
                scene_id: scene.scene.id.clone(),
                scene_name: scene.scene.name.clone(),
                scene_ordinal: scene.scene.ordinal,
                shot_ids: scene.shot_ids.clone(),
            })
            .collect::<Vec<_>>();
        scenes.sort_by(|left, right| {
            left.scene_ordinal
                .cmp(&right.scene_ordinal)
                .then_with(|| left.scene_id.cmp(&right.scene_id))
        });

        Ok(EpisodeScope {
            project_id: project_id.to_owned(),
            series_id: series.series.id.clone(),
            series_name: series.series.name.clone(),
            episode_id: episode.episode.id.clone(),
            episode_name: episode.episode.name.clone(),
            episode_ordinal: episode.episode.ordinal,
            scenes,
        })
    }

    async fn plan_scope(
        &self,
        scope: &EpisodeScope,
        stage: ShotStage,
    ) -> Result<EpisodeProductionPlan, EpisodeProductionError> {
        let mut scenes = Vec::with_capacity(scope.scenes.len());
        for scene in &scope.scenes {
            let plan = self
                .scene_production_service
                .plan_scope(
                    &scope.project_id,
                    &scene.scene_id,
                    &scene.scene_name,
                    &scene.shot_ids,
                    stage,
                )
                .await?;
            scenes.push(scene_plan(scene, plan));
        }

        let scene_total = scenes.len();
        let shot_total = scenes.iter().map(|scene| scene.total).sum();
        let done = scenes.iter().map(|scene| scene.done).sum();
        let prepared = scenes.iter().map(|scene| scene.prepared).sum();
        let eligible = scenes.iter().map(|scene| scene.eligible).sum();
        let blocked = scenes.iter().map(|scene| scene.blocked).sum();
        let ready_scene_count = scenes
            .iter()
            .filter(|scene| scene.classification == EpisodeSceneClassification::Ready)
            .count();
        let blocked_scene_count = scenes.iter().filter(|scene| scene.blocked > 0).count();
        let fully_done_scene_count = scenes
            .iter()
            .filter(|scene| scene.classification == EpisodeSceneClassification::Done)
            .count();
        let has_prepareable_scene = scenes.iter().any(|scene| scene.eligible > 0);
        let can_prepare_all = has_prepareable_scene
            && scenes
                .iter()
                .filter(|scene| {
                    scene.classification != EpisodeSceneClassification::Done
                        && scene.classification != EpisodeSceneClassification::Empty
                })
                .all(|scene| scene.blocked == 0);

        Ok(EpisodeProductionPlan {
            project_id: scope.project_id.clone(),
            series_id: scope.series_id.clone(),
            series_name: scope.series_name.clone(),
            episode_id: scope.episode_id.clone(),
            episode_name: scope.episode_name.clone(),
            episode_ordinal: scope.episode_ordinal,
            stage: stage.as_str().to_owned(),
            scene_total,
            shot_total,
            done,
            prepared,
            eligible,
            blocked,
            ready_scene_count,
            blocked_scene_count,
            fully_done_scene_count,
            can_prepare_all,
            scenes,
        })
    }
}

fn selected_scene_has_blockers(
    plan: &EpisodeProductionPlan,
    results: &[EpisodeProductionScenePrepareResult],
) -> bool {
    results.iter().any(|result| {
        plan.scenes
            .iter()
            .find(|scene| scene.scene_id == result.scene_id)
            .is_some_and(|scene| scene.blocked > 0)
    })
}

struct EpisodeScope {
    project_id: String,
    series_id: String,
    series_name: String,
    episode_id: String,
    episode_name: String,
    episode_ordinal: u32,
    scenes: Vec<EpisodeSceneScope>,
}

struct EpisodeSceneScope {
    scene_id: String,
    scene_name: String,
    scene_ordinal: u32,
    shot_ids: Vec<String>,
}

fn scene_plan(scope: &EpisodeSceneScope, plan: SceneProductionPlan) -> EpisodeProductionScenePlan {
    let classification = if plan.total == 0 {
        EpisodeSceneClassification::Empty
    } else if plan.done == plan.total {
        EpisodeSceneClassification::Done
    } else if plan.blocked > 0 && plan.eligible > 0 {
        EpisodeSceneClassification::Partial
    } else if plan.blocked > 0 {
        EpisodeSceneClassification::Blocked
    } else if plan.eligible == 0 && plan.prepared > 0 {
        EpisodeSceneClassification::Prepared
    } else {
        EpisodeSceneClassification::Ready
    };
    let mut existing_batch_ids = Vec::new();
    let mut blocking_reasons = Vec::new();
    for row in plan.rows {
        if let Some(batch_id) = row.existing_batch_id {
            if !existing_batch_ids.contains(&batch_id) {
                existing_batch_ids.push(batch_id);
            }
        }
        for reason in row.blocking_reasons {
            if !blocking_reasons.contains(&reason) {
                blocking_reasons.push(reason);
            }
        }
    }
    EpisodeProductionScenePlan {
        scene_id: scope.scene_id.clone(),
        scene_name: scope.scene_name.clone(),
        scene_ordinal: scope.scene_ordinal,
        total: plan.total,
        done: plan.done,
        prepared: plan.prepared,
        eligible: plan.eligible,
        blocked: plan.blocked,
        can_prepare: plan.can_prepare,
        classification,
        existing_batch_ids,
        blocking_reasons,
    }
}

fn select_scenes(
    scenes: &[EpisodeSceneScope],
    requested_ids: &[String],
) -> Result<Vec<usize>, EpisodeProductionError> {
    if requested_ids.is_empty() {
        if scenes.len() > MAX_EPISODE_PREPARE_SCENES {
            return Err(EpisodeProductionError::TooManyScenes(scenes.len()));
        }
        return Ok((0..scenes.len()).collect());
    }
    if requested_ids.len() > MAX_EPISODE_PREPARE_SCENES {
        return Err(EpisodeProductionError::ScopeTooLarge(requested_ids.len()));
    }

    let mut requested = HashSet::with_capacity(requested_ids.len());
    for scene_id in requested_ids {
        if !requested.insert(scene_id) {
            return Err(EpisodeProductionError::SelectionInvalid(
                "EPISODE_SCENE_SELECTION_INVALID: scene_ids must be unique".to_owned(),
            ));
        }
    }
    let mut selected = Vec::with_capacity(requested_ids.len());
    for (index, scene) in scenes.iter().enumerate() {
        if requested.contains(&scene.scene_id) {
            selected.push(index);
        }
    }
    if selected.len() != requested_ids.len() {
        return Err(EpisodeProductionError::SelectionInvalid(
            "EPISODE_SCENE_SELECTION_INVALID: every selected scene must belong to the episode"
                .to_owned(),
        ));
    }
    Ok(selected)
}

#[derive(Debug)]
pub enum EpisodeProductionError {
    EpisodeNotFound(String),
    EpisodeProjectMismatch {
        episode_id: String,
        project_id: String,
    },
    SelectionInvalid(String),
    TooManyScenes(usize),
    ScopeTooLarge(usize),
    Blocked(EpisodeProductionPlan),
    Partial(EpisodeProductionPrepareResult),
    Structure(ProductionStructureError),
    SceneProduction(SceneProductionError),
}

impl fmt::Display for EpisodeProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EpisodeNotFound(id) => write!(formatter, "EPISODE_NOT_FOUND: {id}"),
            Self::EpisodeProjectMismatch {
                episode_id,
                project_id,
            } => write!(
                formatter,
                "EPISODE_PROJECT_MISMATCH: episode {episode_id} is not in project {project_id}"
            ),
            Self::SelectionInvalid(message) => formatter.write_str(message),
            Self::TooManyScenes(count) => write!(
                formatter,
                "EPISODE_PRODUCTION_TOO_MANY_SCENES: {count} scenes exceeds {MAX_EPISODE_PREPARE_SCENES}"
            ),
            Self::ScopeTooLarge(count) => write!(
                formatter,
                "EPISODE_BULK_SCOPE_TOO_LARGE: {count} selected scenes exceeds {MAX_EPISODE_PREPARE_SCENES}"
            ),
            Self::Blocked(_) => formatter.write_str("EPISODE_PRODUCTION_BLOCKED"),
            Self::Partial(_) => formatter.write_str("EPISODE_PRODUCTION_PARTIAL"),
            Self::Structure(error) => error.fmt(formatter),
            Self::SceneProduction(error) => error.fmt(formatter),
        }
    }
}

impl Error for EpisodeProductionError {}

impl From<ProductionStructureError> for EpisodeProductionError {
    fn from(error: ProductionStructureError) -> Self {
        Self::Structure(error)
    }
}

impl From<SceneProductionError> for EpisodeProductionError {
    fn from(error: SceneProductionError) -> Self {
        Self::SceneProduction(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        scene_plan, select_scenes, EpisodePrepareStatus, EpisodeProductionError,
        EpisodeProductionService, EpisodeSceneClassification, EpisodeSceneScope,
    };
    use crate::application::ports::{ShotRecord, ShotRepository, ShotStageConfigRecord};
    use crate::application::production_structure_service::{
        CreateEpisodeRequest, CreateSceneRequest, CreateSeriesRequest, ProductionStructureService,
    };
    use crate::application::scene_production_service::{
        SceneProductionPlan, SceneProductionPlanRow, SceneProductionService,
        SceneShotClassification,
    };
    use crate::application::shot_batch_service::ShotBatchService;
    use crate::domain::ShotStage;
    use crate::infrastructure::database::{
        initialize,
        repositories::{
            test_support, SqliteAssetRepository, SqliteGenerationDefinitionRepository,
            SqliteProductionQueueRepository, SqliteProductionStructureRepository,
            SqliteProjectRepository, SqliteShotRepository, SqliteTaskRepository,
        },
    };
    use crate::infrastructure::time::SystemClock;
    use chrono::Utc;
    use serde_json::json;
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tempfile::{tempdir, TempDir};

    const IMAGE_WORKFLOW_JSON: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/simple_i2i/workflow_api.json"
    ));
    const IMAGE_RECIPE_YAML: &str = r#"
schema_version: 1
id: episode_image_test
name: Episode Image Test
workflow:
  file: workflow_api.json
inputs:
  prompt:
    type: textarea
    label: Prompt
    required: true
    default: ""
  steps:
    type: integer
    label: Steps
    required: true
    default: 20
    min: 1
    max: 100
  seed:
    type: seed
    label: Seed
    default: random
bindings:
  - source: prompt
    target:
      node: "6"
      input: text
  - source: steps
    target:
      node: "3"
      input: steps
  - source: seed
    target:
      node: "3"
      input: seed
outputs:
  - id: generated_image
    type: image
    node: "9"
    required: true
"#;

    struct Fixture {
        _directory: TempDir,
        pool: SqlitePool,
        episode: Arc<EpisodeProductionService>,
        scene: Arc<SceneProductionService>,
        episode_id: String,
        scene_ids: Vec<String>,
    }

    async fn fixture(with_ready_scene: bool) -> Fixture {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("episode.db"))
            .await
            .unwrap();
        test_support::seed_task_dependencies(&pool).await;
        sqlx::query("UPDATE projects SET id = 'prj_default' WHERE id = 'project-1'")
            .execute(&pool)
            .await
            .unwrap();

        {
            sqlx::query(
                "INSERT INTO workflows
                 (id, name, category, mode, current_version_id, created_at, updated_at)
                 VALUES ('wfl_kera2_t2i_local_v2', 'Episode Image', 'image', 'image',
                         'wfv_episode_image', '2026-08-18T00:00:00Z', '2026-08-18T00:00:00Z')",
            )
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO workflow_versions
                 (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
                 VALUES ('wfv_episode_image', 'wfl_kera2_t2i_local_v2', '1', ?, 'sha',
                         '2026-08-18T00:00:00Z')",
            )
            .bind(IMAGE_WORKFLOW_JSON)
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO recipes
                 (id, workflow_version_id, version, schema_version, recipe_yaml,
                  recipe_sha256, created_at)
                 VALUES ('rcp_episode_image', 'wfv_episode_image', '1', 1, ?, 'sha',
                         '2026-08-18T00:00:00Z')",
            )
            .bind(IMAGE_RECIPE_YAML)
            .execute(&pool)
            .await
            .unwrap();
        }

        let structure = Arc::new(ProductionStructureService::new(
            Arc::new(SqliteProductionStructureRepository::new(pool.clone())),
            Arc::new(SystemClock),
        ));
        let series = structure
            .create_series(CreateSeriesRequest {
                project_id: "prj_default".to_owned(),
                name: "Series".to_owned(),
                description: String::new(),
            })
            .await
            .unwrap();
        let episode = structure
            .create_episode(CreateEpisodeRequest {
                project_id: "prj_default".to_owned(),
                series_id: series.id,
                name: "Episode".to_owned(),
                description: String::new(),
            })
            .await
            .unwrap();

        let scene_names = if with_ready_scene {
            vec!["Empty", "Blocked", "Ready", "Done"]
        } else {
            vec!["Ready"]
        };
        let mut scene_ids = Vec::new();
        for name in scene_names {
            scene_ids.push(
                structure
                    .create_scene(CreateSceneRequest {
                        project_id: "prj_default".to_owned(),
                        episode_id: episode.id.clone(),
                        name: name.to_owned(),
                        description: String::new(),
                    })
                    .await
                    .unwrap()
                    .id,
            );
        }

        let shot_repository = Arc::new(SqliteShotRepository::new(pool.clone()));
        if with_ready_scene {
            sqlx::query(
                "INSERT INTO assets
                 (id, project_id, type, category, name, original_name, storage_path, sha256,
                  mime_type, width, height, file_size, metadata_json, created_at, updated_at)
                 VALUES ('ast_done', 'prj_default', 'image', 'source_image', 'Done', 'Done',
                         'assets/ast_done.png', 'sha', 'image/png', 1, 1, 1, '{}',
                         '2026-08-18T00:00:00Z', '2026-08-18T00:00:00Z')",
            )
            .execute(&pool)
            .await
            .unwrap();
            insert_shot(&shot_repository, "sht_blocked", 0, None, None).await;
            insert_shot(
                &shot_repository,
                "sht_ready",
                1,
                None,
                Some(("wfv_episode_image", "rcp_episode_image")),
            )
            .await;
            insert_shot(&shot_repository, "sht_done", 2, Some("ast_done"), None).await;
            structure
                .assign_shots("prj_default", &scene_ids[1], &["sht_blocked".to_owned()])
                .await
                .unwrap();
            structure
                .assign_shots("prj_default", &scene_ids[2], &["sht_ready".to_owned()])
                .await
                .unwrap();
            structure
                .assign_shots("prj_default", &scene_ids[3], &["sht_done".to_owned()])
                .await
                .unwrap();
        } else {
            insert_shot(
                &shot_repository,
                "sht_ready",
                0,
                None,
                Some(("wfv_episode_image", "rcp_episode_image")),
            )
            .await;
            structure
                .assign_shots("prj_default", &scene_ids[0], &["sht_ready".to_owned()])
                .await
                .unwrap();
        }

        let shot_batch = Arc::new(ShotBatchService::new(
            shot_repository,
            Arc::new(SqliteProductionQueueRepository::new(pool.clone())),
            Arc::new(SqliteTaskRepository::new(pool.clone())),
            Arc::new(SqliteAssetRepository::new(pool.clone())),
            Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone())),
            Arc::new(SqliteProjectRepository::new(pool.clone())),
            Arc::new(SystemClock),
        ));
        let scene = Arc::new(SceneProductionService::new(structure.clone(), shot_batch));
        let episode_service = Arc::new(EpisodeProductionService::new(structure, scene.clone()));
        Fixture {
            _directory: directory,
            pool,
            episode: episode_service,
            scene,
            episode_id: episode.id,
            scene_ids,
        }
    }

    async fn insert_shot(
        repository: &SqliteShotRepository,
        id: &str,
        ordinal: i64,
        selected_image_asset_id: Option<&str>,
        config: Option<(&str, &str)>,
    ) {
        let now = Utc::now();
        repository
            .insert(&ShotRecord {
                id: id.to_owned(),
                project_id: "prj_default".to_owned(),
                ordinal,
                name: id.to_owned(),
                prompt_text: "test prompt".to_owned(),
                prompt_entry_id: None,
                prompt_version_id: None,
                selected_image_asset_id: selected_image_asset_id.map(str::to_owned),
                selected_video_asset_id: None,
                created_at: now,
                updated_at: now,
            })
            .await
            .unwrap();
        if let Some((workflow_version_id, recipe_id)) = config {
            repository
                .upsert_stage_config(
                    "prj_default",
                    &ShotStageConfigRecord {
                        shot_id: id.to_owned(),
                        stage: ShotStage::Image,
                        workflow_version_id: workflow_version_id.to_owned(),
                        recipe_id: recipe_id.to_owned(),
                        scalar_values: json!({
                            "steps": {"type": "integer", "value": 20},
                            "seed": {"type": "seed_fixed", "value": "1"}
                        }),
                        updated_at: now,
                    },
                )
                .await
                .unwrap();
        }
    }

    fn scope(id: &str, ordinal: u32) -> EpisodeSceneScope {
        EpisodeSceneScope {
            scene_id: id.to_owned(),
            scene_name: id.to_owned(),
            scene_ordinal: ordinal,
            shot_ids: Vec::new(),
        }
    }

    fn plan(
        total: usize,
        done: usize,
        prepared: usize,
        eligible: usize,
        blocked: usize,
    ) -> SceneProductionPlan {
        SceneProductionPlan {
            project_id: "prj".to_owned(),
            scene_id: "scene".to_owned(),
            scene_name: "Scene".to_owned(),
            stage: "image".to_owned(),
            total,
            done,
            prepared,
            eligible,
            blocked,
            can_prepare: blocked == 0 && eligible > 0,
            max_batch_items: 100,
            rows: Vec::new(),
        }
    }

    #[test]
    fn scene_summary_classification_and_totals_use_scene_plan_counts() {
        let ready = plan(2, 0, 0, 2, 0);
        let blocked = plan(2, 0, 0, 0, 2);
        assert_eq!(
            scene_plan(&scope("ready", 2), ready).classification,
            EpisodeSceneClassification::Ready
        );
        assert_eq!(
            scene_plan(&scope("blocked", 1), blocked).classification,
            EpisodeSceneClassification::Blocked
        );
        assert_eq!(
            scene_plan(&scope("empty", 0), plan(0, 0, 0, 0, 0)).classification,
            EpisodeSceneClassification::Empty
        );
        assert_eq!(
            scene_plan(&scope("done", 3), plan(2, 2, 0, 0, 0)).classification,
            EpisodeSceneClassification::Done
        );
        assert_eq!(
            scene_plan(&scope("partial", 4), plan(2, 0, 0, 1, 1)).classification,
            EpisodeSceneClassification::Partial
        );
    }

    #[test]
    fn scene_selection_is_unique_bounded_and_episode_scoped() {
        let scenes = vec![scope("scene-1", 1), scope("scene-2", 2)];
        assert_eq!(select_scenes(&scenes, &[]).unwrap(), [0, 1]);
        assert!(matches!(
            select_scenes(&scenes, &["scene-1".to_owned(), "scene-1".to_owned()]),
            Err(EpisodeProductionError::SelectionInvalid(_))
        ));
        assert!(matches!(
            select_scenes(&scenes, &["foreign".to_owned()]),
            Err(EpisodeProductionError::SelectionInvalid(_))
        ));
        let many = (0..=50)
            .map(|index| scope(&format!("scene-{index}"), index))
            .collect::<Vec<_>>();
        assert!(matches!(
            select_scenes(&many, &[]),
            Err(EpisodeProductionError::TooManyScenes(51))
        ));
    }

    #[test]
    fn scene_rows_keep_the_explicit_plan_status_contract() {
        let mut prepared = plan(2, 0, 2, 0, 0);
        prepared.rows.push(SceneProductionPlanRow {
            shot_id: "shot".to_owned(),
            name: "Shot".to_owned(),
            global_ordinal: 1,
            classification: SceneShotClassification::Prepared,
            blocking_reasons: Vec::new(),
            existing_batch_id: Some("batch".to_owned()),
        });
        let summary = scene_plan(&scope("prepared", 1), prepared);
        assert_eq!(summary.classification, EpisodeSceneClassification::Prepared);
        assert_eq!(summary.existing_batch_ids, ["batch"]);
    }

    #[tokio::test]
    async fn episode_plan_resolves_order_and_sums_scene_plans() {
        let fixture = fixture(true).await;
        let plan = fixture
            .episode
            .plan(&"prj_default", &fixture.episode_id, ShotStage::Image)
            .await
            .unwrap();
        assert_eq!(plan.scene_total, 4);
        assert_eq!(plan.shot_total, 3);
        assert_eq!(plan.done, 1);
        assert_eq!(plan.eligible, 1);
        assert_eq!(plan.blocked, 1);
        assert_eq!(plan.fully_done_scene_count, 1);
        assert!(!plan.can_prepare_all);
        assert_eq!(
            plan.scenes
                .iter()
                .map(|scene| scene.scene_name.as_str())
                .collect::<Vec<_>>(),
            ["Empty", "Blocked", "Ready", "Done"]
        );
    }

    #[tokio::test]
    async fn strict_blocked_prepare_is_zero_mutation_and_cross_project_is_rejected() {
        let fixture = fixture(true).await;
        let error = fixture
            .episode
            .prepare(
                "prj_default",
                &fixture.episode_id,
                ShotStage::Image,
                &[],
                false,
            )
            .await
            .unwrap_err();
        assert!(matches!(error, EpisodeProductionError::Blocked(_)));
        assert_eq!(batch_count(&fixture.pool).await, 0);

        let error = fixture
            .episode
            .plan(
                "prj_00000000-0000-0000-0000-000000000001",
                &fixture.episode_id,
                ShotStage::Image,
            )
            .await
            .unwrap_err();
        assert!(
            error.to_string().starts_with("EPISODE_NOT_FOUND"),
            "unexpected cross-project error: {error:?}"
        );
    }

    #[tokio::test]
    async fn partial_prepare_skips_done_empty_blocked_and_repeat_is_idempotent() {
        let fixture = fixture(true).await;
        let first = fixture
            .episode
            .prepare(
                "prj_default",
                &fixture.episode_id,
                ShotStage::Image,
                &[],
                true,
            )
            .await
            .unwrap();
        assert_eq!(first.status, EpisodePrepareStatus::Partial);
        assert_eq!(first.created_batches, 1);
        assert_eq!(first.created_items, 1);
        assert_eq!(first.skipped_done_scenes.len(), 1);
        assert_eq!(first.skipped_empty_scenes.len(), 1);
        assert_eq!(first.skipped_blocked_scenes.len(), 1);
        assert_eq!(batch_count(&fixture.pool).await, 1);

        let second = fixture
            .episode
            .prepare(
                "prj_default",
                &fixture.episode_id,
                ShotStage::Image,
                &[fixture.scene_ids[2].clone()],
                true,
            )
            .await
            .unwrap();
        assert_eq!(second.status, EpisodePrepareStatus::Noop);
        assert_eq!(second.created_batches, 0);
        assert_eq!(
            second.already_prepared_scenes,
            [fixture.scene_ids[2].clone()]
        );
        assert_eq!(batch_count(&fixture.pool).await, 1);
    }

    #[tokio::test]
    async fn concurrent_episode_and_scene_prepare_share_one_scene_gate() {
        let fixture = fixture(false).await;
        let episode = Arc::clone(&fixture.episode);
        let scene = Arc::clone(&fixture.scene);
        let episode_id = fixture.episode_id.clone();
        let scene_id = fixture.scene_ids[0].clone();
        let scene_id_for_episode = vec![scene_id.clone()];
        let (episode_result, scene_result) = tokio::join!(
            episode.prepare(
                "prj_default",
                &episode_id,
                ShotStage::Image,
                &scene_id_for_episode,
                true,
            ),
            scene.prepare("prj_default", &scene_id, ShotStage::Image, true),
        );
        assert!(episode_result.is_ok() || scene_result.is_ok());
        assert_eq!(batch_count(&fixture.pool).await, 1);

        let first_request = vec![fixture.scene_ids[0].clone()];
        let second_request = vec![fixture.scene_ids[0].clone()];
        let (left, right) = tokio::join!(
            fixture.episode.prepare(
                "prj_default",
                &fixture.episode_id,
                ShotStage::Image,
                &first_request,
                true,
            ),
            fixture.episode.prepare(
                "prj_default",
                &fixture.episode_id,
                ShotStage::Image,
                &second_request,
                true,
            )
        );
        assert!(left.is_ok() && right.is_ok());
        assert_eq!(batch_count(&fixture.pool).await, 1);
    }

    async fn batch_count(pool: &SqlitePool) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM production_batches")
            .fetch_one(pool)
            .await
            .unwrap()
    }
}
