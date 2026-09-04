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
