//! DEV-055 release performance and bulk-read contracts.
//!
//! The fixture is deliberately backed by the real SQLite repositories.  The
//! wrappers below count port calls without changing the production path, so a
//! passing test proves both the result and the shape of the read graph.

use ai_studio_lib::{
    application::{
        comfy_preflight_service::ComfyPreflightService,
        comfy_service::{ComfyRuntime, ComfyService},
        diagnostics_service::DiagnosticsService,
        generation_service::GenerationService,
        ports::{
            ActiveProductionItem, AssetRepository, AssetStore, AvailableGenerationDefinition,
            Clock, ComfyAdapter, ComfyAdapterError, ComfyConnectionConfig, ComfyEventSubscription,
            ComfyExecutionEvent, ComfyHealth, ComfyHistory, ComfyOutputData, ComfyOutputFile,
            ConsistencyProfileRepository, ConsistencyScopeRepository, GenerationDefinition,
            GenerationDefinitionRepository, GenerationSnapshotRepository, NoopTaskUpdateSink,
            ProductionItemReviewRecord, ProductionItemReviewRepository, ProductionQueueRepository,
            ProductionStructureRepository, ProjectRecord, ProjectRepository, PromptSubmission,
            ReferenceSetRepository, RepositoryError, ShotBatchRepository, ShotBulkRepository,
            ShotConsistencyRepository, ShotData, ShotRecord, ShotRepository, ShotStageConfigRecord,
            SystemStats, TaskRepository, WorkflowLibrarySource, WorkflowLibrarySourceError,
            WorkflowPackageFiles, WorkflowPackageLoad, WorkflowPackageStore, WorkflowRunRepository,
            WorkflowRuntimeRepository, WorkflowRuntimeStateRepository,
        },
        production_item_review_service::ProductionItemReviewService,
        production_preparation_service::ProductionPreparationService,
        production_queue_service::ProductionQueueService,
        shot_batch_service::ShotBatchService,
        shot_context_resolver::{ShotContextResolver, ShotContextResolverError},
        shot_readiness_service::ShotReadinessService,
        task_recovery_service::TaskRecoveryService,
        workflow_library_service::WorkflowLibraryService,
        workflow_lifecycle_service::WorkflowLifecycleService,
        workflow_onboarding_service::WorkflowOnboardingService,
    },
    domain::{
        Asset, AssetId, AssetType, GenerationSnapshot, NewTaskEvent, PreparationSnapshotRecord,
        ProductionBatch, ProductionBatchDetail, ProductionBatchId, ProductionBatchItem,
        ProductionBatchItemId, ProductionBatchItemStatus, ProductionBatchStatus,
        ProductionReviewStatus, ProfileType, ShotStage, StoredTaskEvent, Task, TaskId, TaskStatus,
    },
    infrastructure::{
        database::{
            initialize, SqliteAssetRepository, SqliteConsistencyProfileRepository,
            SqliteGenerationDefinitionRepository, SqliteGenerationSnapshotRepository,
            SqliteProductionQueueRepository, SqliteProductionStructureRepository,
            SqliteProjectRepository, SqliteReferenceSetRepository, SqliteShotConsistencyRepository,
            SqliteShotRepository, SqliteTaskRepository, SqliteWorkflowLibraryRepository,
            SqliteWorkflowRunRepository, SqliteWorkflowRuntimeRepository,
            SqliteWorkflowRuntimeStateRepository,
        },
        filesystem::{FileSystemAssetStore, FileSystemWorkflowPackageStore},
        logging::LoggingStatus,
    },
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::{
    collections::HashSet,
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};
use tempfile::TempDir;

const PROJECT_ID: &str = "prj_default";
const CREATED_AT: &str = "2026-08-27T00:00:00Z";
const SHOT_COUNT: usize = 500;
const CONTEXT_BATCH_LIMIT: usize = 500;
const WORKFLOW_JSON: &str = r#"{
  "3": {"inputs": {"seed": 1, "steps": 20, "cfg": 8, "sampler_name": "euler", "scheduler": "normal", "denoise": 1.0, "model": ["4", 0], "positive": ["6", 0], "negative": ["7", 0], "latent_image": ["5", 0]}, "class_type": "KSampler"},
  "4": {"inputs": {"ckpt_name": "model.safetensors"}, "class_type": "CheckpointLoaderSimple"},
  "5": {"inputs": {"width": 512, "height": 512, "batch_size": 1}, "class_type": "EmptyLatentImage"},
  "6": {"inputs": {"text": "original", "clip": ["4", 1]}, "class_type": "CLIPTextEncode"},
  "7": {"inputs": {"text": "", "clip": ["4", 1]}, "class_type": "CLIPTextEncode"},
  "8": {"inputs": {"samples": ["3", 0], "vae": ["4", 2]}, "class_type": "VAEDecode"},
  "9": {"inputs": {"filename_prefix": "ComfyUI", "images": ["8", 0]}, "class_type": "SaveImage"}
}"#;
const RECIPE_YAML: &str = r#"schema_version: 1
id: rcp_dev055_perf
name: DEV-055 Fixture Image
workflow:
  file: workflow_api.json
inputs:
  prompt:
    type: textarea
    label: Prompt
    required: true
    default: ""
bindings:
  - source: prompt
    target:
      node: "6"
      input: text
outputs:
  - id: generated_image
    type: image
    node: "9"
    required: true
"#;

#[derive(Default)]
struct PerfCounters {
    project_find: AtomicUsize,
    structure_load: AtomicUsize,
    shot_list_bulk: AtomicUsize,
    shot_find_single: AtomicUsize,
    shot_bulk_projection: AtomicUsize,
    scope_profile_bulk: AtomicUsize,
    scope_reference_bulk: AtomicUsize,
    profile_list_bulk: AtomicUsize,
    profile_find_single: AtomicUsize,
    costume_single: AtomicUsize,
    costume_bulk: AtomicUsize,
    revision_single: AtomicUsize,
    revision_bulk: AtomicUsize,
    reference_set_bulk: AtomicUsize,
    reference_set_single: AtomicUsize,
    reference_item_single: AtomicUsize,
    reference_item_bulk: AtomicUsize,
    shot_profile_single: AtomicUsize,
    shot_profile_bulk: AtomicUsize,
    shot_reference_single: AtomicUsize,
    shot_reference_bulk: AtomicUsize,
    asset_single: AtomicUsize,
    asset_bulk: AtomicUsize,
    definition_single: AtomicUsize,
    definition_bulk: AtomicUsize,
    definition_list_available: AtomicUsize,
    comfy_health: AtomicUsize,
    comfy_object_info: AtomicUsize,
    comfy_submit: AtomicUsize,
    workflow_source_load: AtomicUsize,
    workflow_runtime_list: AtomicUsize,
    workflow_runtime_find: AtomicUsize,
    workflow_state_list: AtomicUsize,
    workflow_state_find: AtomicUsize,
}

impl PerfCounters {
    fn reset(&self) {
        macro_rules! reset {
            ($($field:ident),+ $(,)?) => {$(self.$field.store(0, Ordering::SeqCst);)+};
        }
        reset!(
            project_find,
            structure_load,
            shot_list_bulk,
            shot_find_single,
            shot_bulk_projection,
            scope_profile_bulk,
            scope_reference_bulk,
            profile_list_bulk,
            profile_find_single,
            costume_single,
            costume_bulk,
            revision_single,
            revision_bulk,
            reference_set_bulk,
            reference_set_single,
            reference_item_single,
            reference_item_bulk,
            shot_profile_single,
            shot_profile_bulk,
            shot_reference_single,
            shot_reference_bulk,
            asset_single,
            asset_bulk,
            definition_single,
            definition_bulk,
            definition_list_available,
            comfy_health,
            comfy_object_info,
            comfy_submit,
            workflow_source_load,
            workflow_runtime_list,
            workflow_runtime_find,
            workflow_state_list,
            workflow_state_find,
        );
    }

    fn print_context(&self) {
        println!(
            "DEV055_CONTEXT_COUNTS project_find={} structure_load={} shot(list_bulk={}, find_single={}, bulk_projection={}) scope(profile_bulk={}, reference_bulk={}) profile(list_bulk={}, find_single={}, costume(single={}, bulk={}), revision(single={}, bulk={})) reference(set_bulk={}, set_single={}, item_single={}, item_bulk={}) shot_consistency(profile_single={}, profile_bulk={}, reference_single={}, reference_bulk={}) asset(single={}, bulk={})",
            self.project_find.load(Ordering::SeqCst),
            self.structure_load.load(Ordering::SeqCst),
            self.shot_list_bulk.load(Ordering::SeqCst),
            self.shot_find_single.load(Ordering::SeqCst),
            self.shot_bulk_projection.load(Ordering::SeqCst),
            self.scope_profile_bulk.load(Ordering::SeqCst),
            self.scope_reference_bulk.load(Ordering::SeqCst),
            self.profile_list_bulk.load(Ordering::SeqCst),
            self.profile_find_single.load(Ordering::SeqCst),
            self.costume_single.load(Ordering::SeqCst),
            self.costume_bulk.load(Ordering::SeqCst),
            self.revision_single.load(Ordering::SeqCst),
            self.revision_bulk.load(Ordering::SeqCst),
            self.reference_set_bulk.load(Ordering::SeqCst),
            self.reference_set_single.load(Ordering::SeqCst),
            self.reference_item_single.load(Ordering::SeqCst),
            self.reference_item_bulk.load(Ordering::SeqCst),
            self.shot_profile_single.load(Ordering::SeqCst),
            self.shot_profile_bulk.load(Ordering::SeqCst),
            self.shot_reference_single.load(Ordering::SeqCst),
            self.shot_reference_bulk.load(Ordering::SeqCst),
            self.asset_single.load(Ordering::SeqCst),
            self.asset_bulk.load(Ordering::SeqCst),
        );
    }
}

fn unsupported() -> RepositoryError {
    RepositoryError::database("DEV-055 counting fixture method is not used")
}

struct CountingProjectRepository {
    inner: Arc<SqliteProjectRepository>,
    counters: Arc<PerfCounters>,
}

#[async_trait]
impl ProjectRepository for CountingProjectRepository {
    async fn list(&self) -> Result<Vec<ProjectRecord>, RepositoryError> {
        Err(unsupported())
    }

    async fn find_by_id(&self, project_id: &str) -> Result<Option<ProjectRecord>, RepositoryError> {
        self.counters.project_find.fetch_add(1, Ordering::SeqCst);
        self.inner.find_by_id(project_id).await
    }

    async fn insert(&self, _: &ProjectRecord) -> Result<(), RepositoryError> {
        Err(unsupported())
    }

    async fn update_metadata(
        &self,
        _: &str,
        _: &str,
        _: Option<&str>,
        _: DateTime<Utc>,
    ) -> Result<Option<ProjectRecord>, RepositoryError> {
        Err(unsupported())
    }

    async fn get_storage_root(&self, _: &str) -> Result<Option<PathBuf>, RepositoryError> {
        Err(unsupported())
    }

    async fn ensure_default_project(
        &self,
        _: &str,
        _: &str,
        _: &PathBuf,
        _: DateTime<Utc>,
    ) -> Result<ProjectRecord, RepositoryError> {
        Err(unsupported())
    }
}

struct CountingStructureRepository {
    inner: Arc<SqliteProductionStructureRepository>,
    counters: Arc<PerfCounters>,
}

#[async_trait]
impl ProductionStructureRepository for CountingStructureRepository {
    async fn load_tree_data(
        &self,
        project_id: &str,
    ) -> Result<ai_studio_lib::application::ports::ProductionStructureTreeData, RepositoryError>
    {
        self.counters.structure_load.fetch_add(1, Ordering::SeqCst);
        self.inner.load_tree_data(project_id).await
    }

    async fn create_series(
        &self,
        _: &ai_studio_lib::domain::ProductionSeries,
    ) -> Result<ai_studio_lib::domain::ProductionSeries, RepositoryError> {
        Err(unsupported())
    }
    async fn update_series(
        &self,
        _: &ai_studio_lib::domain::ProductionSeries,
    ) -> Result<ai_studio_lib::domain::ProductionSeries, RepositoryError> {
        Err(unsupported())
    }
    async fn delete_series(
        &self,
        _: &str,
        _: &ai_studio_lib::domain::ProductionSeriesId,
    ) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn reorder_series(
        &self,
        _: &str,
        _: &[ai_studio_lib::domain::ProductionSeriesId],
        _: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn create_episode(
        &self,
        _: &str,
        _: &ai_studio_lib::domain::ProductionEpisode,
    ) -> Result<ai_studio_lib::domain::ProductionEpisode, RepositoryError> {
        Err(unsupported())
    }
    async fn update_episode(
        &self,
        _: &ai_studio_lib::domain::ProductionEpisode,
        _: &str,
    ) -> Result<ai_studio_lib::domain::ProductionEpisode, RepositoryError> {
        Err(unsupported())
    }
    async fn delete_episode(
        &self,
        _: &str,
        _: &ai_studio_lib::domain::ProductionEpisodeId,
    ) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn reorder_episodes(
        &self,
        _: &str,
        _: &ai_studio_lib::domain::ProductionSeriesId,
        _: &[ai_studio_lib::domain::ProductionEpisodeId],
        _: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn create_scene(
        &self,
        _: &str,
        _: &ai_studio_lib::domain::ProductionScene,
    ) -> Result<ai_studio_lib::domain::ProductionScene, RepositoryError> {
        Err(unsupported())
    }
    async fn update_scene(
        &self,
        _: &ai_studio_lib::domain::ProductionScene,
        _: &str,
    ) -> Result<ai_studio_lib::domain::ProductionScene, RepositoryError> {
        Err(unsupported())
    }
    async fn delete_scene(
        &self,
        _: &str,
        _: &ai_studio_lib::domain::ProductionSceneId,
    ) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn reorder_scenes(
        &self,
        _: &str,
        _: &ai_studio_lib::domain::ProductionEpisodeId,
        _: &[ai_studio_lib::domain::ProductionSceneId],
        _: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn assign_shots_atomic(
        &self,
        _: &str,
        _: &ai_studio_lib::domain::ProductionSceneId,
        _: &[String],
        _: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn unassign_shots_atomic(&self, _: &str, _: &[String]) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn reorder_scene_shots(
        &self,
        _: &ai_studio_lib::domain::ProductionSceneId,
        _: &[String],
        _: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
}

struct CountingShotRepository {
    inner: Arc<SqliteShotRepository>,
    counters: Arc<PerfCounters>,
}

#[async_trait]
impl ShotRepository for CountingShotRepository {
    async fn list(&self, project_id: &str) -> Result<Vec<ShotData>, RepositoryError> {
        self.counters.shot_list_bulk.fetch_add(1, Ordering::SeqCst);
        self.inner.list(project_id).await
    }

    async fn find(
        &self,
        project_id: &str,
        shot_id: &str,
    ) -> Result<Option<ShotData>, RepositoryError> {
        self.counters
            .shot_find_single
            .fetch_add(1, Ordering::SeqCst);
        self.inner.find(project_id, shot_id).await
    }

    async fn insert(&self, _: &ShotRecord) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn update(&self, _: &ShotRecord) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn delete(&self, _: &str, _: &str) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn reorder(
        &self,
        _: &str,
        _: &[String],
        _: DateTime<Utc>,
    ) -> Result<Vec<ShotRecord>, RepositoryError> {
        Err(unsupported())
    }
    async fn upsert_stage_config(
        &self,
        _: &str,
        _: &ShotStageConfigRecord,
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn replace_reference_assets(
        &self,
        _: &str,
        _: &str,
        _: ShotStage,
        _: &[String],
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn select_image(&self, _: &str, _: &str, _: &str) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn select_video(&self, _: &str, _: &str, _: &str) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn link_generation(
        &self,
        _: &str,
        _: &str,
        _: ShotStage,
        _: &str,
        _: Option<&str>,
        _: DateTime<Utc>,
    ) -> Result<ai_studio_lib::application::ports::ShotGenerationLinkRecord, RepositoryError> {
        Err(unsupported())
    }
}

#[async_trait]
impl ShotBulkRepository for CountingShotRepository {
    async fn list_bulk_data(
        &self,
        project_id: &str,
    ) -> Result<Vec<ai_studio_lib::application::ports::ShotBulkData>, RepositoryError> {
        self.counters
            .shot_bulk_projection
            .fetch_add(1, Ordering::SeqCst);
        ShotBulkRepository::list_bulk_data(self.inner.as_ref(), project_id).await
    }

    async fn insert_shots_atomic(
        &self,
        _: &str,
        _: &[ShotRecord],
        _: &[ai_studio_lib::application::ports::ShotStagePromptRecord],
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }

    async fn update_stage_prompts_atomic(
        &self,
        _: &str,
        _: &[ai_studio_lib::application::ports::ShotStagePromptRecord],
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }

    async fn upsert_stage_configs_atomic(
        &self,
        _: &str,
        _: &[ShotStageConfigRecord],
        _: &[ai_studio_lib::application::ports::ShotStagePromptRecord],
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
}

struct CountingScopeRepository {
    inner: Arc<
        ai_studio_lib::infrastructure::database::repositories::SqliteConsistencyScopeRepository,
    >,
    counters: Arc<PerfCounters>,
}

#[async_trait]
impl ConsistencyScopeRepository for CountingScopeRepository {
    async fn list_profile_bindings_for_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<ai_studio_lib::domain::ScopedProfileBinding>, RepositoryError> {
        self.counters
            .scope_profile_bulk
            .fetch_add(1, Ordering::SeqCst);
        self.inner
            .list_profile_bindings_for_project(project_id)
            .await
    }

    async fn list_reference_set_bindings_for_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<ai_studio_lib::domain::ScopedReferenceSetBinding>, RepositoryError> {
        self.counters
            .scope_reference_bulk
            .fetch_add(1, Ordering::SeqCst);
        self.inner
            .list_reference_set_bindings_for_project(project_id)
            .await
    }

    async fn replace_profile_bindings(
        &self,
        _: &str,
        _: ai_studio_lib::domain::ConsistencyScopeType,
        _: &str,
        _: &[ai_studio_lib::domain::ScopedProfileBinding],
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }

    async fn replace_reference_set_bindings(
        &self,
        _: &str,
        _: ai_studio_lib::domain::ConsistencyScopeType,
        _: &str,
        _: &[ai_studio_lib::domain::ScopedReferenceSetBinding],
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
}

struct CountingProfiles {
    inner: Arc<SqliteConsistencyProfileRepository>,
    counters: Arc<PerfCounters>,
}

#[async_trait]
impl ConsistencyProfileRepository for CountingProfiles {
    async fn list_profiles(
        &self,
        project_id: &str,
        profile_type: ProfileType,
    ) -> Result<Vec<ai_studio_lib::domain::ConsistencyProfileRecord>, RepositoryError> {
        self.counters
            .profile_list_bulk
            .fetch_add(1, Ordering::SeqCst);
        self.inner.list_profiles(project_id, profile_type).await
    }

    async fn find_profile(
        &self,
        project_id: &str,
        profile_type: ProfileType,
        profile_id: &str,
    ) -> Result<Option<ai_studio_lib::domain::ConsistencyProfileRecord>, RepositoryError> {
        self.counters
            .profile_find_single
            .fetch_add(1, Ordering::SeqCst);
        self.inner
            .find_profile(project_id, profile_type, profile_id)
            .await
    }

    async fn insert_profile(
        &self,
        _: &ai_studio_lib::domain::ConsistencyProfileRecord,
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn update_profile(
        &self,
        _: &ai_studio_lib::domain::ConsistencyProfileRecord,
    ) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn delete_profile(
        &self,
        _: &str,
        _: ProfileType,
        _: &str,
    ) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn list_costume_variants(
        &self,
        profile_id: &str,
    ) -> Result<Vec<ai_studio_lib::domain::CostumeVariant>, RepositoryError> {
        self.counters.costume_single.fetch_add(1, Ordering::SeqCst);
        self.inner.list_costume_variants(profile_id).await
    }
    async fn list_costume_variants_many(
        &self,
        profile_ids: &[String],
    ) -> Result<Vec<ai_studio_lib::domain::CostumeVariant>, RepositoryError> {
        self.counters.costume_bulk.fetch_add(1, Ordering::SeqCst);
        self.inner.list_costume_variants_many(profile_ids).await
    }
    async fn find_costume_variant(
        &self,
        _: &str,
    ) -> Result<Option<ai_studio_lib::domain::CostumeVariant>, RepositoryError> {
        Err(unsupported())
    }
    async fn insert_costume_variant(
        &self,
        _: &ai_studio_lib::domain::CostumeVariant,
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn update_costume_variant(
        &self,
        _: &ai_studio_lib::domain::CostumeVariant,
    ) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn delete_costume_variant(&self, _: &str) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn list_profile_revisions(
        &self,
        _: ProfileType,
        _: &str,
    ) -> Result<Vec<ai_studio_lib::domain::ProfileRevision>, RepositoryError> {
        Err(unsupported())
    }
    async fn find_profile_revision(
        &self,
        revision_id: &str,
    ) -> Result<Option<ai_studio_lib::domain::ProfileRevision>, RepositoryError> {
        self.counters.revision_single.fetch_add(1, Ordering::SeqCst);
        self.inner.find_profile_revision(revision_id).await
    }
    async fn find_profile_revisions_many(
        &self,
        revision_ids: &[String],
    ) -> Result<Vec<ai_studio_lib::domain::ProfileRevision>, RepositoryError> {
        self.counters.revision_bulk.fetch_add(1, Ordering::SeqCst);
        self.inner.find_profile_revisions_many(revision_ids).await
    }
    async fn insert_profile_revision(
        &self,
        _: &ai_studio_lib::domain::ProfileRevision,
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
}

struct CountingReferenceSets {
    inner: Arc<SqliteReferenceSetRepository>,
    counters: Arc<PerfCounters>,
}

#[async_trait]
impl ReferenceSetRepository for CountingReferenceSets {
    async fn list_reference_sets(
        &self,
        project_id: &str,
        purpose: Option<ai_studio_lib::domain::ReferenceSetPurpose>,
    ) -> Result<Vec<ai_studio_lib::domain::ReferenceSet>, RepositoryError> {
        self.counters
            .reference_set_bulk
            .fetch_add(1, Ordering::SeqCst);
        self.inner.list_reference_sets(project_id, purpose).await
    }
    async fn find_reference_set(
        &self,
        project_id: &str,
        reference_set_id: &str,
    ) -> Result<Option<ai_studio_lib::domain::ReferenceSet>, RepositoryError> {
        self.counters
            .reference_set_single
            .fetch_add(1, Ordering::SeqCst);
        self.inner
            .find_reference_set(project_id, reference_set_id)
            .await
    }
    async fn insert_reference_set(
        &self,
        _: &ai_studio_lib::domain::ReferenceSet,
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn update_reference_set(
        &self,
        _: &ai_studio_lib::domain::ReferenceSet,
    ) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn delete_reference_set(&self, _: &str, _: &str) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn list_items(
        &self,
        reference_set_id: &str,
    ) -> Result<Vec<ai_studio_lib::domain::ReferenceSetItem>, RepositoryError> {
        self.counters
            .reference_item_single
            .fetch_add(1, Ordering::SeqCst);
        self.inner.list_items(reference_set_id).await
    }
    async fn list_items_many(
        &self,
        reference_set_ids: &[String],
    ) -> Result<Vec<ai_studio_lib::domain::ReferenceSetItem>, RepositoryError> {
        self.counters
            .reference_item_bulk
            .fetch_add(1, Ordering::SeqCst);
        self.inner.list_items_many(reference_set_ids).await
    }
    async fn replace_items(
        &self,
        _: &str,
        _: &[ai_studio_lib::domain::ReferenceSetItem],
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
}

struct CountingShotConsistency {
    inner: Arc<SqliteShotConsistencyRepository>,
    counters: Arc<PerfCounters>,
}

#[async_trait]
impl ShotConsistencyRepository for CountingShotConsistency {
    async fn list_profile_bindings(
        &self,
        shot_id: &str,
    ) -> Result<Vec<ai_studio_lib::domain::ShotProfileBinding>, RepositoryError> {
        self.counters
            .shot_profile_single
            .fetch_add(1, Ordering::SeqCst);
        self.inner.list_profile_bindings(shot_id).await
    }
    async fn list_profile_bindings_many(
        &self,
        shot_ids: &[String],
    ) -> Result<Vec<ai_studio_lib::domain::ShotProfileBinding>, RepositoryError> {
        self.counters
            .shot_profile_bulk
            .fetch_add(1, Ordering::SeqCst);
        self.inner.list_profile_bindings_many(shot_ids).await
    }
    async fn replace_profile_bindings(
        &self,
        _: &str,
        _: &[ai_studio_lib::domain::ShotProfileBinding],
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn list_reference_set_bindings(
        &self,
        shot_id: &str,
    ) -> Result<Vec<ai_studio_lib::domain::ShotReferenceSetBinding>, RepositoryError> {
        self.counters
            .shot_reference_single
            .fetch_add(1, Ordering::SeqCst);
        self.inner.list_reference_set_bindings(shot_id).await
    }
    async fn list_reference_set_bindings_many(
        &self,
        shot_ids: &[String],
    ) -> Result<Vec<ai_studio_lib::domain::ShotReferenceSetBinding>, RepositoryError> {
        self.counters
            .shot_reference_bulk
            .fetch_add(1, Ordering::SeqCst);
        self.inner.list_reference_set_bindings_many(shot_ids).await
    }
    async fn replace_reference_set_bindings(
        &self,
        _: &str,
        _: &[ai_studio_lib::domain::ShotReferenceSetBinding],
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
}

struct CountingAssets {
    inner: Arc<SqliteAssetRepository>,
    counters: Arc<PerfCounters>,
}

#[async_trait]
impl AssetRepository for CountingAssets {
    async fn insert_many(&self, _: &[Asset]) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn find_by_id(&self, asset_id: &AssetId) -> Result<Option<Asset>, RepositoryError> {
        self.counters.asset_single.fetch_add(1, Ordering::SeqCst);
        self.inner.find_by_id(asset_id).await
    }
    async fn find_many_by_ids(&self, asset_ids: &[AssetId]) -> Result<Vec<Asset>, RepositoryError> {
        self.counters.asset_bulk.fetch_add(1, Ordering::SeqCst);
        self.inner.find_many_by_ids(asset_ids).await
    }
    async fn list_by_source_task(&self, _: &TaskId) -> Result<Vec<Asset>, RepositoryError> {
        Err(unsupported())
    }
    async fn list_by_source_tasks(&self, _: &[TaskId]) -> Result<Vec<Asset>, RepositoryError> {
        Err(unsupported())
    }
    async fn list_recent(&self, _: &str, _: u32) -> Result<Vec<Asset>, RepositoryError> {
        Err(unsupported())
    }
}

struct CountingDefinitions {
    inner: Arc<SqliteGenerationDefinitionRepository>,
    counters: Arc<PerfCounters>,
}

#[async_trait]
impl GenerationDefinitionRepository for CountingDefinitions {
    async fn find(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
    ) -> Result<Option<GenerationDefinition>, RepositoryError> {
        self.counters
            .definition_single
            .fetch_add(1, Ordering::SeqCst);
        self.inner.find(workflow_version_id, recipe_id).await
    }
    async fn find_many(
        &self,
        definitions: &[(String, String)],
    ) -> Result<Vec<GenerationDefinition>, RepositoryError> {
        self.counters.definition_bulk.fetch_add(1, Ordering::SeqCst);
        self.inner.find_many(definitions).await
    }
    async fn list_available(&self) -> Result<Vec<AvailableGenerationDefinition>, RepositoryError> {
        self.counters
            .definition_list_available
            .fetch_add(1, Ordering::SeqCst);
        self.inner.list_available().await
    }
}

struct CountingRuntimeRepository {
    inner: Arc<SqliteWorkflowRuntimeRepository>,
    counters: Arc<PerfCounters>,
}

#[async_trait]
impl WorkflowRuntimeRepository for CountingRuntimeRepository {
    async fn list_versions(
        &self,
    ) -> Result<Vec<ai_studio_lib::application::ports::RuntimeWorkflowVersionRecord>, RepositoryError>
    {
        self.counters
            .workflow_runtime_list
            .fetch_add(1, Ordering::SeqCst);
        self.inner.list_versions().await
    }
    async fn find_version(
        &self,
        workflow_version_id: &str,
    ) -> Result<
        Option<ai_studio_lib::application::ports::RuntimeWorkflowVersionRecord>,
        RepositoryError,
    > {
        self.counters
            .workflow_runtime_find
            .fetch_add(1, Ordering::SeqCst);
        self.inner.find_version(workflow_version_id).await
    }
    async fn inspect_deletion(
        &self,
        _: &str,
    ) -> Result<Option<ai_studio_lib::application::ports::WorkflowDeletionCounts>, RepositoryError>
    {
        Err(unsupported())
    }
    async fn delete_version(
        &self,
        _: &str,
        _: &str,
        _: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
}

struct CountingRuntimeStateRepository {
    inner: Arc<SqliteWorkflowRuntimeStateRepository>,
    counters: Arc<PerfCounters>,
}

#[async_trait]
impl WorkflowRuntimeStateRepository for CountingRuntimeStateRepository {
    async fn is_enabled(&self, _: &str) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn set_enabled(&self, _: &str, _: bool, _: DateTime<Utc>) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn find_state(
        &self,
        package_name: &str,
    ) -> Result<Option<ai_studio_lib::application::ports::WorkflowRuntimeState>, RepositoryError>
    {
        self.counters
            .workflow_state_find
            .fetch_add(1, Ordering::SeqCst);
        self.inner.find_state(package_name).await
    }
    async fn set_archived(
        &self,
        _: &str,
        _: bool,
        _: bool,
        _: Option<DateTime<Utc>>,
        _: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn list_states(
        &self,
    ) -> Result<Vec<ai_studio_lib::application::ports::WorkflowRuntimeState>, RepositoryError> {
        self.counters
            .workflow_state_list
            .fetch_add(1, Ordering::SeqCst);
        self.inner.list_states().await
    }
}

#[derive(Clone)]
struct CountingWorkflowSource {
    package: WorkflowPackageFiles,
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl WorkflowLibrarySource for CountingWorkflowSource {
    async fn load_packages(&self) -> Result<Vec<WorkflowPackageLoad>, WorkflowLibrarySourceError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(vec![WorkflowPackageLoad::Loaded(self.package.clone())])
    }
}

struct EmptyComfyEvents;

#[async_trait]
impl ComfyEventSubscription for EmptyComfyEvents {
    async fn next_event(&mut self) -> Result<Option<ComfyExecutionEvent>, ComfyAdapterError> {
        Ok(None)
    }
}

#[derive(Clone)]
struct CountingComfyAdapter {
    counters: Arc<PerfCounters>,
}

#[async_trait]
impl ComfyAdapter for CountingComfyAdapter {
    async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
        self.counters.comfy_health.fetch_add(1, Ordering::SeqCst);
        Ok(ComfyHealth {
            system: SystemStats {
                comfyui_version: Some("dev055-test".to_owned()),
                python_version: Some("3.12".to_owned()),
                os: Some("test".to_owned()),
                ram_total: None,
                ram_free: None,
                devices: Vec::new(),
            },
        })
    }

    async fn get_system_stats(&self) -> Result<SystemStats, ComfyAdapterError> {
        Ok(SystemStats {
            comfyui_version: Some("dev055-test".to_owned()),
            python_version: Some("3.12".to_owned()),
            os: Some("test".to_owned()),
            ram_total: None,
            ram_free: None,
            devices: Vec::new(),
        })
    }

    async fn get_object_info(&self) -> Result<Value, ComfyAdapterError> {
        self.counters
            .comfy_object_info
            .fetch_add(1, Ordering::SeqCst);
        Ok(json!({
            "KSampler": {},
            "CheckpointLoaderSimple": {},
            "EmptyLatentImage": {},
            "CLIPTextEncode": {},
            "VAEDecode": {},
            "SaveImage": {},
            "LoadImage": {}
        }))
    }

    async fn get_history(&self, prompt_id: &str) -> Result<ComfyHistory, ComfyAdapterError> {
        Err(ComfyAdapterError::HistoryNotFound(prompt_id.to_owned()))
    }

    async fn download_output(
        &self,
        file: &ComfyOutputFile,
    ) -> Result<ComfyOutputData, ComfyAdapterError> {
        Err(ComfyAdapterError::OutputDownload(file.filename.clone()))
    }

    async fn submit_workflow(
        &self,
        _: &str,
        _: &str,
        _: Value,
    ) -> Result<PromptSubmission, ComfyAdapterError> {
        self.counters.comfy_submit.fetch_add(1, Ordering::SeqCst);
        Err(ComfyAdapterError::Incompatible(
            "DEV-055 fixture must not submit workflows".to_owned(),
        ))
    }

    async fn subscribe_events(
        &self,
        _: &str,
    ) -> Result<Box<dyn ComfyEventSubscription>, ComfyAdapterError> {
        Ok(Box::new(EmptyComfyEvents))
    }
}

#[derive(Clone, Copy)]
struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(CREATED_AT)
            .expect("fixed timestamp")
            .with_timezone(&Utc)
    }
}

async fn seed_performance_fixture(
    pool: &SqlitePool,
    workflow_version_id: &str,
    recipe_id: &str,
) -> Vec<String> {
    let mut transaction = pool
        .begin()
        .await
        .expect("DEV-055 fixture transaction should begin");

    sqlx::query(
        "INSERT INTO assets
         (id, project_id, type, category, name, original_name, storage_path, sha256,
          mime_type, width, height, file_size, metadata_json, created_at, updated_at)
         VALUES ('ast_dev055_reference', ?, 'image', 'source_image', 'DEV-055 reference',
                 'reference.png', 'reference.png', 'sha-dev055-reference', 'image/png',
                 512, 512, 1, '{}', ?, ?)",
    )
    .bind(PROJECT_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(&mut *transaction)
    .await
    .expect("reference asset fixture should insert");

    sqlx::query(
        "INSERT INTO production_series
         (id, project_id, ordinal, name, description, created_at, updated_at)
         VALUES ('ser_dev055', ?, 0, 'DEV-055 Series', 'Performance fixture', ?, ?)",
    )
    .bind(PROJECT_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(&mut *transaction)
    .await
    .expect("series fixture should insert");
    sqlx::query(
        "INSERT INTO production_episodes
         (id, series_id, ordinal, name, description, created_at, updated_at)
         VALUES ('epi_dev055', 'ser_dev055', 0, 'DEV-055 Episode', 'Performance fixture', ?, ?)",
    )
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(&mut *transaction)
    .await
    .expect("episode fixture should insert");
    sqlx::query(
        "INSERT INTO production_scenes
         (id, episode_id, ordinal, name, description, created_at, updated_at)
         VALUES ('scn_dev055', 'epi_dev055', 0, 'DEV-055 Scene', '500 distinct shots', ?, ?)",
    )
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(&mut *transaction)
    .await
    .expect("scene fixture should insert");

    sqlx::query(
        "INSERT INTO character_profiles
         (id, project_id, name, description, canonical_prompt, negative_prompt,
          default_style_profile_id, default_reference_set_id, active_revision_id,
          metadata_json, created_at, updated_at)
         VALUES ('cp_dev055', ?, 'DEV-055 Character', '', 'character anchor',
                 'low quality', NULL, NULL, NULL, '{}', ?, ?)",
    )
    .bind(PROJECT_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(&mut *transaction)
    .await
    .expect("character profile fixture should insert");
    sqlx::query(
        "INSERT INTO prop_profiles
         (id, project_id, name, description, canonical_prompt, material_prompt,
          scale_prompt, default_reference_set_id, active_revision_id, created_at, updated_at)
         VALUES ('pp_dev055', ?, 'DEV-055 Prop', '', 'prop anchor', 'metal',
                 'handheld', NULL, NULL, ?, ?)",
    )
    .bind(PROJECT_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(&mut *transaction)
    .await
    .expect("prop profile fixture should insert");

    sqlx::query(
        "INSERT INTO reference_sets
         (id, project_id, name, purpose, description, owner_profile_type, owner_profile_id,
          active_revision_id, created_at, updated_at)
         VALUES ('rs_dev055_scope', ?, 'DEV-055 Scope References', 'CHARACTER', '',
                 NULL, NULL, NULL, ?, ?),
                ('rs_dev055_shot', ?, 'DEV-055 Shot References', 'SHOT', '',
                 NULL, NULL, NULL, ?, ?)",
    )
    .bind(PROJECT_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .bind(PROJECT_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(&mut *transaction)
    .await
    .expect("reference set fixtures should insert");
    sqlx::query(
        "INSERT INTO reference_set_items
         (reference_set_id, asset_id, ordinal, role, is_primary, created_at)
         VALUES ('rs_dev055_scope', 'ast_dev055_reference', 0, 'CHARACTER', 1, ?),
                ('rs_dev055_shot', 'ast_dev055_reference', 0, 'SHOT', 1, ?)",
    )
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(&mut *transaction)
    .await
    .expect("reference item fixtures should insert");

    sqlx::query(
        "INSERT INTO consistency_scope_profile_bindings
         (id, project_id, scope_type, scope_id, role, profile_type, profile_id,
          costume_variant_id, ordinal, inheritance_mode, created_at, updated_at)
         VALUES ('csp_dev055', ?, 'PROJECT', ?, 'CHARACTER', 'CHARACTER',
                 'cp_dev055', NULL, 0, 'EXPLICIT', ?, ?)",
    )
    .bind(PROJECT_ID)
    .bind(PROJECT_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(&mut *transaction)
    .await
    .expect("scope profile binding should insert");
    sqlx::query(
        "INSERT INTO consistency_scope_reference_set_bindings
         (id, project_id, scope_type, scope_id, role, reference_set_id, ordinal,
          required, inheritance_mode, created_at, updated_at)
         VALUES ('csr_dev055', ?, 'PROJECT', ?, 'CHARACTER', 'rs_dev055_scope',
                 0, 1, 'EXPLICIT', ?, ?)",
    )
    .bind(PROJECT_ID)
    .bind(PROJECT_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(&mut *transaction)
    .await
    .expect("scope reference binding should insert");

    let mut shot_ids = Vec::with_capacity(SHOT_COUNT);
    for ordinal in 0..SHOT_COUNT {
        let shot_id = format!("shot_dev055_{ordinal:03}");
        let profile_binding_id = format!("spb_dev055_{ordinal:03}");
        let reference_binding_id = format!("srb_dev055_{ordinal:03}");
        shot_ids.push(shot_id.clone());

        sqlx::query(
            "INSERT INTO shots
             (id, project_id, ordinal, name, prompt_text, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&shot_id)
        .bind(PROJECT_ID)
        .bind(i64::try_from(ordinal).expect("shot ordinal fits"))
        .bind(format!("DEV-055 Shot {ordinal:03}"))
        .bind(format!("unique performance prompt {ordinal:03}"))
        .bind(CREATED_AT)
        .bind(CREATED_AT)
        .execute(&mut *transaction)
        .await
        .expect("shot fixture should insert");
        sqlx::query(
            "INSERT INTO shot_scene_assignments
             (shot_id, scene_id, ordinal, created_at, updated_at)
             VALUES (?, 'scn_dev055', ?, ?, ?)",
        )
        .bind(&shot_id)
        .bind(i64::try_from(ordinal).expect("assignment ordinal fits"))
        .bind(CREATED_AT)
        .bind(CREATED_AT)
        .execute(&mut *transaction)
        .await
        .expect("shot scene assignment should insert");
        sqlx::query(
            "INSERT INTO shot_stage_configs
             (shot_id, stage, workflow_version_id, recipe_id, scalar_values_json, updated_at)
             VALUES (?, 'image', ?, ?, '{}', ?)",
        )
        .bind(&shot_id)
        .bind(workflow_version_id)
        .bind(recipe_id)
        .bind(CREATED_AT)
        .execute(&mut *transaction)
        .await
        .expect("shot stage config should insert");
        sqlx::query(
            "INSERT INTO shot_profile_bindings
             (id, shot_id, role, profile_type, profile_id, costume_variant_id, ordinal,
              inheritance_mode, created_at, updated_at)
             VALUES (?, ?, 'PROP', 'PROP', 'pp_dev055', NULL, 0, 'EXPLICIT', ?, ?)",
        )
        .bind(&profile_binding_id)
        .bind(&shot_id)
        .bind(CREATED_AT)
        .bind(CREATED_AT)
        .execute(&mut *transaction)
        .await
        .expect("shot profile binding should insert");
        sqlx::query(
            "INSERT INTO shot_reference_set_bindings
             (id, shot_id, role, reference_set_id, ordinal, required, inheritance_mode,
              created_at, updated_at)
             VALUES (?, ?, 'SHOT_REFERENCE', 'rs_dev055_shot', 0, 1, 'EXPLICIT', ?, ?)",
        )
        .bind(&reference_binding_id)
        .bind(&shot_id)
        .bind(CREATED_AT)
        .bind(CREATED_AT)
        .execute(&mut *transaction)
        .await
        .expect("shot reference binding should insert");
    }

    transaction
        .commit()
        .await
        .expect("DEV-055 fixture transaction should commit");
    shot_ids
}

struct PerformanceHarness {
    _directory: TempDir,
    pool: SqlitePool,
    resolver: Arc<ShotContextResolver>,
    preparation: Arc<ProductionPreparationService>,
    shots: Arc<CountingShotRepository>,
    counters: Arc<PerfCounters>,
    source_calls: Arc<AtomicUsize>,
    shot_ids: Vec<String>,
}

async fn performance_harness() -> PerformanceHarness {
    let directory = tempfile::tempdir().expect("DEV-055 tempdir should exist");
    let pool = initialize(&directory.path().join("dev055-performance.db"))
        .await
        .expect("DEV-055 migrations should run");
    let workflow_library_root = directory.path().join("workflow-library");
    let workflow_staging_root = directory.path().join("workflow-staging");
    fs::create_dir_all(&workflow_library_root).expect("workflow library root should exist");
    fs::create_dir_all(&workflow_staging_root).expect("workflow staging root should exist");

    let raw_project = Arc::new(SqliteProjectRepository::new(pool.clone()));
    raw_project
        .ensure_default_project(
            PROJECT_ID,
            "DEV-055 performance fixture",
            &directory.path().join("project"),
            FixedClock.now(),
        )
        .await
        .expect("project fixture should be created");

    let source_calls = Arc::new(AtomicUsize::new(0));
    let source = Arc::new(CountingWorkflowSource {
        package: WorkflowPackageFiles {
            package_name: "dev055_fixture".to_owned(),
            package_source_path: None,
            manifest_yaml: "schema_version: 1\nid: wfl_dev055_fixture\nname: DEV-055 Fixture\nworkflow_version: 1.0.0\nrecipe_version: 1.0.0\ncategory: image\nmode: t2i\n".to_owned(),
            recipe_yaml: RECIPE_YAML.to_owned(),
            workflow_json: WORKFLOW_JSON.to_owned(),
        },
        calls: source_calls.clone(),
    });
    let workflow_library_service = Arc::new(WorkflowLibraryService::new(
        source.clone(),
        Arc::new(SqliteWorkflowLibraryRepository::new(pool.clone())),
        Arc::new(FixedClock),
    ));
    let sync = workflow_library_service
        .sync()
        .await
        .expect("fixture workflow package should sync");
    assert_eq!(sync.valid, 1, "fixture workflow package must be valid");

    let raw_definition = Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone()));
    let definitions = raw_definition
        .list_available()
        .await
        .expect("fixture definition should be available");
    assert_eq!(definitions.len(), 1);
    let workflow_version_id = definitions[0].workflow_version_id.clone();
    let recipe_id = definitions[0].recipe_id.clone();
    let shot_ids = seed_performance_fixture(&pool, &workflow_version_id, &recipe_id).await;
    assert_eq!(shot_ids.len(), SHOT_COUNT);
    assert_eq!(shot_ids.iter().collect::<HashSet<_>>().len(), SHOT_COUNT);

    let counters = Arc::new(PerfCounters::default());
    let clock: Arc<dyn Clock> = Arc::new(FixedClock);
    let project = Arc::new(CountingProjectRepository {
        inner: raw_project,
        counters: counters.clone(),
    });
    let structure = Arc::new(CountingStructureRepository {
        inner: Arc::new(SqliteProductionStructureRepository::new(pool.clone())),
        counters: counters.clone(),
    });
    let shots = Arc::new(CountingShotRepository {
        inner: Arc::new(SqliteShotRepository::new(pool.clone())),
        counters: counters.clone(),
    });
    let scope = Arc::new(CountingScopeRepository {
        inner: Arc::new(
            ai_studio_lib::infrastructure::database::repositories::SqliteConsistencyScopeRepository::new(
                pool.clone(),
            ),
        ),
        counters: counters.clone(),
    });
    let profiles = Arc::new(CountingProfiles {
        inner: Arc::new(SqliteConsistencyProfileRepository::new(pool.clone())),
        counters: counters.clone(),
    });
    let reference_sets = Arc::new(CountingReferenceSets {
        inner: Arc::new(SqliteReferenceSetRepository::new(pool.clone())),
        counters: counters.clone(),
    });
    let shot_consistency = Arc::new(CountingShotConsistency {
        inner: Arc::new(SqliteShotConsistencyRepository::new(pool.clone())),
        counters: counters.clone(),
    });
    let assets = Arc::new(CountingAssets {
        inner: Arc::new(SqliteAssetRepository::new(pool.clone())),
        counters: counters.clone(),
    });
    let definitions = Arc::new(CountingDefinitions {
        inner: raw_definition,
        counters: counters.clone(),
    });
    let resolver = Arc::new(ShotContextResolver::new(
        project.clone(),
        structure.clone(),
        shots.clone(),
        scope,
        profiles,
        reference_sets,
        shot_consistency,
        assets.clone(),
        clock.clone(),
    ));

    let comfy = Arc::new(CountingComfyAdapter {
        counters: counters.clone(),
    });
    let comfy_adapter: Arc<dyn ComfyAdapter> = comfy.clone();
    let comfy_service = Arc::new(ComfyService::from_runtime(Arc::new(ComfyRuntime::new(
        comfy_adapter.clone(),
        ComfyConnectionConfig::default(),
    ))));
    let runtime = Arc::new(CountingRuntimeRepository {
        inner: Arc::new(SqliteWorkflowRuntimeRepository::new(pool.clone())),
        counters: counters.clone(),
    });
    let runtime_state = Arc::new(CountingRuntimeStateRepository {
        inner: Arc::new(SqliteWorkflowRuntimeStateRepository::new(pool.clone())),
        counters: counters.clone(),
    });
    let runtime_trait: Arc<dyn WorkflowRuntimeRepository> = runtime.clone();
    let runtime_state_trait: Arc<dyn WorkflowRuntimeStateRepository> = runtime_state.clone();
    let package_store: Arc<dyn WorkflowPackageStore> = Arc::new(
        FileSystemWorkflowPackageStore::new(workflow_library_root, workflow_staging_root),
    );
    let workflow_run_repository: Arc<dyn WorkflowRunRepository> =
        Arc::new(SqliteWorkflowRunRepository::new(pool.clone()));
    let onboarding = Arc::new(
        WorkflowOnboardingService::new(
            source.clone(),
            comfy_adapter.clone(),
            workflow_library_service.clone(),
            workflow_run_repository,
            package_store.clone(),
            clock.clone(),
        )
        .with_runtime_state(runtime_trait.clone(), runtime_state_trait.clone()),
    );
    let lifecycle = Arc::new(WorkflowLifecycleService::new(
        source,
        workflow_library_service,
        onboarding,
        runtime_trait,
        runtime_state_trait,
        package_store,
        clock.clone(),
    ));

    let raw_queue = Arc::new(SqliteProductionQueueRepository::new(pool.clone()));
    let production_queue_repository: Arc<dyn ProductionQueueRepository> = raw_queue.clone();
    let shot_batch_repository: Arc<dyn ShotBatchRepository> = raw_queue.clone();
    let task_repository: Arc<dyn TaskRepository> =
        Arc::new(SqliteTaskRepository::new(pool.clone()));
    let snapshot_repository: Arc<dyn GenerationSnapshotRepository> =
        Arc::new(SqliteGenerationSnapshotRepository::new(pool.clone()));
    let asset_store: Arc<dyn AssetStore> = Arc::new(FileSystemAssetStore::new());
    let generation_service = Arc::new(GenerationService::new(
        task_repository.clone(),
        snapshot_repository.clone(),
        definitions.clone(),
        comfy_adapter.clone(),
        project.clone(),
        asset_store.clone(),
        assets.clone(),
        clock.clone(),
    ));
    let recovery_service = Arc::new(TaskRecoveryService::new(
        task_repository.clone(),
        snapshot_repository,
        assets.clone(),
        comfy_adapter.clone(),
        project.clone(),
        asset_store,
        clock.clone(),
        Arc::new(NoopTaskUpdateSink),
    ));
    let queue_service = Arc::new(ProductionQueueService::new(
        production_queue_repository.clone(),
        task_repository.clone(),
        definitions.clone(),
        generation_service,
        shot_batch_repository.clone(),
        recovery_service,
        clock.clone(),
    ));
    let diagnostics = Arc::new(DiagnosticsService::new(
        pool.clone(),
        task_repository,
        comfy_service.clone(),
        lifecycle.clone(),
        queue_service.clone(),
        directory.path().join("logs"),
        LoggingStatus {
            available: false,
            retention_days: 7,
        },
    ));
    let preflight = Arc::new(ComfyPreflightService::new(
        comfy_service,
        diagnostics,
        lifecycle.clone(),
    ));
    let readiness = Arc::new(ShotReadinessService::new(
        resolver.clone(),
        preflight,
        lifecycle,
        structure.clone(),
    ));
    let shot_batch_service = Arc::new(ShotBatchService::new(
        shots.clone(),
        shot_batch_repository.clone(),
        Arc::new(SqliteTaskRepository::new(pool.clone())),
        assets,
        definitions.clone(),
        project.clone(),
        clock.clone(),
    ));
    let preparation = Arc::new(ProductionPreparationService::new(
        shot_batch_service,
        shot_batch_repository,
        readiness,
        definitions,
        project,
        clock,
    ));

    counters.reset();
    source_calls.store(0, Ordering::SeqCst);
    comfy.counters.reset();
    PerformanceHarness {
        _directory: directory,
        pool,
        resolver,
        preparation,
        shots,
        counters,
        source_calls,
        shot_ids,
    }
}

fn assert_no_single_shot_bulk_fallbacks(counters: &PerfCounters) {
    assert_eq!(counters.shot_find_single.load(Ordering::SeqCst), 0);
    assert_eq!(counters.profile_find_single.load(Ordering::SeqCst), 0);
    assert_eq!(counters.costume_single.load(Ordering::SeqCst), 0);
    assert_eq!(counters.revision_single.load(Ordering::SeqCst), 0);
    assert_eq!(counters.reference_set_single.load(Ordering::SeqCst), 0);
    assert_eq!(counters.reference_item_single.load(Ordering::SeqCst), 0);
    assert_eq!(counters.shot_profile_single.load(Ordering::SeqCst), 0);
    assert_eq!(counters.shot_reference_single.load(Ordering::SeqCst), 0);
    assert_eq!(counters.asset_single.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dev055_real_500_shots_resolve_in_bulk_and_enforce_the_limit() {
    let harness = performance_harness().await;
    assert_eq!(harness.shot_ids.len(), SHOT_COUNT);
    assert_eq!(
        harness.shot_ids.iter().collect::<HashSet<_>>().len(),
        SHOT_COUNT,
        "fixture must contain 500 different Shot ids"
    );

    let list_started = Instant::now();
    let listed = ShotRepository::list(harness.shots.as_ref(), PROJECT_ID)
        .await
        .expect("real SQLite Shot list should succeed");
    let list_ms = list_started.elapsed().as_millis();
    println!("500_SHOT_LIST_MS={list_ms}");
    assert_eq!(listed.len(), SHOT_COUNT);
    assert_eq!(
        listed
            .iter()
            .map(|data| data.shot.id.as_str())
            .collect::<HashSet<_>>()
            .len(),
        SHOT_COUNT
    );

    let bulk_projection = ShotBulkRepository::list_bulk_data(harness.shots.as_ref(), PROJECT_ID)
        .await
        .expect("real Shot bulk projection should succeed");
    assert_eq!(bulk_projection.len(), SHOT_COUNT);

    harness.counters.reset();
    let resolve_started = Instant::now();
    let contexts = harness
        .resolver
        .resolve_many_draft(PROJECT_ID, &harness.shot_ids, ShotStage::Image)
        .await
        .expect("500 distinct Shots should resolve");
    let resolve_ms = resolve_started.elapsed().as_millis();
    println!("500_CONTEXT_RESOLVE_MS={resolve_ms}");

    assert_eq!(contexts.len(), SHOT_COUNT);
    let resolved_ids = contexts
        .iter()
        .map(|context| context.structure.shot.id.clone())
        .collect::<Vec<_>>();
    assert_eq!(resolved_ids, harness.shot_ids);
    assert_eq!(
        contexts
            .iter()
            .map(|context| context.prompt_context.rendered_text.clone())
            .collect::<HashSet<_>>()
            .len(),
        SHOT_COUNT,
        "the 500 resolved prompts must remain distinct"
    );

    assert_eq!(harness.counters.project_find.load(Ordering::SeqCst), 1);
    assert_eq!(harness.counters.structure_load.load(Ordering::SeqCst), 1);
    assert_eq!(harness.counters.shot_list_bulk.load(Ordering::SeqCst), 1);
    assert_eq!(
        harness.counters.shot_bulk_projection.load(Ordering::SeqCst),
        0
    );
    assert_eq!(
        harness.counters.scope_profile_bulk.load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        harness.counters.scope_reference_bulk.load(Ordering::SeqCst),
        1
    );
    assert_eq!(harness.counters.shot_profile_bulk.load(Ordering::SeqCst), 1);
    assert_eq!(
        harness.counters.shot_reference_bulk.load(Ordering::SeqCst),
        1
    );
    assert_eq!(harness.counters.profile_list_bulk.load(Ordering::SeqCst), 4);
    assert_eq!(harness.counters.costume_bulk.load(Ordering::SeqCst), 1);
    assert_eq!(harness.counters.revision_bulk.load(Ordering::SeqCst), 1);
    assert_eq!(
        harness.counters.reference_set_bulk.load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        harness.counters.reference_item_bulk.load(Ordering::SeqCst),
        1
    );
    assert_eq!(harness.counters.asset_bulk.load(Ordering::SeqCst), 1);
    assert_no_single_shot_bulk_fallbacks(&harness.counters);

    let too_many = (0..SHOT_COUNT + 1)
        .map(|index| format!("shot_dev055_over_{index:03}"))
        .collect::<Vec<_>>();
    let error = harness
        .resolver
        .resolve_many_draft(PROJECT_ID, &too_many, ShotStage::Image)
        .await
        .expect_err("501 shots must be rejected before repository reads");
    assert!(matches!(
        error,
        ShotContextResolverError::ContextBatchLimit {
            limit: CONTEXT_BATCH_LIMIT
        }
    ));

    if list_ms > 1_000 || resolve_ms > 5_000 {
        println!(
            "P1_RISK=500-shot local benchmark exceeded threshold (list_ms={list_ms}, resolve_ms={resolve_ms})"
        );
    } else {
        println!("P1_RISK=none");
    }
    harness.counters.print_context();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dev055_readiness_500_has_one_resolver_runtime_and_definition_bulk_contract() {
    let harness = performance_harness().await;
    harness.counters.reset();
    harness.source_calls.store(0, Ordering::SeqCst);
    let readiness_started = Instant::now();
    let plans = harness
        .preparation
        .plan_many(PROJECT_ID, &harness.shot_ids, ShotStage::Image)
        .await
        .expect("500-shot preparation plan should succeed");
    let readiness_ms = readiness_started.elapsed().as_millis();
    println!("500_READINESS_MS={readiness_ms}");

    assert_eq!(plans.len(), SHOT_COUNT);
    assert_eq!(
        plans
            .iter()
            .map(|plan| plan.shot_id.as_str())
            .collect::<HashSet<_>>()
            .len(),
        SHOT_COUNT
    );
    assert!(plans
        .iter()
        .all(|plan| plan.workflow_version_id.is_some() && plan.recipe_id.is_some()));

    assert_eq!(harness.counters.project_find.load(Ordering::SeqCst), 1);
    assert_eq!(harness.counters.structure_load.load(Ordering::SeqCst), 1);
    assert_eq!(harness.counters.shot_list_bulk.load(Ordering::SeqCst), 2);
    assert_eq!(
        harness.counters.scope_profile_bulk.load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        harness.counters.scope_reference_bulk.load(Ordering::SeqCst),
        1
    );
    assert_eq!(harness.counters.shot_profile_bulk.load(Ordering::SeqCst), 1);
    assert_eq!(
        harness.counters.shot_reference_bulk.load(Ordering::SeqCst),
        1
    );
    assert_eq!(harness.counters.profile_list_bulk.load(Ordering::SeqCst), 4);
    assert_eq!(harness.counters.costume_bulk.load(Ordering::SeqCst), 1);
    assert_eq!(harness.counters.revision_bulk.load(Ordering::SeqCst), 1);
    assert_eq!(
        harness.counters.reference_set_bulk.load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        harness.counters.reference_item_bulk.load(Ordering::SeqCst),
        1
    );
    assert_eq!(harness.counters.asset_bulk.load(Ordering::SeqCst), 1);
    assert_no_single_shot_bulk_fallbacks(&harness.counters);

    assert_eq!(harness.counters.comfy_health.load(Ordering::SeqCst), 1);
    assert_eq!(harness.counters.comfy_object_info.load(Ordering::SeqCst), 2);
    assert_eq!(harness.counters.comfy_submit.load(Ordering::SeqCst), 0);
    assert_eq!(
        harness.source_calls.load(Ordering::SeqCst),
        1,
        "one source refresh belongs to the live preflight"
    );
    assert_eq!(
        harness
            .counters
            .workflow_runtime_list
            .load(Ordering::SeqCst),
        2
    );
    assert_eq!(
        harness
            .counters
            .workflow_runtime_find
            .load(Ordering::SeqCst),
        0
    );
    assert_eq!(
        harness.counters.workflow_state_find.load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        harness.counters.workflow_state_list.load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        harness
            .counters
            .definition_list_available
            .load(Ordering::SeqCst),
        0
    );
    assert_eq!(harness.counters.definition_single.load(Ordering::SeqCst), 0);
    assert_eq!(harness.counters.definition_bulk.load(Ordering::SeqCst), 1);
    println!(
        "DEV055_READINESS_COUNTS resolver_shot_list={} comfy_health={} comfy_object_info={} workflow_source={} runtime_list={} runtime_find={} state_find={} state_list={} definition_single={} definition_bulk={}",
        harness.counters.shot_list_bulk.load(Ordering::SeqCst),
        harness.counters.comfy_health.load(Ordering::SeqCst),
        harness.counters.comfy_object_info.load(Ordering::SeqCst),
        harness.source_calls.load(Ordering::SeqCst),
        harness
            .counters
            .workflow_runtime_list
            .load(Ordering::SeqCst),
        harness
            .counters
            .workflow_runtime_find
            .load(Ordering::SeqCst),
        harness.counters.workflow_state_find.load(Ordering::SeqCst),
        harness.counters.workflow_state_list.load(Ordering::SeqCst),
        harness.counters.definition_single.load(Ordering::SeqCst),
        harness.counters.definition_bulk.load(Ordering::SeqCst),
    );
    harness.counters.print_context();
}

#[derive(Default)]
struct ReviewCounters {
    task_single: AtomicUsize,
    task_bulk: AtomicUsize,
    asset_single: AtomicUsize,
    asset_bulk: AtomicUsize,
    review_find: AtomicUsize,
    review_list_batch: AtomicUsize,
    review_ensure_many: AtomicUsize,
    lineage_single: AtomicUsize,
    lineage_bulk: AtomicUsize,
    snapshot_single: AtomicUsize,
    snapshot_batch: AtomicUsize,
    shot_links_batch: AtomicUsize,
}

impl ReviewCounters {
    fn print(&self) {
        println!(
            "DEV055_REVIEW_100_COUNTS task(single={}, bulk={}) asset(source_single={}, bulk={}) review(find={}, list_batch={}, ensure_many={}) lineage(single={}, bulk={}) snapshot(single={}, batch={}) shot_links_batch={}",
            self.task_single.load(Ordering::SeqCst),
            self.task_bulk.load(Ordering::SeqCst),
            self.asset_single.load(Ordering::SeqCst),
            self.asset_bulk.load(Ordering::SeqCst),
            self.review_find.load(Ordering::SeqCst),
            self.review_list_batch.load(Ordering::SeqCst),
            self.review_ensure_many.load(Ordering::SeqCst),
            self.lineage_single.load(Ordering::SeqCst),
            self.lineage_bulk.load(Ordering::SeqCst),
            self.snapshot_single.load(Ordering::SeqCst),
            self.snapshot_batch.load(Ordering::SeqCst),
            self.shot_links_batch.load(Ordering::SeqCst),
        );
    }
}

struct ReviewTasks {
    counters: Arc<ReviewCounters>,
    tasks: Vec<Task>,
}

#[async_trait]
impl TaskRepository for ReviewTasks {
    async fn create(&self, _: &Task, _: &NewTaskEvent) -> Result<StoredTaskEvent, RepositoryError> {
        Err(unsupported())
    }
    async fn persist_transition(
        &self,
        _: &Task,
        _: &NewTaskEvent,
        _: TaskStatus,
    ) -> Result<StoredTaskEvent, RepositoryError> {
        Err(unsupported())
    }
    async fn persist_runtime_update(
        &self,
        _: &Task,
        _: &NewTaskEvent,
    ) -> Result<StoredTaskEvent, RepositoryError> {
        Err(unsupported())
    }
    async fn find_by_id(&self, _: &TaskId) -> Result<Option<Task>, RepositoryError> {
        self.counters.task_single.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }
    async fn find_many_by_ids(&self, _: &[TaskId]) -> Result<Vec<Task>, RepositoryError> {
        self.counters.task_bulk.fetch_add(1, Ordering::SeqCst);
        Ok(self.tasks.clone())
    }
    async fn find_by_submission_idempotency_key(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Option<Task>, RepositoryError> {
        Err(unsupported())
    }
    async fn list_recent(&self, _: &str, _: u32) -> Result<Vec<Task>, RepositoryError> {
        Err(unsupported())
    }
    async fn list_active(&self) -> Result<Vec<Task>, RepositoryError> {
        Err(unsupported())
    }
    async fn list_events(&self, _: &TaskId) -> Result<Vec<StoredTaskEvent>, RepositoryError> {
        Err(unsupported())
    }
}

struct ReviewAssets {
    counters: Arc<ReviewCounters>,
    assets: Vec<Asset>,
}

#[async_trait]
impl AssetRepository for ReviewAssets {
    async fn insert_many(&self, _: &[Asset]) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn find_by_id(&self, _: &AssetId) -> Result<Option<Asset>, RepositoryError> {
        self.counters.asset_single.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }
    async fn list_by_source_task(&self, _: &TaskId) -> Result<Vec<Asset>, RepositoryError> {
        self.counters.asset_single.fetch_add(1, Ordering::SeqCst);
        Ok(Vec::new())
    }
    async fn list_by_source_tasks(&self, _: &[TaskId]) -> Result<Vec<Asset>, RepositoryError> {
        self.counters.asset_bulk.fetch_add(1, Ordering::SeqCst);
        Ok(self.assets.clone())
    }
    async fn list_recent(&self, _: &str, _: u32) -> Result<Vec<Asset>, RepositoryError> {
        Err(unsupported())
    }
}

struct ReviewRecords {
    counters: Arc<ReviewCounters>,
    records: Arc<Mutex<Vec<ProductionItemReviewRecord>>>,
}

#[async_trait]
impl ProductionItemReviewRepository for ReviewRecords {
    async fn list_for_batch(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Vec<ProductionItemReviewRecord>, RepositoryError> {
        self.counters
            .review_list_batch
            .fetch_add(1, Ordering::SeqCst);
        Ok(self.records.lock().expect("review fixture lock").clone())
    }
    async fn list_for_lineage(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Vec<ProductionItemReviewRecord>, RepositoryError> {
        self.counters.lineage_single.fetch_add(1, Ordering::SeqCst);
        Ok(Vec::new())
    }
    async fn list_for_lineages(
        &self,
        _: &str,
        _: &[String],
    ) -> Result<Vec<ProductionItemReviewRecord>, RepositoryError> {
        self.counters.lineage_bulk.fetch_add(1, Ordering::SeqCst);
        Ok(self.records.lock().expect("review fixture lock").clone())
    }
    async fn find_for_item(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Option<ProductionItemReviewRecord>, RepositoryError> {
        self.counters.review_find.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }
    async fn ensure_for_item(
        &self,
        record: &ProductionItemReviewRecord,
    ) -> Result<ProductionItemReviewRecord, RepositoryError> {
        Ok(record.clone())
    }
    async fn ensure_for_items(
        &self,
        records: &[ProductionItemReviewRecord],
    ) -> Result<Vec<ProductionItemReviewRecord>, RepositoryError> {
        self.counters
            .review_ensure_many
            .fetch_add(1, Ordering::SeqCst);
        Ok(records.to_vec())
    }
    async fn insert(&self, _: &ProductionItemReviewRecord) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn set_status(
        &self,
        _: &str,
        _: &str,
        _: ProductionReviewStatus,
        _: DateTime<Utc>,
    ) -> Result<ProductionItemReviewRecord, RepositoryError> {
        Err(unsupported())
    }
    async fn set_note(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: DateTime<Utc>,
    ) -> Result<ProductionItemReviewRecord, RepositoryError> {
        Err(unsupported())
    }
}

struct ReviewShotBatch {
    counters: Arc<ReviewCounters>,
    links: Vec<ai_studio_lib::application::ports::ProductionBatchShotLink>,
}

#[async_trait]
impl ShotBatchRepository for ReviewShotBatch {
    async fn insert_batch_with_bindings(
        &self,
        _: &ProductionBatch,
        _: &[ProductionBatchItem],
        _: &[ai_studio_lib::application::ports::ShotBatchBinding],
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn list_preparation_snapshots_for_batch(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Vec<PreparationSnapshotRecord>, RepositoryError> {
        self.counters.snapshot_batch.fetch_add(1, Ordering::SeqCst);
        Ok(Vec::new())
    }
    async fn find_preparation_snapshot(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Option<PreparationSnapshotRecord>, RepositoryError> {
        self.counters.snapshot_single.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }
    async fn list_shot_links_for_batch(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Vec<ai_studio_lib::application::ports::ProductionBatchShotLink>, RepositoryError>
    {
        self.counters
            .shot_links_batch
            .fetch_add(1, Ordering::SeqCst);
        Ok(self.links.clone())
    }
    async fn bind_shot_item_task(
        &self,
        _: &str,
        _: &str,
        _: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn append_requeue_item_with_binding(
        &self,
        _: &ProductionBatchItem,
        _: &str,
        _: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn append_requeue_items_with_bindings(
        &self,
        _: &[ProductionBatchItem],
        _: DateTime<Utc>,
    ) -> Result<(Vec<String>, Vec<String>), RepositoryError> {
        Err(unsupported())
    }
    async fn has_active_shot_binding(&self, _: &str, _: &str) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn list_active_shot_bindings(
        &self,
        _: &str,
        _: ShotStage,
        _: &[String],
    ) -> Result<Vec<ai_studio_lib::application::ports::ActiveShotBatchBinding>, RepositoryError>
    {
        Err(unsupported())
    }
}

struct ReviewQueue {
    detail: ProductionBatchDetail,
}

#[async_trait]
impl ProductionQueueRepository for ReviewQueue {
    async fn insert(
        &self,
        _: &ProductionBatch,
        _: &[ProductionBatchItem],
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn list(&self, _: &str) -> Result<Vec<ProductionBatch>, RepositoryError> {
        Err(unsupported())
    }
    async fn list_running(&self) -> Result<Vec<ProductionBatch>, RepositoryError> {
        Err(unsupported())
    }
    async fn list_active_items(&self) -> Result<Vec<ActiveProductionItem>, RepositoryError> {
        Err(unsupported())
    }
    async fn find_detail(
        &self,
        _: &str,
        _: &ProductionBatchId,
    ) -> Result<Option<ProductionBatchDetail>, RepositoryError> {
        Ok(Some(self.detail.clone()))
    }
    async fn set_batch_status(
        &self,
        _: &str,
        _: &ProductionBatchId,
        _: ProductionBatchStatus,
        _: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn set_item_dispatching(
        &self,
        _: &ProductionBatchItemId,
        _: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn cancel_pending_items(
        &self,
        _: &str,
        _: &ProductionBatchId,
        _: DateTime<Utc>,
    ) -> Result<u64, RepositoryError> {
        Err(unsupported())
    }
    async fn link_item_task(
        &self,
        _: &ProductionBatchItemId,
        _: &str,
        _: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn finish_item(
        &self,
        _: &ProductionBatchItemId,
        _: ProductionBatchItemStatus,
        _: Option<&str>,
        _: Option<&str>,
        _: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn set_item_skipped(
        &self,
        _: &ProductionBatchItemId,
        _: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn append_requeue_item(
        &self,
        _: &ProductionBatchItem,
        _: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn set_archived_at(
        &self,
        _: &str,
        _: &ProductionBatchId,
        _: Option<DateTime<Utc>>,
        _: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn delete_batch(&self, _: &str, _: &ProductionBatchId) -> Result<bool, RepositoryError> {
        Err(unsupported())
    }
    async fn recover_uncertain_dispatches(
        &self,
        _: DateTime<Utc>,
    ) -> Result<Vec<ProductionBatchId>, RepositoryError> {
        Err(unsupported())
    }
}

struct ReviewDefinitions;

#[async_trait]
impl GenerationDefinitionRepository for ReviewDefinitions {
    async fn find(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Option<GenerationDefinition>, RepositoryError> {
        Err(unsupported())
    }
    async fn list_available(&self) -> Result<Vec<AvailableGenerationDefinition>, RepositoryError> {
        Err(unsupported())
    }
}

struct ReviewSnapshots;

#[async_trait]
impl GenerationSnapshotRepository for ReviewSnapshots {
    async fn insert(&self, _: &GenerationSnapshot) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn find_by_task_id(
        &self,
        _: &TaskId,
    ) -> Result<Option<GenerationSnapshot>, RepositoryError> {
        Err(unsupported())
    }
}

struct ReviewProjects;

#[async_trait]
impl ProjectRepository for ReviewProjects {
    async fn list(&self) -> Result<Vec<ProjectRecord>, RepositoryError> {
        Err(unsupported())
    }
    async fn find_by_id(&self, _: &str) -> Result<Option<ProjectRecord>, RepositoryError> {
        Err(unsupported())
    }
    async fn insert(&self, _: &ProjectRecord) -> Result<(), RepositoryError> {
        Err(unsupported())
    }
    async fn update_metadata(
        &self,
        _: &str,
        _: &str,
        _: Option<&str>,
        _: DateTime<Utc>,
    ) -> Result<Option<ProjectRecord>, RepositoryError> {
        Err(unsupported())
    }
    async fn get_storage_root(&self, _: &str) -> Result<Option<PathBuf>, RepositoryError> {
        Err(unsupported())
    }
    async fn ensure_default_project(
        &self,
        _: &str,
        _: &str,
        _: &PathBuf,
        _: DateTime<Utc>,
    ) -> Result<ProjectRecord, RepositoryError> {
        Err(unsupported())
    }
}

fn review_productivity_fixture() -> (
    ProductionItemReviewService,
    Arc<ReviewCounters>,
    String,
    String,
) {
    let counters = Arc::new(ReviewCounters::default());
    let project_id = "prj_dev055_review".to_owned();
    let batch_id = "pbt_dev055_review".to_owned();
    let now = FixedClock.now();
    let batch = ProductionBatch {
        id: ProductionBatchId::parse(batch_id.clone()).expect("review batch id"),
        project_id: project_id.clone(),
        name: "DEV-055 review 100 item batch".to_owned(),
        status: ProductionBatchStatus::Completed,
        continue_on_failure: true,
        archived_at: None,
        created_at: now,
        updated_at: now,
    };
    let mut items = Vec::with_capacity(100);
    let mut tasks = Vec::with_capacity(100);
    let mut assets = Vec::with_capacity(100);
    let mut reviews = Vec::with_capacity(100);
    let mut links = Vec::with_capacity(100);
    for ordinal in 0..100_u32 {
        let item_id = format!("pbi_dev055_review_{ordinal:03}");
        let task_id = format!("tsk_dev055_review_{ordinal:03}");
        let asset_id = format!("ast_dev055_review_{ordinal:03}");
        let item_id_domain = ProductionBatchItemId::parse(item_id.clone()).expect("review item id");
        let task_id_domain = TaskId::parse(task_id.clone()).expect("review task id");
        items.push(ProductionBatchItem {
            id: item_id_domain.clone(),
            batch_id: batch.id.clone(),
            ordinal,
            workflow_version_id: "wv_dev055_review".to_owned(),
            recipe_id: "rcp_dev055_review".to_owned(),
            values_json: json!({
                "prompt": {"type": "string", "value": format!("review prompt {ordinal:03}")},
                "width": {"type": "integer", "value": 1024},
                "height": {"type": "integer", "value": 576}
            }),
            status: ProductionBatchItemStatus::Succeeded,
            task_id: Some(task_id.clone()),
            retry_of_item_id: None,
            error_code: None,
            error_message: None,
            created_at: now,
            updated_at: now,
        });
        let mut task = Task::new(
            &project_id,
            "workflow_dev055_review",
            "wv_dev055_review",
            "rcp_dev055_review",
            now,
        );
        task.id = task_id_domain.clone();
        task.status = TaskStatus::Succeeded;
        task.finished_at = Some(now);
        tasks.push(task);
        assets.push(Asset {
            id: AssetId::parse(asset_id.clone()).expect("review asset id"),
            project_id: project_id.clone(),
            asset_type: AssetType::Image,
            category: "generated_image".to_owned(),
            name: format!("review-candidate-{ordinal}"),
            original_name: format!("review-candidate-{ordinal}"),
            storage_path: format!("generated/{asset_id}.png"),
            thumbnail_path: Some(format!("thumbs/{asset_id}.jpg")),
            sha256: format!("sha-dev055-{ordinal}"),
            mime_type: "image/png".to_owned(),
            width: 1024,
            height: 576,
            duration_ms: None,
            file_size: 1,
            source_task_id: Some(task_id_domain),
            metadata_json: json!({}),
            created_at: now,
            updated_at: now,
        });
        reviews.push(ProductionItemReviewRecord {
            id: format!("pri_dev055_review_{ordinal:03}"),
            project_id: project_id.clone(),
            production_batch_id: batch_id.clone(),
            production_batch_item_id: item_id.clone(),
            task_id: Some(task_id),
            result_asset_id: Some(asset_id.clone()),
            review_status: ProductionReviewStatus::Unreviewed,
            review_note: String::new(),
            version: 1,
            lineage_key: format!("lineage_dev055_review_{ordinal:03}"),
            parent_batch_id: None,
            parent_item_id: None,
            created_at: now,
            updated_at: now,
        });
        links.push(ai_studio_lib::application::ports::ProductionBatchShotLink {
            production_batch_item_id: item_id,
            shot_id: format!("shot_dev055_review_{ordinal:03}"),
            stage: ShotStage::Image,
            selected_image_asset_id: Some(asset_id),
            selected_video_asset_id: None,
        });
    }

    let queue_repository: Arc<dyn ProductionQueueRepository> = Arc::new(ReviewQueue {
        detail: ProductionBatchDetail { batch, items },
    });
    let task_repository: Arc<dyn TaskRepository> = Arc::new(ReviewTasks {
        counters: counters.clone(),
        tasks,
    });
    let asset_repository: Arc<dyn AssetRepository> = Arc::new(ReviewAssets {
        counters: counters.clone(),
        assets,
    });
    let review_repository: Arc<dyn ProductionItemReviewRepository> = Arc::new(ReviewRecords {
        counters: counters.clone(),
        records: Arc::new(Mutex::new(reviews)),
    });
    let shot_batch_repository: Arc<dyn ShotBatchRepository> = Arc::new(ReviewShotBatch {
        counters: counters.clone(),
        links,
    });
    let definition_repository: Arc<dyn GenerationDefinitionRepository> =
        Arc::new(ReviewDefinitions);
    let snapshot_repository: Arc<dyn GenerationSnapshotRepository> = Arc::new(ReviewSnapshots);
    let project_repository: Arc<dyn ProjectRepository> = Arc::new(ReviewProjects);
    let comfy: Arc<dyn ComfyAdapter> = Arc::new(CountingComfyAdapter {
        counters: Arc::new(PerfCounters::default()),
    });
    let asset_store: Arc<dyn AssetStore> = Arc::new(FileSystemAssetStore::new());
    let clock: Arc<dyn Clock> = Arc::new(FixedClock);
    let generation_service = Arc::new(GenerationService::new(
        task_repository.clone(),
        snapshot_repository.clone(),
        definition_repository.clone(),
        comfy.clone(),
        project_repository.clone(),
        asset_store.clone(),
        asset_repository.clone(),
        clock.clone(),
    ));
    let recovery_service = Arc::new(TaskRecoveryService::new(
        task_repository.clone(),
        snapshot_repository,
        asset_repository.clone(),
        comfy,
        project_repository,
        asset_store,
        clock.clone(),
        Arc::new(NoopTaskUpdateSink),
    ));
    let queue_service = Arc::new(ProductionQueueService::new(
        queue_repository.clone(),
        task_repository.clone(),
        definition_repository,
        generation_service,
        shot_batch_repository.clone(),
        recovery_service,
        clock.clone(),
    ));
    let service = ProductionItemReviewService::new_with_shot_batch_repository(
        review_repository,
        queue_repository,
        queue_service,
        task_repository,
        asset_repository,
        shot_batch_repository,
        clock,
    );
    (service, counters, project_id, batch_id)
}

#[tokio::test]
async fn dev055_review_100_uses_bulk_task_asset_lineage_and_snapshot_reads() {
    let (service, counters, project_id, batch_id) = review_productivity_fixture();
    let view = service
        .get_productivity_view(&project_id, &batch_id)
        .await
        .expect("100-item review productivity view should load");
    assert_eq!(view.total, 100);
    assert_eq!(view.items.len(), 100);
    assert_eq!(view.success_count, 100);
    assert_eq!(view.unreviewed_count, 100);
    assert!(view.items.iter().all(|item| item.reviewable));
    assert!(view
        .items
        .iter()
        .all(|item| item.frozen_context.snapshot_available == false));
    assert_eq!(counters.task_single.load(Ordering::SeqCst), 0);
    assert_eq!(counters.task_bulk.load(Ordering::SeqCst), 1);
    assert_eq!(counters.asset_single.load(Ordering::SeqCst), 0);
    assert_eq!(counters.asset_bulk.load(Ordering::SeqCst), 1);
    assert_eq!(counters.review_find.load(Ordering::SeqCst), 0);
    assert_eq!(counters.review_list_batch.load(Ordering::SeqCst), 1);
    assert_eq!(counters.review_ensure_many.load(Ordering::SeqCst), 0);
    assert_eq!(counters.lineage_single.load(Ordering::SeqCst), 0);
    assert_eq!(counters.lineage_bulk.load(Ordering::SeqCst), 1);
    assert_eq!(counters.snapshot_single.load(Ordering::SeqCst), 0);
    assert_eq!(counters.snapshot_batch.load(Ordering::SeqCst), 1);
    assert_eq!(counters.shot_links_batch.load(Ordering::SeqCst), 1);
    counters.print();
}

async fn seed_audit_snapshot_identity_fixture(pool: &SqlitePool, shot_ids: &[String]) {
    let workflow_version_id =
        sqlx::query_scalar::<_, String>("SELECT id FROM workflow_versions ORDER BY id LIMIT 1")
            .fetch_one(pool)
            .await
            .expect("audit fixture workflow version should exist");
    let recipe_id = sqlx::query_scalar::<_, String>("SELECT id FROM recipes ORDER BY id LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("audit fixture recipe should exist");
    let mut transaction = pool
        .begin()
        .await
        .expect("DEV-055 audit fixture transaction should begin");
    sqlx::query(
        "INSERT INTO production_batches
         (id, project_id, name, status, continue_on_failure, archived_at, created_at, updated_at)
         VALUES ('pbt_dev055_audit', ?, 'DEV-055 audit identity batch', 'COMPLETED', 1,
                 NULL, ?, ?)",
    )
    .bind(PROJECT_ID)
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(&mut *transaction)
    .await
    .expect("audit batch fixture should insert");

    for (ordinal, shot_id) in shot_ids.iter().enumerate() {
        let item_id = format!("pbi_dev055_audit_{ordinal:03}");
        let snapshot_id = format!("pps_dev055_audit_{ordinal:03}");
        sqlx::query(
            "INSERT INTO production_batch_items
             (id, batch_id, ordinal, workflow_version_id, recipe_id, values_json, status,
              task_id, error_code, error_message, retry_of_item_id, created_at, updated_at)
             VALUES (?, 'pbt_dev055_audit', ?, ?, ?, '{}', 'SUCCEEDED',
                     NULL, NULL, NULL, NULL, ?, ?)",
        )
        .bind(&item_id)
        .bind(i64::try_from(ordinal).expect("audit item ordinal fits"))
        .bind(&workflow_version_id)
        .bind(&recipe_id)
        .bind(CREATED_AT)
        .bind(CREATED_AT)
        .execute(&mut *transaction)
        .await
        .expect("audit item fixture should insert");
        sqlx::query(
            "INSERT INTO production_preparation_snapshots
             (id, project_id, shot_id, stage, context_hash, production_batch_id,
              production_batch_item_id, snapshot_json, created_at)
             VALUES (?, ?, ?, 'image', ?, 'pbt_dev055_audit', ?, ?, ?)",
        )
        .bind(&snapshot_id)
        .bind(PROJECT_ID)
        .bind(shot_id)
        .bind(format!("context-dev055-{ordinal:03}"))
        .bind(&item_id)
        .bind(format!("not-json-payload-{ordinal:03}"))
        .bind(CREATED_AT)
        .execute(&mut *transaction)
        .await
        .expect("audit snapshot identity fixture should insert");
    }
    transaction
        .commit()
        .await
        .expect("DEV-055 audit fixture transaction should commit");
}

fn source_section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_at = source
        .find(start)
        .unwrap_or_else(|| panic!("missing source section start: {start}"));
    let rest = &source[start_at..];
    let end_at = rest
        .find(end)
        .unwrap_or_else(|| panic!("missing source section end: {end}"));
    &rest[..end_at]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dev055_command_center_and_audit_keep_500_shots_identity_only() {
    let harness = performance_harness().await;
    seed_audit_snapshot_identity_fixture(&harness.pool, &harness.shot_ids).await;

    let command_center = ai_studio_lib::application::project_command_center_service::
        ProjectCommandCenterService::new(harness.pool.clone());
    let center = command_center
        .get(PROJECT_ID)
        .await
        .expect("command center should load 500-shot project");
    assert_eq!(center.shots.total, SHOT_COUNT);
    assert_eq!(center.preparation.snapshot_count, SHOT_COUNT);

    let audit = ai_studio_lib::application::production_audit_service::ProductionAuditService::new(
        harness.pool.clone(),
    );
    let summary = audit
        .summary(PROJECT_ID)
        .await
        .expect("audit summary should load snapshot identities");
    assert_eq!(summary.assets, 1);
    assert_eq!(summary.unassigned_shots, SHOT_COUNT as u64);
    let activity = audit
        .recent_activity(PROJECT_ID, Some(20))
        .await
        .expect("audit activity should load snapshot identities");
    assert_eq!(activity.len(), 20);
    let lineage = audit
        .lineage(PROJECT_ID, "SHOT", &harness.shot_ids[250])
        .await
        .expect("shot lineage should load snapshot identities");
    assert_eq!(lineage.root_type, "SHOT");
    assert!(lineage
        .nodes
        .iter()
        .any(|node| node.shot_id.as_deref() == Some(harness.shot_ids[250].as_str())));
    let integrity = audit
        .integrity(PROJECT_ID)
        .await
        .expect("audit integrity should load snapshot identities");
    assert_eq!(integrity.project_id, PROJECT_ID);

    let command_center_source =
        include_str!("../src/application/project_command_center_service.rs");
    for forbidden in [
        "ShotContextResolver",
        "resolve_many_draft",
        "find_preparation_snapshot",
    ] {
        assert!(
            !command_center_source.contains(forbidden),
            "Command Center must not contain {forbidden}"
        );
    }
    let audit_source = include_str!("../src/application/production_audit_service.rs");
    let load_graph = source_section(
        audit_source,
        "async fn load_graph",
        "\n}\n\n#[derive(Debug, FromRow)]\nstruct RunRow",
    );
    assert!(load_graph.contains("production_preparation_snapshots"));
    assert!(
        !load_graph.contains("snapshot_json"),
        "Audit graph must not select or parse the 500 snapshot payloads"
    );
    assert!(
        audit_source.contains("pub async fn snapshot_detail"),
        "payload access must remain an explicit single-item operation"
    );
    println!(
        "DEV055_COMMAND_CENTER_AUDIT=500_shots_identity_set_based_no_resolver_no_payload_find"
    );
}
