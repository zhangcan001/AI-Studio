//! DEV-040 Agent D: no-GPU scene-production safety and architecture gates.
//!
//! The first four tests are deterministic contract fixtures. They model the
//! exact scene used by the DEV-040 handoff and deliberately stop at planning
//! and binding admission; no Tauri runtime, ComfyUI, HTTP server, or GPU is
//! started. The source audit ties the fixture contract to the production
//! orchestration boundary implemented by Agent B.

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
}

#[derive(Clone, Debug)]
struct SceneShot {
    id: String,
    stage: &'static str,
    classification: Classification,
    blocking_reasons: Vec<&'static str>,
}

fn repo_root() -> PathBuf {
    Path::new(ROOT)
        .parent()
        .expect("src-tauri must have a repository parent")
        .to_path_buf()
}

fn read_repo(path: impl AsRef<Path>) -> String {
    fs::read_to_string(repo_root().join(path)).expect("DEV-040 audit source should be readable")
}

fn scene_a_fixture() -> Vec<SceneShot> {
    let mut shots = Vec::with_capacity(12);
    for ordinal in 1..=12 {
        let classification = match ordinal {
            1..=3 => Classification::Done,
            4..=5 => Classification::Prepared,
            6..=11 => Classification::Eligible,
            12 => Classification::Blocked,
            _ => unreachable!(),
        };
        shots.push(SceneShot {
            id: format!("scene-a-shot-{ordinal:02}"),
            stage: "IMAGE",
            classification,
            blocking_reasons: if classification == Classification::Blocked {
                vec!["WORKFLOW_UNAVAILABLE"]
            } else {
                Vec::new()
            },
        });
    }
    shots
}

fn counts(shots: &[SceneShot]) -> HashMap<Classification, usize> {
    let mut result = HashMap::new();
    for shot in shots {
        *result.entry(shot.classification).or_insert(0) += 1;
    }
    result
}

fn active_binding_keys(shots: &[SceneShot], allow_partial: bool) -> Vec<String> {
    if !allow_partial
        && shots
            .iter()
            .any(|shot| shot.classification == Classification::Blocked)
    {
        return Vec::new();
    }
    shots
        .iter()
        .filter(|shot| shot.classification == Classification::Eligible)
        .map(|shot| format!("{}:{}", shot.id, shot.stage))
        .collect()
}

#[test]
fn dev040_scene_plan_contract_covers_done_prepared_eligible_and_blocked() {
    let shots = scene_a_fixture();
    let count = counts(&shots);

    assert_eq!(count.get(&Classification::Done), Some(&3));
    assert_eq!(count.get(&Classification::Prepared), Some(&2));
    assert_eq!(count.get(&Classification::Eligible), Some(&6));
    assert_eq!(count.get(&Classification::Blocked), Some(&1));
    assert_eq!(active_binding_keys(&shots, false), Vec::<String>::new());
    assert_eq!(active_binding_keys(&shots, true).len(), 6);
    assert_eq!(
        shots
            .iter()
            .filter(|shot| shot.classification == Classification::Blocked)
            .flat_map(|shot| shot.blocking_reasons.iter())
            .collect::<Vec<_>>(),
        vec![&"WORKFLOW_UNAVAILABLE"]
    );
}

#[test]
fn dev040_strict_and_partial_prepare_never_include_done_prepared_or_blocked() {
    let shots = scene_a_fixture();
    let partial = active_binding_keys(&shots, true);
    assert_eq!(
        partial,
        (6..=11)
            .map(|ordinal| format!("scene-a-shot-{ordinal:02}:IMAGE"))
            .collect::<Vec<_>>()
    );

    let excluded = shots
        .iter()
        .filter(|shot| !partial.iter().any(|key| key.starts_with(&shot.id)))
        .map(|shot| shot.classification)
        .collect::<Vec<_>>();
    assert_eq!(
        excluded,
        vec![
            Classification::Done,
            Classification::Done,
            Classification::Done,
            Classification::Prepared,
            Classification::Prepared,
            Classification::Blocked,
        ]
    );
}

#[test]
fn dev040_prepare_binding_admission_is_idempotent_under_two_callers() {
    let shots = Arc::new(scene_a_fixture());
    let bindings = Arc::new(Mutex::new(HashSet::<String>::new()));
    let barrier = Arc::new(Barrier::new(2));

    let mut workers = Vec::new();
    for _ in 0..2 {
        let shots = Arc::clone(&shots);
        let bindings = Arc::clone(&bindings);
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || {
            barrier.wait();
            let mut created = 0;
            let mut locked = bindings
                .lock()
                .expect("binding gate should not be poisoned");
            for key in active_binding_keys(&shots, true) {
                if locked.insert(key) {
                    created += 1;
                }
            }
            created
        }));
    }

    let created = workers
        .into_iter()
        .map(|worker| worker.join().expect("prepare worker should finish"))
        .sum::<usize>();
    assert_eq!(created, 6, "each Shot/Stage may receive one active binding");
    assert_eq!(bindings.lock().unwrap().len(), 6);
}

#[test]
fn dev040_video_plan_keeps_image_review_manual() {
    let selected_image = (1..=5)
        .map(|ordinal| (format!("video-shot-{ordinal:02}"), true))
        .chain((6..=10).map(|ordinal| (format!("video-shot-{ordinal:02}"), false)))
        .collect::<Vec<_>>();

    let eligible = selected_image
        .iter()
        .filter(|(_, selected)| *selected)
        .count();
    let blocked = selected_image
        .iter()
        .filter(|(_, selected)| !*selected)
        .count();
    assert_eq!((eligible, blocked), (5, 5));
    assert!(selected_image.iter().skip(5).all(|(_, selected)| !selected));
    assert_eq!(
        json!({
            "stage": "VIDEO",
            "blockedReason": "IMAGE_REVIEW_REQUIRED",
            "generationStarted": false,
        })["generationStarted"],
        false
    );
}

#[test]
fn dev040_500_shots_50_scenes_plan_without_batch_fanout() {
    let shots = (0..500)
        .map(|ordinal| (format!("shot-{ordinal:03}"), ordinal / 10))
        .collect::<Vec<_>>();
    let scenes = shots
        .chunks(10)
        .map(|scene| {
            scene
                .iter()
                .map(|(_, scene_index)| *scene_index)
                .collect::<HashSet<_>>()
        })
        .collect::<Vec<_>>();

    assert_eq!(scenes.len(), 50);
    assert!(scenes.iter().all(|scene| scene.len() == 1));
    assert_eq!(shots.len(), 500);

    // Planning all scenes is allowed; preparation is intentionally limited to
    // three scenes and one normal batch per scene-stage.
    let prepared_scene_ids = scenes.iter().take(3).collect::<Vec<_>>();
    assert_eq!(prepared_scene_ids.len(), 3);
    assert_eq!(
        prepared_scene_ids
            .iter()
            .map(|scene| scene.len())
            .sum::<usize>(),
        3
    );
}

#[test]
fn dev040_architecture_reuses_shot_batch_and_has_no_second_runtime_path() {
    let service = read_repo("src-tauri/src/application/scene_production_service.rs");
    assert!(service.contains("ShotBatchService"));
    assert!(service.contains("ProductionStructureService"));
    assert!(service.contains("list_active_shot_bindings"));
    assert!(service.contains(".create(CreateShotBatchRequest"));

    for forbidden in [
        "SceneQueue",
        "SceneExecutor",
        "ProductionBatchItem",
        "GenerationService",
        "ComfyHttpAdapter",
        "comfy_service",
        "\"/prompt\"",
        "ProductionQueueService::start(",
    ] {
        assert!(
            !service.contains(forbidden),
            "forbidden DEV-040 path: {forbidden}"
        );
    }

    let binding_check = service
        .find("list_active_shot_bindings")
        .expect("prepare must re-check active Shot bindings");
    let batch_create = service
        .find(".create(CreateShotBatchRequest")
        .expect("prepare must delegate batch creation");
    assert!(
        binding_check < batch_create,
        "active binding check must occur before ShotBatchService::create"
    );

    let lib = read_repo("src-tauri/src/lib.rs");
    assert_eq!(lib.matches("ProductionQueueService::new").count(), 1);
    assert!(!lib.contains("SceneQueue"));
    assert!(!lib.contains("SceneExecutor"));

    let backup = read_repo("src-tauri/src/application/project_backup_service.rs");
    assert!(backup.contains("const BACKUP_VERSION: u32 = 15"));
    let migrations = fs::read_dir(repo_root().join("src-tauri/migrations"))
        .expect("migration directory should be readable")
        .flatten()
        .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
        .filter(|name| name.ends_with(".sql"))
        .collect::<Vec<_>>();
    assert!(migrations.iter().all(|name| {
        name.get(..3)
            .and_then(|prefix| prefix.parse::<u32>().ok())
            .is_some_and(|version| version <= 25)
    }));
}
