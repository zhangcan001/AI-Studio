//! DEV-081 regression coverage for graph-aware onboarding.
//!
//! The graph fixture is intentionally sanitized.  The real desktop workflow is
//! only used to derive topology locally and is never read by this test.
//!
//! Compile dependencies are the existing public onboarding, compiler, domain,
//! and Production Package APIs.  The graph-aware assertions are test-first:
//! they are expected to turn green when the DEV-081 analyzer is merged.

use ai_studio_lib::application::{
    ports::{
        Clock, ComfyAdapter, ComfyAdapterError, ComfyEventSubscription, ComfyHealth, ComfyHistory,
        ComfyOutputData, ComfyOutputFile, PromptSubmission, RepositoryError, SystemStats,
        WorkflowRunRepository,
    },
    production_package_inspector::{
        ProductionPackageInspector, ProductionPackageItemStatus as InspectorItemStatus,
    },
    workflow_library_service::WorkflowLibraryService,
    workflow_onboarding_service::{
        detect_comfy_workflow_format, CapabilityState, WorkflowAutoOnboardingPlanView,
        WorkflowAutoOnboardingState, WorkflowOnboardingService,
    },
};
use ai_studio_lib::compiler::{RecipeParser, RecipeValidator, WorkflowCompiler, WorkflowValidator};
use ai_studio_lib::domain::{
    CompileRequest, InputValue, ProductionPackage, SeedValue, WorkflowDocument,
};
use ai_studio_lib::infrastructure::{
    database::{initialize, SqliteWorkflowLibraryRepository},
    filesystem::{FileSystemWorkflowLibrarySource, FileSystemWorkflowPackageStore},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::{json, Map, Value};
use std::{collections::BTreeMap, fs, path::PathBuf, sync::Arc};
use tempfile::{tempdir, TempDir};

const WORKFLOW_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/workflows/minimax_h3_8step_graph_api.json"
));

const RECIPE_YAML: &str = r#"
schema_version: 1
id: dev081_minimax_h3_8step
name: DEV-081 MiniMax H3 8-step
workflow:
  file: workflow_api.json
inputs:
  prompt:
    type: textarea
    label: Prompt
    required: true
    default: A cinematic test prompt
  width:
    type: integer
    label: Width
    required: true
    default: 768
    min: 64
    max: 2048
    step: 8
  height:
    type: integer
    label: Height
    required: true
    default: 432
    min: 64
    max: 2048
    step: 8
  duration_seconds:
    type: integer
    label: Duration Seconds
    required: true
    default: 5
    min: 1
    max: 30
    step: 1
  seed:
    type: seed
    label: Seed
    default: random
  steps:
    type: integer
    label: Steps
    required: false
    default: 8
    min: 1
    max: 100
    step: 1
  denoise:
    type: integer
    label: Denoise
    required: false
    default: 1
    min: 0
    max: 1
    step: 1
  fps:
    type: integer
    label: FPS
    required: false
    default: 24
    min: 1
    max: 240
    step: 1
bindings:
  - source: prompt
    target:
      node: "59"
      input: text
  - source: width
    target:
      node: "63"
      input: width
  - source: height
    target:
      node: "63"
      input: height
  - source: duration_seconds
    target:
      node: "49"
      input: value
  - source: seed
    target:
      node: "2"
      input: noise_seed
  - source: steps
    target:
      node: "50"
      input: steps
  - source: denoise
    target:
      node: "50"
      input: denoise
  - source: fps
    target:
      node: "62"
      input: frame_rate
outputs:
  - id: video
    type: video
    node: "62"
    required: true
"#;

fn fixture_value() -> Value {
    serde_json::from_str(WORKFLOW_JSON).expect("sanitized workflow fixture should be valid JSON")
}

fn object_info_for(workflow: &Value) -> Value {
    let mut object_info = Map::new();
    for node in workflow
        .as_object()
        .expect("API workflow root should be an object")
        .values()
    {
        let class_type = node["class_type"]
            .as_str()
            .expect("fixture node should have a class_type");
        let output = if matches!(
            class_type,
            "VHS_VideoCombine" | "SaveVideo" | "PreviewVideo"
        ) {
            json!(["VIDEO"])
        } else {
            Value::Null
        };
        let mut capability = json!({"input": {"required": {}}});
        if !output.is_null() {
            capability["output"] = output;
        }
        object_info.insert(class_type.to_owned(), capability);
    }
    Value::Object(object_info)
}

#[derive(Clone)]
struct FixtureComfyAdapter {
    object_info: Value,
}

#[async_trait]
impl ComfyAdapter for FixtureComfyAdapter {
    async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
        Err(ComfyAdapterError::Offline(
            "DEV-081 fixture does not execute".to_owned(),
        ))
    }

    async fn get_system_stats(&self) -> Result<SystemStats, ComfyAdapterError> {
        Err(ComfyAdapterError::Offline(
            "DEV-081 fixture does not execute".to_owned(),
        ))
    }

    async fn get_object_info(&self) -> Result<Value, ComfyAdapterError> {
        Ok(self.object_info.clone())
    }

    async fn get_history(&self, _prompt_id: &str) -> Result<ComfyHistory, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible(
            "history is not used by onboarding".to_owned(),
        ))
    }

    async fn download_output(
        &self,
        _file: &ComfyOutputFile,
    ) -> Result<ComfyOutputData, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible(
            "output is not used by onboarding".to_owned(),
        ))
    }

    async fn submit_workflow(
        &self,
        _client_id: &str,
        _prompt_id: &str,
        _workflow: Value,
    ) -> Result<PromptSubmission, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible(
            "generation is not used by onboarding".to_owned(),
        ))
    }

    async fn subscribe_events(
        &self,
        _client_id: &str,
    ) -> Result<Box<dyn ComfyEventSubscription>, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible(
            "events are not used by onboarding".to_owned(),
        ))
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
struct FixtureClock;

impl Clock for FixtureClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

struct Harness {
    _directory: TempDir,
    service: WorkflowOnboardingService,
}

async fn harness(workflow: &Value) -> Harness {
    let directory = tempdir().expect("fixture directory should exist");
    let data_root = directory.path().join("AIStudioData");
    let library_root = data_root.join("workflow_library");
    let staging_root = data_root.join("workflow_staging");
    fs::create_dir_all(&library_root).expect("library root should exist");
    fs::create_dir_all(&staging_root).expect("staging root should exist");

    let pool = initialize(&data_root.join("app.db"))
        .await
        .expect("database should initialize");
    let source = Arc::new(FileSystemWorkflowLibrarySource::new(library_root.clone()));
    let clock = Arc::new(FixtureClock);
    let library_service = Arc::new(WorkflowLibraryService::new(
        source.clone(),
        Arc::new(SqliteWorkflowLibraryRepository::new(pool)),
        clock.clone(),
    ));
    let service = WorkflowOnboardingService::new(
        source,
        Arc::new(FixtureComfyAdapter {
            object_info: object_info_for(workflow),
        }),
        library_service,
        Arc::new(FixtureRunRepository),
        Arc::new(FileSystemWorkflowPackageStore::new(
            library_root,
            staging_root,
        )),
        clock,
    );

    Harness {
        _directory: directory,
        service,
    }
}

async fn auto_plan(workflow: Value, filename: &str) -> WorkflowAutoOnboardingPlanView {
    let harness = harness(&workflow).await;
    harness
        .service
        .auto_onboard_bytes(
            serde_json::to_vec(&workflow).expect("workflow should serialize"),
            filename.to_owned(),
            None,
        )
        .await
        .expect("fixture workflow should produce an onboarding plan")
}

#[tokio::test]
#[ignore]
async fn dev081_real_workflow_graph_inference() {
    let path = PathBuf::from(
        std::env::var("AI_STUDIO_REAL_WORKFLOW_FIXTURE")
            .expect("set AI_STUDIO_REAL_WORKFLOW_FIXTURE to the local real JSON"),
    );
    let bytes = fs::read(&path).expect("real workflow fixture should be readable");
    let value: Value = serde_json::from_slice(&bytes).expect("real workflow should be JSON");
    let harness = harness(&value).await;
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("real-workflow.json")
        .to_owned();
    let plan = harness
        .service
        .auto_onboard_bytes(bytes.clone(), filename, None)
        .await
        .expect("real workflow should produce an onboarding plan");
    let nodes = value
        .as_object()
        .expect("real workflow root should be an object");
    let binding_is = |key: &str, node: &str, input: &str| {
        plan.input_mappings.iter().any(|mapping| {
            mapping.semantic_key == key
                && mapping.target_node == node
                && mapping.target_input == input
        })
    };
    let output = plan.output_mappings.first();

    println!("FORMAT={}", detect_comfy_workflow_format(&bytes).as_str());
    println!("NODE_COUNT={}", nodes.len());
    println!(
        "OUTPUT_NODE={}",
        output
            .map(|mapping| mapping.node_id.as_str())
            .unwrap_or("NONE")
    );
    println!(
        "OUTPUT_TYPE={}",
        output
            .map(|mapping| mapping.output_type.as_str())
            .unwrap_or("NONE")
    );
    println!(
        "PROMPT_BINDING={}",
        if binding_is("prompt", "59", "text") {
            "PASS"
        } else {
            "FAIL"
        }
    );
    println!(
        "WIDTH_BINDING={}",
        if binding_is("width", "63", "width") {
            "PASS"
        } else {
            "FAIL"
        }
    );
    println!(
        "HEIGHT_BINDING={}",
        if binding_is("height", "63", "height") {
            "PASS"
        } else {
            "FAIL"
        }
    );
    println!(
        "DURATION_BINDING={}",
        if binding_is("duration_seconds", "49", "value") {
            "PASS"
        } else {
            "FAIL"
        }
    );
    println!(
        "SEED_BINDING={}",
        if binding_is("seed", "2", "noise_seed") {
            "PASS"
        } else {
            "FAIL"
        }
    );
    println!(
        "OPTIONAL_PARAMETERS={}",
        ["steps", "denoise", "fps"]
            .iter()
            .filter_map(|key| {
                plan.input_mappings
                    .iter()
                    .find(|mapping| mapping.semantic_key == *key)
                    .map(|mapping| {
                        format!("{}={}", key, mapping.default_value.as_deref().unwrap_or(""))
                    })
            })
            .collect::<Vec<_>>()
            .join(",")
    );
    println!("MODE={}", plan.metadata.mode);
    println!(
        "AUTO_PUBLISHABLE={}",
        if plan.auto_publishable { "YES" } else { "NO" }
    );
    println!("CAPABILITY={:?}", plan.capability.state);

    assert_eq!(
        detect_comfy_workflow_format(&bytes),
        ai_studio_lib::application::workflow_onboarding_service::ComfyWorkflowInputFormat::Api
    );
    assert_eq!(
        output.map(|mapping| mapping.output_type.as_str()),
        Some("video")
    );
    assert!(binding_is("prompt", "59", "text"));
    assert!(binding_is("width", "63", "width"));
    assert!(binding_is("height", "63", "height"));
    assert!(binding_is("duration_seconds", "49", "value"));
    assert!(binding_is("seed", "2", "noise_seed"));
    assert_eq!(plan.metadata.mode, "text_to_video");
    assert!(plan.auto_publishable);
}

fn mapping<'a>(
    plan: &'a WorkflowAutoOnboardingPlanView,
    semantic_key: &str,
) -> &'a ai_studio_lib::application::workflow_onboarding_service::WorkflowInputMappingView {
    plan.input_mappings
        .iter()
        .find(|candidate| candidate.semantic_key == semantic_key)
        .unwrap_or_else(|| panic!("missing inferred mapping {semantic_key}"))
}

#[test]
fn dev081_fixture_is_api_sanitized_and_topology_complete() {
    let workflow = fixture_value();
    assert_eq!(
        detect_comfy_workflow_format(WORKFLOW_JSON.as_bytes()).as_str(),
        "API"
    );
    let document = WorkflowDocument::parse(workflow.clone()).expect("fixture should parse");
    WorkflowValidator::validate(&document).expect("fixture should pass workflow validation");

    let nodes = workflow
        .as_object()
        .expect("fixture root should be an object");
    assert_eq!(nodes.len(), 21);
    for (node_id, class_type) in [
        ("2", "RandomNoise"),
        ("35", "ComfyMathExpression"),
        ("49", "FloatConstant"),
        ("50", "BasicScheduler"),
        ("59", "Text Multiline"),
        ("61", "ResolutionSelector"),
        ("62", "VHS_VideoCombine"),
        ("63", "MiniMaxH3ReferenceToVideo"),
    ] {
        assert_eq!(nodes[node_id]["class_type"], class_type);
    }

    assert_eq!(nodes["59"]["inputs"]["text"], "A cinematic test prompt");
    assert_eq!(nodes["49"]["inputs"]["value"], 5);
    assert_eq!(nodes["50"]["inputs"]["steps"], 8);
    assert_eq!(nodes["50"]["inputs"]["denoise"], 1);
    assert_eq!(nodes["62"]["inputs"]["frame_rate"], 24);
    assert_eq!(nodes["35"]["inputs"]["values.a"], json!(["49", 0]));
    assert_eq!(nodes["63"]["inputs"]["prompt"], json!(["59", 0]));
    assert_eq!(nodes["63"]["inputs"]["width"], json!(["61", 0]));
    assert_eq!(nodes["63"]["inputs"]["height"], json!(["61", 1]));
    assert_eq!(nodes["63"]["inputs"]["length"], json!(["35", 1]));
    assert_eq!(nodes["62"]["inputs"]["images"], json!(["39", 0]));
    assert_eq!(nodes["62"]["inputs"]["audio"], json!(["18", 0]));

    for forbidden in ["C:\\", "C:/", "Users\\", "Users/", "\\\\"] {
        assert!(
            !WORKFLOW_JSON.contains(forbidden),
            "sanitized fixture leaked a local path marker: {forbidden}"
        );
    }
    for node_id in ["3", "6", "56", "57", "60"] {
        assert!(
            nodes[node_id]["inputs"]
                .as_object()
                .expect("inputs should be an object")
                .values()
                .filter_map(Value::as_str)
                .all(|value| value.contains("PLACEHOLDER") || value == "default"),
            "node {node_id} should contain only placeholder model values"
        );
    }
}

#[tokio::test]
async fn dev081_fixture_auto_onboarding_infers_production_bindings() {
    let plan = auto_plan(fixture_value(), "dev081_minimax_h3_8step.json").await;

    assert_eq!(plan.state, WorkflowAutoOnboardingState::AutoPublished);
    assert_eq!(plan.capability.state, CapabilityState::Ready);
    assert_eq!(plan.workflow_kind, "VIDEO");
    assert_eq!(plan.metadata.category, "video");
    assert_eq!(plan.metadata.mode, "text_to_video");
    assert!(plan.auto_publishable);
    assert!(plan.published.is_some());
    assert!(plan.validation.dry_run);

    assert_eq!(mapping(&plan, "prompt").target_node, "59");
    assert_eq!(mapping(&plan, "prompt").target_input, "text");
    assert!(mapping(&plan, "prompt").required);
    assert_eq!(mapping(&plan, "width").target_node, "63");
    assert_eq!(mapping(&plan, "width").target_input, "width");
    assert!(mapping(&plan, "width").required);
    assert_eq!(mapping(&plan, "height").target_node, "63");
    assert_eq!(mapping(&plan, "height").target_input, "height");
    assert!(mapping(&plan, "height").required);
    assert_eq!(mapping(&plan, "duration_seconds").target_node, "49");
    assert_eq!(mapping(&plan, "duration_seconds").target_input, "value");
    assert!(mapping(&plan, "duration_seconds").required);
    assert_eq!(mapping(&plan, "seed").target_node, "2");
    assert_eq!(mapping(&plan, "seed").target_input, "noise_seed");
    assert_eq!(
        mapping(&plan, "seed").default_value.as_deref(),
        Some("random")
    );

    for (key, default) in [("steps", "8"), ("denoise", "1"), ("fps", "24")] {
        let candidate = mapping(&plan, key);
        assert!(!candidate.required, "{key} should be optional");
        assert_eq!(candidate.default_value.as_deref(), Some(default));
    }

    for internal in [
        "aspect_ratio",
        "megapixels",
        "multiple",
        "clip_name",
        "vae_name",
        "unet_name",
        "lora_name",
        "sampler_name",
        "scheduler",
        "attention",
        "filename_prefix",
    ] {
        assert!(
            plan.input_mappings
                .iter()
                .all(|candidate| candidate.target_input != internal),
            "internal input {internal} must not become a production input"
        );
    }
    assert_eq!(plan.output_mappings.len(), 1);
    assert_eq!(plan.output_mappings[0].node_id, "62");
    assert_eq!(plan.output_mappings[0].output_type, "video");
}

#[tokio::test]
async fn dev081_output_scoring_prefers_media_and_reports_ties() {
    let fixture_plan = auto_plan(fixture_value(), "dev081_output_o1.json").await;
    assert_eq!(fixture_plan.output_mappings.len(), 1);
    assert_eq!(fixture_plan.output_mappings[0].node_id, "62");
    assert_eq!(fixture_plan.output_mappings[0].output_type, "video");

    let save_and_preview = json!({
        "1": {"inputs": {}, "class_type": "SaveVideo"},
        "2": {"inputs": {"video": ["1", 0]}, "class_type": "PreviewVideo"}
    });
    let saved = auto_plan(save_and_preview, "dev081_output_o2.json").await;
    assert_eq!(saved.state, WorkflowAutoOnboardingState::AutoPublished);
    assert_eq!(saved.output_mappings[0].node_id, "1");
    assert_eq!(saved.output_mappings[0].output_type, "video");

    let tied = json!({
        "1": {"inputs": {}, "class_type": "SaveVideo"},
        "2": {"inputs": {}, "class_type": "SaveVideo"}
    });
    let ambiguous = auto_plan(tied, "dev081_output_o3.json").await;
    assert_eq!(ambiguous.state, WorkflowAutoOnboardingState::NeedsReview);
    assert!(ambiguous.published.is_none());
    assert!(ambiguous.issues.iter().any(|issue| {
        issue.code == "AMBIGUOUS_OUTPUT"
            && issue
                .candidates
                .iter()
                .map(|candidate| candidate.node_id.as_deref())
                .any(|node_id| node_id == Some("1"))
            && issue
                .candidates
                .iter()
                .map(|candidate| candidate.node_id.as_deref())
                .any(|node_id| node_id == Some("2"))
    }));

    let utility_only = json!({
        "40": {"inputs": {}, "class_type": "easy clearCacheAll"}
    });
    let unknown = auto_plan(utility_only, "dev081_output_o4.json").await;
    assert_eq!(unknown.state, WorkflowAutoOnboardingState::NeedsReview);
    assert!(unknown.published.is_none());
    assert!(unknown
        .issues
        .iter()
        .any(|issue| issue.code == "UNKNOWN_OUTPUT"));
}

#[test]
fn dev081_recipe_dry_run_overrides_runtime_values_without_mutating_source_graph() {
    let recipe = RecipeParser::parse(RECIPE_YAML).expect("DEV-081 recipe should parse");
    RecipeValidator::validate(&recipe).expect("DEV-081 recipe should validate");
    let workflow = WorkflowDocument::parse(fixture_value()).expect("fixture should parse");
    let original = workflow.value().clone();

    let mut values = BTreeMap::new();
    values.insert(
        "prompt".to_owned(),
        InputValue::String("DEV081 test".to_owned()),
    );
    values.insert("width".to_owned(), InputValue::Integer(768));
    values.insert("height".to_owned(), InputValue::Integer(432));
    values.insert("duration_seconds".to_owned(), InputValue::Integer(5));
    values.insert("seed".to_owned(), InputValue::Seed(SeedValue::Fixed(123)));

    let compiled = WorkflowCompiler
        .compile(&workflow, &recipe, &CompileRequest::new(values))
        .expect("DEV-081 production recipe should dry-run compile");
    assert_eq!(compiled.workflow["59"]["inputs"]["text"], "DEV081 test");
    assert_eq!(compiled.workflow["63"]["inputs"]["width"], 768);
    assert_eq!(compiled.workflow["63"]["inputs"]["height"], 432);
    assert_eq!(compiled.workflow["49"]["inputs"]["value"], 5);
    assert_eq!(compiled.workflow["2"]["inputs"]["noise_seed"], 123);
    assert_eq!(compiled.workflow["50"]["inputs"]["steps"], 8);
    assert_eq!(compiled.workflow["50"]["inputs"]["denoise"], 1);
    assert_eq!(compiled.workflow["62"]["inputs"]["frame_rate"], 24);
    assert_eq!(
        compiled.workflow["63"]["inputs"]["prompt"],
        json!(["59", 0])
    );
    assert_eq!(
        compiled.workflow["63"]["inputs"]["length"],
        json!(["35", 1])
    );
    assert_eq!(
        compiled.workflow["35"]["inputs"]["values.a"],
        json!(["49", 0])
    );
    assert_eq!(workflow.value(), &original);
}

#[tokio::test]
async fn dev081_text_to_video_production_package_contract_remains_compatible() {
    let package_value = json!({
        "schemaVersion": 1,
        "packageType": "AI_STUDIO_VIDEO_PRODUCTION",
        "packageId": "dev081-package",
        "name": "DEV-081 text to video package",
        "defaults": {
            "durationSeconds": 5,
            "width": 768,
            "height": 432,
            "mode": "FL2VA_TEXT_TO_VIDEO"
        },
        "items": [{
            "id": "DEV081-SH001",
            "name": "Test shot",
            "videoPrompt": "A cinematic test prompt",
            "durationSeconds": 5,
            "width": 768,
            "height": 432,
            "mode": "FL2VA_TEXT_TO_VIDEO"
        }]
    });
    let package: ProductionPackage = serde_json::from_value(package_value.clone())
        .expect("Production Package V1 should accept the inferred text-to-video shape");
    assert_eq!(package.schema_version, 1);
    assert_eq!(package.package_type, "AI_STUDIO_VIDEO_PRODUCTION");
    assert_eq!(package.defaults.duration_seconds, Some(5));
    assert_eq!(package.defaults.width, Some(768));
    assert_eq!(package.defaults.height, Some(432));
    assert_eq!(
        package.defaults.mode.as_deref(),
        Some("FL2VA_TEXT_TO_VIDEO")
    );
    assert!(package.items[0].reference_images.is_empty());
    assert!(package.items[0].reference_audios.is_empty());
    assert!(package.items[0].reference_videos.is_empty());

    let directory = tempdir().expect("package root should exist");
    let inspectable_package = json!({
        "schemaVersion": 1,
        "packageType": "AI_STUDIO_VIDEO_PRODUCTION",
        "name": "DEV-081 inspectable text package",
        "defaults": {
            "durationSeconds": 5,
            "width": 960,
            "height": 544,
            "mode": "FL2VA_TEXT_TO_VIDEO"
        },
        "items": [{
            "id": "DEV081-SH001",
            "name": "Test shot",
            "videoPrompt": "A cinematic test prompt",
            "mode": "FL2VA_TEXT_TO_VIDEO"
        }]
    });
    fs::write(
        directory.path().join("production-package.json"),
        serde_json::to_vec(&inspectable_package).expect("package should serialize"),
    )
    .expect("package manifest should write");
    let inspection = ProductionPackageInspector::new()
        .inspect(directory.path())
        .await
        .expect("text-to-video package should pass package inspection");
    assert_eq!(inspection.status, InspectorItemStatus::Ready);
    assert_eq!(inspection.item_count, 1);
    assert_eq!(inspection.ready_count, 1);
    assert_eq!(inspection.items[0].mode, "FL2VA_TEXT_TO_VIDEO");
}
