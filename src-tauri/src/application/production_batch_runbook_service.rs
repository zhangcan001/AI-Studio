//! Read-only production batch runbook.
//!
//! The runbook is deliberately a projection, not another queue or run state
//! machine.  `ProductionBatchRunbookRepository` is the small read-side seam
//! for the existing ProductionQueue/ShotBatch/ProductionStructure stores. A
//! production adapter should implement it with one joined query (or a small
//! bounded set of queries) and must return only shot-bound items; generic
//! production batches therefore never enter this projection.

use crate::application::ports::RepositoryError;
use crate::application::production_queue_service::{
    ProductionAdmissionView, ProductionQueueError, ProductionQueueService,
};
use crate::application::production_structure_service::{
    ProductionStructureError, ProductionStructureService, ProductionStructureTreeView,
};
use crate::domain::{ProductionBatch, ProductionBatchItemStatus, ProductionBatchStatus, ShotStage};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    error::Error,
    fmt,
    sync::Arc,
};

pub const RUNBOOK_MIXED_SCOPE: &str = "RUNBOOK_MIXED_SCOPE";
pub const RECENT_COMPLETED_BATCH_DAYS: i64 = 7;

/// A fully hydrated row supplied by the read-side query helper.
///
/// `scene_id` and `stage` come from the ShotBatch binding -> Shot -> scene
/// assignment relation. They must not be reconstructed from a batch name.
#[derive(Clone, Debug, PartialEq)]
pub struct ProductionBatchRunbookSourceRow {
    pub batch: ProductionBatch,
    pub item_id: String,
    pub item_status: ProductionBatchItemStatus,
    pub shot_id: String,
    pub stage: ShotStage,
    pub scene_id: String,
}

/// Read-only seam for the existing ProductionBatch/ShotBatch/Structure data.
/// Implementations should hydrate all rows for a project set-wise.
#[async_trait]
pub trait ProductionBatchRunbookRepository: Send + Sync {
    async fn list_project_shot_batch_runbook_rows(
        &self,
        project_id: &str,
    ) -> Result<Vec<ProductionBatchRunbookSourceRow>, RepositoryError>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionBatchRunbookWarning {
    pub code: String,
    pub batch_id: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionBatchRunbookRow {
    pub batch_id: String,
    pub batch_name: String,
    pub batch_status: String,
    pub stage: Option<String>,
    pub series_id: Option<String>,
    pub series_name: Option<String>,
    pub series_ordinal: Option<u32>,
    pub episode_id: Option<String>,
    pub episode_name: Option<String>,
    pub episode_ordinal: Option<u32>,
    pub scene_id: Option<String>,
    pub scene_name: Option<String>,
    pub scene_ordinal: Option<u32>,
    pub shot_count: usize,
    pub pending: usize,
    pub active: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub skipped: usize,
    pub created_at: DateTime<Utc>,
    pub ready_to_start: bool,
    pub blocked_reason: Option<String>,
    pub mixed_scope: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionBatchRunbookSummary {
    pub batch_total: usize,
    pub ready_batches: usize,
    pub running_batches: usize,
    pub paused_batches: usize,
    pub completed_batches: usize,
    pub pending: usize,
    pub active: usize,
    pub succeeded: usize,
    pub failed: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionBatchRunbook {
    pub project_id: String,
    pub series_id: Option<String>,
    pub rows: Vec<ProductionBatchRunbookRow>,
    pub summary: ProductionBatchRunbookSummary,
    pub warnings: Vec<ProductionBatchRunbookWarning>,
    pub recommended_batch_id: Option<String>,
    pub recommendation_reason: Option<String>,
}

pub struct ProductionBatchRunbookService {
    structure_service: Arc<ProductionStructureService>,
    runbook_repository: Arc<dyn ProductionBatchRunbookRepository>,
    queue_service: Arc<ProductionQueueService>,
}

impl ProductionBatchRunbookService {
    pub fn new(
        structure_service: Arc<ProductionStructureService>,
        runbook_repository: Arc<dyn ProductionBatchRunbookRepository>,
        queue_service: Arc<ProductionQueueService>,
    ) -> Self {
        Self {
            structure_service,
            runbook_repository,
            queue_service,
        }
    }

    /// Loads the tree, joined batch rows, and the existing queue admission
    /// status once each. It never starts, pauses, schedules, or mutates a
    /// batch; starting remains the responsibility of ProductionQueueService.
    pub async fn list(
        &self,
        project_id: &str,
        series_id: Option<&str>,
    ) -> Result<ProductionBatchRunbook, ProductionBatchRunbookError> {
        let tree = self.structure_service.tree(project_id).await?;
        let rows = self
            .runbook_repository
            .list_project_shot_batch_runbook_rows(project_id)
            .await?;
        let admission = self
            .queue_service
            .admission_status()
            .await
            .map_err(|error| ProductionBatchRunbookError::Queue(error.to_string()))?;
        build_runbook(
            project_id,
            series_id,
            &tree,
            rows,
            &admission,
            Utc::now() - Duration::days(RECENT_COMPLETED_BATCH_DAYS),
        )
    }
}

/// Pure projection used by the service and by focused backend tests.
/// `source_rows` is already set-hydrated, so this function performs no query
/// per batch, item, shot, or scene.
pub fn build_runbook(
    project_id: &str,
    series_id: Option<&str>,
    tree: &ProductionStructureTreeView,
    source_rows: Vec<ProductionBatchRunbookSourceRow>,
    admission: &ProductionAdmissionView,
    completed_since: DateTime<Utc>,
) -> Result<ProductionBatchRunbook, ProductionBatchRunbookError> {
    if tree.project_id != project_id {
        return Err(ProductionBatchRunbookError::InvalidInput(
            "RUNBOOK_PROJECT_MISMATCH".to_owned(),
        ));
    }

    let scene_index = scene_index(tree);
    let series_exists = series_id.is_none()
        || tree
            .series
            .iter()
            .any(|series| Some(series.series.id.as_str()) == series_id);
    if !series_exists {
        return Err(ProductionBatchRunbookError::NotFound(
            "SERIES_NOT_FOUND".to_owned(),
        ));
    }

    let mut groups = BTreeMap::<String, BatchGroup>::new();
    for source in source_rows {
        if source.batch.project_id != project_id || source.batch.archived_at.is_some() {
            continue;
        }
        if source.batch.status == ProductionBatchStatus::Completed
            && source.batch.updated_at < completed_since
        {
            continue;
        }

        let batch_id = source.batch.id.as_str().to_owned();
        let group = groups
            .entry(batch_id)
            .or_insert_with(|| BatchGroup::new(source.batch.clone()));
        if let Some(scene) = scene_index.get(&source.scene_id) {
            group.scopes.insert(scene.key());
        } else {
            group.missing_scope = true;
        }
        group.stages.insert(source.stage.as_str().to_owned());
        group.shot_ids.insert(source.shot_id.clone());
        group.items.entry(source.item_id.clone()).or_insert(source);
    }

    let mut rows = Vec::with_capacity(groups.len());
    let mut warnings = Vec::new();
    for group in groups.into_values() {
        let selected = series_id.is_none()
            || group
                .scopes
                .iter()
                .any(|scope| Some(scope.series_id.as_str()) == series_id);
        if !selected {
            continue;
        }

        let mixed_scope = group.missing_scope || group.scopes.len() != 1 || group.stages.len() != 1;
        let scope = (!mixed_scope)
            .then(|| group.scopes.first().expect("one scope after mixed check"))
            .and_then(|key| scene_index.get(&key.scene_id));
        let stage = (!mixed_scope).then(|| {
            group
                .stages
                .first()
                .expect("one stage after mixed check")
                .clone()
        });
        let counts = item_counts(group.items.values());
        let warning = mixed_scope.then(|| ProductionBatchRunbookWarning {
            code: RUNBOOK_MIXED_SCOPE.to_owned(),
            batch_id: group.batch.id.as_str().to_owned(),
            message: "batch bindings span more than one scene or stage".to_owned(),
        });
        if let Some(warning) = warning.clone() {
            warnings.push(warning);
        }

        rows.push(ProductionBatchRunbookRow {
            batch_id: group.batch.id.as_str().to_owned(),
            batch_name: group.batch.name.clone(),
            batch_status: group.batch.status.as_str().to_owned(),
            stage,
            series_id: scope.map(|value| value.series_id.clone()),
            series_name: scope.map(|value| value.series_name.clone()),
            series_ordinal: scope.map(|value| value.series_ordinal),
            episode_id: scope.map(|value| value.episode_id.clone()),
            episode_name: scope.map(|value| value.episode_name.clone()),
            episode_ordinal: scope.map(|value| value.episode_ordinal),
            scene_id: scope.map(|value| value.scene_id.clone()),
            scene_name: scope.map(|value| value.scene_name.clone()),
            scene_ordinal: scope.map(|value| value.scene_ordinal),
            shot_count: group.shot_ids.len(),
            pending: counts.pending,
            active: counts.active,
            succeeded: counts.succeeded,
            failed: counts.failed,
            cancelled: counts.cancelled,
            skipped: counts.skipped,
            created_at: group.batch.created_at,
            ready_to_start: group.batch.status == ProductionBatchStatus::Ready
                && counts.pending > 0
                && !mixed_scope,
            blocked_reason: warning.map(|value| value.code),
            mixed_scope,
        });
    }

    rows.sort_by(runbook_row_order);
    warnings.sort_by(|left, right| left.batch_id.cmp(&right.batch_id));
    let summary = summarize(&rows);
    let (recommended_batch_id, recommendation_reason) = recommend(&rows, admission);

    Ok(ProductionBatchRunbook {
        project_id: project_id.to_owned(),
        series_id: series_id.map(ToOwned::to_owned),
        rows,
        summary,
        warnings,
        recommended_batch_id,
        recommendation_reason,
    })
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ScopeKey {
    series_id: String,
    episode_id: String,
    scene_id: String,
}

#[derive(Clone, Debug)]
struct SceneContext {
    series_id: String,
    series_name: String,
    series_ordinal: u32,
    episode_id: String,
    episode_name: String,
    episode_ordinal: u32,
    scene_id: String,
    scene_name: String,
    scene_ordinal: u32,
}

impl SceneContext {
    fn key(&self) -> ScopeKey {
        ScopeKey {
            series_id: self.series_id.clone(),
            episode_id: self.episode_id.clone(),
            scene_id: self.scene_id.clone(),
        }
    }
}

fn scene_index(tree: &ProductionStructureTreeView) -> HashMap<String, SceneContext> {
    let mut index = HashMap::new();
    for series in &tree.series {
        for episode in &series.episodes {
            for scene in &episode.scenes {
                index.insert(
                    scene.scene.id.clone(),
                    SceneContext {
                        series_id: series.series.id.clone(),
                        series_name: series.series.name.clone(),
                        series_ordinal: series.series.ordinal,
                        episode_id: episode.episode.id.clone(),
                        episode_name: episode.episode.name.clone(),
                        episode_ordinal: episode.episode.ordinal,
                        scene_id: scene.scene.id.clone(),
                        scene_name: scene.scene.name.clone(),
                        scene_ordinal: scene.scene.ordinal,
                    },
                );
            }
        }
    }
    index
}

struct BatchGroup {
    batch: ProductionBatch,
    items: BTreeMap<String, ProductionBatchRunbookSourceRow>,
    scopes: BTreeSet<ScopeKey>,
    stages: BTreeSet<String>,
    shot_ids: BTreeSet<String>,
    missing_scope: bool,
}

impl BatchGroup {
    fn new(batch: ProductionBatch) -> Self {
        Self {
            batch,
            items: BTreeMap::new(),
            scopes: BTreeSet::new(),
            stages: BTreeSet::new(),
            shot_ids: BTreeSet::new(),
            missing_scope: false,
        }
    }
}

#[derive(Default)]
struct ItemCounts {
    pending: usize,
    active: usize,
    succeeded: usize,
    failed: usize,
    cancelled: usize,
    skipped: usize,
}

fn item_counts<'a>(items: impl Iterator<Item = &'a ProductionBatchRunbookSourceRow>) -> ItemCounts {
    let mut counts = ItemCounts::default();
    for item in items {
        match item.item_status {
            ProductionBatchItemStatus::Pending => counts.pending += 1,
            ProductionBatchItemStatus::Dispatching | ProductionBatchItemStatus::Dispatched => {
                counts.active += 1
            }
            ProductionBatchItemStatus::Succeeded => counts.succeeded += 1,
            ProductionBatchItemStatus::Failed => counts.failed += 1,
            ProductionBatchItemStatus::Cancelled => counts.cancelled += 1,
            ProductionBatchItemStatus::Skipped => counts.skipped += 1,
        }
    }
    counts
}

fn runbook_row_order(
    left: &ProductionBatchRunbookRow,
    right: &ProductionBatchRunbookRow,
) -> std::cmp::Ordering {
    (
        left.series_ordinal.unwrap_or(u32::MAX),
        left.episode_ordinal.unwrap_or(u32::MAX),
        left.scene_ordinal.unwrap_or(u32::MAX),
        stage_priority(left.stage.as_deref()),
        left.created_at,
        left.batch_id.as_str(),
    )
        .cmp(&(
            right.series_ordinal.unwrap_or(u32::MAX),
            right.episode_ordinal.unwrap_or(u32::MAX),
            right.scene_ordinal.unwrap_or(u32::MAX),
            stage_priority(right.stage.as_deref()),
            right.created_at,
            right.batch_id.as_str(),
        ))
}

fn stage_priority(stage: Option<&str>) -> u8 {
    match stage {
        Some("image") => 0,
        Some("video") => 1,
        _ => 2,
    }
}

fn summarize(rows: &[ProductionBatchRunbookRow]) -> ProductionBatchRunbookSummary {
    let mut summary = ProductionBatchRunbookSummary {
        batch_total: rows.len(),
        ..ProductionBatchRunbookSummary::default()
    };
    for row in rows {
        match row.batch_status.as_str() {
            "READY" => summary.ready_batches += 1,
            "RUNNING" => summary.running_batches += 1,
            "PAUSED" => summary.paused_batches += 1,
            "COMPLETED" => summary.completed_batches += 1,
            _ => {}
        }
        summary.pending += row.pending;
        summary.active += row.active;
        summary.succeeded += row.succeeded;
        summary.failed += row.failed;
    }
    summary
}

fn recommend(
    rows: &[ProductionBatchRunbookRow],
    admission: &ProductionAdmissionView,
) -> (Option<String>, Option<String>) {
    if let Some(row) = rows.iter().find(|row| row.batch_status == "RUNNING") {
        return (Some(row.batch_id.clone()), Some("当前正在生产".to_owned()));
    }
    if admission.busy {
        if let Some(batch_id) = admission.batch_id.as_deref() {
            if let Some(row) = rows.iter().find(|row| row.batch_id == batch_id) {
                return (
                    Some(row.batch_id.clone()),
                    Some("当前队列阻塞批次".to_owned()),
                );
            }
        }
    }
    if let Some(row) = rows.iter().find(|row| row.batch_status == "PAUSED") {
        return (Some(row.batch_id.clone()), Some("当前暂停批次".to_owned()));
    }
    rows.iter()
        .find(|row| row.batch_status == "READY" && row.ready_to_start)
        .map(|row| {
            (
                Some(row.batch_id.clone()),
                Some("按生产顺序的下一批".to_owned()),
            )
        })
        .unwrap_or((None, None))
}

#[derive(Debug)]
pub enum ProductionBatchRunbookError {
    InvalidInput(String),
    NotFound(String),
    Structure(ProductionStructureError),
    Repository(RepositoryError),
    Queue(String),
}

impl fmt::Display for ProductionBatchRunbookError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) | Self::NotFound(message) | Self::Queue(message) => {
                formatter.write_str(message)
            }
            Self::Structure(error) => write!(formatter, "{error}"),
            Self::Repository(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for ProductionBatchRunbookError {}

impl From<ProductionStructureError> for ProductionBatchRunbookError {
    fn from(value: ProductionStructureError) -> Self {
        Self::Structure(value)
    }
}

impl From<RepositoryError> for ProductionBatchRunbookError {
    fn from(value: RepositoryError) -> Self {
        Self::Repository(value)
    }
}

impl From<ProductionQueueError> for ProductionBatchRunbookError {
    fn from(value: ProductionQueueError) -> Self {
        Self::Queue(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::production_structure_service::{
        ProductionEpisodeTreeView, ProductionEpisodeView, ProductionSceneTreeView,
        ProductionSceneView, ProductionSeriesTreeView, ProductionSeriesView,
    };
    use crate::domain::{ProductionBatchId, ProductionBatchItemId};
    use chrono::TimeZone;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 18, 0, 0, 0).unwrap()
    }

    fn tree() -> ProductionStructureTreeView {
        tree_with_scenes(&["scn_scene_a", "scn_scene_b"])
    }

    fn tree_with_scenes(scene_ids: &[&str]) -> ProductionStructureTreeView {
        ProductionStructureTreeView {
            project_id: "prj_demo".to_owned(),
            series: vec![ProductionSeriesTreeView {
                series: ProductionSeriesView {
                    id: "ser_series_a".to_owned(),
                    project_id: "prj_demo".to_owned(),
                    ordinal: 0,
                    name: "Season A".to_owned(),
                    description: String::new(),
                    created_at: now(),
                    updated_at: now(),
                },
                episodes: vec![ProductionEpisodeTreeView {
                    episode: ProductionEpisodeView {
                        id: "epi_episode_a".to_owned(),
                        series_id: "ser_series_a".to_owned(),
                        ordinal: 0,
                        name: "Episode A".to_owned(),
                        description: String::new(),
                        created_at: now(),
                        updated_at: now(),
                    },
                    scenes: scene_ids
                        .iter()
                        .enumerate()
                        .map(|(ordinal, id)| ProductionSceneTreeView {
                            scene: ProductionSceneView {
                                id: (*id).to_owned(),
                                episode_id: "epi_episode_a".to_owned(),
                                ordinal: ordinal as u32,
                                name: format!("Scene {ordinal}"),
                                description: String::new(),
                                created_at: now(),
                                updated_at: now(),
                            },
                            shot_ids: vec![format!("shot_{ordinal}")],
                        })
                        .collect(),
                }],
            }],
            unassigned_shot_ids: Vec::new(),
        }
    }

    fn batch(
        id: &str,
        status: ProductionBatchStatus,
        created_at: DateTime<Utc>,
    ) -> ProductionBatch {
        ProductionBatch {
            id: ProductionBatchId::parse(id.to_owned()).unwrap(),
            project_id: "prj_demo".to_owned(),
            name: id.to_owned(),
            status,
            continue_on_failure: false,
            archived_at: None,
            created_at,
            updated_at: created_at,
        }
    }

    fn source(
        batch: ProductionBatch,
        item_id: &str,
        status: ProductionBatchItemStatus,
        shot_id: &str,
        scene_id: &str,
        stage: ShotStage,
    ) -> ProductionBatchRunbookSourceRow {
        ProductionBatchRunbookSourceRow {
            batch,
            item_id: ProductionBatchItemId::parse(item_id.to_owned())
                .unwrap()
                .as_str()
                .to_owned(),
            item_status: status,
            shot_id: shot_id.to_owned(),
            stage,
            scene_id: scene_id.to_owned(),
        }
    }

    fn build(rows: Vec<ProductionBatchRunbookSourceRow>) -> ProductionBatchRunbook {
        build_runbook(
            "prj_demo",
            Some("ser_series_a"),
            &tree(),
            rows,
            &ProductionAdmissionView::default(),
            now() - Duration::days(7),
        )
        .unwrap()
    }

    #[test]
    fn maps_batch_to_scene_episode_series_and_binding_stage() {
        let runbook = build(vec![source(
            batch("pbt_batch_a", ProductionBatchStatus::Ready, now()),
            "pbi_item_a",
            ProductionBatchItemStatus::Pending,
            "shot_0",
            "scn_scene_a",
            ShotStage::Image,
        )]);
        let row = &runbook.rows[0];
        assert_eq!(row.series_id.as_deref(), Some("ser_series_a"));
        assert_eq!(row.episode_id.as_deref(), Some("epi_episode_a"));
        assert_eq!(row.scene_id.as_deref(), Some("scn_scene_a"));
        assert_eq!(row.stage.as_deref(), Some("image"));
        assert_eq!(row.shot_count, 1);
        assert_eq!(row.pending, 1);
        assert!(row.ready_to_start);
    }

    #[test]
    fn orders_by_structure_then_stage_then_created_at_and_id() {
        let time = now();
        let runbook = build(vec![
            source(
                batch("pbt_video", ProductionBatchStatus::Ready, time),
                "pbi_video",
                ProductionBatchItemStatus::Pending,
                "shot_0",
                "scn_scene_a",
                ShotStage::Video,
            ),
            source(
                batch("pbt_image", ProductionBatchStatus::Ready, time),
                "pbi_image",
                ProductionBatchItemStatus::Pending,
                "shot_0",
                "scn_scene_a",
                ShotStage::Image,
            ),
            source(
                batch("pbt_later_scene", ProductionBatchStatus::Ready, time),
                "pbi_later",
                ProductionBatchItemStatus::Pending,
                "shot_1",
                "scn_scene_b",
                ShotStage::Image,
            ),
        ]);
        assert_eq!(
            runbook
                .rows
                .iter()
                .map(|row| row.batch_id.as_str())
                .collect::<Vec<_>>(),
            vec!["pbt_image", "pbt_video", "pbt_later_scene"]
        );
    }

    #[test]
    fn recommends_running_before_ready() {
        let runbook = build(vec![
            source(
                batch("pbt_ready", ProductionBatchStatus::Ready, now()),
                "pbi_ready",
                ProductionBatchItemStatus::Pending,
                "shot_0",
                "scn_scene_a",
                ShotStage::Image,
            ),
            source(
                batch("pbt_running", ProductionBatchStatus::Running, now()),
                "pbi_running",
                ProductionBatchItemStatus::Dispatched,
                "shot_1",
                "scn_scene_b",
                ShotStage::Image,
            ),
        ]);
        assert_eq!(runbook.recommended_batch_id.as_deref(), Some("pbt_running"));
        assert_eq!(
            runbook.recommendation_reason.as_deref(),
            Some("当前正在生产")
        );
    }

    #[test]
    fn recommends_queue_blocker_then_paused_then_ready() {
        let paused = source(
            batch("pbt_paused", ProductionBatchStatus::Paused, now()),
            "pbi_paused",
            ProductionBatchItemStatus::Pending,
            "shot_0",
            "scn_scene_a",
            ShotStage::Image,
        );
        let admission = ProductionAdmissionView {
            busy: true,
            batch_id: Some("pbt_paused".to_owned()),
            ..ProductionAdmissionView::default()
        };
        let runbook = build_runbook(
            "prj_demo",
            Some("ser_series_a"),
            &tree(),
            vec![paused],
            &admission,
            now() - Duration::days(7),
        )
        .unwrap();
        assert_eq!(runbook.recommended_batch_id.as_deref(), Some("pbt_paused"));
        assert_eq!(
            runbook.recommendation_reason.as_deref(),
            Some("当前队列阻塞批次")
        );
    }

    #[test]
    fn excludes_archived_and_stale_completed_batches() {
        let mut archived = batch("pbt_archived", ProductionBatchStatus::Ready, now());
        archived.archived_at = Some(now());
        let stale = batch(
            "pbt_stale",
            ProductionBatchStatus::Completed,
            now() - Duration::days(8),
        );
        let recent = batch(
            "pbt_recent",
            ProductionBatchStatus::Completed,
            now() - Duration::days(1),
        );
        let runbook = build(vec![
            source(
                archived,
                "pbi_archived",
                ProductionBatchItemStatus::Pending,
                "shot_0",
                "scn_scene_a",
                ShotStage::Image,
            ),
            source(
                stale,
                "pbi_stale",
                ProductionBatchItemStatus::Succeeded,
                "shot_0",
                "scn_scene_a",
                ShotStage::Image,
            ),
            source(
                recent,
                "pbi_recent",
                ProductionBatchItemStatus::Succeeded,
                "shot_0",
                "scn_scene_a",
                ShotStage::Image,
            ),
        ]);
        assert_eq!(runbook.rows.len(), 1);
        assert_eq!(runbook.rows[0].batch_id, "pbt_recent");
    }

    #[test]
    fn marks_mixed_scene_scope_without_assigning_first_scene() {
        let batch = batch("pbt_mixed", ProductionBatchStatus::Ready, now());
        let runbook = build(vec![
            source(
                batch.clone(),
                "pbi_item_a",
                ProductionBatchItemStatus::Pending,
                "shot_0",
                "scn_scene_a",
                ShotStage::Image,
            ),
            source(
                batch,
                "pbi_item_b",
                ProductionBatchItemStatus::Pending,
                "shot_1",
                "scn_scene_b",
                ShotStage::Image,
            ),
        ]);
        let row = &runbook.rows[0];
        assert_eq!(row.blocked_reason.as_deref(), Some(RUNBOOK_MIXED_SCOPE));
        assert_eq!(row.scene_id, None);
        assert!(!row.ready_to_start);
        assert_eq!(runbook.warnings[0].code, RUNBOOK_MIXED_SCOPE);
    }

    #[test]
    fn summarizes_all_item_states_from_one_set_hydrated_input() {
        let batch = batch("pbt_states", ProductionBatchStatus::Paused, now());
        let runbook = build(vec![
            source(
                batch.clone(),
                "pbi_pending",
                ProductionBatchItemStatus::Pending,
                "shot_0",
                "scn_scene_a",
                ShotStage::Image,
            ),
            source(
                batch.clone(),
                "pbi_active",
                ProductionBatchItemStatus::Dispatched,
                "shot_1",
                "scn_scene_b",
                ShotStage::Image,
            ),
            source(
                batch.clone(),
                "pbi_success",
                ProductionBatchItemStatus::Succeeded,
                "shot_0",
                "scn_scene_a",
                ShotStage::Image,
            ),
            source(
                batch,
                "pbi_failed",
                ProductionBatchItemStatus::Failed,
                "shot_1",
                "scn_scene_b",
                ShotStage::Image,
            ),
        ]);
        assert_eq!(runbook.rows[0].pending, 1);
        assert_eq!(runbook.rows[0].active, 1);
        assert_eq!(runbook.rows[0].succeeded, 1);
        assert_eq!(runbook.rows[0].failed, 1);
        assert_eq!(runbook.summary.batch_total, 1);
        assert_eq!(runbook.summary.pending, 1);
        assert_eq!(runbook.summary.active, 1);
    }

    #[test]
    fn generic_batches_are_excluded_because_only_shot_bound_rows_enter_the_projection() {
        let runbook = build(vec![source(
            batch("pbt_shot_bound", ProductionBatchStatus::Ready, now()),
            "pbi_shot_bound",
            ProductionBatchItemStatus::Pending,
            "shot_0",
            "scn_scene_a",
            ShotStage::Image,
        )]);
        assert_eq!(runbook.rows.len(), 1);
        assert_ne!(runbook.rows[0].batch_name, "generic batch");
    }
}
