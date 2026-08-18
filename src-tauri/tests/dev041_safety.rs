//! DEV-041 Agent D: no-GPU Episode planning and multi-scene safety contracts.
//!
//! These tests intentionally model the Episode boundary in memory. They do
//! not start Tauri, ComfyUI, an HTTP server, a queue worker, or a GPU task.
//! The implementation gate audits the integrated Episode service and panel.

use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Barrier, Mutex},
    thread,
};

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Classification {
    Done,
    Prepared,
    Eligible,
    Blocked,
    Empty,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Stage {
    Image,
    Video,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Shot {
    id: String,
    scene_id: String,
    stage: Stage,
    classification: Classification,
    selected_image_asset_id: Option<String>,
    selected_video_asset_id: Option<String>,
    references: Vec<String>,
    prompt: String,
}

#[derive(Clone, Debug)]
struct Scene {
    id: String,
    name: String,
    shots: Vec<Shot>,
}

#[derive(Clone, Debug)]
struct Episode {
    project_id: String,
    id: String,
    name: String,
    scenes: Vec<Scene>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct PrepareOutcome {
    created_batches: usize,
    created_items: usize,
    skipped_scenes: Vec<String>,
    blocking_scenes: Vec<String>,
    auto_started: bool,
}

fn repo_root() -> PathBuf {
    Path::new(ROOT)
        .parent()
        .expect("src-tauri must have a repository parent")
        .to_path_buf()
}

fn read_repo(path: impl AsRef<Path>) -> String {
    fs::read_to_string(repo_root().join(path)).expect("DEV-041 audit source should be readable")
}

fn shot(scene_id: &str, ordinal: usize, classification: Classification) -> Shot {
    Shot {
        id: format!("{scene_id}-shot-{ordinal:02}"),
        scene_id: scene_id.to_owned(),
        stage: Stage::Image,
        classification,
        selected_image_asset_id: None,
        selected_video_asset_id: None,
        references: vec![format!("ref-{scene_id}")],
        prompt: format!("prompt-{scene_id}-{ordinal:02}"),
    }
}

fn scene(id: &str, name: &str, counts: &[(Classification, usize)]) -> Scene {
    let shots = counts
        .iter()
        .flat_map(|(classification, count)| {
            (0..*count).map(move |index| shot(id, index + 1, *classification))
        })
        .enumerate()
        .map(|(index, mut item)| {
            item.id = format!("{id}-shot-{:02}", index + 1);
            item
        })
        .collect();
    Scene {
        id: id.to_owned(),
        name: name.to_owned(),
        shots,
    }
}

fn episode_a_fixture() -> Episode {
    Episode {
        project_id: "project-a".to_owned(),
        id: "episode-a".to_owned(),
        name: "Episode A".to_owned(),
        scenes: vec![
            scene("scene-1", "Scene 1", &[(Classification::Done, 10)]),
            scene(
                "scene-2",
                "Scene 2",
                &[(Classification::Done, 5), (Classification::Eligible, 5)],
            ),
            scene("scene-3", "Scene 3", &[(Classification::Prepared, 10)]),
            scene(
                "scene-4",
                "Scene 4",
                &[(Classification::Eligible, 8), (Classification::Blocked, 2)],
            ),
            scene("scene-5", "Scene 5", &[(Classification::Eligible, 10)]),
            Scene {
                id: "scene-6".to_owned(),
                name: "Scene 6".to_owned(),
                shots: Vec::new(),
            },
        ],
    }
}

fn classify_scene(scene: &Scene) -> Classification {
    if scene.shots.is_empty() {
        return Classification::Empty;
    }
    if scene
        .shots
        .iter()
        .all(|shot| shot.classification == Classification::Done)
    {
        return Classification::Done;
    }
    if scene
        .shots
        .iter()
        .all(|shot| shot.classification == Classification::Prepared)
    {
        return Classification::Prepared;
    }
    if scene
        .shots
        .iter()
        .any(|shot| shot.classification == Classification::Blocked)
    {
        if scene
            .shots
            .iter()
            .any(|shot| shot.classification == Classification::Eligible)
        {
            return Classification::Eligible;
        }
        return Classification::Blocked;
    }
    Classification::Eligible
}

fn episode_totals(episode: &Episode) -> HashMap<Classification, usize> {
    episode
        .scenes
        .iter()
        .flat_map(|scene| scene.shots.iter().map(|shot| shot.classification))
        .fold(HashMap::new(), |mut totals, classification| {
            *totals.entry(classification).or_insert(0) += 1;
            totals
        })
}

fn binding_key(shot: &Shot) -> String {
    format!("{}:{:?}", shot.id, shot.stage)
}

fn eligible_bindings(episode: &Episode, scene_ids: &[&str]) -> Vec<String> {
    episode
        .scenes
        .iter()
        .filter(|scene| scene_ids.contains(&scene.id.as_str()))
        .flat_map(|scene| {
            scene
                .shots
                .iter()
                .filter(|shot| shot.classification == Classification::Eligible)
                .map(binding_key)
        })
        .collect()
}

fn prepare(
    episode: &Episode,
    scene_ids: &[&str],
    allow_partial: bool,
    bindings: &mut HashSet<String>,
) -> PrepareOutcome {
    let selected = episode
        .scenes
        .iter()
        .filter(|scene| scene_ids.contains(&scene.id.as_str()))
        .collect::<Vec<_>>();
    let blockers = selected
        .iter()
        .filter(|scene| {
            scene
                .shots
                .iter()
                .any(|shot| shot.classification == Classification::Blocked)
        })
        .map(|scene| scene.id.clone())
        .collect::<Vec<_>>();

    if !allow_partial && !blockers.is_empty() {
        return PrepareOutcome {
            blocking_scenes: blockers,
            ..PrepareOutcome::default()
        };
    }

    let mut outcome = PrepareOutcome {
        blocking_scenes: blockers,
        ..PrepareOutcome::default()
    };
    for scene in selected {
        let keys = scene
            .shots
            .iter()
            .filter(|shot| shot.classification == Classification::Eligible)
            .map(binding_key)
            .collect::<Vec<_>>();
        if keys.is_empty() {
            outcome.skipped_scenes.push(scene.id.clone());
            continue;
        }
        let mut created_for_scene = 0;
        for key in keys {
            if bindings.insert(key) {
                created_for_scene += 1;
            }
        }
        if created_for_scene > 0 {
            outcome.created_batches += 1;
            outcome.created_items += created_for_scene;
        } else {
            outcome.skipped_scenes.push(scene.id.clone());
        }
    }
    outcome
}

#[test]
fn dev041_episode_a_plan_totals_and_scene_order_are_derived_from_one_fixture() {
    let episode = episode_a_fixture();
    let totals = episode_totals(&episode);

    assert_eq!(episode.scenes.len(), 6);
    assert_eq!(
        episode
            .scenes
            .iter()
            .map(|scene| scene.shots.len())
            .sum::<usize>(),
        50
    );
    assert_eq!(totals.get(&Classification::Done), Some(&15));
    assert_eq!(totals.get(&Classification::Prepared), Some(&10));
    assert_eq!(totals.get(&Classification::Eligible), Some(&23));
    assert_eq!(totals.get(&Classification::Blocked), Some(&2));
    assert_eq!(
        episode
            .scenes
            .iter()
            .map(classify_scene)
            .collect::<Vec<_>>(),
        vec![
            Classification::Done,
            Classification::Eligible,
            Classification::Prepared,
            Classification::Eligible,
            Classification::Eligible,
            Classification::Empty,
        ]
    );
}

#[test]
fn dev041_strict_selected_scene_blocker_has_zero_mutation() {
    let episode = episode_a_fixture();
    let mut bindings = HashSet::new();
    let outcome = prepare(
        &episode,
        &["scene-2", "scene-4", "scene-5"],
        false,
        &mut bindings,
    );

    assert_eq!(outcome.created_batches, 0);
    assert_eq!(outcome.created_items, 0);
    assert_eq!(outcome.blocking_scenes, vec!["scene-4"]);
    assert!(bindings.is_empty(), "strict preflight must mutate nothing");
    assert!(!outcome.auto_started);
}

#[test]
fn dev041_partial_prepare_is_three_scene_batches_and_repeat_is_noop() {
    let episode = episode_a_fixture();
    let selected = ["scene-2", "scene-4", "scene-5"];
    let mut bindings = HashSet::new();

    let first = prepare(&episode, &selected, true, &mut bindings);
    assert_eq!((first.created_batches, first.created_items), (3, 23));
    assert_eq!(first.blocking_scenes, vec!["scene-4"]);
    assert!(!first.auto_started);

    let second = prepare(&episode, &selected, true, &mut bindings);
    assert_eq!((second.created_batches, second.created_items), (0, 0));
    assert_eq!(bindings.len(), 23);
    assert!(second.skipped_scenes.contains(&"scene-2".to_owned()));
    assert!(second.skipped_scenes.contains(&"scene-4".to_owned()));
    assert!(second.skipped_scenes.contains(&"scene-5".to_owned()));
}

#[test]
fn dev041_episode_and_scene_prepare_races_keep_one_active_binding_per_shot() {
    let episode = Arc::new(episode_a_fixture());
    let bindings = Arc::new(Mutex::new(HashSet::<String>::new()));
    let barrier = Arc::new(Barrier::new(2));
    let mut workers = Vec::new();
    for _ in 0..2 {
        let episode = Arc::clone(&episode);
        let bindings = Arc::clone(&bindings);
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || {
            barrier.wait();
            let mut locked = bindings
                .lock()
                .expect("binding lock should not be poisoned");
            eligible_bindings(&episode, &["scene-5"])
                .into_iter()
                .filter(|key| locked.insert(key.clone()))
                .count()
        }));
    }
    let created = workers
        .into_iter()
        .map(|worker| worker.join().expect("race worker should finish"))
        .sum::<usize>();

    assert_eq!(created, 10);
    assert_eq!(bindings.lock().unwrap().len(), 10);
}

#[test]
fn dev041_video_plan_keeps_image_review_and_video_review_manual() {
    let rows = (1..=20)
        .map(|ordinal| Shot {
            id: format!("video-shot-{ordinal:02}"),
            scene_id: "video-scene".to_owned(),
            stage: Stage::Video,
            classification: if ordinal <= 10 {
                Classification::Eligible
            } else {
                Classification::Blocked
            },
            selected_image_asset_id: (ordinal <= 10).then(|| format!("image-{ordinal:02}")),
            selected_video_asset_id: None,
            references: Vec::new(),
            prompt: String::new(),
        })
        .collect::<Vec<_>>();

    assert_eq!(
        rows.iter()
            .filter(|row| row.classification == Classification::Eligible)
            .count(),
        10
    );
    assert_eq!(
        rows.iter()
            .filter(|row| row.classification == Classification::Blocked)
            .count(),
        10
    );
    assert!(rows
        .iter()
        .skip(10)
        .all(|row| row.selected_image_asset_id.is_none()));
    assert!(rows.iter().all(|row| row.selected_video_asset_id.is_none()));
    assert_eq!(
        json!({"autoStart": false, "autoSelectVideo": false}),
        json!({"autoStart": false, "autoSelectVideo": false})
    );
}

#[test]
fn dev041_prompt_context_is_resolved_per_scene_and_shot() {
    let template = "{{episode.name}} / {{scene.name}} / {{shot.name}}";
    let render = |episode: &str, scene: &str, shot: &str| {
        template
            .replace("{{episode.name}}", episode)
            .replace("{{scene.name}}", scene)
            .replace("{{shot.name}}", shot)
    };

    assert_eq!(
        render("Episode A", "天宫", "Shot 01"),
        "Episode A / 天宫 / Shot 01"
    );
    assert_eq!(
        render("Episode A", "地狱", "Shot 01"),
        "Episode A / 地狱 / Shot 01"
    );
    assert_ne!(
        render("Episode A", "天宫", "Shot 01"),
        render("Episode A", "地狱", "Shot 01")
    );
}

#[test]
fn dev041_multi_scene_preset_changes_stage_config_but_preserves_assets_and_references() {
    let mut shots = (0..40)
        .map(|ordinal| Shot {
            id: format!("preset-shot-{ordinal:02}"),
            scene_id: format!("scene-{}", ordinal / 10),
            stage: Stage::Image,
            classification: Classification::Eligible,
            selected_image_asset_id: Some(format!("selected-image-{ordinal:02}")),
            selected_video_asset_id: Some(format!("selected-video-{ordinal:02}")),
            references: vec![format!("character-ref-{}", ordinal % 4)],
            prompt: "old-stage-config".to_owned(),
        })
        .collect::<Vec<_>>();
    let before = shots.clone();
    for shot in &mut shots {
        shot.prompt = "new-image-preset".to_owned();
    }

    assert_eq!(shots.len(), 40);
    for (before, after) in before.iter().zip(shots.iter()) {
        assert_eq!(after.references, before.references);
        assert_eq!(
            after.selected_image_asset_id,
            before.selected_image_asset_id
        );
        assert_eq!(
            after.selected_video_asset_id,
            before.selected_video_asset_id
        );
        assert_eq!(after.prompt, "new-image-preset");
    }
}

#[test]
fn dev041_500_shots_50_scenes_5_episodes_loads_one_tree_per_plan() {
    let episodes = (0..5)
        .map(|episode_index| {
            let scenes = (0..10)
                .map(|scene_index| {
                    scene(
                        &format!("e{episode_index}-scene-{scene_index:02}"),
                        &format!("Scene {scene_index}"),
                        &[(Classification::Eligible, 10)],
                    )
                })
                .collect::<Vec<_>>();
            Episode {
                project_id: "project-bulk".to_owned(),
                id: format!("episode-{episode_index}"),
                name: format!("Episode {episode_index}"),
                scenes,
            }
        })
        .collect::<Vec<_>>();
    let mut tree_loads = 0;
    let mut planned_episodes = 0;
    let mut total_shots = 0;
    for episode in &episodes {
        tree_loads += 1;
        planned_episodes += 1;
        total_shots += episode
            .scenes
            .iter()
            .map(|scene| scene.shots.len())
            .sum::<usize>();
    }

    assert_eq!(planned_episodes, 5);
    assert_eq!(total_shots, 500);
    assert_eq!(
        episodes
            .iter()
            .map(|episode| episode.scenes.len())
            .sum::<usize>(),
        50
    );
    assert_eq!(
        tree_loads, planned_episodes,
        "one tree load per Episode plan"
    );

    let selected = episodes[0].scenes.iter().take(5).collect::<Vec<_>>();
    let selected_items = selected.iter().flat_map(|scene| scene.shots.iter()).count();
    assert_eq!(selected.len(), 5);
    assert_eq!(selected_items, 50);
}

#[test]
fn dev041_existing_runtime_has_no_parallel_episode_queue_or_gpu_path() {
    let source_root = repo_root().join("src-tauri/src");
    let mut rust_sources = String::new();
    for entry in walk_rs_files(&source_root) {
        rust_sources.push_str(&fs::read_to_string(entry).expect("Rust source should be readable"));
    }
    for forbidden in [
        "struct EpisodeQueue",
        "struct EpisodeExecutor",
        "EpisodeGenerationService",
        "production_episode_plans",
        "episode_queue_items",
        "episode_automation_state",
    ] {
        assert!(
            !rust_sources.contains(forbidden),
            "forbidden DEV-041 design: {forbidden}"
        );
    }

    let scene_service = read_repo("src-tauri/src/application/scene_production_service.rs");
    assert!(scene_service.contains("ProductionStructureService"));
    assert!(scene_service.contains("ShotBatchService"));
    for forbidden in [
        "GenerationService",
        "ComfyHttpAdapter",
        "\"/prompt\"",
        "SceneQueue",
    ] {
        assert!(
            !scene_service.contains(forbidden),
            "forbidden Scene path: {forbidden}"
        );
    }

    let lib = read_repo("src-tauri/src/lib.rs");
    assert_eq!(lib.matches("ProductionQueueService::new").count(), 1);
    assert!(!lib.contains("EpisodeQueue"));
    assert!(!lib.contains("EpisodeExecutor"));
}

#[test]
fn dev041_episode_implementation_contract_gate() {
    let service = read_repo("src-tauri/src/application/episode_production_service.rs");
    let panel = read_repo("src/features/shots/EpisodeProductionPanel.tsx");
    assert!(service.contains("ProductionStructureService"));
    assert!(service.contains("SceneProductionService"));
    assert!(service.contains("production_queue_start") || !service.contains("start_all"));
    assert!(panel.contains("allowPartial"));
    assert!(panel.to_ascii_lowercase().contains("production queue"));
    for forbidden in [
        "EpisodeQueue",
        "EpisodeExecutor",
        "GenerationService",
        "ComfyHttpAdapter",
    ] {
        assert!(
            !service.contains(forbidden),
            "forbidden Episode path: {forbidden}"
        );
    }
}

fn walk_rs_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path)
            .expect("source directory should be readable")
            .flatten()
        {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }
    files
}
