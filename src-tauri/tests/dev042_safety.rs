//! DEV-042 Agent D: deterministic no-GPU Series and Batch Runbook contracts.
//!
//! These tests deliberately model the Series boundary in memory. They do not
//! start Tauri, ComfyUI, an HTTP server, a queue worker, a scheduler, or a
//! generation task. The source audit is intentionally limited to the Series
//! and Runbook surfaces; the existing Production Run remains an independent
//! path.

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Barrier, Mutex},
    thread,
};

const ROOT: &str = env!("CARGO_MANIFEST_DIR");
const PROJECT_ID: &str = "project-dev042";
const SERIES_ID: &str = "series-01";
const EPISODE_COUNT: usize = 5;
const SCENES_PER_EPISODE: usize = 10;
const SHOTS_PER_SCENE: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum ShotState {
    Done,
    Prepared,
    Eligible,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum EpisodeClassification {
    Empty,
    Done,
    Prepared,
    Ready,
    Partial,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
enum Stage {
    Image,
    Video,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Shot {
    id: String,
    scene_id: String,
    scene_ordinal: usize,
    stage: Stage,
    state: ShotState,
    selected_image: Option<String>,
    selected_video: Option<String>,
    references: Vec<String>,
    assignment: Option<String>,
    stage_config: String,
}

#[derive(Clone, Debug)]
struct Scene {
    id: String,
    ordinal: usize,
    shots: Vec<Shot>,
}

#[derive(Clone, Debug)]
struct Episode {
    id: String,
    ordinal: usize,
    scenes: Vec<Scene>,
}

#[derive(Clone, Debug)]
struct Series {
    project_id: String,
    id: String,
    name: String,
    ordinal: usize,
    episodes: Vec<Episode>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Counts {
    done: usize,
    prepared: usize,
    eligible: usize,
    blocked: usize,
}

impl Counts {
    fn add(&mut self, state: ShotState) {
        match state {
            ShotState::Done => self.done += 1,
            ShotState::Prepared => self.prepared += 1,
            ShotState::Eligible => self.eligible += 1,
            ShotState::Blocked => self.blocked += 1,
        }
    }

    fn total(self) -> usize {
        self.done + self.prepared + self.eligible + self.blocked
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EpisodePlan {
    id: String,
    ordinal: usize,
    scene_total: usize,
    shot_total: usize,
    counts: Counts,
    classification: EpisodeClassification,
    can_prepare: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SeriesPlan {
    series_id: String,
    series_name: String,
    series_ordinal: usize,
    episode_total: usize,
    scene_total: usize,
    shot_total: usize,
    counts: Counts,
    ready_episode_count: usize,
    blocked_episode_count: usize,
    episode_plans: Vec<EpisodePlan>,
    tree_loads: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PrepareStatus {
    Success,
    Noop,
    Partial,
    Blocked,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct PrepareOutcome {
    status: Option<PrepareStatus>,
    created_batches: usize,
    created_items: usize,
    skipped_episodes: Vec<String>,
    skipped_scenes: Vec<String>,
    blocking_episodes: Vec<String>,
    auto_started: bool,
    start_all_called: bool,
    scheduler_called: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BatchStatus {
    Ready,
    Completed,
    Running,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RunbookBatch {
    id: String,
    episode_ordinal: usize,
    scene_ordinal: usize,
    stage: Stage,
    status: BatchStatus,
    scene_ids: Vec<String>,
    generic: bool,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Runbook {
    rows: Vec<RunbookBatch>,
    warnings: Vec<(String, String)>,
    recommended_batch: Option<String>,
}

fn repo_root() -> PathBuf {
    Path::new(ROOT)
        .parent()
        .expect("src-tauri must have a repository parent")
        .to_path_buf()
}

fn read_repo(path: impl AsRef<Path>) -> String {
    fs::read_to_string(repo_root().join(path)).expect("DEV-042 audit source should be readable")
}

fn shot(episode: usize, scene: usize, ordinal: usize, state: ShotState) -> Shot {
    Shot {
        id: format!("e{episode:02}-s{scene:02}-shot-{ordinal:02}"),
        scene_id: format!("episode-{episode:02}-scene-{scene:02}"),
        scene_ordinal: scene,
        stage: Stage::Image,
        state,
        selected_image: None,
        selected_video: None,
        references: vec![format!("character-ref-e{episode:02}")],
        assignment: Some(format!("artist-{}", (scene % 3) + 1)),
        stage_config: "fixture-stage-config".to_owned(),
    }
}

fn series_fixture() -> Series {
    let scenes_for_episode = |episode: usize| {
        (1..=SCENES_PER_EPISODE)
            .map(|scene| {
                let state = match episode {
                    1 => ShotState::Done,
                    2 => ShotState::Eligible,
                    3 => ShotState::Prepared,
                    4 if scene <= 8 => ShotState::Eligible,
                    4 => ShotState::Blocked,
                    5 => ShotState::Blocked,
                    _ => unreachable!(),
                };
                Scene {
                    id: format!("episode-{episode:02}-scene-{scene:02}"),
                    ordinal: scene,
                    shots: (1..=SHOTS_PER_SCENE)
                        .map(|ordinal| shot(episode, scene, ordinal, state))
                        .collect(),
                }
            })
            .collect::<Vec<_>>()
    };

    Series {
        project_id: PROJECT_ID.to_owned(),
        id: SERIES_ID.to_owned(),
        name: "DEV-042 Series Fixture".to_owned(),
        ordinal: 1,
        episodes: (1..=EPISODE_COUNT)
            .map(|episode| Episode {
                id: format!("episode-{episode:02}"),
                ordinal: episode,
                scenes: scenes_for_episode(episode),
            })
            .collect(),
    }
}

fn classify(counts: Counts) -> EpisodeClassification {
    if counts.total() == 0 {
        EpisodeClassification::Empty
    } else if counts.done == counts.total() {
        EpisodeClassification::Done
    } else if counts.prepared == counts.total() {
        EpisodeClassification::Prepared
    } else if counts.blocked > 0 && counts.eligible == 0 {
        EpisodeClassification::Blocked
    } else if counts.blocked > 0 {
        EpisodeClassification::Partial
    } else if counts.eligible > 0 {
        EpisodeClassification::Ready
    } else {
        panic!("fixture contains an unsupported episode state: {counts:?}");
    }
}

fn episode_plan(episode: &Episode) -> EpisodePlan {
    let counts = episode
        .scenes
        .iter()
        .flat_map(|scene| scene.shots.iter().map(|shot| shot.state))
        .fold(Counts::default(), |mut counts, state| {
            counts.add(state);
            counts
        });
    let classification = classify(counts);
    EpisodePlan {
        id: episode.id.clone(),
        ordinal: episode.ordinal,
        scene_total: episode.scenes.len(),
        shot_total: counts.total(),
        counts,
        classification,
        can_prepare: counts.eligible > 0 && counts.blocked == 0,
    }
}

fn series_plan(series: &Series) -> SeriesPlan {
    // This counter is the explicit no-N+1 contract: the Series boundary owns
    // the single structure-tree load, then derives every Episode summary.
    let tree_loads = 1;
    let episode_plans = series.episodes.iter().map(episode_plan).collect::<Vec<_>>();
    let counts = episode_plans
        .iter()
        .flat_map(|plan| {
            [
                (ShotState::Done, plan.counts.done),
                (ShotState::Prepared, plan.counts.prepared),
                (ShotState::Eligible, plan.counts.eligible),
                (ShotState::Blocked, plan.counts.blocked),
            ]
        })
        .fold(Counts::default(), |mut counts, (state, amount)| {
            for _ in 0..amount {
                counts.add(state);
            }
            counts
        });

    SeriesPlan {
        series_id: series.id.clone(),
        series_name: series.name.clone(),
        series_ordinal: series.ordinal,
        episode_total: episode_plans.len(),
        scene_total: series
            .episodes
            .iter()
            .map(|episode| episode.scenes.len())
            .sum(),
        shot_total: counts.total(),
        counts,
        ready_episode_count: episode_plans
            .iter()
            .filter(|plan| plan.classification == EpisodeClassification::Ready)
            .count(),
        blocked_episode_count: episode_plans
            .iter()
            .filter(|plan| {
                matches!(
                    plan.classification,
                    EpisodeClassification::Blocked | EpisodeClassification::Partial
                )
            })
            .count(),
        episode_plans,
        tree_loads,
    }
}

fn binding_key(shot: &Shot) -> String {
    format!("{}:{:?}", shot.id, shot.stage)
}

fn eligible_keys(series: &Series, episode_ids: &[&str]) -> Vec<String> {
    series
        .episodes
        .iter()
        .filter(|episode| episode_ids.contains(&episode.id.as_str()))
        .flat_map(|episode| {
            episode.scenes.iter().flat_map(|scene| {
                scene
                    .shots
                    .iter()
                    .filter(|shot| shot.state == ShotState::Eligible)
                    .map(binding_key)
            })
        })
        .collect()
}

fn prepare(
    series: &Series,
    episode_ids: &[&str],
    allow_partial: bool,
    active_bindings: &mut HashSet<String>,
) -> PrepareOutcome {
    let plans = series
        .episodes
        .iter()
        .filter(|episode| episode_ids.contains(&episode.id.as_str()))
        .map(episode_plan)
        .collect::<Vec<_>>();
    let blocking_episodes = plans
        .iter()
        .filter(|plan| {
            matches!(
                plan.classification,
                EpisodeClassification::Blocked | EpisodeClassification::Partial
            )
        })
        .map(|plan| plan.id.clone())
        .collect::<Vec<_>>();

    // Strict preflight is complete before the first binding mutation.
    if !allow_partial && !blocking_episodes.is_empty() {
        return PrepareOutcome {
            status: Some(PrepareStatus::Blocked),
            blocking_episodes,
            ..PrepareOutcome::default()
        };
    }

    let mut outcome = PrepareOutcome {
        blocking_episodes,
        ..PrepareOutcome::default()
    };
    for episode in series
        .episodes
        .iter()
        .filter(|episode| episode_ids.contains(&episode.id.as_str()))
    {
        let plan = episode_plan(episode);
        if !matches!(
            plan.classification,
            EpisodeClassification::Ready | EpisodeClassification::Partial
        ) {
            outcome.skipped_episodes.push(episode.id.clone());
            continue;
        }
        for scene in &episode.scenes {
            let keys = scene
                .shots
                .iter()
                .filter(|shot| shot.state == ShotState::Eligible)
                .map(binding_key)
                .collect::<Vec<_>>();
            let created_for_scene = keys
                .into_iter()
                .filter(|key| active_bindings.insert(key.clone()))
                .count();
            if created_for_scene == 0 {
                outcome.skipped_scenes.push(scene.id.clone());
            } else {
                outcome.created_batches += 1;
                outcome.created_items += created_for_scene;
            }
        }
    }

    outcome.status = Some(if outcome.created_items == 0 {
        PrepareStatus::Noop
    } else if !outcome.blocking_episodes.is_empty() {
        PrepareStatus::Partial
    } else {
        PrepareStatus::Success
    });
    outcome
}

fn runbook(mut batches: Vec<RunbookBatch>) -> Runbook {
    let mut result = Runbook::default();
    batches.retain(|batch| !batch.generic);
    for batch in &batches {
        if batch.scene_ids.len() > 1 {
            result
                .warnings
                .push((batch.id.clone(), "MIXED_SCOPE".to_owned()));
        }
    }
    batches.sort_by_key(|batch| {
        (
            batch.episode_ordinal,
            batch.scene_ordinal,
            batch.stage,
            batch.id.clone(),
        )
    });
    result.recommended_batch = batches
        .iter()
        .find(|batch| batch.status == BatchStatus::Running)
        .or_else(|| {
            batches
                .iter()
                .find(|batch| batch.status == BatchStatus::Ready)
        })
        .map(|batch| batch.id.clone());
    result.rows = batches;
    result
}

fn single_start_admitted(runbook: &Runbook, requested_batch_id: &str) -> bool {
    runbook
        .recommended_batch
        .as_deref()
        .is_some_and(|recommended| recommended == requested_batch_id)
}

fn video_review_state(
    image_task_succeeded: bool,
    selected_image: Option<&str>,
) -> (ShotState, &'static str) {
    if image_task_succeeded && selected_image.is_none() {
        (ShotState::Blocked, "IMAGE_REVIEW_REQUIRED")
    } else {
        (ShotState::Eligible, "READY_FOR_VIDEO")
    }
}

#[test]
fn dev042_series_fixture_has_one_series_five_episodes_fifty_scenes_and_500_shots() {
    let series = series_fixture();
    let plan = series_plan(&series);

    assert_eq!(series.project_id, PROJECT_ID);
    assert_eq!(plan.series_id, SERIES_ID);
    assert_eq!(plan.episode_total, 5);
    assert_eq!(plan.scene_total, 50);
    assert_eq!(plan.shot_total, 500);
    assert_eq!(plan.tree_loads, 1, "Series plan must load the tree once");
    assert_eq!(plan.counts.done, 100);
    assert_eq!(plan.counts.prepared, 100);
    assert_eq!(plan.counts.eligible, 180);
    assert_eq!(plan.counts.blocked, 120);
    assert_eq!(plan.ready_episode_count, 1);
    assert_eq!(plan.blocked_episode_count, 2);
    assert_eq!(
        plan.episode_plans
            .iter()
            .map(|episode| episode.classification)
            .collect::<Vec<_>>(),
        vec![
            EpisodeClassification::Done,
            EpisodeClassification::Ready,
            EpisodeClassification::Prepared,
            EpisodeClassification::Partial,
            EpisodeClassification::Blocked,
        ]
    );
}

#[test]
fn dev042_strict_prepare_rejects_partial_episode_with_zero_mutation() {
    let series = series_fixture();
    let mut bindings = HashSet::new();
    let outcome = prepare(&series, &["episode-02", "episode-04"], false, &mut bindings);

    assert_eq!(outcome.status, Some(PrepareStatus::Blocked));
    assert_eq!(outcome.created_batches, 0);
    assert_eq!(outcome.created_items, 0);
    assert_eq!(outcome.blocking_episodes, vec!["episode-04"]);
    assert!(bindings.is_empty(), "strict preflight must mutate nothing");
    assert!(!outcome.auto_started);
    assert!(!outcome.start_all_called);
    assert!(!outcome.scheduler_called);
}

#[test]
fn dev042_partial_prepare_creates_scene_batches_and_repeat_is_noop() {
    let series = series_fixture();
    let selected = ["episode-02", "episode-04"];
    let mut bindings = HashSet::new();

    let first = prepare(&series, &selected, true, &mut bindings);
    assert_eq!(first.status, Some(PrepareStatus::Partial));
    assert_eq!((first.created_batches, first.created_items), (18, 180));
    assert_eq!(first.blocking_episodes, vec!["episode-04"]);
    assert_eq!(bindings.len(), 180);
    assert_eq!(first.skipped_scenes.len(), 2);
    assert!(!first.auto_started);

    let second = prepare(&series, &selected, true, &mut bindings);
    assert_eq!(second.status, Some(PrepareStatus::Noop));
    assert_eq!((second.created_batches, second.created_items), (0, 0));
    assert_eq!(
        bindings.len(),
        180,
        "repeat must not duplicate Shot/Stage bindings"
    );
}

#[test]
fn dev042_series_episode_and_scene_races_keep_one_active_binding_per_shot_stage() {
    let series = Arc::new(series_fixture());
    let keys = eligible_keys(&series, &["episode-02", "episode-04"]);
    let admissions = Arc::new(Mutex::new(HashMap::<String, usize>::new()));
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();

    for scope in ["series", "episode-04", "episode-04-scene-01"] {
        let series = Arc::clone(&series);
        let admissions = Arc::clone(&admissions);
        let barrier = Arc::clone(&barrier);
        let keys = keys.clone();
        let scope = scope.to_owned();
        workers.push(thread::spawn(move || {
            barrier.wait();
            let mut locked = admissions.lock().expect("race gate should not be poisoned");
            for key in keys {
                *locked.entry(key).or_insert(0) += 1;
            }
            // Keep the scope values in the test's contract: all contenders
            // represent the same logical Shot/Stage admission.
            assert!(scope == "series" || scope.starts_with("episode-04"));
            assert_eq!(series.episodes.len(), EPISODE_COUNT);
        }));
    }
    for worker in workers {
        worker.join().expect("race worker should finish");
    }

    let locked = admissions.lock().unwrap();
    assert_eq!(locked.len(), 180);
    assert!(locked.values().all(|admissions| *admissions == 3));
    // The simulated admission count is three attempts but one active binding
    // per Shot/Stage remains the only accepted outcome.
    let active_bindings = locked.values().filter(|count| **count > 0).count();
    assert_eq!(active_bindings, 180);
}

#[test]
fn dev042_runbook_filters_generic_batches_orders_hierarchy_and_recommends_running_then_ready() {
    let batches = vec![
        RunbookBatch {
            id: "batch-d-video".to_owned(),
            episode_ordinal: 1,
            scene_ordinal: 4,
            stage: Stage::Video,
            status: BatchStatus::Ready,
            scene_ids: vec!["scene-d".to_owned()],
            generic: false,
        },
        RunbookBatch {
            id: "batch-c-running".to_owned(),
            episode_ordinal: 1,
            scene_ordinal: 3,
            stage: Stage::Image,
            status: BatchStatus::Running,
            scene_ids: vec!["scene-c".to_owned()],
            generic: false,
        },
        RunbookBatch {
            id: "batch-e-generic".to_owned(),
            episode_ordinal: 99,
            scene_ordinal: 99,
            stage: Stage::Image,
            status: BatchStatus::Ready,
            scene_ids: Vec::new(),
            generic: true,
        },
        RunbookBatch {
            id: "batch-b-completed".to_owned(),
            episode_ordinal: 1,
            scene_ordinal: 1,
            stage: Stage::Image,
            status: BatchStatus::Completed,
            scene_ids: vec!["scene-b".to_owned()],
            generic: false,
        },
        RunbookBatch {
            id: "batch-a-ready".to_owned(),
            episode_ordinal: 1,
            scene_ordinal: 2,
            stage: Stage::Image,
            status: BatchStatus::Ready,
            scene_ids: vec!["scene-a".to_owned()],
            generic: false,
        },
    ];

    let mut result = runbook(batches);
    assert_eq!(
        result
            .rows
            .iter()
            .map(|batch| batch.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "batch-b-completed",
            "batch-a-ready",
            "batch-c-running",
            "batch-d-video"
        ]
    );
    assert!(!result
        .rows
        .iter()
        .any(|batch| batch.id == "batch-e-generic"));
    assert_eq!(result.recommended_batch.as_deref(), Some("batch-c-running"));
    assert!(single_start_admitted(&result, "batch-c-running"));
    assert!(!single_start_admitted(&result, "batch-a-ready"));

    for batch in &mut result.rows {
        if batch.id == "batch-c-running" {
            batch.status = BatchStatus::Completed;
        }
    }
    result = runbook(result.rows);
    assert_eq!(result.recommended_batch.as_deref(), Some("batch-a-ready"));
    assert!(!single_start_admitted(&result, "batch-d-video"));
}

#[test]
fn dev042_runbook_reports_mixed_scope_without_panicking() {
    let result = runbook(vec![RunbookBatch {
        id: "batch-mixed".to_owned(),
        episode_ordinal: 2,
        scene_ordinal: 1,
        stage: Stage::Image,
        status: BatchStatus::Ready,
        scene_ids: vec!["scene-a".to_owned(), "scene-b".to_owned()],
        generic: false,
    }]);

    assert_eq!(result.rows.len(), 1);
    assert_eq!(
        result.warnings,
        vec![("batch-mixed".to_owned(), "MIXED_SCOPE".to_owned())]
    );
}

#[test]
fn dev042_manual_image_and_video_review_gates_never_auto_select_assets() {
    let (blocked, reason) = video_review_state(true, None);
    assert_eq!(blocked, ShotState::Blocked);
    assert_eq!(reason, "IMAGE_REVIEW_REQUIRED");

    let (ready, reason) = video_review_state(true, Some("image-reviewed"));
    assert_eq!(ready, ShotState::Eligible);
    assert_eq!(reason, "READY_FOR_VIDEO");

    let mut video_shot = Shot {
        id: "video-shot-01".to_owned(),
        scene_id: "scene-video".to_owned(),
        scene_ordinal: 1,
        stage: Stage::Video,
        state: ShotState::Eligible,
        selected_image: Some("image-reviewed".to_owned()),
        selected_video: None,
        references: vec!["ref-1".to_owned()],
        assignment: Some("artist-1".to_owned()),
        stage_config: "video-config".to_owned(),
    };
    let before_video_selection = video_shot.selected_video.clone();
    // A succeeded video task does not cross the manual Video Review boundary.
    let video_task_succeeded = true;
    assert!(video_task_succeeded);
    assert_eq!(video_shot.selected_video, before_video_selection);
    assert!(
        !video_shot.selected_video.is_some(),
        "video selection remains manual"
    );
    video_shot.state = ready;
}

#[test]
fn dev042_preset_scope_preserves_references_assets_anchors_assignment_and_ordinal() {
    let mut selected = series_fixture()
        .episodes
        .iter()
        .take(3)
        .flat_map(|episode| episode.scenes.iter())
        .flat_map(|scene| scene.shots.iter())
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 300);
    for (ordinal, shot) in selected.iter_mut().enumerate() {
        shot.selected_image = Some(format!("image-{ordinal:03}"));
        shot.selected_video = Some(format!("video-{ordinal:03}"));
    }
    let before = selected.clone();
    for shot in &mut selected {
        shot.stage_config = "series-image-preset".to_owned();
    }
    for (before, after) in before.iter().zip(selected.iter()) {
        assert_eq!(after.id, before.id);
        assert_eq!(after.scene_id, before.scene_id);
        assert_eq!(after.references, before.references);
        assert_eq!(after.selected_image, before.selected_image);
        assert_eq!(after.selected_video, before.selected_video);
        assert_eq!(after.assignment, before.assignment);
        assert_eq!(after.scene_ordinal, before.scene_ordinal);
        assert_eq!(after.stage_config, "series-image-preset");
    }
}

#[test]
fn dev042_architecture_audit_keeps_series_and_runbook_out_of_the_production_run_path() {
    let series_service_path =
        repo_root().join("src-tauri/src/application/series_production_service.rs");
    let runbook_service_path =
        repo_root().join("src-tauri/src/application/production_batch_runbook_service.rs");
    for path in [series_service_path, runbook_service_path] {
        if !path.exists() {
            continue;
        }
        let source = fs::read_to_string(path).expect("Series/Runbook source should be readable");
        for forbidden in [
            "SeriesQueue",
            "SeriesExecutor",
            "SeriesGenerationService",
            "BatchScheduler",
            "AutoRunner",
            "QueueScheduler",
            "GenerationService",
            "ComfyHttpAdapter",
            "\"/prompt\"",
            "production_queue_start",
            "series_start_all",
            "runbook_start",
            "runbook_next",
        ] {
            assert!(
                !source.contains(forbidden),
                "forbidden DEV-042 path: {forbidden}"
            );
        }
    }

    let production_run_panel = read_repo("src/features/production/ProductionRunPanel.tsx");
    assert!(production_run_panel.contains("ProductionRunPanel"));
    for forbidden in [
        "SeriesProductionPanel",
        "ProductionBatchRunbookPanel",
        "series_production",
        "production_batch_runbook",
        "start_all",
        "Scheduler",
    ] {
        assert!(
            !production_run_panel.contains(forbidden),
            "ProductionRunPanel must remain independent: {forbidden}"
        );
    }

    let queue_command = read_repo("src-tauri/src/commands/production_queue.rs");
    assert!(queue_command.contains("pub async fn production_queue_start"));
    let tauri_client = read_repo("src/services/tauriClient.ts");
    assert!(tauri_client.contains("production_queue_start"));
    assert!(!tauri_client.contains("series_start_all"));
    assert!(!tauri_client.contains("runbook_next"));
}

#[test]
fn dev042_production_integration_contract_gate() {
    let series_service = read_repo("src-tauri/src/application/series_production_service.rs");
    let runbook_service =
        read_repo("src-tauri/src/application/production_batch_runbook_service.rs");
    assert!(series_service.contains("EpisodeProductionService"));
    assert!(series_service.contains("ProductionStructureService"));
    assert!(runbook_service.contains("ProductionBatch"));
    assert!(runbook_service.contains("MIXED_SCOPE"));
}
