//! DEV-079 integration coverage for universal workflow add.
//!
//! These tests exercise the real onboarding service, SQLite repositories,
//! filesystem package store, and generation catalog.  ComfyUI is replaced by
//! a deterministic in-process adapter; no GPU work or external process is
//! started.

use ai_studio_lib::application::{
    generation_catalog_service::GenerationCatalogService,
    ports::{
        Clock, ComfyAdapter, ComfyAdapterError, ComfyEventSubscription, ComfyHealth, ComfyHistory,
        ComfyOutputData, ComfyOutputFile, RepositoryError, SystemStats, WorkflowRunRepository,
        WorkflowRuntimeRepository, WorkflowRuntimeStateRepository,
    },
    workflow_library_service::WorkflowLibraryService,
    workflow_onboarding_service::{
        detect_comfy_workflow_format, CapabilityState, ComfyWorkflowInputFormat,
        WorkflowAutoOnboardingState, WorkflowOnboardingService,
    },
};
use ai_studio_lib::infrastructure::{
    database::{
        initialize, SqliteGenerationDefinitionRepository, SqliteWorkflowLibraryRepository,
        SqliteWorkflowRuntimeRepository, SqliteWorkflowRuntimeStateRepository,
    },
    filesystem::{FileSystemWorkflowLibrarySource, FileSystemWorkflowPackageStore},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tempfile::{tempdir, TempDir};

const API_IMAGE_WORKFLOW: &str = r#"{
  "1": {
    "class_type": "Sampler",
    "inputs": {
      "prompt": "positive prompt",
      "negative_prompt": "negative prompt",
      "seed": 7,
      "steps": 20,
      "cfg": 7.5,
      "width": 512,
      "height": 512
    }
  },
  "2": {
    "class_type": "SaveImage",
    "inputs": {"images": ["1", 0]}
  }
}"#;

const API_VIDEO_WORKFLOW: &str = r#"{
  "1": {
    "class_type": "VideoSampler",
    "inputs": {
      "prompt": "motion prompt",
      "seed": 11,
      "video": "reference.mp4",
      "audio": "reference.wav"
    }
  },
  "2": {
    "class_type": "SaveVideo",
    "inputs": {"video": ["1", 0]}
  }
}"#;

const UI_WORKFLOW: &str = r#"{
  "last_node_id": 2,
  "last_link_id": 1,
  "nodes": [
    {
      "id": 1,
      "type": "KSampler",
      "pos": [0, 0],
      "size": [315, 278],
      "flags": {},
      "order": 0,
      "mode": 0,
      "inputs": [],
      "outputs": [{"name": "IMAGE", "type": "IMAGE", "links": [1]}],
      "properties": {},
      "widgets_values": [7, "euler", "normal", 20, 7.5, "randomize"]
    },
    {
      "id": 2,
      "type": "SaveImage",
      "pos": [400, 0],
      "size": [315, 58],
      "flags": {},
      "order": 1,
      "mode": 0,
      "inputs": [{"name": "images", "type": "IMAGE", "link": 1}],
      "outputs": [],
      "properties": {},
      "widgets_values": ["ComfyUI"]
    }
  ],
  "links": [[1, 1, 0, 2, 0, "IMAGE"]],
  "groups": [],
  "config": {},
  "extra": {},
  "version": 0.4
}"#;

const OBJECT_INFO: &str = r#"{
  "Sampler": {"output": ["IMAGE"], "input": {"required": {
    "prompt": ["STRING", {}],
    "negative_prompt": ["STRING", {}],
    "seed": ["INT", {"min": 0, "max": 999999, "step": 1}],
    "steps": ["INT", {"min": 1, "max": 100, "step": 1}],
    "cfg": ["FLOAT", {"min": 0, "max": 30, "step": 0.5}],
    "width": ["INT", {"min": 64, "max": 2048, "step": 64}],
    "height": ["INT", {"min": 64, "max": 2048, "step": 64}]
  }}},
  "SaveImage": {"output_node": true, "input": {"required": {
    "images": ["IMAGE", {}]
  }}},
  "VideoSampler": {"output": ["VIDEO"], "input": {"required": {
    "prompt": ["STRING", {}],
    "seed": ["INT", {"min": 0, "max": 999999, "step": 1}],
    "video": ["VIDEO", {}],
    "audio": ["AUDIO", {}]
  }}},
  "SaveVideo": {"output_node": true, "input": {"required": {
    "video": ["VIDEO", {}]
  }}},
  "TextNode": {"input": {"required": {
    "prompt_a": ["STRING", {}],
    "prompt_b": ["STRING", {}]
  }}}
}"#;

const AMBIGUOUS_WORKFLOW: &str = r#"{
  "1": {
    "class_type": "TextNode",
    "inputs": {"prompt_a": "one", "prompt_b": "two"}
  },
  "2": {
    "class_type": "SaveImage",
    "inputs": {"images": ["1", 0]}
  }
}"#;

const MISSING_NODE_WORKFLOW: &str = r#"{
  "1": {
    "class_type": "MissingCustomNode",
    "inputs": {"prompt": "kept on disk"}
  },
  "2": {
    "class_type": "SaveImage",
    "inputs": {"images": ["1", 0]}
  }
}"#;

#[derive(Clone)]
struct FixtureComfyAdapter {
    object_info: Value,
    offline: bool,
    submit_calls: Arc<AtomicUsize>,
}

impl FixtureComfyAdapter {
    fn new(object_info: Value) -> Self {
        Self {
            object_info,
            offline: false,
            submit_calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn offline() -> Self {
        Self {
            object_info: Value::Null,
            offline: true,
            submit_calls: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[async_trait]
impl ComfyAdapter for FixtureComfyAdapter {
    async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
        Err(ComfyAdapterError::Offline(
            "DEV-079 fixture does not execute".to_owned(),
        ))
    }

    async fn get_system_stats(&self) -> Result<SystemStats, ComfyAdapterError> {
        Err(ComfyAdapterError::Offline(
            "DEV-079 fixture does not execute".to_owned(),
        ))
    }

    async fn get_object_info(&self) -> Result<Value, ComfyAdapterError> {
        if self.offline {
            return Err(ComfyAdapterError::Offline(
                "DEV-082 fixture ComfyUI is offline".to_owned(),
            ));
        }
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
    ) -> Result<ai_studio_lib::application::ports::PromptSubmission, ComfyAdapterError> {
        self.submit_calls.fetch_add(1, Ordering::SeqCst);
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
    library_root: PathBuf,
    service: WorkflowOnboardingService,
    pool: SqlitePool,
    runtime_repository: Arc<SqliteWorkflowRuntimeRepository>,
    runtime_state_repository: Arc<SqliteWorkflowRuntimeStateRepository>,
    comfy: Arc<FixtureComfyAdapter>,
}

async fn harness(object_info: Value) -> Harness {
    harness_with_comfy(Arc::new(FixtureComfyAdapter::new(object_info))).await
}

async fn offline_harness() -> Harness {
    harness_with_comfy(Arc::new(FixtureComfyAdapter::offline())).await
}

async fn harness_with_comfy(comfy: Arc<FixtureComfyAdapter>) -> Harness {
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
        Arc::new(SqliteWorkflowLibraryRepository::new(pool.clone())),
        clock.clone(),
    ));
    let runtime_repository = Arc::new(SqliteWorkflowRuntimeRepository::new(pool.clone()));
    let runtime_state_repository =
        Arc::new(SqliteWorkflowRuntimeStateRepository::new(pool.clone()));
    let service = WorkflowOnboardingService::new(
        source,
        comfy.clone(),
        library_service,
        Arc::new(FixtureRunRepository),
        Arc::new(FileSystemWorkflowPackageStore::new(
            library_root.clone(),
            staging_root,
        )),
        clock,
    )
    .with_runtime_state(runtime_repository.clone(), runtime_state_repository.clone());

    Harness {
        _directory: directory,
        library_root,
        service,
        pool,
        runtime_repository,
        runtime_state_repository,
        comfy,
    }
}

fn object_info() -> Value {
    serde_json::from_str(OBJECT_INFO).expect("object_info fixture should be valid JSON")
}

fn api_image_bytes() -> Vec<u8> {
    API_IMAGE_WORKFLOW.as_bytes().to_vec()
}

fn api_image_pretty_bytes() -> Vec<u8> {
    let workflow: Value = serde_json::from_str(API_IMAGE_WORKFLOW)
        .expect("DEV-079 image workflow should remain valid JSON");
    serde_json::to_vec_pretty(&workflow).expect("DEV-079 pretty workflow should serialize")
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn assert_no_package_or_staging(harness: &Harness) {
    let package_entries = fs::read_dir(&harness.library_root)
        .expect("library root should be readable")
        .count();
    assert_eq!(
        package_entries, 0,
        "rejected imports must not publish a package"
    );
}

fn assert_format_diagnostic(
    result: Result<
        ai_studio_lib::application::workflow_onboarding_service::WorkflowAutoOnboardingPlanView,
        ai_studio_lib::application::workflow_onboarding_service::WorkflowOnboardingError,
    >,
    marker: &str,
    user_message: &str,
) {
    match result {
        Ok(plan) => {
            assert!(
                plan.published.is_none(),
                "format diagnostics must never publish"
            );
            let diagnostics = plan
                .issues
                .iter()
                .map(|issue| format!("{} {}", issue.code, issue.message))
                .chain(std::iter::once(plan.message))
                .collect::<Vec<_>>()
                .join(" ");
            assert!(
                diagnostics.contains(marker),
                "missing {marker} in {diagnostics}"
            );
            assert!(
                diagnostics.contains(user_message),
                "missing user message in {diagnostics}"
            );
        }
        Err(error) => {
            let diagnostics = format!("{} {error}", error.code());
            assert!(
                diagnostics.contains(marker),
                "missing {marker} in {diagnostics}"
            );
            assert!(
                diagnostics.contains(user_message),
                "missing user message in {diagnostics}"
            );
        }
    }
}

#[test]
fn detects_real_api_ui_unknown_array_and_invalid_inputs_before_onboarding() {
    assert_eq!(
        detect_comfy_workflow_format(API_IMAGE_WORKFLOW.as_bytes()),
        ComfyWorkflowInputFormat::Api
    );
    assert_eq!(
        detect_comfy_workflow_format(UI_WORKFLOW.as_bytes()),
        ComfyWorkflowInputFormat::Ui
    );
    assert_eq!(
        detect_comfy_workflow_format(br#"{}"#),
        ComfyWorkflowInputFormat::Unknown
    );
    assert_eq!(
        detect_comfy_workflow_format(br#"[]"#),
        ComfyWorkflowInputFormat::Unknown
    );
    assert_eq!(
        detect_comfy_workflow_format(br#"{"#),
        ComfyWorkflowInputFormat::InvalidJson
    );
}

#[tokio::test]
async fn dev082_importability_is_independent_from_comfy_capability() {
    let offline = offline_harness().await;
    let offline_draft = offline
        .service
        .import_bytes(
            api_image_bytes(),
            "dev082_offline_api.json".to_owned(),
            None,
        )
        .await
        .expect("valid API JSON remains importable while ComfyUI is offline");
    assert!(offline_draft.validation.api_format);
    assert_eq!(offline_draft.capability.state, CapabilityState::NotChecked);
    let offline_capability = offline
        .service
        .check_capability(&offline_draft.draft_id)
        .await
        .expect("offline capability should be reported, not make import fail");
    assert_eq!(offline_capability.state, CapabilityState::ComfyOffline);
    assert!(offline.service.get(&offline_draft.draft_id).is_ok());

    let missing = harness(object_info()).await;
    let missing_draft = missing
        .service
        .import_bytes(
            MISSING_NODE_WORKFLOW.as_bytes().to_vec(),
            "dev082_missing_node_api.json".to_owned(),
            None,
        )
        .await
        .expect("valid API JSON remains importable with missing custom nodes");
    assert!(missing_draft.validation.api_format);
    let missing_capability = missing
        .service
        .check_capability(&missing_draft.draft_id)
        .await
        .expect("missing node capability should be reported after import");
    assert_eq!(missing_capability.state, CapabilityState::MissingNodes);
    assert!(missing.service.get(&missing_draft.draft_id).is_ok());
}

#[tokio::test]
async fn auto_adds_image_and_refreshes_the_real_generation_catalog() {
    let harness = harness(object_info()).await;
    let plan = harness
        .service
        .auto_onboard_bytes(api_image_bytes(), "dev079_image.json".to_owned(), None)
        .await
        .expect("deterministic API image workflow should be auto-added");

    assert_eq!(plan.state, WorkflowAutoOnboardingState::AutoPublished);
    assert_eq!(plan.metadata.category, "image");
    assert_eq!(plan.metadata.mode, "text_to_image");
    assert_eq!(plan.workflow_kind, "IMAGE");
    assert!(plan.published.is_some());
    assert_eq!(plan.output_mappings.len(), 1);
    assert_eq!(plan.output_mappings[0].output_type, "image");
    for key in [
        "prompt",
        "negative_prompt",
        "seed",
        "steps",
        "cfg",
        "width",
        "height",
    ] {
        assert!(
            plan.input_mappings
                .iter()
                .any(|mapping| mapping.semantic_key == key),
            "expected inferred mapping {key}"
        );
    }
    assert_ne!(
        plan.input_mappings
            .iter()
            .find(|mapping| mapping.semantic_key == "prompt")
            .and_then(|mapping| mapping.default_value.as_deref()),
        plan.input_mappings
            .iter()
            .find(|mapping| mapping.semantic_key == "negative_prompt")
            .and_then(|mapping| mapping.default_value.as_deref())
    );
    assert_eq!(harness.comfy.submit_calls.load(Ordering::SeqCst), 0);

    let published = plan.published.expect("publish result should exist");
    let runtime_version = harness
        .runtime_repository
        .list_versions()
        .await
        .expect("runtime version should be registered")
        .into_iter()
        .find(|version| {
            version.workflow_id == published.workflow_id
                && version.workflow_version == published.workflow_version
        })
        .expect("published runtime version should exist");
    let catalog = GenerationCatalogService::new(Arc::new(
        SqliteGenerationDefinitionRepository::new(harness.pool.clone()),
    ))
    .list()
    .await
    .expect("generation catalog should refresh after publication");
    assert!(catalog.iter().any(|recipe| {
        recipe.workflow_version_id == runtime_version.workflow_version_id
            && recipe.workflow_id == published.workflow_id
            && recipe.recipe_id == published.recipe_id
    }));
}

#[tokio::test]
async fn auto_adds_video_without_claiming_an_exact_h3_mode() {
    let harness = harness(object_info()).await;
    let plan = harness
        .service
        .auto_onboard_bytes(
            API_VIDEO_WORKFLOW.as_bytes().to_vec(),
            "dev079_video.json".to_owned(),
            None,
        )
        .await
        .expect("deterministic generic video workflow should be auto-added");

    assert_eq!(plan.state, WorkflowAutoOnboardingState::AutoPublished);
    assert_eq!(plan.metadata.category, "video");
    assert_eq!(plan.workflow_kind, "VIDEO");
    assert_eq!(plan.output_mappings[0].output_type, "video");
    assert!(
        ["text_to_video", "image_to_video", "reference_to_video"]
            .contains(&plan.metadata.mode.as_str()),
        "unexpected generic video mode: {}",
        plan.metadata.mode
    );
    assert!(![
        "FL2VA_TEXT_TO_VIDEO",
        "FL2VA_IMAGE_TO_VIDEO",
        "FL2VA_FIRST_LAST",
        "REF2VA_IMAGE",
        "REF2VA_AUDIO",
        "REF2VA_IMAGE_AUDIO",
        "REF2VA_VIDEO_IMAGE",
    ]
    .contains(&plan.metadata.mode.as_str()));
    assert!(plan
        .input_mappings
        .iter()
        .any(|mapping| mapping.semantic_key == "reference_video"));
    assert!(plan
        .input_mappings
        .iter()
        .any(|mapping| mapping.semantic_key == "reference_audio"));
}

#[tokio::test]
async fn ambiguous_imports_wait_for_review_and_missing_nodes_publish_disabled() {
    let ambiguous_harness = harness(object_info()).await;
    let ambiguous = ambiguous_harness
        .service
        .auto_onboard_bytes(
            AMBIGUOUS_WORKFLOW.as_bytes().to_vec(),
            "dev079_ambiguous.json".to_owned(),
            None,
        )
        .await
        .expect("ambiguous workflow should remain an explicit plan");
    assert_eq!(ambiguous.state, WorkflowAutoOnboardingState::NeedsReview);
    assert!(ambiguous.published.is_none());
    assert!(ambiguous
        .issues
        .iter()
        .any(|issue| issue.code == "AMBIGUOUS_INPUT"));
    assert!(ambiguous
        .input_mappings
        .iter()
        .all(|mapping| mapping.semantic_key != "prompt"));
    assert_no_package_or_staging(&ambiguous_harness);

    let missing_harness = harness(json!({
        "SaveImage": {"output_node": true, "input": {"required": {}}}
    }))
    .await;
    let missing = missing_harness
        .service
        .auto_onboard_bytes(
            MISSING_NODE_WORKFLOW.as_bytes().to_vec(),
            "dev079_missing.json".to_owned(),
            None,
        )
        .await
        .expect("missing-node workflow must remain available for explicit handling");
    assert_eq!(missing.state, WorkflowAutoOnboardingState::AutoPublished);
    let published = missing
        .published
        .as_ref()
        .expect("importable missing-node workflow should be saved");
    assert_eq!(missing.capability.state, CapabilityState::MissingNodes);
    assert!(missing.recognition.importable);
    assert!(!missing.recognition.executable);
    assert!(missing
        .issues
        .iter()
        .any(|issue| issue.code == "MISSING_NODES"));
    assert!(missing_harness.service.get(&missing.draft_id).is_ok());
    let runtime_version = missing_harness
        .runtime_repository
        .list_versions()
        .await
        .expect("missing-node package should register a runtime version")
        .into_iter()
        .find(|version| {
            version.workflow_id == published.workflow_id
                && version.workflow_version == published.workflow_version
        })
        .expect("missing-node runtime version should exist");
    let state = missing_harness
        .runtime_state_repository
        .find_state(&runtime_version.workflow_version_id)
        .await
        .expect("runtime state should be readable")
        .expect("missing-node package should have explicit runtime state");
    assert!(!state.enabled);
    assert!(!state.archived);
}

#[tokio::test]
async fn duplicate_and_archived_imports_reuse_existing_identity_without_new_versions() {
    let harness = harness(object_info()).await;
    let first = harness
        .service
        .auto_onboard_bytes(api_image_bytes(), "dev079_original.json".to_owned(), None)
        .await
        .expect("first import should publish");
    let published = first
        .published
        .expect("first import should have a publish result");

    let duplicate = harness
        .service
        .auto_onboard_bytes(api_image_bytes(), "renamed_copy.json".to_owned(), None)
        .await
        .expect("duplicate should return a plan");
    assert_eq!(duplicate.state, WorkflowAutoOnboardingState::AlreadyExists);
    assert!(duplicate.published.is_none());
    assert_eq!(
        duplicate.existing_workflow_id.as_deref(),
        Some(published.workflow_id.as_str())
    );
    assert_eq!(
        duplicate.existing_workflow_version.as_deref(),
        Some("1.0.0")
    );

    let version_id = harness
        .runtime_repository
        .list_versions()
        .await
        .expect("runtime version should be registered")
        .into_iter()
        .find(|version| version.workflow_id == published.workflow_id)
        .expect("published runtime version should exist")
        .workflow_version_id;
    harness
        .runtime_state_repository
        .set_archived(&version_id, true, false, Some(Utc::now()), Utc::now())
        .await
        .expect("fixture version should be archivable");

    let archived = harness
        .service
        .auto_onboard_bytes(api_image_bytes(), "renamed_copy.json".to_owned(), None)
        .await
        .expect("archived duplicate should return a plan");
    assert_eq!(
        archived.state,
        WorkflowAutoOnboardingState::AlreadyExistsArchived
    );
    assert!(archived.published.is_none());
    assert_eq!(
        archived.existing_workflow_id.as_deref(),
        Some(published.workflow_id.as_str())
    );
}

#[tokio::test]
async fn existing_sha_outdated_recipe_is_regenerated_without_workflow_version_or_spam() {
    let harness = harness(object_info()).await;
    let original_bytes = api_image_bytes();
    let semantic_reimport_bytes = api_image_pretty_bytes();
    assert_ne!(
        sha256_bytes(&original_bytes),
        sha256_bytes(&semantic_reimport_bytes),
        "the semantic reimport must prove raw SHA is not the identity"
    );
    let first = harness
        .service
        .auto_onboard_bytes(original_bytes, "dev081_regenerate.json".to_owned(), None)
        .await
        .expect("first import should publish");
    let published = first.published.expect("first import should publish");
    let old_recipe = format!(
        "schema_version: 1\nid: {}\nname: Old Recipe\nworkflow:\n  file: workflow_api.json\ninputs:\n  prompt:\n    type: textarea\n    label: Prompt\n    required: true\n    default: old\n  seed:\n    type: seed\n    label: Seed\n    default: random\nbindings:\n  - source: prompt\n    target:\n      node: \"1\"\n      input: prompt\n  - source: seed\n    target:\n      node: \"1\"\n      input: seed\noutputs:\n  - id: output_1\n    type: image\n    node: \"2\"\n    required: true\n",
        published.recipe_id
    );
    fs::write(
        harness
            .library_root
            .join(&published.package_name)
            .join("recipe.yaml"),
        old_recipe,
    )
    .expect("old recipe should be replaced only in the isolated fixture");

    let outdated = harness
        .service
        .auto_onboard_bytes(
            semantic_reimport_bytes.clone(),
            "renamed_again.json".to_owned(),
            None,
        )
        .await
        .expect("semantic-equivalent reimport should return a diagnostic plan");
    assert_eq!(outdated.state, WorkflowAutoOnboardingState::NeedsReview);
    let issue = outdated
        .issues
        .iter()
        .find(|issue| issue.code == "EXISTING_RECIPE_OUTDATED")
        .expect("outdated recipe issue should be exposed");
    assert_eq!(
        outdated.existing_match_type.as_deref(),
        Some("SEMANTIC_SHA")
    );
    assert!(issue.message.contains(&published.recipe_id));
    assert!(issue.message.contains("width"));
    assert_eq!(outdated.existing_recipes.len(), 1);
    assert_eq!(outdated.existing_recipes[0].recipe_id, published.recipe_id);

    let workflow_versions_before =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflow_versions")
            .fetch_one(&harness.pool)
            .await
            .unwrap();
    let recipes_before = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM recipes")
        .fetch_one(&harness.pool)
        .await
        .unwrap();
    let regenerated = harness
        .service
        .regenerate_recipe_draft(
            &published.workflow_id,
            &published.workflow_version,
            Some("1.0.0"),
        )
        .await
        .expect("regeneration should publish the current inference");
    assert_eq!(
        regenerated.state,
        WorkflowAutoOnboardingState::AutoPublished
    );
    let new_publish = regenerated
        .published
        .as_ref()
        .expect("regeneration should publish a new recipe");
    assert_ne!(new_publish.recipe_id, published.recipe_id);
    assert_eq!(new_publish.workflow_id, published.workflow_id);
    assert_eq!(new_publish.workflow_version, published.workflow_version);
    assert_eq!(new_publish.workflow_sha256, published.workflow_sha256);
    assert_eq!(regenerated.metadata.recipe_version, "1.0.1");
    assert!(regenerated
        .input_mappings
        .iter()
        .any(|mapping| mapping.semantic_key == "width"));
    assert!(regenerated
        .input_mappings
        .iter()
        .any(|mapping| mapping.semantic_key == "height"));

    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflow_versions")
            .fetch_one(&harness.pool)
            .await
            .unwrap(),
        workflow_versions_before
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM recipes")
            .fetch_one(&harness.pool)
            .await
            .unwrap(),
        recipes_before + 1
    );
    let old_db_recipe: String = sqlx::query_scalar("SELECT recipe_yaml FROM recipes WHERE id = ?")
        .bind(&published.recipe_id)
        .fetch_one(&harness.pool)
        .await
        .unwrap();
    assert!(old_db_recipe.contains("negative_prompt"));
    assert!(old_db_recipe.contains("width"));

    let current = harness
        .service
        .auto_onboard_bytes(semantic_reimport_bytes, "same_again.json".to_owned(), None)
        .await
        .expect("current regenerated recipe should deduplicate semantically");
    assert_eq!(current.state, WorkflowAutoOnboardingState::AlreadyExists);
    assert!(current
        .issues
        .iter()
        .any(|issue| issue.code == "EXISTING_RECIPE_CURRENT"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM recipes")
            .fetch_one(&harness.pool)
            .await
            .unwrap(),
        recipes_before + 1
    );
}

#[tokio::test]
async fn invalid_unknown_and_ui_inputs_are_explicitly_rejected_without_publishing() {
    let invalid_harness = harness(object_info()).await;
    assert_format_diagnostic(
        invalid_harness
            .service
            .auto_onboard_bytes(b"{\"nodes\":".to_vec(), "invalid.json".to_owned(), None)
            .await,
        "INVALID_JSON",
        "无法读取这个文件，它不是有效的 JSON。",
    );
    assert_no_package_or_staging(&invalid_harness);

    let unknown_harness = harness(object_info()).await;
    assert_format_diagnostic(
        unknown_harness
            .service
            .auto_onboard_bytes(
                br#"{"name":"not a ComfyUI workflow","values":[1,2,3]}"#.to_vec(),
                "unknown.json".to_owned(),
                None,
            )
            .await,
        "UNKNOWN",
        "这个 JSON 不是可识别的 ComfyUI 工作流。",
    );
    assert_no_package_or_staging(&unknown_harness);

    let ui_harness = harness(object_info()).await;
    assert_format_diagnostic(
        ui_harness
            .service
            .auto_onboard_bytes(UI_WORKFLOW.as_bytes().to_vec(), "ui.json".to_owned(), None)
            .await,
        "UNSUPPORTED_UI_FORMAT",
        "请在 ComfyUI 中将该工作流导出为 API Format JSON，然后重新选择该文件。",
    );
    assert_no_package_or_staging(&ui_harness);
}
