use crate::application::episode_production_service::{
    EpisodePrepareStatus, EpisodeProductionError, EpisodeProductionPlan, EpisodeProductionService,
};
use crate::application::production_structure_service::{
    ProductionEpisodeTreeView, ProductionStructureError, ProductionStructureService,
};
use crate::domain::ShotStage;
use serde::Serialize;
use std::{collections::HashSet, error::Error, fmt, sync::Arc};

pub const MAX_SERIES_PREPARE_EPISODES: usize = 20;
pub const MAX_SERIES_PREPARE_SCENES: usize = 100;
pub const MAX_SERIES_BULK_SHOTS: usize = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SeriesEpisodeClassification {
    Empty,
    Done,
    Prepared,
    Ready,
    Partial,
    Blocked,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesProductionEpisodePlan {
    pub episode_id: String,
    pub episode_name: String,
    pub episode_ordinal: u32,
    pub scene_total: usize,
    pub shot_total: usize,
    pub done: usize,
    pub prepared: usize,
    pub eligible: usize,
    pub blocked: usize,
    pub classification: SeriesEpisodeClassification,
    pub can_prepare: bool,
    pub existing_batch_ids: Vec<String>,
    pub blocking_reasons: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesProductionPlan {
    pub project_id: String,
    pub series_id: String,
    pub series_name: String,
    pub series_ordinal: u32,
    pub stage: String,
    pub episode_total: usize,
    pub scene_total: usize,
    pub shot_total: usize,
    pub done: usize,
    pub prepared: usize,
    pub eligible: usize,
    pub blocked: usize,
    pub ready_episode_count: usize,
    pub blocked_episode_count: usize,
    pub completed_episode_count: usize,
    pub can_prepare_all: bool,
    pub episodes: Vec<SeriesProductionEpisodePlan>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SeriesPrepareStatus {
    Success,
    Noop,
    Partial,
    Blocked,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesEpisodePrepareResult {
    pub episode_id: String,
    pub episode_name: String,
    pub status: EpisodePrepareStatus,
    pub created_batches: usize,
    pub created_items: usize,
    pub already_prepared: bool,
    pub skipped: bool,
    pub blocking_reasons: Vec<String>,
    pub batch_ids: Vec<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesProductionPrepareResult {
    pub project_id: String,
    pub series_id: String,
    pub stage: String,
    pub status: SeriesPrepareStatus,
    pub requested_episodes: usize,
    pub requested_scenes: usize,
    pub created_batches: usize,
    pub created_items: usize,
    pub already_prepared_episodes: Vec<String>,
    pub skipped_done_episodes: Vec<String>,
    pub skipped_empty_episodes: Vec<String>,
    pub skipped_blocked_episodes: Vec<String>,
    pub episode_results: Vec<SeriesEpisodePrepareResult>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeriesScope {
    pub project_id: String,
    pub series_id: String,
    pub series_name: String,
    pub series_ordinal: u32,
    pub episodes: Vec<SeriesEpisodeScope>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeriesEpisodeScope {
    pub episode_id: String,
    pub episode_name: String,
    pub episode_ordinal: u32,
    pub scenes: Vec<SeriesSceneScope>,
    pub(crate) tree: ProductionEpisodeTreeView,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeriesSceneScope {
    pub scene_id: String,
    pub scene_name: String,
    pub scene_ordinal: u32,
    pub shot_ids: Vec<String>,
}

pub struct SeriesProductionService {
    structure_service: Arc<ProductionStructureService>,
    episode_production_service: Arc<EpisodeProductionService>,
}

impl SeriesProductionService {
    pub fn new(
        structure_service: Arc<ProductionStructureService>,
        episode_production_service: Arc<EpisodeProductionService>,
    ) -> Self {
        Self {
            structure_service,
            episode_production_service,
        }
    }

    pub async fn series_scope(
        &self,
        project_id: &str,
        series_id: &str,
    ) -> Result<SeriesScope, SeriesProductionError> {
        // One tree read is the scope boundary for both plan and prepare.
        let tree = self.structure_service.tree(project_id).await?;
        let Some(series) = tree
            .series
            .into_iter()
            .find(|series| series.series.id == series_id)
        else {
            return Err(SeriesProductionError::SeriesNotFound(series_id.to_owned()));
        };
        if series.series.project_id != project_id {
            return Err(SeriesProductionError::SeriesProjectMismatch {
                series_id: series_id.to_owned(),
                project_id: project_id.to_owned(),
            });
        }

        let mut episodes = series
            .episodes
            .into_iter()
            .map(series_episode_scope)
            .collect::<Vec<_>>();
        episodes.sort_by(|left, right| {
            left.episode_ordinal
                .cmp(&right.episode_ordinal)
                .then_with(|| left.episode_id.cmp(&right.episode_id))
        });

        Ok(SeriesScope {
            project_id: project_id.to_owned(),
            series_id: series.series.id,
            series_name: series.series.name,
            series_ordinal: series.series.ordinal,
            episodes,
        })
    }

    pub async fn plan(
        &self,
        project_id: &str,
        series_id: &str,
        stage: ShotStage,
    ) -> Result<SeriesProductionPlan, SeriesProductionError> {
        let scope = self.series_scope(project_id, series_id).await?;
        let selected = (0..scope.episodes.len()).collect::<Vec<_>>();
        self.plan_scope(&scope, &selected, stage).await
    }

    pub async fn prepare(
        &self,
        project_id: &str,
        series_id: &str,
        stage: ShotStage,
        episode_ids: &[String],
        allow_partial: bool,
    ) -> Result<SeriesProductionPrepareResult, SeriesProductionError> {
        let scope = self.series_scope(project_id, series_id).await?;
        let selected = select_episode_indices(&scope.episodes, episode_ids)?;
        validate_prepare_limits(&scope, &selected)?;

        // Plan every selected episode before the first prepare call. This is the
        // strict zero-mutation gate and also makes the series totals source-of-truth.
        let plan = self.plan_scope(&scope, &selected, stage).await?;
        if !allow_partial
            && plan.episodes.iter().any(|episode| {
                matches!(
                    episode.classification,
                    SeriesEpisodeClassification::Blocked | SeriesEpisodeClassification::Partial
                )
            })
        {
            return Err(SeriesProductionError::Blocked(plan));
        }

        let mut result = SeriesProductionPrepareResult {
            project_id: project_id.to_owned(),
            series_id: series_id.to_owned(),
            stage: stage.as_str().to_owned(),
            status: SeriesPrepareStatus::Noop,
            requested_episodes: selected.len(),
            requested_scenes: plan.scene_total,
            created_batches: 0,
            created_items: 0,
            already_prepared_episodes: Vec::new(),
            skipped_done_episodes: Vec::new(),
            skipped_empty_episodes: Vec::new(),
            skipped_blocked_episodes: Vec::new(),
            episode_results: Vec::with_capacity(selected.len()),
        };

        for (episode_index, episode_plan) in selected.iter().zip(&plan.episodes) {
            let episode = &scope.episodes[*episode_index];
            match episode_plan.classification {
                SeriesEpisodeClassification::Done => {
                    result
                        .skipped_done_episodes
                        .push(episode.episode_id.clone());
                    result.episode_results.push(SeriesEpisodePrepareResult {
                        episode_id: episode.episode_id.clone(),
                        episode_name: episode.episode_name.clone(),
                        status: EpisodePrepareStatus::Noop,
                        created_batches: 0,
                        created_items: 0,
                        already_prepared: false,
                        skipped: true,
                        blocking_reasons: Vec::new(),
                        batch_ids: Vec::new(),
                        error: None,
                    });
                }
                SeriesEpisodeClassification::Empty => {
                    result
                        .skipped_empty_episodes
                        .push(episode.episode_id.clone());
                    result.episode_results.push(SeriesEpisodePrepareResult {
                        episode_id: episode.episode_id.clone(),
                        episode_name: episode.episode_name.clone(),
                        status: EpisodePrepareStatus::Noop,
                        created_batches: 0,
                        created_items: 0,
                        already_prepared: false,
                        skipped: true,
                        blocking_reasons: Vec::new(),
                        batch_ids: Vec::new(),
                        error: None,
                    });
                }
                SeriesEpisodeClassification::Prepared => {
                    result
                        .already_prepared_episodes
                        .push(episode.episode_id.clone());
                    result.episode_results.push(SeriesEpisodePrepareResult {
                        episode_id: episode.episode_id.clone(),
                        episode_name: episode.episode_name.clone(),
                        status: EpisodePrepareStatus::Noop,
                        created_batches: 0,
                        created_items: 0,
                        already_prepared: true,
                        skipped: true,
                        blocking_reasons: Vec::new(),
                        batch_ids: episode_plan.existing_batch_ids.clone(),
                        error: None,
                    });
                }
                SeriesEpisodeClassification::Blocked => {
                    result
                        .skipped_blocked_episodes
                        .push(episode.episode_id.clone());
                    result.episode_results.push(SeriesEpisodePrepareResult {
                        episode_id: episode.episode_id.clone(),
                        episode_name: episode.episode_name.clone(),
                        status: EpisodePrepareStatus::Blocked,
                        created_batches: 0,
                        created_items: 0,
                        already_prepared: false,
                        skipped: true,
                        blocking_reasons: episode_plan.blocking_reasons.clone(),
                        batch_ids: episode_plan.existing_batch_ids.clone(),
                        error: None,
                    });
                }
                SeriesEpisodeClassification::Ready | SeriesEpisodeClassification::Partial => {
                    let scene_ids = episode
                        .scenes
                        .iter()
                        .map(|scene| scene.scene_id.clone())
                        .collect::<Vec<_>>();
                    let prepared = match self
                        .episode_production_service
                        .prepare_tree_scope(
                            project_id,
                            &scope.series_id,
                            &scope.series_name,
                            &episode.tree,
                            stage,
                            &scene_ids,
                            true,
                        )
                        .await
                    {
                        Ok(prepared) => prepared,
                        Err(EpisodeProductionError::Partial(partial)) => {
                            result.created_batches += partial.created_batches;
                            result.created_items += partial.created_items;
                            result.status = SeriesPrepareStatus::Partial;
                            return Err(SeriesProductionError::Partial(result));
                        }
                        Err(error) => return Err(error.into()),
                    };
                    result.created_batches += prepared.created_batches;
                    result.created_items += prepared.created_items;
                    result.episode_results.push(SeriesEpisodePrepareResult {
                        episode_id: episode.episode_id.clone(),
                        episode_name: episode.episode_name.clone(),
                        status: prepared.status,
                        created_batches: prepared.created_batches,
                        created_items: prepared.created_items,
                        already_prepared: prepared.created_batches == 0,
                        skipped: false,
                        blocking_reasons: episode_plan.blocking_reasons.clone(),
                        batch_ids: prepared
                            .results
                            .iter()
                            .filter_map(|row| row.batch_id.clone())
                            .collect(),
                        error: None,
                    });
                }
            }
        }

        result.status = if result.created_batches == 0
            && !result.skipped_blocked_episodes.is_empty()
            && result.already_prepared_episodes.is_empty()
            && result.skipped_done_episodes.is_empty()
            && result.skipped_empty_episodes.is_empty()
        {
            SeriesPrepareStatus::Blocked
        } else if allow_partial && plan.blocked > 0 {
            SeriesPrepareStatus::Partial
        } else if result.created_batches == 0 {
            SeriesPrepareStatus::Noop
        } else {
            SeriesPrepareStatus::Success
        };
        Ok(result)
    }

    async fn plan_scope(
        &self,
        scope: &SeriesScope,
        selected: &[usize],
        stage: ShotStage,
    ) -> Result<SeriesProductionPlan, SeriesProductionError> {
        let mut episodes = Vec::with_capacity(selected.len());
        for index in selected {
            let episode = &scope.episodes[*index];
            let plan = self
                .episode_production_service
                .plan_tree_scope(
                    &scope.project_id,
                    &scope.series_id,
                    &scope.series_name,
                    &episode.tree,
                    stage,
                )
                .await?;
            episodes.push(make_episode_plan(&plan));
        }
        Ok(make_series_plan(scope, stage, episodes))
    }
}

fn series_episode_scope(episode: ProductionEpisodeTreeView) -> SeriesEpisodeScope {
    let mut scenes = episode
        .scenes
        .iter()
        .map(|scene| SeriesSceneScope {
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
    SeriesEpisodeScope {
        episode_id: episode.episode.id.clone(),
        episode_name: episode.episode.name.clone(),
        episode_ordinal: episode.episode.ordinal,
        scenes,
        tree: episode,
    }
}

fn make_episode_plan(plan: &EpisodeProductionPlan) -> SeriesProductionEpisodePlan {
    let classification = classify_episode(
        plan.shot_total,
        plan.done,
        plan.prepared,
        plan.eligible,
        plan.blocked,
    );
    let mut existing_batch_ids = Vec::new();
    let mut blocking_reasons = Vec::new();
    for scene in &plan.scenes {
        for batch_id in &scene.existing_batch_ids {
            if !existing_batch_ids.contains(batch_id) {
                existing_batch_ids.push(batch_id.clone());
            }
        }
        for reason in &scene.blocking_reasons {
            if !blocking_reasons.contains(reason) {
                blocking_reasons.push(reason.clone());
            }
        }
    }
    SeriesProductionEpisodePlan {
        episode_id: plan.episode_id.clone(),
        episode_name: plan.episode_name.clone(),
        episode_ordinal: plan.episode_ordinal,
        scene_total: plan.scene_total,
        shot_total: plan.shot_total,
        done: plan.done,
        prepared: plan.prepared,
        eligible: plan.eligible,
        blocked: plan.blocked,
        classification,
        can_prepare: plan.can_prepare_all,
        existing_batch_ids,
        blocking_reasons,
    }
}

fn make_series_plan(
    scope: &SeriesScope,
    stage: ShotStage,
    episodes: Vec<SeriesProductionEpisodePlan>,
) -> SeriesProductionPlan {
    let scene_total = episodes.iter().map(|episode| episode.scene_total).sum();
    let shot_total = episodes.iter().map(|episode| episode.shot_total).sum();
    let done = episodes.iter().map(|episode| episode.done).sum();
    let prepared = episodes.iter().map(|episode| episode.prepared).sum();
    let eligible = episodes.iter().map(|episode| episode.eligible).sum();
    let blocked = episodes.iter().map(|episode| episode.blocked).sum();
    let ready_episode_count = episodes
        .iter()
        .filter(|episode| episode.classification == SeriesEpisodeClassification::Ready)
        .count();
    let blocked_episode_count = episodes
        .iter()
        .filter(|episode| episode.blocked > 0)
        .count();
    let completed_episode_count = episodes
        .iter()
        .filter(|episode| episode.classification == SeriesEpisodeClassification::Done)
        .count();
    SeriesProductionPlan {
        project_id: scope.project_id.clone(),
        series_id: scope.series_id.clone(),
        series_name: scope.series_name.clone(),
        series_ordinal: scope.series_ordinal,
        stage: stage.as_str().to_owned(),
        episode_total: episodes.len(),
        scene_total,
        shot_total,
        done,
        prepared,
        eligible,
        blocked,
        ready_episode_count,
        blocked_episode_count,
        completed_episode_count,
        can_prepare_all: eligible > 0 && blocked_episode_count == 0,
        episodes,
    }
}

fn classify_episode(
    shot_total: usize,
    done: usize,
    prepared: usize,
    eligible: usize,
    blocked: usize,
) -> SeriesEpisodeClassification {
    if shot_total == 0 {
        SeriesEpisodeClassification::Empty
    } else if done == shot_total {
        SeriesEpisodeClassification::Done
    } else if blocked > 0 && eligible > 0 {
        SeriesEpisodeClassification::Partial
    } else if blocked > 0 {
        SeriesEpisodeClassification::Blocked
    } else if eligible == 0 && prepared > 0 {
        SeriesEpisodeClassification::Prepared
    } else {
        SeriesEpisodeClassification::Ready
    }
}

fn select_episode_indices(
    episodes: &[SeriesEpisodeScope],
    requested_ids: &[String],
) -> Result<Vec<usize>, SeriesProductionError> {
    if requested_ids.is_empty() {
        if episodes.len() > MAX_SERIES_PREPARE_EPISODES {
            return Err(SeriesProductionError::TooManyEpisodes(episodes.len()));
        }
        return Ok((0..episodes.len()).collect());
    }
    if requested_ids.len() > MAX_SERIES_PREPARE_EPISODES {
        return Err(SeriesProductionError::TooManyEpisodes(requested_ids.len()));
    }
    let episode_ids = episodes
        .iter()
        .map(|episode| episode.episode_id.clone())
        .collect::<Vec<_>>();
    select_episode_ids(&episode_ids, requested_ids).map_err(|reason| {
        SeriesProductionError::EpisodeSelectionInvalid(format!(
            "SERIES_EPISODE_SELECTION_INVALID: {reason}"
        ))
    })
}

fn validate_prepare_limits(
    scope: &SeriesScope,
    selected: &[usize],
) -> Result<(), SeriesProductionError> {
    let scene_total = selected
        .iter()
        .map(|index| scope.episodes[*index].scenes.len())
        .sum::<usize>();
    if scene_total > MAX_SERIES_PREPARE_SCENES {
        return Err(SeriesProductionError::TooManyScenes(scene_total));
    }
    let shot_ids = selected
        .iter()
        .flat_map(|index| {
            scope.episodes[*index]
                .scenes
                .iter()
                .flat_map(|scene| scene.shot_ids.iter().cloned())
        })
        .collect::<HashSet<_>>();
    if shot_ids.len() > MAX_SERIES_BULK_SHOTS {
        return Err(SeriesProductionError::ScopeTooLarge(shot_ids.len()));
    }
    Ok(())
}

#[derive(Debug)]
pub enum SeriesProductionError {
    SeriesNotFound(String),
    SeriesProjectMismatch {
        series_id: String,
        project_id: String,
    },
    EpisodeSelectionInvalid(String),
    TooManyEpisodes(usize),
    TooManyScenes(usize),
    ScopeTooLarge(usize),
    Blocked(SeriesProductionPlan),
    Partial(SeriesProductionPrepareResult),
    Structure(ProductionStructureError),
    Episode(EpisodeProductionError),
}

impl fmt::Display for SeriesProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SeriesNotFound(id) => write!(formatter, "SERIES_NOT_FOUND: {id}"),
            Self::SeriesProjectMismatch {
                series_id,
                project_id,
            } => write!(
                formatter,
                "SERIES_PROJECT_MISMATCH: series {series_id} is not in project {project_id}"
            ),
            Self::EpisodeSelectionInvalid(message) => formatter.write_str(message),
            Self::TooManyEpisodes(count) => write!(
                formatter,
                "SERIES_PRODUCTION_TOO_MANY_EPISODES: {count} exceeds {MAX_SERIES_PREPARE_EPISODES}"
            ),
            Self::TooManyScenes(count) => write!(
                formatter,
                "SERIES_PRODUCTION_TOO_MANY_SCENES: {count} exceeds {MAX_SERIES_PREPARE_SCENES}"
            ),
            Self::ScopeTooLarge(count) => write!(
                formatter,
                "SERIES_BULK_SCOPE_TOO_LARGE: {count} unique shots exceeds {MAX_SERIES_BULK_SHOTS}"
            ),
            Self::Blocked(_) => formatter.write_str("SERIES_PRODUCTION_BLOCKED"),
            Self::Partial(_) => formatter.write_str("SERIES_PRODUCTION_PARTIAL"),
            Self::Structure(error) => error.fmt(formatter),
            Self::Episode(error) => error.fmt(formatter),
        }
    }
}

impl Error for SeriesProductionError {}

impl From<ProductionStructureError> for SeriesProductionError {
    fn from(error: ProductionStructureError) -> Self {
        Self::Structure(error)
    }
}

impl From<EpisodeProductionError> for SeriesProductionError {
    fn from(error: EpisodeProductionError) -> Self {
        Self::Episode(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_episode, select_episode_ids, SeriesEpisodeClassification};

    #[test]
    fn episode_classification_matches_episode_totals() {
        assert_eq!(
            classify_episode(0, 0, 0, 0, 0),
            SeriesEpisodeClassification::Empty
        );
        assert_eq!(
            classify_episode(4, 4, 0, 0, 0),
            SeriesEpisodeClassification::Done
        );
        assert_eq!(
            classify_episode(4, 0, 4, 0, 0),
            SeriesEpisodeClassification::Prepared
        );
        assert_eq!(
            classify_episode(4, 0, 0, 4, 0),
            SeriesEpisodeClassification::Ready
        );
        assert_eq!(
            classify_episode(4, 0, 0, 2, 2),
            SeriesEpisodeClassification::Partial
        );
        assert_eq!(
            classify_episode(4, 0, 0, 0, 4),
            SeriesEpisodeClassification::Blocked
        );
    }

    #[test]
    fn episode_selection_is_unique_bounded_and_series_scoped() {
        let episodes = ["e1", "e2", "e3"].map(str::to_owned);
        assert_eq!(select_episode_ids(&episodes, &[]).unwrap(), vec![0, 1, 2]);
        assert_eq!(
            select_episode_ids(&episodes, &["e3".to_owned(), "e1".to_owned()]).unwrap(),
            vec![0, 2]
        );
        assert_eq!(
            select_episode_ids(&episodes, &["e1".to_owned(), "e1".to_owned()]),
            Err("episode_ids must be unique")
        );
        assert_eq!(
            select_episode_ids(&episodes, &["missing".to_owned()]),
            Err("every selected episode must belong to the series")
        );
    }
}

fn select_episode_ids(
    episode_ids: &[String],
    requested_ids: &[String],
) -> Result<Vec<usize>, &'static str> {
    if requested_ids.is_empty() {
        return Ok((0..episode_ids.len()).collect());
    }
    let mut requested = HashSet::new();
    if requested_ids.iter().any(|id| !requested.insert(id)) {
        return Err("episode_ids must be unique");
    }
    let selected = episode_ids
        .iter()
        .enumerate()
        .filter_map(|(index, id)| requested.contains(id).then_some(index))
        .collect::<Vec<_>>();
    (selected.len() == requested_ids.len())
        .then_some(selected)
        .ok_or("every selected episode must belong to the series")
}
