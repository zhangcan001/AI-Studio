//! Phase 2B UI-source onboarding lifecycle coverage.

use ai_studio_lib::application::{
    ports::{
        Clock, ComfyAdapter, ComfyAdapterError, ComfyEventSubscription, ComfyHealth, ComfyHistory,
        ComfyOutputData, ComfyOutputFile, PromptSubmission, RepositoryError, SystemStats,
        WorkflowRunRepository,
    },
    workflow_library_service::WorkflowLibraryService,
    workflow_onboarding_service::{
        CapabilityState, WorkflowAutoOnboardingState, WorkflowNormalizationState,
        WorkflowOnboardingService,
    },
};
use ai_studio_lib::infrastructure::database::{initialize, SqliteWorkflowLibraryRepository};
use ai_studio_lib::infrastructure::filesystem::{
    FileSystemWorkflowLibrarySource, FileSystemWorkflowPackageStore,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use tempfile::{tempdir, TempDir};

const KERA2_UI: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/workflow_ui/phase2b/kera2_t2i/ui_workflow.json"
));
const KERA2_OBJECT_INFO: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/workflow_ui/phase2b/kera2_t2i/object_info.json"
));

#[derive(Clone)]
struct CountingComfyAdapter {
    object_info: Arc<Mutex<Option<Value>>>,
    calls: Arc<AtomicUsize>,
}

impl CountingComfyAdapter {
    fn offline() -> Self {
        Self {
            object_info: Arc::new(Mutex::new(None)),
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn set_object_info(&self, object_info: Value) {
        *self.object_info.lock().expect("adapter state lock") = Some(object_info);
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ComfyAdapter for CountingComfyAdapter {
    async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
        Err(ComfyAdapterError::Offline("test adapter".to_owned()))
    }

    async fn get_system_stats(&self) -> Result<SystemStats, ComfyAdapterError> {
        Err(ComfyAdapterError::Offline("test adapter".to_owned()))
    }

    async fn get_object_info(&self) -> Result<Value, ComfyAdapterError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.object_info
            .lock()
            .expect("adapter state lock")
            .clone()
            .ok_or_else(|| ComfyAdapterError::Offline("test adapter".to_owned()))
    }

    async fn get_history(&self, _prompt_id: &str) -> Result<ComfyHistory, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible("not used".to_owned()))
    }

    async fn download_output(
        &self,
        _file: &ComfyOutputFile,
    ) -> Result<ComfyOutputData, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible("not used".to_owned()))
    }

    async fn submit_workflow(
        &self,
        _client_id: &str,
        _prompt_id: &str,
        _workflow: Value,
    ) -> Result<PromptSubmission, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible("not used".to_owned()))
    }

    async fn subscribe_events(
        &self,
        _client_id: &str,
    ) -> Result<Box<dyn ComfyEventSubscription>, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible("not used".to_owned()))
    }
}

struct TestRunRepository;

#[async_trait]
impl WorkflowRunRepository for TestRunRepository {
    async fn has_successful_run(
        &self,
        _workflow_id: &str,
        _workflow_version: &str,
    ) -> Result<bool, RepositoryError> {
        Ok(false)
    }
}

#[derive(Clone)]
struct TestClock;

impl Clock for TestClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

struct Harness {
    _directory: TempDir,
    adapter: CountingComfyAdapter,
    service: WorkflowOnboardingService,
}

async fn harness() -> Harness {
    let directory = tempdir().expect("test directory should exist");
    let root = directory.path().join("AIStudioData");
    let library_root = root.join("workflow_library");
    let staging_root = root.join("workflow_staging");
    tokio::fs::create_dir_all(&library_root)
        .await
        .expect("library root should exist");
    tokio::fs::create_dir_all(&staging_root)
        .await
        .expect("staging root should exist");
    let pool = initialize(&root.join("app.db"))
        .await
        .expect("database should initialize");
    let source = Arc::new(FileSystemWorkflowLibrarySource::new(library_root.clone()));
    let clock = Arc::new(TestClock);
    let adapter = CountingComfyAdapter::offline();
    let service = WorkflowOnboardingService::new(
        source.clone(),
        Arc::new(adapter.clone()),
        Arc::new(WorkflowLibraryService::new(
            source,
            Arc::new(SqliteWorkflowLibraryRepository::new(pool)),
            clock.clone(),
        )),
        Arc::new(TestRunRepository),
        Arc::new(FileSystemWorkflowPackageStore::new(
            library_root,
            staging_root,
        )),
        clock,
    );
    Harness {
        _directory: directory,
        adapter,
        service,
    }
}

#[tokio::test]
async fn ui_import_is_pending_without_api_document_or_identity() {
    let harness = harness().await;
    let draft = harness
        .service
        .import_bytes(KERA2_UI.to_vec(), "kera2.json".to_owned(), None)
        .await
        .expect("UI source should create a draft");

    assert_eq!(draft.source_format, "UI");
    assert_eq!(
        draft.normalization_state,
        WorkflowNormalizationState::UiSourcePending
    );
    assert_eq!(draft.capability.state, CapabilityState::NotChecked);
    assert!(draft.nodes.is_empty());
    assert!(!draft.raw_sha256.is_empty());
    assert!(!draft.validation.api_format);
    assert!(!draft.validation.ready_to_publish);

    let pending = harness
        .service
        .analyze_draft(&draft.draft_id)
        .await
        .expect("offline UI analysis should return a pending plan");
    assert_eq!(pending.draft_id, draft.draft_id);
    assert_eq!(
        pending.state,
        WorkflowAutoOnboardingState::WaitingForComfyUi
    );
    assert_eq!(
        pending.normalization_state,
        WorkflowNormalizationState::UiSourcePending
    );
    assert_eq!(pending.capability.state, CapabilityState::ComfyOffline);
    assert!(!pending.auto_publishable);
    assert_eq!(harness.adapter.calls(), 1);
}

#[tokio::test]
async fn ui_reanalysis_normalizes_same_draft_with_one_schema_fetch() {
    let harness = harness().await;
    let draft = harness
        .service
        .import_bytes(KERA2_UI.to_vec(), "kera2.json".to_owned(), None)
        .await
        .expect("UI source should create a draft");
    let pending = harness
        .service
        .analyze_draft(&draft.draft_id)
        .await
        .expect("offline UI analysis should return a pending plan");
    assert_eq!(pending.draft_id, draft.draft_id);

    harness.adapter.set_object_info(
        serde_json::from_str(KERA2_OBJECT_INFO).expect("fixture schema should be valid JSON"),
    );
    let calls_before_resume = harness.adapter.calls();
    let ready = harness
        .service
        .reanalyze_draft(&draft.draft_id)
        .await
        .expect("online reanalysis should normalize the same draft");

    assert_eq!(ready.draft_id, draft.draft_id);
    assert_eq!(
        ready.normalization_state,
        WorkflowNormalizationState::NormalizedApiReady
    );
    assert_eq!(ready.source_format, "UI");
    assert_ne!(ready.workflow_sha256, ready.raw_sha256);
    assert_eq!(harness.adapter.calls() - calls_before_resume, 1);
    assert!(ready.node_count > 0);
    assert!(!ready.input_mappings.is_empty());
    assert!(ready
        .input_mappings
        .iter()
        .any(|mapping| mapping.semantic_key == "prompt"));
    assert!(ready
        .output_mappings
        .iter()
        .any(|mapping| mapping.output_type == "image"));

    let view = harness
        .service
        .get(&draft.draft_id)
        .expect("same draft should remain addressable");
    assert_eq!(view.draft_id, draft.draft_id);
    assert_eq!(
        view.normalization_state,
        WorkflowNormalizationState::NormalizedApiReady
    );
}

#[tokio::test]
async fn pending_ui_draft_cannot_publish_or_open_advanced_api_state() {
    let harness = harness().await;
    let draft = harness
        .service
        .import_bytes(KERA2_UI.to_vec(), "kera2.json".to_owned(), None)
        .await
        .expect("UI source should create a draft");

    let error = harness
        .service
        .publish(&draft.draft_id)
        .await
        .expect_err("pending UI source must not publish");
    assert_eq!(error.code(), "NORMALIZATION_REQUIRED");
}
