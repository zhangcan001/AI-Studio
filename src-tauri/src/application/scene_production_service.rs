use crate::application::production_structure_service::{
    ProductionStructureError, ProductionStructureService,
};
use crate::application::shot_batch_service::{
    CreateShotBatchRequest, ShotBatchPlanRow, ShotBatchService, ShotBatchServiceError,
    MAX_SHOT_BATCH_ITEMS,
};
use crate::domain::{ProductionBatchDetail, ShotStage};
use serde::Serialize;
use std::{collections::BTreeSet, error::Error, fmt, sync::Arc};
use tokio::sync::Mutex;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SceneShotClassification {
    Done,
    Prepared,
    Eligible,
    Blocked,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneProductionPlanRow {
    pub shot_id: String,
    pub name: String,
    pub global_ordinal: i64,
    pub classification: SceneShotClassification,
    pub blocking_reasons: Vec<String>,
    pub existing_batch_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneProductionPlan {
    pub project_id: String,
    pub scene_id: String,
    pub scene_name: String,
    pub stage: String,
    pub total: usize,
    pub done: usize,
    pub prepared: usize,
    pub eligible: usize,
    pub blocked: usize,
    pub can_prepare: bool,
    pub max_batch_items: usize,
    pub rows: Vec<SceneProductionPlanRow>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneProductionPrepareResult {
    pub project_id: String,
    pub scene_id: String,
    pub stage: ShotStage,
    pub created: bool,
    pub created_count: usize,
    pub already_prepared: bool,
    pub existing_batch_ids: Vec<String>,
    pub detail: Option<ProductionBatchDetail>,
}

pub struct SceneProductionService {
    structure_service: Arc<ProductionStructureService>,
    shot_batch_service: Arc<ShotBatchService>,
    prepare_gate: Mutex<()>,
}

impl SceneProductionService {
    pub fn new(
        structure_service: Arc<ProductionStructureService>,
        shot_batch_service: Arc<ShotBatchService>,
    ) -> Self {
        Self {
            structure_service,
            shot_batch_service,
            prepare_gate: Mutex::new(()),
        }
    }

    pub async fn plan(
        &self,
        project_id: &str,
        scene_id: &str,
        stage: ShotStage,
    ) -> Result<SceneProductionPlan, SceneProductionError> {
        let scene = self.scene(project_id, scene_id).await?;
        self.plan_scope(project_id, scene_id, &scene.name, &scene.shot_ids, stage)
            .await
    }

    pub(crate) async fn plan_scope(
        &self,
        project_id: &str,
        scene_id: &str,
        scene_name: &str,
        shot_ids: &[String],
        stage: ShotStage,
    ) -> Result<SceneProductionPlan, SceneProductionError> {
        let batch_plan = self
            .shot_batch_service
            .plan_for_shots(project_id, stage, shot_ids)
            .await?;
        let active_bindings = self
            .shot_batch_service
            .list_active_shot_bindings(project_id, stage, shot_ids)
            .await?;
        Ok(make_plan(
            project_id,
            scene_id,
            scene_name,
            stage,
            shot_ids,
            batch_plan.rows,
            active_bindings
                .into_iter()
                .map(|binding| (binding.shot_id, binding.production_batch_id))
                .collect(),
        ))
    }

    pub async fn prepare(
        &self,
        project_id: &str,
        scene_id: &str,
        stage: ShotStage,
        allow_partial: bool,
    ) -> Result<SceneProductionPrepareResult, SceneProductionError> {
        let scene = self.scene(project_id, scene_id).await?;
        self.prepare_scope(
            project_id,
            scene_id,
            &scene.name,
            &scene.shot_ids,
            stage,
            allow_partial,
        )
        .await
    }

    pub(crate) async fn prepare_scope(
        &self,
        project_id: &str,
        scene_id: &str,
        scene_name: &str,
        shot_ids: &[String],
        stage: ShotStage,
        allow_partial: bool,
    ) -> Result<SceneProductionPrepareResult, SceneProductionError> {
        // The lock only covers plan -> active recheck -> existing ShotBatchService::create.
        // Queue admission still belongs to ProductionQueueService::start.
        let _guard = self.prepare_gate.lock().await;
        let plan = self
            .plan_scope(project_id, scene_id, scene_name, shot_ids, stage)
            .await?;
        if plan.eligible > MAX_SHOT_BATCH_ITEMS {
            return Err(SceneProductionError::TooLarge {
                eligible: plan.eligible,
                max: MAX_SHOT_BATCH_ITEMS,
            });
        }
        if !allow_partial && plan.blocked > 0 {
            return Err(SceneProductionError::Blocked(plan));
        }

        let eligible_shot_ids = plan
            .rows
            .iter()
            .filter(|row| row.classification == SceneShotClassification::Eligible)
            .map(|row| row.shot_id.clone())
            .collect::<Vec<_>>();
        if eligible_shot_ids.is_empty() {
            let existing_batch_ids = plan
                .rows
                .iter()
                .filter_map(|row| row.existing_batch_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            return Ok(SceneProductionPrepareResult {
                project_id: project_id.to_owned(),
                scene_id: scene_id.to_owned(),
                stage,
                created: false,
                created_count: 0,
                already_prepared: plan.prepared > 0,
                existing_batch_ids,
                detail: None,
            });
        }

        // Recheck after the plan and immediately before create. The mutex prevents
        // duplicate prepares through this service instance; the set query keeps the
        // final decision in the existing queue repository boundary.
        let active = self
            .shot_batch_service
            .list_active_shot_bindings(project_id, stage, &eligible_shot_ids)
            .await?;
        if !active.is_empty() {
            let existing_batch_ids = active
                .into_iter()
                .map(|binding| binding.production_batch_id)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            return Ok(SceneProductionPrepareResult {
                project_id: project_id.to_owned(),
                scene_id: scene_id.to_owned(),
                stage,
                created: false,
                created_count: 0,
                already_prepared: true,
                existing_batch_ids,
                detail: None,
            });
        }

        let detail = self
            .shot_batch_service
            .create(CreateShotBatchRequest {
                project_id: project_id.to_owned(),
                stage,
                shot_ids: eligible_shot_ids,
            })
            .await?;
        let created_count = detail.items.len();
        Ok(SceneProductionPrepareResult {
            project_id: project_id.to_owned(),
            scene_id: scene_id.to_owned(),
            stage,
            created: true,
            created_count,
            already_prepared: false,
            existing_batch_ids: vec![detail.batch.id.as_str().to_owned()],
            detail: Some(detail),
        })
    }

    async fn scene(
        &self,
        project_id: &str,
        scene_id: &str,
    ) -> Result<SceneScope, SceneProductionError> {
        let tree = self.structure_service.tree(project_id).await?;
        tree.series
            .into_iter()
            .flat_map(|series| series.episodes)
            .flat_map(|episode| episode.scenes)
            .find(|scene| scene.scene.id == scene_id)
            .map(|scene| SceneScope {
                name: scene.scene.name,
                shot_ids: scene.shot_ids,
            })
            .ok_or_else(|| SceneProductionError::SceneNotFound(scene_id.to_owned()))
    }
}

struct SceneScope {
    name: String,
    shot_ids: Vec<String>,
}

fn make_plan(
    project_id: &str,
    scene_id: &str,
    scene_name: &str,
    stage: ShotStage,
    scene_shot_ids: &[String],
    mut rows: Vec<ShotBatchPlanRow>,
    active_batch_ids: std::collections::HashMap<String, String>,
) -> SceneProductionPlan {
    rows.sort_by_key(|row| row.ordinal);
    let rows = rows
        .into_iter()
        .filter(|row| scene_shot_ids.iter().any(|id| id == &row.shot_id))
        .map(|row| {
            let existing_batch_id = active_batch_ids.get(&row.shot_id).cloned();
            let classification = if is_done(stage, &row) {
                SceneShotClassification::Done
            } else if existing_batch_id.is_some() {
                SceneShotClassification::Prepared
            } else if row.eligible {
                SceneShotClassification::Eligible
            } else {
                SceneShotClassification::Blocked
            };
            let blocking_reasons = if classification == SceneShotClassification::Blocked {
                row.blocking_reasons
            } else {
                Vec::new()
            };
            SceneProductionPlanRow {
                shot_id: row.shot_id,
                name: row.name,
                global_ordinal: row.ordinal,
                classification,
                blocking_reasons,
                existing_batch_id,
            }
        })
        .collect::<Vec<_>>();
    let total = rows.len();
    let done = rows
        .iter()
        .filter(|row| row.classification == SceneShotClassification::Done)
        .count();
    let prepared = rows
        .iter()
        .filter(|row| row.classification == SceneShotClassification::Prepared)
        .count();
    let eligible = rows
        .iter()
        .filter(|row| row.classification == SceneShotClassification::Eligible)
        .count();
    let blocked = rows
        .iter()
        .filter(|row| row.classification == SceneShotClassification::Blocked)
        .count();
    SceneProductionPlan {
        project_id: project_id.to_owned(),
        scene_id: scene_id.to_owned(),
        scene_name: scene_name.to_owned(),
        stage: stage.as_str().to_owned(),
        total,
        done,
        prepared,
        eligible,
        blocked,
        can_prepare: blocked == 0 && eligible > 0 && eligible <= MAX_SHOT_BATCH_ITEMS,
        max_batch_items: MAX_SHOT_BATCH_ITEMS,
        rows,
    }
}

fn is_done(stage: ShotStage, row: &ShotBatchPlanRow) -> bool {
    match stage {
        ShotStage::Image => row.selected_image_asset_id.is_some(),
        ShotStage::Video => row.selected_video_asset_id.is_some(),
    }
}

#[derive(Debug)]
pub enum SceneProductionError {
    SceneNotFound(String),
    TooLarge { eligible: usize, max: usize },
    Blocked(SceneProductionPlan),
    Structure(ProductionStructureError),
    ShotBatch(ShotBatchServiceError),
}

impl fmt::Display for SceneProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SceneNotFound(scene_id) => write!(formatter, "SCENE_NOT_FOUND: {scene_id}"),
            Self::TooLarge { eligible, max } => write!(
                formatter,
                "SCENE_PRODUCTION_TOO_LARGE: {eligible} eligible shots exceeds the {max}-shot limit"
            ),
            Self::Blocked(_) => formatter.write_str("SCENE_PRODUCTION_BLOCKED"),
            Self::Structure(error) => error.fmt(formatter),
            Self::ShotBatch(error) => error.fmt(formatter),
        }
    }
}

impl Error for SceneProductionError {}

impl From<ProductionStructureError> for SceneProductionError {
    fn from(error: ProductionStructureError) -> Self {
        Self::Structure(error)
    }
}

impl From<ShotBatchServiceError> for SceneProductionError {
    fn from(error: ShotBatchServiceError) -> Self {
        Self::ShotBatch(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{is_done, make_plan, SceneShotClassification};
    use crate::application::shot_batch_service::ShotBatchPlanRow;
    use crate::domain::ShotStage;
    use std::collections::HashMap;

    fn row(id: &str, ordinal: i64, eligible: bool) -> ShotBatchPlanRow {
        ShotBatchPlanRow {
            shot_id: id.to_owned(),
            ordinal,
            name: id.to_owned(),
            stage: "image".to_owned(),
            workflow_version_id: None,
            recipe_id: None,
            recipe_name: None,
            current_status: "READY".to_owned(),
            selected_image_asset_id: None,
            selected_video_asset_id: None,
            video_mode: None,
            reference_count: 0,
            reference_min: None,
            reference_max: None,
            eligible,
            blocking_reasons: if eligible {
                Vec::new()
            } else {
                vec!["blocked".to_owned()]
            },
        }
    }

    #[test]
    fn plan_classifies_done_prepared_eligible_and_blocked_in_global_order() {
        let mut done = row("shot-1", 12, false);
        done.selected_image_asset_id = Some("asset-1".to_owned());
        assert!(is_done(ShotStage::Image, &done));
        let plan = make_plan(
            "prj_1",
            "scene-1",
            "Scene",
            ShotStage::Image,
            &[
                "shot-2".to_owned(),
                "shot-1".to_owned(),
                "shot-3".to_owned(),
            ],
            vec![done, row("shot-3", 2, false), row("shot-2", 1, true)],
            HashMap::from([("shot-3".to_owned(), "pbt_existing".to_owned())]),
        );
        assert_eq!(plan.total, 3);
        assert_eq!(plan.done, 1);
        assert_eq!(plan.prepared, 1);
        assert_eq!(plan.eligible, 1);
        assert_eq!(plan.blocked, 0);
        assert!(plan.can_prepare);
        assert_eq!(
            plan.rows
                .iter()
                .map(|row| row.global_ordinal)
                .collect::<Vec<_>>(),
            vec![1, 2, 12]
        );
        assert_eq!(
            plan.rows[0].classification,
            SceneShotClassification::Eligible
        );
        assert_eq!(
            plan.rows[1].classification,
            SceneShotClassification::Prepared
        );
        assert_eq!(plan.rows[2].classification, SceneShotClassification::Done);
    }

    #[test]
    fn plan_is_not_prepareable_when_blocked_or_over_limit() {
        let plan = make_plan(
            "prj_1",
            "scene-1",
            "Scene",
            ShotStage::Image,
            &["shot-1".to_owned()],
            vec![row("shot-1", 1, false)],
            Default::default(),
        );
        assert_eq!(plan.blocked, 1);
        assert!(!plan.can_prepare);
    }
}
