//! DEV-082 Agent-D regression contracts.
//!
//! This file owns sanitized import/identity fixtures and the cross-feature
//! regression checklist for the unified workflow library.

use ai_studio_lib::application::{
    ports::WorkflowPackageFiles,
    workflow_onboarding_service::{detect_comfy_workflow_format, ComfyWorkflowInputFormat},
    workflow_recognition_service::{
        recognize_workflow, structural_workflow_sha256, RecipeFreshness, RuntimeCapabilityState,
        RuntimeCapabilitySummary, WorkflowIdentity, WorkflowRecognitionFormat,
    },
    workflow_semantic_identity::semantic_workflow_sha256,
};
use ai_studio_lib::compiler::RecipeParser;
use ai_studio_lib::domain::WorkflowDocument;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

const ROOT: &str = env!("CARGO_MANIFEST_DIR");
const DEV081_GRAPH: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/workflows/minimax_h3_8step_graph_api.json"
));
const AITUDOU_MANIFEST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/runtime_packages/aitudou_minimax_h3_lightx2v_8step_fast_1_0_0/manifest.yaml"
));
const AITUDOU_RECIPE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/runtime_packages/aitudou_minimax_h3_lightx2v_8step_fast_1_0_0/recipe.yaml"
));
const AITUDOU_WORKFLOW: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/runtime_packages/aitudou_minimax_h3_lightx2v_8step_fast_1_0_0/workflow_api.json"
));
const DEV081_PACKAGE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/production_packages/dev081_t2v_3_items/production-package.json"
));

const UI_WORKFLOW: &str = r#"{
  "last_node_id": 2,
  "last_link_id": 1,
  "nodes": [{"id": 1, "type": "KSampler"}, {"id": 2, "type": "SaveImage"}],
  "links": [[1, 1, 0, 2, 0, "IMAGE"]]
}"#;

const SEMANTIC_A: &str = r#"{
  "1": {"class_type": "Sampler", "inputs": {"prompt": "same", "steps": 8, "megapixels": 0.9}},
  "2": {"class_type": "SaveVideo", "inputs": {"video": ["1", 0]}}
}"#;

const SEMANTIC_B: &str = r#"{
  "2": {"inputs": {"video": ["1", 0]}, "class_type": "SaveVideo"},
  "1": {"inputs": {"megapixels": 0.9, "steps": 8, "prompt": "same"}, "class_type": "Sampler"}
}"#;

fn repo_root() -> PathBuf {
    PathBuf::from(ROOT)
        .parent()
        .expect("src-tauri should have a repository parent")
        .to_path_buf()
}

fn read_repo(relative: &str) -> String {
    fs::read_to_string(repo_root().join(relative))
        .unwrap_or_else(|error| panic!("{relative} should be readable: {error}"))
        .replace("\r\n", "\n")
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn semantic_sha(value: Value) -> String {
    let workflow = WorkflowDocument::parse(value).expect("sanitized API graph should parse");
    semantic_workflow_sha256(&workflow)
}

fn semantic_package(workflow_json: &str) -> WorkflowPackageFiles {
    WorkflowPackageFiles {
        package_name: "dev082-sanitized-existing".to_owned(),
        package_source_path: None,
        manifest_yaml: "schema_version: 1\nid: wfl_dev082_existing\nname: DEV-082 Existing\nworkflow_version: 1.0.0\nrecipe_version: 1.0.0\ncategory: video\nmode: text_to_video\n".to_owned(),
        recipe_yaml: "schema_version: 1\nid: rcp_dev082_existing\nname: DEV-082 Existing\nworkflow:\n  file: workflow_api.json\ninputs: {}\nbindings: []\noutputs: []\n".to_owned(),
        workflow_json: workflow_json.to_owned(),
    }
}

fn assert_contains_all(source: &str, needles: &[&str]) {
    for needle in needles {
        assert!(
            source.contains(needle),
            "missing DEV-082 contract fragment: {needle}"
        );
    }
}

fn assert_contains_none(source: &str, needles: &[&str]) {
    for needle in needles {
        assert!(
            !source.contains(needle),
            "forbidden DEV-082 contract fragment remains: {needle}"
        );
    }
}

#[test]
fn dev082_sanitized_importability_and_identity_matrix_is_complete() {
    let api = serde_json::from_str::<Value>(DEV081_GRAPH).expect("sanitized API fixture parses");
    assert_eq!(
        detect_comfy_workflow_format(DEV081_GRAPH.as_bytes()),
        ComfyWorkflowInputFormat::Api
    );
    WorkflowDocument::parse(api.clone()).expect("sanitized API fixture graph parses");

    assert_eq!(
        detect_comfy_workflow_format(UI_WORKFLOW.as_bytes()),
        ComfyWorkflowInputFormat::Ui
    );
    assert_eq!(
        detect_comfy_workflow_format(br#"{"#),
        ComfyWorkflowInputFormat::InvalidJson
    );
    assert_eq!(
        detect_comfy_workflow_format(br#"{"name":"unknown"}"#),
        ComfyWorkflowInputFormat::Unknown
    );

    let report = recognize_workflow(DEV081_GRAPH.as_bytes(), &[]);
    assert_eq!(report.format, WorkflowRecognitionFormat::Api);
    assert!(report.recognized);
    assert!(report.importable);
    assert!(!report.executable);
    assert_eq!(
        report.runtime_capability,
        RuntimeCapabilityState::NotChecked
    );
    assert!(
        report
            .clone()
            .with_runtime_capability(RuntimeCapabilitySummary {
                state: RuntimeCapabilityState::Ready,
                issues: Vec::new(),
            })
            .executable
    );
    for state in [
        RuntimeCapabilityState::Offline,
        RuntimeCapabilityState::MissingNodes,
    ] {
        let capability = report
            .clone()
            .with_runtime_capability(RuntimeCapabilitySummary {
                state,
                issues: vec![format!("{state:?}")],
            });
        assert!(capability.importable);
        assert!(!capability.executable);
    }
    let ui_report = recognize_workflow(UI_WORKFLOW.as_bytes(), &[]);
    assert_eq!(ui_report.format, WorkflowRecognitionFormat::Ui);
    assert!(ui_report.recognized);
    assert!(!ui_report.importable);
    assert!(!recognize_workflow(br#"{"#, &[]).recognized);
    assert!(!recognize_workflow(br#"{"name":"unknown"}"#, &[]).recognized);

    let semantic_a = serde_json::from_str::<Value>(SEMANTIC_A).unwrap();
    let semantic_b = serde_json::from_str::<Value>(SEMANTIC_B).unwrap();
    assert_eq!(sha256(SEMANTIC_A.as_bytes()), sha256(SEMANTIC_A.as_bytes()));
    assert_ne!(
        sha256(SEMANTIC_A.as_bytes()),
        sha256(SEMANTIC_B.as_bytes()),
        "D2 must use different raw bytes"
    );
    assert_eq!(semantic_sha(semantic_a.clone()), semantic_sha(semantic_b));

    let exact_raw = recognize_workflow(SEMANTIC_A.as_bytes(), &[semantic_package(SEMANTIC_A)]);
    assert_eq!(exact_raw.identity, WorkflowIdentity::ExactRaw);
    let exact_semantic = recognize_workflow(SEMANTIC_B.as_bytes(), &[semantic_package(SEMANTIC_A)]);
    assert_eq!(exact_semantic.identity, WorkflowIdentity::ExactSemantic);

    let mut prompt_variant = semantic_a.clone();
    prompt_variant["1"]["inputs"]["prompt"] = json!("changed");
    let mut steps_variant = semantic_a.clone();
    steps_variant["1"]["inputs"]["steps"] = json!(20);
    let mut megapixels_variant = semantic_a.clone();
    megapixels_variant["1"]["inputs"]["megapixels"] = json!(0.4);
    let mut node_added = semantic_a;
    node_added["3"] = json!({"class_type": "Preview", "inputs": {}});

    for (label, variant) in [
        ("prompt", prompt_variant.clone()),
        ("steps", steps_variant.clone()),
        ("megapixels", megapixels_variant.clone()),
        ("node-added", node_added.clone()),
    ] {
        assert_ne!(
            semantic_sha(serde_json::from_str(SEMANTIC_A).unwrap()),
            semantic_sha(variant),
            "D3-D6 {label} variants must not be exact semantic duplicates"
        );
    }

    let package = semantic_package(SEMANTIC_A);
    let prompt_report = recognize_workflow(
        &serde_json::to_vec(&prompt_variant).unwrap(),
        std::slice::from_ref(&package),
    );
    assert_eq!(prompt_report.identity, WorkflowIdentity::StructuralVariant);
    assert!(prompt_report.importable);
    assert_eq!(prompt_report.suggested_action, "CHOOSE_VARIANT_ACTION");
    assert!(prompt_report
        .issues
        .iter()
        .any(|issue| issue.code == "STRUCTURAL_VARIANT"));
    assert_eq!(
        structural_workflow_sha256(
            &WorkflowDocument::parse(serde_json::from_str(SEMANTIC_A).unwrap()).unwrap()
        ),
        structural_workflow_sha256(
            &WorkflowDocument::parse(serde_json::from_value(prompt_variant).unwrap()).unwrap()
        )
    );
    let steps_report = recognize_workflow(
        &serde_json::to_vec(&steps_variant).unwrap(),
        std::slice::from_ref(&package),
    );
    assert_eq!(steps_report.identity, WorkflowIdentity::StructuralVariant);
    let megapixels_report = recognize_workflow(
        &serde_json::to_vec(&megapixels_variant).unwrap(),
        std::slice::from_ref(&package),
    );
    assert_eq!(
        megapixels_report.identity,
        WorkflowIdentity::StructuralVariant
    );
    let new_node_report = recognize_workflow(
        &serde_json::to_vec(&node_added).unwrap(),
        std::slice::from_ref(&package),
    );
    assert_eq!(new_node_report.identity, WorkflowIdentity::New);
}

#[test]
fn dev082_aitudou_and_three_item_package_fixtures_are_product_safe() {
    let manifest = read_repo(
        "src-tauri/runtime_packages/aitudou_minimax_h3_lightx2v_8step_fast_1_0_0/manifest.yaml",
    );
    assert_eq!(manifest, AITUDOU_MANIFEST);
    assert!(manifest.contains("id: wfl_aitudou_minimax_h3_lightx2v_8step_fast"));
    assert!(manifest.contains("recipe_version: 1.0.0"));

    let recipe = RecipeParser::parse(AITUDOU_RECIPE).expect("AITUDOU Recipe parses");
    assert_eq!(
        recipe.inputs.len(),
        2,
        "the old product Recipe is the fixture"
    );
    assert!(recipe.inputs.contains_key("prompt"));
    assert!(recipe.inputs.contains_key("seed"));
    for input in [
        "duration_seconds",
        "width",
        "height",
        "steps",
        "denoise",
        "fps",
    ] {
        assert!(
            !recipe.inputs.contains_key(input),
            "old AITUDOU Recipe must not pre-seed {input}"
        );
    }

    let workflow =
        serde_json::from_str::<Value>(AITUDOU_WORKFLOW).expect("AITUDOU product workflow parses");
    let recognition = recognize_workflow(AITUDOU_WORKFLOW.as_bytes(), &[]);
    assert_eq!(recognition.identity, WorkflowIdentity::New);
    assert_eq!(recognition.category, "video");
    assert_eq!(recognition.mode, "text_to_video");
    for key in [
        "prompt",
        "duration_seconds",
        "width",
        "height",
        "seed",
        "steps",
        "denoise",
        "fps",
    ] {
        assert!(
            recognition
                .inputs
                .iter()
                .any(|input| input.semantic_key == key),
            "AITUDOU recognition should infer {key} from the graph"
        );
    }
    assert!(recognition
        .outputs
        .iter()
        .any(|output| output.output_type == "video"));
    let product_report = recognize_workflow(
        AITUDOU_WORKFLOW.as_bytes(),
        &[WorkflowPackageFiles {
            package_name: "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0".to_owned(),
            package_source_path: None,
            manifest_yaml: AITUDOU_MANIFEST.to_owned(),
            recipe_yaml: AITUDOU_RECIPE.to_owned(),
            workflow_json: AITUDOU_WORKFLOW.to_owned(),
        }],
    );
    assert_eq!(product_report.identity, WorkflowIdentity::ExactRaw);
    assert_eq!(product_report.recipe_status, RecipeFreshness::Outdated);
    assert_eq!(
        product_report.existing_workflow_id.as_deref(),
        Some("wfl_aitudou_minimax_h3_lightx2v_8step_fast")
    );
    assert_eq!(
        product_report.existing_workflow_version.as_deref(),
        Some("1.0.0")
    );
    assert_eq!(workflow["50"]["inputs"]["steps"], 8);
    assert_eq!(workflow["50"]["inputs"]["denoise"], 1);
    assert_eq!(workflow["62"]["inputs"]["frame_rate"], 24);
    assert_eq!(workflow["62"]["class_type"], "VHS_VideoCombine");
    assert!(workflow["59"]["inputs"]["text"].is_string());

    let package = serde_json::from_str::<Value>(DEV081_PACKAGE)
        .expect("sanitized three-item Production Package parses");
    assert_eq!(package["items"].as_array().map(Vec::len), Some(3));
    assert_eq!(package["defaults"]["durationSeconds"], 5);
    assert_eq!(package["defaults"]["width"], 960);
    assert_eq!(package["defaults"]["height"], 544);
    assert!(package["items"].as_array().unwrap().iter().all(|item| {
        item["mode"] == "T2V"
            && item["durationSeconds"].is_null()
            && item["width"].is_null()
            && item["height"].is_null()
    }));
}

#[test]
fn dev082_existing_production_regression_seams_are_reused() {
    let dev079 = read_repo("src-tauri/tests/dev079_workflow_add.rs");
    assert_contains_all(
        &dev079,
        &[
            "detects_real_api_ui_unknown_array_and_invalid_inputs_before_onboarding",
            "dev082_importability_is_independent_from_comfy_capability",
            "duplicate_and_archived_imports_reuse_existing_identity_without_new_versions",
        ],
    );

    let dev059 = read_repo("src-tauri/tests/dev059_production_package.rs");
    assert_contains_all(
        &dev059,
        &[
            "dev081_real_uat_regenerates_recipe_then_creates_three_items_and_reaches_fake_comfy",
            "DEV078_EXACT_ADMISSION=PASS",
            "AUTO_START_ON_CREATE=NO",
        ],
    );

    let dev081 = read_repo("src-tauri/tests/dev081_complex_workflow_onboarding.rs");
    assert_contains_all(
        &dev081,
        &[
            "dev081_semantic_reimport_reuses_builtin_identity",
            "dev081_builtin_old_recipe_is_reported_outdated",
            "dev081_regenerate_builtin_recipe_keeps_workflow",
        ],
    );
}

#[test]
fn dev082_recognition_engine_contract_is_ready_for_integration() {
    let recognition = read_repo("src-tauri/src/application/workflow_recognition_service.rs");
    assert_contains_all(
        &recognition,
        &[
            "WorkflowRecognitionReport",
            "pub importable: bool",
            "pub executable: bool",
            "WorkflowIdentity::StructuralVariant",
            "runtime_capability",
            "RuntimeCapabilityState::Offline",
            "RuntimeCapabilityState::MissingNodes",
            "RecognitionConfidence::High",
            "RecognitionConfidence::Medium",
            "RecognitionConfidence::Low",
        ],
    );
    assert_contains_none(
        &recognition,
        &["if workflow_id ==", "node49", "node59", "node63"],
    );
}

#[test]
fn dev082_deletion_restore_binding_and_legacy_contract_is_ready_for_integration() {
    let lifecycle = read_repo("src-tauri/src/application/workflow_lifecycle_service.rs");
    assert_contains_all(
        &lifecycle,
        &[
            "WorkflowDeletionInspection",
            "active_task_count",
            "active_queue_item_count",
            "can_hard_delete",
            "restore_version",
            "delete_workflow",
        ],
    );
    assert_contains_none(
        &lifecycle,
        &[
            "WORKFLOW_BUILTIN_DELETE_BLOCKED",
            "built-in Runtime Packages cannot be permanently deleted",
        ],
    );
    assert_contains_all(&lifecycle, &["if inspection.delete_action == \"REMOVE\""]);

    let binding_port =
        read_repo("src-tauri/src/application/ports/project_workflow_binding_repository.rs");
    let binding_repo =
        read_repo("src-tauri/src/infrastructure/database/repositories/project_workflow_binding.rs");
    assert_contains_all(&binding_port, &["clear_by_workflow_version"]);
    assert_contains_all(&binding_repo, &["clear_by_workflow_version"]);

    let binding_service =
        read_repo("src-tauri/src/application/project_workflow_binding_service.rs");
    assert_contains_all(
        &binding_service,
        &["is_workflow_available_for_recipe", "archived", "enabled"],
    );

    let package_service = read_repo("src-tauri/src/application/production_package_service.rs");
    assert_contains_all(
        &package_service,
        &[
            "LEGACY_FALLBACK",
            "is_workflow_available_for_recipe",
            "legacy H3 workflow configuration points to an unavailable workflow",
        ],
    );
}

#[cfg(test)]
mod lifecycle_e2e {
    use ai_studio_lib::application::{
        builtin_runtime_packages,
        ports::{
            Clock, ComfyAdapter, ComfyAdapterError, ComfyEventSubscription, ComfyHealth,
            ComfyHistory, ComfyOutputData, ComfyOutputFile, GenerationDefinitionRepository,
            ProjectRepository, ProjectWorkflowBindingRecord, ProjectWorkflowBindingRepository,
            PromptSubmission, RepositoryError, WorkflowLibrarySource, WorkflowPackageStore,
            WorkflowRunRepository, WorkflowRuntimeRepository, WorkflowRuntimeStateRepository,
        },
        project_workflow_binding_service::{
            ProjectWorkflowBindingInput, ProjectWorkflowBindingService,
            ProjectWorkflowConfigUpdateRequest,
        },
        workflow_library_service::WorkflowLibraryService,
        workflow_lifecycle_service::WorkflowLifecycleService,
        workflow_onboarding_service::{CapabilityState, WorkflowOnboardingService},
    };
    use ai_studio_lib::infrastructure::{
        database::{
            initialize, SqliteGenerationDefinitionRepository, SqliteProjectRepository,
            SqliteProjectWorkflowBindingRepository, SqliteWorkflowLibraryRepository,
            SqliteWorkflowRuntimeRepository, SqliteWorkflowRuntimeStateRepository,
        },
        filesystem::{FileSystemWorkflowLibrarySource, FileSystemWorkflowPackageStore},
    };
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use serde_json::{json, Map, Value};
    use sqlx::SqlitePool;
    use std::{
        fs,
        path::PathBuf,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
    };
    use tempfile::{tempdir, TempDir};

    const PRODUCT_PACKAGE_NAME: &str = "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0";
    const USER_RACE_PACKAGE_NAME: &str = "dev082_user_race";
    const USER_RACE_WORKFLOW_ID: &str = "wfl_dev082_user_race";
    const USER_RACE_RECIPE_ID: &str = "rcp_dev082_user_race";

    #[derive(Clone, Copy)]
    enum CapabilityFixture {
        Ready,
        MissingNodes,
        Offline,
    }

    #[derive(Clone, Copy)]
    struct FixedClock;

    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            "2026-01-01T00:00:00Z"
                .parse()
                .expect("fixed clock timestamp")
        }
    }

    struct FixtureRunRepository;

    #[async_trait]
    impl WorkflowRunRepository for FixtureRunRepository {
        async fn has_successful_run(
            &self,
            _workflow_id: &str,
            _workflow_version: &str,
        ) -> Result<bool, RepositoryError> {
            Ok(false)
        }
    }

    #[derive(Clone)]
    struct FixtureComfyAdapter {
        object_info: Value,
        offline: bool,
    }

    #[async_trait]
    impl ComfyAdapter for FixtureComfyAdapter {
        async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
            Err(ComfyAdapterError::Offline(
                "DEV-082 lifecycle fixture does not execute".to_owned(),
            ))
        }

        async fn get_system_stats(
            &self,
        ) -> Result<ai_studio_lib::application::ports::SystemStats, ComfyAdapterError> {
            Err(ComfyAdapterError::Offline(
                "DEV-082 lifecycle fixture does not execute".to_owned(),
            ))
        }

        async fn get_object_info(&self) -> Result<Value, ComfyAdapterError> {
            if self.offline {
                return Err(ComfyAdapterError::Offline(
                    "DEV-082 lifecycle fixture ComfyUI is offline".to_owned(),
                ));
            }
            Ok(self.object_info.clone())
        }

        async fn get_history(&self, _prompt_id: &str) -> Result<ComfyHistory, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible(
                "history is not used by DEV-082 lifecycle tests".to_owned(),
            ))
        }

        async fn download_output(
            &self,
            _file: &ComfyOutputFile,
        ) -> Result<ComfyOutputData, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible(
                "output is not used by DEV-082 lifecycle tests".to_owned(),
            ))
        }

        async fn submit_workflow(
            &self,
            _client_id: &str,
            _prompt_id: &str,
            _workflow: Value,
        ) -> Result<PromptSubmission, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible(
                "generation is not used by DEV-082 lifecycle tests".to_owned(),
            ))
        }

        async fn subscribe_events(
            &self,
            _client_id: &str,
        ) -> Result<Box<dyn ComfyEventSubscription>, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible(
                "events are not used by DEV-082 lifecycle tests".to_owned(),
            ))
        }
    }

    #[derive(Clone)]
    struct FakeBindingRepository {
        records: Arc<Mutex<Vec<ProjectWorkflowBindingRecord>>>,
        clear_should_fail: bool,
        list_calls: Arc<AtomicUsize>,
        late_binding_after_two_lists: bool,
    }

    impl FakeBindingRepository {
        fn with_records(
            records: Vec<ProjectWorkflowBindingRecord>,
            clear_should_fail: bool,
            late_binding_after_two_lists: bool,
        ) -> Self {
            Self {
                records: Arc::new(Mutex::new(records)),
                clear_should_fail,
                list_calls: Arc::new(AtomicUsize::new(0)),
                late_binding_after_two_lists,
            }
        }

        fn snapshot(&self) -> Vec<ProjectWorkflowBindingRecord> {
            self.records.lock().expect("fake bindings lock").clone()
        }
    }

    #[async_trait]
    impl ProjectWorkflowBindingRepository for FakeBindingRepository {
        async fn list_for_project(
            &self,
            project_id: &str,
        ) -> Result<Vec<ProjectWorkflowBindingRecord>, RepositoryError> {
            Ok(self
                .snapshot()
                .into_iter()
                .filter(|binding| binding.project_id == project_id)
                .collect())
        }

        async fn replace_for_project(
            &self,
            project_id: &str,
            bindings: &[ProjectWorkflowBindingRecord],
        ) -> Result<(), RepositoryError> {
            let mut records = self.records.lock().expect("fake bindings lock");
            records.retain(|binding| binding.project_id != project_id);
            records.extend(bindings.iter().cloned());
            Ok(())
        }

        async fn list_for_workflow_version(
            &self,
            workflow_version_id: &str,
        ) -> Result<Vec<ProjectWorkflowBindingRecord>, RepositoryError> {
            let call = self.list_calls.fetch_add(1, Ordering::SeqCst);
            if self.late_binding_after_two_lists && call < 2 {
                return Ok(Vec::new());
            }
            Ok(self
                .snapshot()
                .into_iter()
                .filter(|binding| binding.workflow_version_id == workflow_version_id)
                .collect())
        }

        async fn clear_by_workflow_version(
            &self,
            workflow_version_id: &str,
        ) -> Result<u64, RepositoryError> {
            if self.clear_should_fail {
                return Err(RepositoryError::database(
                    "DEV-082 forced project binding cleanup failure",
                ));
            }
            let mut records = self.records.lock().expect("fake bindings lock");
            let before = records.len();
            records.retain(|binding| binding.workflow_version_id != workflow_version_id);
            Ok((before - records.len()) as u64)
        }
    }

    struct TestEnvironment {
        _directory: TempDir,
        db_path: PathBuf,
        library_root: PathBuf,
        staging_root: PathBuf,
        pool: SqlitePool,
    }

    struct Services {
        library_service: Arc<WorkflowLibraryService>,
        runtime_repository: Arc<SqliteWorkflowRuntimeRepository>,
        state_repository: Arc<SqliteWorkflowRuntimeStateRepository>,
        package_store: Arc<FileSystemWorkflowPackageStore>,
        project_repository: Arc<SqliteProjectRepository>,
        binding_repository: Arc<dyn ProjectWorkflowBindingRepository>,
        binding_service: ProjectWorkflowBindingService,
        generation_repository: Arc<SqliteGenerationDefinitionRepository>,
        lifecycle: Arc<WorkflowLifecycleService>,
    }

    #[derive(Clone)]
    struct WorkflowIdentity {
        workflow_version_id: String,
        recipe_id: String,
        package_name: String,
    }

    impl TestEnvironment {
        async fn new() -> Self {
            let directory = tempdir().expect("DEV-082 lifecycle tempdir should exist");
            let data_root = directory.path().join("data");
            let library_root = data_root.join("workflow_library");
            let staging_root = data_root.join("workflow_staging");
            fs::create_dir_all(&library_root).expect("workflow library root should exist");
            fs::create_dir_all(&staging_root).expect("workflow staging root should exist");
            builtin_runtime_packages::ensure_installed(&library_root)
                .expect("builtin fixture packages should install");
            let db_path = data_root.join("app.db");
            let pool = initialize(&db_path)
                .await
                .expect("DEV-082 lifecycle database should initialize");
            Self {
                _directory: directory,
                db_path,
                library_root,
                staging_root,
                pool,
            }
        }

        async fn reopen_database(&mut self) {
            self.pool.close().await;
            self.pool = initialize(&self.db_path)
                .await
                .expect("DEV-082 lifecycle database should reopen");
        }

        fn add_user_race_package(&self) {
            let package_root = self.library_root.join(USER_RACE_PACKAGE_NAME);
            fs::create_dir_all(&package_root).expect("user race package root should exist");
            fs::write(
                package_root.join("manifest.yaml"),
                format!(
                    "schema_version: 1\nid: {USER_RACE_WORKFLOW_ID}\nname: DEV-082 race fixture\nworkflow_version: 1.0.0\nrecipe_version: 1.0.0\ncategory: image\nmode: text_to_image\n"
                ),
            )
            .expect("user race manifest should be written");
            fs::write(
                package_root.join("recipe.yaml"),
                format!(
                    "schema_version: 1\nid: {USER_RACE_RECIPE_ID}\nname: DEV-082 race fixture\nworkflow:\n  file: workflow_api.json\ninputs: {{}}\nbindings: []\noutputs: []\n"
                ),
            )
            .expect("user race recipe should be written");
            fs::write(
                package_root.join("workflow_api.json"),
                r#"{"3":{"inputs":{},"class_type":"KSampler"}}"#,
            )
            .expect("user race workflow should be written");
        }

        fn services(
            &self,
            capability: CapabilityFixture,
            binding_repository: Option<Arc<dyn ProjectWorkflowBindingRepository>>,
        ) -> Services {
            let clock: Arc<dyn Clock> = Arc::new(FixedClock);
            let source: Arc<dyn WorkflowLibrarySource> = Arc::new(
                FileSystemWorkflowLibrarySource::new(self.library_root.clone()),
            );
            let library_service = Arc::new(WorkflowLibraryService::new(
                source.clone(),
                Arc::new(SqliteWorkflowLibraryRepository::new(self.pool.clone())),
                clock.clone(),
            ));
            let runtime_repository =
                Arc::new(SqliteWorkflowRuntimeRepository::new(self.pool.clone()));
            let state_repository =
                Arc::new(SqliteWorkflowRuntimeStateRepository::new(self.pool.clone()));
            let package_store = Arc::new(FileSystemWorkflowPackageStore::new(
                self.library_root.clone(),
                self.staging_root.clone(),
            ));
            let product_workflow: Value = serde_json::from_str(
                &fs::read_to_string(
                    self.library_root
                        .join(PRODUCT_PACKAGE_NAME)
                        .join("workflow_api.json"),
                )
                .expect("product workflow should be readable"),
            )
            .expect("product workflow should be valid JSON");
            let (object_info, offline) = match capability {
                CapabilityFixture::Ready => (object_info_for(&product_workflow, false), false),
                CapabilityFixture::MissingNodes => {
                    (object_info_for(&product_workflow, true), false)
                }
                CapabilityFixture::Offline => (Value::Null, true),
            };
            let comfy: Arc<dyn ComfyAdapter> = Arc::new(FixtureComfyAdapter {
                object_info,
                offline,
            });
            let onboarding_service = Arc::new(
                WorkflowOnboardingService::new(
                    source.clone(),
                    comfy,
                    library_service.clone(),
                    Arc::new(FixtureRunRepository),
                    package_store.clone(),
                    clock.clone(),
                )
                .with_runtime_state(runtime_repository.clone(), state_repository.clone()),
            );
            let binding_repository: Arc<dyn ProjectWorkflowBindingRepository> = binding_repository
                .unwrap_or_else(|| {
                    Arc::new(SqliteProjectWorkflowBindingRepository::new(
                        self.pool.clone(),
                    ))
                });
            let lifecycle = Arc::new(
                WorkflowLifecycleService::new(
                    source,
                    library_service.clone(),
                    onboarding_service,
                    runtime_repository.clone(),
                    state_repository.clone(),
                    package_store.clone(),
                    clock.clone(),
                )
                .with_project_workflow_binding_repository(binding_repository.clone()),
            );
            let project_repository = Arc::new(SqliteProjectRepository::new(self.pool.clone()));
            let binding_service = ProjectWorkflowBindingService::new(
                binding_repository.clone(),
                project_repository.clone(),
                runtime_repository.clone(),
                state_repository.clone(),
                clock,
            );
            Services {
                library_service,
                runtime_repository,
                state_repository,
                package_store,
                project_repository,
                binding_repository,
                binding_service,
                generation_repository: Arc::new(SqliteGenerationDefinitionRepository::new(
                    self.pool.clone(),
                )),
                lifecycle,
            }
        }
    }

    fn object_info_for(workflow: &Value, omit_one_node: bool) -> Value {
        let mut object_info = Map::new();
        let mut omitted = false;
        for node in workflow
            .as_object()
            .expect("API workflow should have an object root")
            .values()
        {
            let class_type = node["class_type"]
                .as_str()
                .expect("API fixture node should have class_type");
            if omit_one_node && !omitted && class_type != "VHS_VideoCombine" {
                omitted = true;
                continue;
            }
            let mut capability = json!({"input": {"required": {}}});
            if class_type == "VHS_VideoCombine" {
                capability["output"] = json!(["VIDEO"]);
            }
            object_info.insert(class_type.to_owned(), capability);
        }
        Value::Object(object_info)
    }

    async fn sync(services: &Services) {
        let report = services
            .library_service
            .sync()
            .await
            .expect("workflow library sync should complete");
        assert_eq!(
            report.invalid, 0,
            "fixture package sync errors: {:?}",
            report.errors
        );
        assert!(report.valid > 0, "at least one product fixture should sync");
    }

    async fn product(services: &Services) -> WorkflowIdentity {
        let version = services
            .runtime_repository
            .list_versions()
            .await
            .expect("runtime versions should be readable")
            .into_iter()
            .find(|version| version.package_name.as_deref() == Some(PRODUCT_PACKAGE_NAME))
            .expect("product runtime version should be registered");
        let recipe = version
            .recipes
            .first()
            .expect("product runtime recipe should be registered");
        WorkflowIdentity {
            workflow_version_id: version.workflow_version_id,
            recipe_id: recipe.recipe_id.clone(),
            package_name: PRODUCT_PACKAGE_NAME.to_owned(),
        }
    }

    async fn user_race_workflow(services: &Services) -> WorkflowIdentity {
        let version = services
            .runtime_repository
            .list_versions()
            .await
            .expect("runtime versions should be readable")
            .into_iter()
            .find(|version| version.workflow_id == USER_RACE_WORKFLOW_ID)
            .expect("user race runtime version should be registered");
        WorkflowIdentity {
            workflow_version_id: version.workflow_version_id,
            recipe_id: USER_RACE_RECIPE_ID.to_owned(),
            package_name: USER_RACE_PACKAGE_NAME.to_owned(),
        }
    }

    async fn ensure_project(services: &Services, project_id: &str) {
        services
            .project_repository
            .ensure_default_project(
                project_id,
                "DEV-082 lifecycle project",
                &PathBuf::from(format!("{project_id}-root")),
                FixedClock.now(),
            )
            .await
            .expect("project fixture should be created");
    }

    async fn bind(services: &Services, project_id: &str, workflow: &WorkflowIdentity, mode: &str) {
        services
            .binding_service
            .replace(
                project_id,
                ProjectWorkflowConfigUpdateRequest {
                    bindings: vec![ProjectWorkflowBindingInput {
                        stage: "VIDEO".to_owned(),
                        mode: mode.to_owned(),
                        workflow_version_id: workflow.workflow_version_id.clone(),
                        recipe_id: workflow.recipe_id.clone(),
                    }],
                },
            )
            .await
            .expect("project workflow binding should be saved");
    }

    async fn available(services: &Services, workflow: &WorkflowIdentity) -> bool {
        services
            .binding_service
            .is_workflow_available_for_recipe(&workflow.workflow_version_id, &workflow.recipe_id)
            .await
            .expect("workflow availability should be readable")
    }

    async fn catalog_contains(services: &Services, workflow: &WorkflowIdentity) -> bool {
        services
            .generation_repository
            .list_available()
            .await
            .expect("generation catalog should be readable")
            .into_iter()
            .any(|definition| {
                definition.workflow_version_id == workflow.workflow_version_id
                    && definition.recipe_id == workflow.recipe_id
            })
    }

    async fn state(services: &Services, workflow_version_id: &str) -> (bool, bool) {
        let state = services
            .state_repository
            .find_state(workflow_version_id)
            .await
            .expect("runtime state should be readable")
            .expect("runtime state should exist after lifecycle action");
        (state.archived, state.enabled)
    }

    fn assert_restore_payload(
        payload: Value,
        workflow_version_id: &str,
        enabled: bool,
        capability: &str,
    ) {
        let object = payload
            .as_object()
            .expect("restore result should be a JSON object");
        assert_eq!(
            object.get("workflowVersionId").and_then(Value::as_str),
            Some(workflow_version_id)
        );
        assert_eq!(object.get("archived").and_then(Value::as_bool), Some(false));
        assert_eq!(
            object.get("enabled").and_then(Value::as_bool),
            Some(enabled)
        );
        assert_eq!(
            object.get("capability").and_then(Value::as_str),
            Some(capability)
        );
        assert_eq!(
            object.get("readiness").and_then(Value::as_str),
            Some(if enabled {
                "ACTIVE"
            } else {
                "RESTORED_NEEDS_ATTENTION"
            })
        );
    }

    async fn delete_product(services: &Services, workflow: &WorkflowIdentity) {
        let inspection = services
            .lifecycle
            .inspect_deletion(&workflow.workflow_version_id)
            .await
            .expect("product deletion inspection should succeed");
        assert!(inspection.builtin);
        assert_eq!(inspection.delete_action, "REMOVE");
        services
            .lifecycle
            .delete_version(&workflow.workflow_version_id)
            .await
            .expect("product deletion should succeed");
    }

    async fn restore_product(
        services: &Services,
        workflow: &WorkflowIdentity,
        expected_enabled: bool,
        expected_capability: CapabilityState,
    ) {
        let payload = services
            .lifecycle
            .restore_version(&workflow.workflow_version_id)
            .await
            .expect("restore should succeed");
        assert_restore_payload(
            serde_json::to_value(payload).expect("restore result should serialize"),
            &workflow.workflow_version_id,
            expected_enabled,
            match expected_capability {
                CapabilityState::Ready => "READY",
                CapabilityState::MissingNodes => "MISSING_NODES",
                CapabilityState::ComfyOffline => "COMFY_OFFLINE",
                CapabilityState::IncompatibleInputValues => "INCOMPATIBLE_INPUT_VALUES",
                CapabilityState::NotChecked => "NOT_CHECKED",
            },
        );
        let capability = services
            .lifecycle
            .recheck_capability(&workflow.workflow_version_id)
            .await
            .expect("restore capability should be readable");
        assert_eq!(capability.state, expected_capability);
        assert_eq!(
            state(services, &workflow.workflow_version_id).await,
            (false, expected_enabled)
        );
    }

    fn binding_record(
        project_id: &str,
        workflow: &WorkflowIdentity,
    ) -> ProjectWorkflowBindingRecord {
        ProjectWorkflowBindingRecord {
            project_id: project_id.to_owned(),
            stage: "VIDEO".to_owned(),
            mode: "DEFAULT".to_owned(),
            workflow_version_id: workflow.workflow_version_id.clone(),
            recipe_id: workflow.recipe_id.clone(),
            created_at: FixedClock.now(),
            updated_at: FixedClock.now(),
        }
    }

    #[tokio::test]
    async fn dev082_product_delete_restart_restore_full_lifecycle_e2e() {
        let mut environment = TestEnvironment::new().await;
        let first = environment.services(CapabilityFixture::Ready, None);
        sync(&first).await;
        let workflow = product(&first).await;
        ensure_project(&first, "dev082-product-project").await;
        bind(&first, "dev082-product-project", &workflow, "DEFAULT").await;
        let package_before = first
            .package_store
            .read_runtime(&workflow.package_name)
            .await
            .expect("product package should be readable before deletion");
        let version_before = first
            .runtime_repository
            .find_version(&workflow.workflow_version_id)
            .await
            .expect("product version should be readable before deletion")
            .expect("product version should exist before deletion");

        let inspection = first
            .lifecycle
            .inspect_deletion(&workflow.workflow_version_id)
            .await
            .expect("product deletion inspection should succeed");
        assert!(inspection.builtin);
        assert_eq!(inspection.project_binding_count, 1);
        assert_eq!(inspection.delete_action, "REMOVE");
        assert!(available(&first, &workflow).await);
        assert!(catalog_contains(&first, &workflow).await);

        let deleted = first
            .lifecycle
            .delete_version(&workflow.workflow_version_id)
            .await
            .expect("product deletion should succeed");
        assert_eq!(deleted.delete_action, "REMOVE");
        assert_eq!(deleted.project_binding_count, 1);
        assert_eq!(
            state(&first, &workflow.workflow_version_id).await,
            (true, false)
        );
        assert!(first
            .binding_repository
            .list_for_workflow_version(&workflow.workflow_version_id)
            .await
            .expect("deleted product bindings should be readable")
            .is_empty());
        assert!(!available(&first, &workflow).await);
        assert!(!catalog_contains(&first, &workflow).await);
        assert_eq!(
            first
                .package_store
                .read_runtime(&workflow.package_name)
                .await
                .unwrap(),
            package_before
        );
        assert_eq!(
            first
                .runtime_repository
                .find_version(&workflow.workflow_version_id)
                .await
                .unwrap()
                .unwrap(),
            version_before
        );

        drop(first);
        environment.reopen_database().await;
        builtin_runtime_packages::ensure_installed(&environment.library_root)
            .expect("restart ensure_installed should preserve product package");
        let restarted = environment.services(CapabilityFixture::Ready, None);
        sync(&restarted).await;
        assert_eq!(
            state(&restarted, &workflow.workflow_version_id).await,
            (true, false)
        );
        assert!(!available(&restarted, &workflow).await);
        assert!(!catalog_contains(&restarted, &workflow).await);
        assert!(restarted
            .binding_repository
            .list_for_workflow_version(&workflow.workflow_version_id)
            .await
            .unwrap()
            .is_empty());

        restore_product(&restarted, &workflow, true, CapabilityState::Ready).await;
        assert!(available(&restarted, &workflow).await);
        assert!(catalog_contains(&restarted, &workflow).await);
        assert!(restarted
            .binding_repository
            .list_for_workflow_version(&workflow.workflow_version_id)
            .await
            .unwrap()
            .is_empty());
        bind(&restarted, "dev082-product-project", &workflow, "DEFAULT").await;
        let bindings = restarted
            .binding_repository
            .list_for_workflow_version(&workflow.workflow_version_id)
            .await
            .unwrap();
        assert_eq!(bindings.len(), 1);
    }

    #[tokio::test]
    async fn dev082_restore_ready_reenables_workflow() {
        let environment = TestEnvironment::new().await;
        let services = environment.services(CapabilityFixture::Ready, None);
        sync(&services).await;
        let workflow = product(&services).await;
        delete_product(&services, &workflow).await;
        restore_product(&services, &workflow, true, CapabilityState::Ready).await;
        assert!(available(&services, &workflow).await);
        assert!(catalog_contains(&services, &workflow).await);
    }

    #[tokio::test]
    async fn dev082_restore_missing_nodes_keeps_workflow_disabled() {
        let environment = TestEnvironment::new().await;
        let services = environment.services(CapabilityFixture::MissingNodes, None);
        sync(&services).await;
        let workflow = product(&services).await;
        delete_product(&services, &workflow).await;
        restore_product(&services, &workflow, false, CapabilityState::MissingNodes).await;
        assert!(!available(&services, &workflow).await);
        assert!(!catalog_contains(&services, &workflow).await);
    }

    #[tokio::test]
    async fn dev082_restore_offline_succeeds_but_stays_disabled() {
        let environment = TestEnvironment::new().await;
        let services = environment.services(CapabilityFixture::Offline, None);
        sync(&services).await;
        let workflow = product(&services).await;
        delete_product(&services, &workflow).await;
        restore_product(&services, &workflow, false, CapabilityState::ComfyOffline).await;
        assert!(!available(&services, &workflow).await);
        assert!(!catalog_contains(&services, &workflow).await);
    }

    #[tokio::test]
    async fn dev082_remove_clears_project_bindings_inside_lifecycle_service() {
        let environment = TestEnvironment::new().await;
        let services = environment.services(CapabilityFixture::Ready, None);
        sync(&services).await;
        let workflow = product(&services).await;
        ensure_project(&services, "dev082-binding-project-a").await;
        ensure_project(&services, "dev082-binding-project-b").await;
        bind(&services, "dev082-binding-project-a", &workflow, "DEFAULT").await;
        bind(
            &services,
            "dev082-binding-project-b",
            &workflow,
            "FL2VA_TEXT_TO_VIDEO",
        )
        .await;
        let inspection = services
            .lifecycle
            .inspect_deletion(&workflow.workflow_version_id)
            .await
            .unwrap();
        assert_eq!(inspection.project_binding_count, 2);
        let result = services
            .lifecycle
            .delete_version(&workflow.workflow_version_id)
            .await
            .expect("lifecycle should own binding cleanup");
        assert_eq!(result.project_binding_count, 2);
        assert!(services
            .binding_repository
            .list_for_workflow_version(&workflow.workflow_version_id)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn dev082_remove_binding_failure_rolls_back_archive_state() {
        let environment = TestEnvironment::new().await;
        let bootstrap = environment.services(CapabilityFixture::Ready, None);
        sync(&bootstrap).await;
        let workflow = product(&bootstrap).await;
        drop(bootstrap);
        let fake = FakeBindingRepository::with_records(
            vec![binding_record("dev082-compensation-project", &workflow)],
            true,
            false,
        );
        let fake_handle = fake.clone();
        let services = environment.services(
            CapabilityFixture::Ready,
            Some(Arc::new(fake) as Arc<dyn ProjectWorkflowBindingRepository>),
        );
        let error = services
            .lifecycle
            .delete_version(&workflow.workflow_version_id)
            .await
            .expect_err("binding cleanup failure should fail closed");
        assert!(error.to_string().contains("binding"));
        assert_eq!(
            state(&services, &workflow.workflow_version_id).await,
            (false, true)
        );
        assert_eq!(fake_handle.snapshot().len(), 1);
        assert!(services
            .package_store
            .read_runtime(&workflow.package_name)
            .await
            .is_ok());
        assert!(services
            .runtime_repository
            .find_version(&workflow.workflow_version_id)
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn dev082_hard_delete_late_binding_downgrades_to_remove() {
        let environment = TestEnvironment::new().await;
        environment.add_user_race_package();
        let bootstrap = environment.services(CapabilityFixture::Ready, None);
        sync(&bootstrap).await;
        let workflow = user_race_workflow(&bootstrap).await;
        drop(bootstrap);
        let fake = FakeBindingRepository::with_records(
            vec![binding_record("dev082-late-binding-project", &workflow)],
            false,
            true,
        );
        let services = environment.services(
            CapabilityFixture::Ready,
            Some(Arc::new(fake) as Arc<dyn ProjectWorkflowBindingRepository>),
        );
        let initial = services
            .lifecycle
            .inspect_deletion(&workflow.workflow_version_id)
            .await
            .unwrap();
        assert_eq!(initial.delete_action, "HARD_DELETE");
        let result = services
            .lifecycle
            .delete_version(&workflow.workflow_version_id)
            .await;
        match result {
            Ok(result) => assert_eq!(result.delete_action, "REMOVE"),
            Err(error) => assert!(
                error.code() == "WORKFLOW_DELETE_REINSPECT_REQUIRED"
                    || error.to_string().contains("REINSPECT")
            ),
        }
        assert!(services
            .runtime_repository
            .find_version(&workflow.workflow_version_id)
            .await
            .unwrap()
            .is_some());
        assert!(services
            .package_store
            .read_runtime(&workflow.package_name)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn dev082_deleted_product_is_unavailable_and_restored_product_can_be_explicitly_rebound()
    {
        let environment = TestEnvironment::new().await;
        let services = environment.services(CapabilityFixture::Ready, None);
        sync(&services).await;
        let workflow = product(&services).await;
        ensure_project(&services, "dev082-explicit-rebind-project").await;
        bind(
            &services,
            "dev082-explicit-rebind-project",
            &workflow,
            "DEFAULT",
        )
        .await;
        delete_product(&services, &workflow).await;
        assert!(!available(&services, &workflow).await);
        assert!(!catalog_contains(&services, &workflow).await);
        assert!(services
            .binding_repository
            .list_for_workflow_version(&workflow.workflow_version_id)
            .await
            .unwrap()
            .is_empty());

        restore_product(&services, &workflow, true, CapabilityState::Ready).await;
        assert!(available(&services, &workflow).await);
        assert!(catalog_contains(&services, &workflow).await);
        bind(
            &services,
            "dev082-explicit-rebind-project",
            &workflow,
            "DEFAULT",
        )
        .await;
        let config = services
            .binding_service
            .get("dev082-explicit-rebind-project")
            .await
            .expect("restored product should be readable in project settings");
        assert_eq!(
            config
                .video_default
                .as_ref()
                .map(|binding| binding.workflow_version_id.as_str()),
            Some(workflow.workflow_version_id.as_str())
        );
    }
}
