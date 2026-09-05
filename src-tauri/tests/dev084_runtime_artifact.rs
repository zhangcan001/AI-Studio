use ai_studio_lib::application::{
    generation_service::{
        CreateGenerationRequest, GenerationService, GenerationServiceError,
        WORKFLOW_UNAVAILABLE_FOR_NEW_GENERATION,
    },
    ports::{
        AssetRepository, AssetStore, Clock, ComfyAdapter, ComfyAdapterError,
        ComfyEventSubscription, ComfyHistory, ComfyOutputData, ComfyOutputFile,
        GenerationDefinitionRepository, GenerationSnapshotRepository, ProjectRepository,
        ProjectWorkflowBindingRecord, ProjectWorkflowBindingRepository, PromptSubmission,
        RepositoryError, SystemStats, TaskRepository, WorkflowLibrarySource, WorkflowPackageBytes,
        WorkflowPackageQuarantineResult, WorkflowPackageStore, WorkflowPackageStoreError,
        WorkflowPurgeOperationEntry, WorkflowPurgeOperationRecord, WorkflowRegistryRepository,
        WorkflowRunRepository, WorkflowRuntimeArtifactRecord, WorkflowRuntimeArtifactRepository,
        WorkflowRuntimeRepository, WorkflowRuntimeStateRepository,
    },
    workflow_library_service::WorkflowLibraryService,
    workflow_lifecycle_service::WorkflowLifecycleService,
    workflow_onboarding_service::{
        CapabilityCheckView, CapabilityState, WorkflowOnboardingService,
    },
    workflow_registry_service::WorkflowRegistryService,
    workflow_workspace_query_service::WorkflowWorkspaceQueryService,
};
use ai_studio_lib::infrastructure::database::{
    initialize, SqliteAssetRepository, SqliteGenerationDefinitionRepository,
    SqliteGenerationSnapshotRepository, SqliteProjectRepository,
    SqliteProjectWorkflowBindingRepository, SqliteTaskRepository, SqliteWorkflowLibraryRepository,
    SqliteWorkflowRegistryRepository, SqliteWorkflowRuntimeArtifactRepository,
    SqliteWorkflowRuntimeRepository, SqliteWorkflowRuntimeStateRepository,
};
use ai_studio_lib::infrastructure::{
    filesystem::{
        FileSystemAssetStore, FileSystemWorkflowLibrarySource, FileSystemWorkflowPackageStore,
    },
    time::SystemClock,
};
use chrono::Utc;
use serde_json::Value;
use sqlx::{sqlite::SqliteConnectOptions, sqlite::SqlitePoolOptions, SqlitePool};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use tempfile::tempdir;

const MIGRATIONS_THROUGH_028: [&str; 28] = [
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/001_initial.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/002_browse_indexes.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/003_presets.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/004_video_outputs.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/005_workflow_runtime_state.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/006_production_queue.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/007_production_queue_operations.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/008_organization.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/009_prompt_library.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/010_shot_production.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/011_asset_video_prompt.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/012_production_item_review.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/013_workflow_archive_and_package_metadata.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/014_workflow_benchmark.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/015_runtime_provenance.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/016_generation_telemetry.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/017_submission_idempotency.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/018_production_orchestrator.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/019_shot_stage_prompts.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/020_reference_anchors.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/021_production_structure.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/022_consistency_profiles_and_reference_sets.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/023_consistency_scope_bindings.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/024_production_preparation_snapshots.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/025_script_draft_foundation.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/026_production_package_batch_bindings.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/027_project_workflow_bindings.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/028_workflow_registry_v2.sql"
    )),
];

fn artifact(
    id: &str,
    recipe_id: &str,
    package_name: &str,
    workflow_sha256: &str,
    recipe_sha256: &str,
) -> WorkflowRuntimeArtifactRecord {
    WorkflowRuntimeArtifactRecord {
        id: id.to_owned(),
        workflow_version_id: "dev084-version".to_owned(),
        recipe_id: recipe_id.to_owned(),
        package_name: package_name.to_owned(),
        source_kind: "USER".to_owned(),
        package_source_path: Some(format!("C:/{package_name}")),
        workflow_sha256: workflow_sha256.to_owned(),
        recipe_sha256: recipe_sha256.to_owned(),
        created_at: Utc::now(),
    }
}

async fn seed_exact_recipe_database() -> (tempfile::TempDir, SqlitePool) {
    let directory = tempdir().expect("temporary directory should exist");
    let pool = initialize(&directory.path().join("app.db"))
        .await
        .expect("database should initialize");
    sqlx::query(
        "INSERT INTO workflows
         (id, name, category, mode, current_version_id, created_at, updated_at)
         VALUES ('dev084-workflow', 'DEV-084', 'video', 'text_to_video', NULL, ?, ?)",
    )
    .bind("2026-01-01T00:00:00Z")
    .bind("2026-01-01T00:00:00Z")
    .execute(&pool)
    .await
    .expect("workflow fixture should insert");
    sqlx::query(
        "INSERT INTO workflow_versions
         (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
         VALUES ('dev084-version', 'dev084-workflow', '1.0.0', '{}', 'workflow-sha', ?)",
    )
    .bind("2026-01-01T00:00:00Z")
    .execute(&pool)
    .await
    .expect("workflow version fixture should insert");
    for (recipe_id, recipe_sha256) in [
        ("dev084-recipe-a", "recipe-sha-a"),
        ("dev084-recipe-b", "recipe-sha-b"),
    ] {
        sqlx::query(
            "INSERT INTO recipes
             (id, workflow_version_id, version, schema_version, recipe_yaml,
              recipe_sha256, created_at)
             VALUES (?, 'dev084-version', '1.0.0', 1, 'schema_version: 1', ?, ?)",
        )
        .bind(recipe_id)
        .bind(recipe_sha256)
        .bind("2026-01-01T00:00:00Z")
        .execute(&pool)
        .await
        .expect("recipe fixture should insert");
    }
    (directory, pool)
}

async fn legacy_pool() -> (tempfile::TempDir, SqlitePool) {
    let directory = tempdir().expect("temporary directory should exist");
    let options = SqliteConnectOptions::new()
        .filename(directory.path().join("legacy.db"))
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("legacy database should connect");
    // Keep 028 unapplied so the fixture can insert legacy rows before its
    // provisional artifact backfill runs.
    for migration in MIGRATIONS_THROUGH_028.iter().take(27).copied() {
        sqlx::raw_sql(migration)
            .execute(&pool)
            .await
            .expect("legacy migration should apply");
    }
    (directory, pool)
}

struct RegistryPurgeHarness {
    _directory: tempfile::TempDir,
    library_root: PathBuf,
    package_name: String,
    workflow_id: String,
    pool: SqlitePool,
    registry: Arc<WorkflowRegistryService>,
}

async fn registry_purge_harness(
    source_package_name: &str,
    package_name: &str,
    workflow_id: &str,
) -> RegistryPurgeHarness {
    let directory = tempdir().expect("temporary workflow data directory should exist");
    let library_root = directory.path().join("workflow-library");
    let staging_root = directory.path().join("workflow-staging");
    let package_root = library_root.join(package_name);
    fs::create_dir_all(&package_root).expect("workflow package directory should exist");
    fs::create_dir_all(&staging_root).expect("workflow staging directory should exist");
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("runtime_packages")
        .join(source_package_name);
    for file_name in ["manifest.yaml", "recipe.yaml", "workflow_api.json"] {
        fs::copy(source_root.join(file_name), package_root.join(file_name))
            .expect("fixture package file should copy");
    }

    let pool = initialize(&directory.path().join("app.db"))
        .await
        .expect("database should initialize");
    sqlx::query(
        "INSERT INTO projects
         (id, name, description, root_path, created_at, updated_at)
         VALUES ('prj_default', 'DEV-084 Test Project', NULL, ?, ?, ?)",
    )
    .bind(
        directory
            .path()
            .join("project")
            .to_string_lossy()
            .to_string(),
    )
    .bind("2026-01-01T00:00:00Z")
    .bind("2026-01-01T00:00:00Z")
    .execute(&pool)
    .await
    .expect("default project fixture should insert");
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let library_service = WorkflowLibraryService::new(
        Arc::new(FileSystemWorkflowLibrarySource::new(library_root.clone())),
        Arc::new(SqliteWorkflowLibraryRepository::new(pool.clone())),
        clock.clone(),
    );
    let report = library_service
        .sync()
        .await
        .expect("runtime package sync should succeed");
    assert_eq!(report.packages_found, 1);
    assert_eq!(report.valid, 1);
    assert_eq!(report.invalid, 0);

    let runtime_repository: Arc<dyn WorkflowRuntimeRepository> =
        Arc::new(SqliteWorkflowRuntimeRepository::new(pool.clone()));
    let state_repository: Arc<dyn WorkflowRuntimeStateRepository> =
        Arc::new(SqliteWorkflowRuntimeStateRepository::new(pool.clone()));
    let binding_repository: Arc<dyn ProjectWorkflowBindingRepository> =
        Arc::new(SqliteProjectWorkflowBindingRepository::new(pool.clone()));
    let registry_repository = Arc::new(SqliteWorkflowRegistryRepository::new(pool.clone()));
    let artifact_repository = Arc::new(SqliteWorkflowRuntimeArtifactRepository::new(pool.clone()));
    let package_store = Arc::new(FileSystemWorkflowPackageStore::new(
        library_root.clone(),
        staging_root,
    ));
    let registry = Arc::new(
        WorkflowRegistryService::new(
            runtime_repository,
            state_repository,
            binding_repository,
            clock,
        )
        .with_registry_repository(registry_repository)
        .with_runtime_artifact_repository(artifact_repository)
        .with_package_store(package_store),
    );

    RegistryPurgeHarness {
        _directory: directory,
        library_root,
        package_name: package_name.to_owned(),
        workflow_id: workflow_id.to_owned(),
        pool,
        registry,
    }
}

fn registry_service_with_package_store(
    harness: &RegistryPurgeHarness,
    package_store: Arc<dyn WorkflowPackageStore>,
) -> Arc<WorkflowRegistryService> {
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let runtime_repository: Arc<dyn WorkflowRuntimeRepository> =
        Arc::new(SqliteWorkflowRuntimeRepository::new(harness.pool.clone()));
    let state_repository: Arc<dyn WorkflowRuntimeStateRepository> = Arc::new(
        SqliteWorkflowRuntimeStateRepository::new(harness.pool.clone()),
    );
    let binding_repository: Arc<dyn ProjectWorkflowBindingRepository> = Arc::new(
        SqliteProjectWorkflowBindingRepository::new(harness.pool.clone()),
    );
    let registry_repository = Arc::new(SqliteWorkflowRegistryRepository::new(harness.pool.clone()));
    let artifact_repository = Arc::new(SqliteWorkflowRuntimeArtifactRepository::new(
        harness.pool.clone(),
    ));
    Arc::new(
        WorkflowRegistryService::new(
            runtime_repository,
            state_repository,
            binding_repository,
            clock,
        )
        .with_registry_repository(registry_repository)
        .with_runtime_artifact_repository(artifact_repository)
        .with_package_store(package_store),
    )
}

fn filesystem_package_store(harness: &RegistryPurgeHarness) -> FileSystemWorkflowPackageStore {
    FileSystemWorkflowPackageStore::new(
        harness.library_root.clone(),
        harness._directory.path().join("workflow-staging"),
    )
}

async fn prepare_and_quarantine(
    harness: &RegistryPurgeHarness,
    operation_id: &str,
    package_names: &[&str],
    moved_package_names: &[&str],
) {
    let store = filesystem_package_store(harness);
    store
        .prepare_purge_operation(&WorkflowPurgeOperationRecord {
            schema_version: 1,
            operation_id: operation_id.to_owned(),
            workflow_id: harness.workflow_id.clone(),
            package_names: package_names
                .iter()
                .map(|package_name| (*package_name).to_owned())
                .collect(),
            created_at: Utc::now().to_rfc3339(),
        })
        .await
        .expect("purge journal should be durable before package movement");
    for package_name in moved_package_names {
        assert_eq!(
            store
                .quarantine_published(operation_id, package_name)
                .await
                .expect("package quarantine should succeed"),
            WorkflowPackageQuarantineResult::Quarantined
        );
    }
}

struct CleanupFailingPackageStore {
    inner: FileSystemWorkflowPackageStore,
}

#[async_trait::async_trait]
impl WorkflowPackageStore for CleanupFailingPackageStore {
    async fn stage(
        &self,
        staging_id: &str,
        package: &WorkflowPackageBytes,
    ) -> Result<(), WorkflowPackageStoreError> {
        self.inner.stage(staging_id, package).await
    }

    async fn read_staging(
        &self,
        staging_id: &str,
    ) -> Result<WorkflowPackageBytes, WorkflowPackageStoreError> {
        self.inner.read_staging(staging_id).await
    }

    async fn publish_atomic(
        &self,
        staging_id: &str,
        package_name: &str,
    ) -> Result<(), WorkflowPackageStoreError> {
        self.inner.publish_atomic(staging_id, package_name).await
    }

    async fn remove_staging(&self, staging_id: &str) -> Result<(), WorkflowPackageStoreError> {
        self.inner.remove_staging(staging_id).await
    }

    async fn remove_published(&self, package_name: &str) -> Result<(), WorkflowPackageStoreError> {
        self.inner.remove_published(package_name).await
    }

    async fn prepare_purge_operation(
        &self,
        operation: &WorkflowPurgeOperationRecord,
    ) -> Result<(), WorkflowPackageStoreError> {
        self.inner.prepare_purge_operation(operation).await
    }

    async fn list_purge_operations(
        &self,
    ) -> Result<Vec<WorkflowPurgeOperationEntry>, WorkflowPackageStoreError> {
        self.inner.list_purge_operations().await
    }

    async fn list_quarantined_packages(
        &self,
        operation_id: &str,
    ) -> Result<Vec<String>, WorkflowPackageStoreError> {
        self.inner.list_quarantined_packages(operation_id).await
    }

    async fn read_quarantined(
        &self,
        operation_id: &str,
        package_name: &str,
    ) -> Result<WorkflowPackageBytes, WorkflowPackageStoreError> {
        self.inner
            .read_quarantined(operation_id, package_name)
            .await
    }

    async fn quarantine_published(
        &self,
        operation_id: &str,
        package_name: &str,
    ) -> Result<WorkflowPackageQuarantineResult, WorkflowPackageStoreError> {
        self.inner
            .quarantine_published(operation_id, package_name)
            .await
    }

    async fn restore_quarantined(
        &self,
        operation_id: &str,
        package_name: &str,
    ) -> Result<(), WorkflowPackageStoreError> {
        self.inner
            .restore_quarantined(operation_id, package_name)
            .await
    }

    async fn remove_quarantine(
        &self,
        _operation_id: &str,
    ) -> Result<(), WorkflowPackageStoreError> {
        Err(WorkflowPackageStoreError {
            message: "test quarantine cleanup failure".to_owned(),
        })
    }

    async fn list_published(&self) -> Result<Vec<String>, WorkflowPackageStoreError> {
        self.inner.list_published().await
    }

    async fn read_runtime(
        &self,
        package_name: &str,
    ) -> Result<WorkflowPackageBytes, WorkflowPackageStoreError> {
        self.inner.read_runtime(package_name).await
    }

    async fn list_staging_ids(&self) -> Result<Vec<String>, WorkflowPackageStoreError> {
        self.inner.list_staging_ids().await
    }
}

struct NonSubmittingComfy;

#[async_trait::async_trait]
impl ComfyAdapter for NonSubmittingComfy {
    async fn health_check(
        &self,
    ) -> Result<ai_studio_lib::application::ports::ComfyHealth, ComfyAdapterError> {
        Err(ComfyAdapterError::Offline(
            "DEV-084 admission fixture".to_owned(),
        ))
    }

    async fn get_system_stats(&self) -> Result<SystemStats, ComfyAdapterError> {
        Err(ComfyAdapterError::Offline(
            "DEV-084 admission fixture".to_owned(),
        ))
    }

    async fn get_object_info(&self) -> Result<Value, ComfyAdapterError> {
        Err(ComfyAdapterError::Offline(
            "DEV-084 admission fixture".to_owned(),
        ))
    }

    async fn get_history(&self, _prompt_id: &str) -> Result<ComfyHistory, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible(
            "DEV-084 admission fixture".to_owned(),
        ))
    }

    async fn download_output(
        &self,
        _file: &ComfyOutputFile,
    ) -> Result<ComfyOutputData, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible(
            "DEV-084 admission fixture".to_owned(),
        ))
    }

    async fn submit_workflow(
        &self,
        _client_id: &str,
        _prompt_id: &str,
        _workflow: Value,
    ) -> Result<PromptSubmission, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible(
            "DEV-084 admission fixture".to_owned(),
        ))
    }

    async fn subscribe_events(
        &self,
        _client_id: &str,
    ) -> Result<Box<dyn ComfyEventSubscription>, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible(
            "DEV-084 admission fixture".to_owned(),
        ))
    }
}

struct NoopWorkflowRunRepository;

#[async_trait::async_trait]
impl WorkflowRunRepository for NoopWorkflowRunRepository {
    async fn has_successful_run(
        &self,
        _workflow_id: &str,
        _workflow_version: &str,
    ) -> Result<bool, RepositoryError> {
        Ok(false)
    }
}

fn onboarding_service_for(harness: &RegistryPurgeHarness) -> Arc<WorkflowOnboardingService> {
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let source: Arc<dyn WorkflowLibrarySource> = Arc::new(FileSystemWorkflowLibrarySource::new(
        harness.library_root.clone(),
    ));
    let package_store: Arc<dyn WorkflowPackageStore> =
        Arc::new(FileSystemWorkflowPackageStore::new(
            harness.library_root.clone(),
            harness._directory.path().join("workflow-staging"),
        ));
    let runtime_repository: Arc<dyn WorkflowRuntimeRepository> =
        Arc::new(SqliteWorkflowRuntimeRepository::new(harness.pool.clone()));
    let state_repository: Arc<dyn WorkflowRuntimeStateRepository> = Arc::new(
        SqliteWorkflowRuntimeStateRepository::new(harness.pool.clone()),
    );
    let library_service = Arc::new(WorkflowLibraryService::new(
        source.clone(),
        Arc::new(SqliteWorkflowLibraryRepository::new(harness.pool.clone())),
        clock.clone(),
    ));
    Arc::new(
        WorkflowOnboardingService::new(
            source,
            Arc::new(NonSubmittingComfy),
            library_service,
            Arc::new(NoopWorkflowRunRepository),
            package_store.clone(),
            clock.clone(),
        )
        .with_runtime_state(runtime_repository.clone(), state_repository.clone())
        .with_registry_service(harness.registry.clone()),
    )
}

fn workspace_query_service_for(harness: &RegistryPurgeHarness) -> WorkflowWorkspaceQueryService {
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let package_store: Arc<dyn WorkflowPackageStore> =
        Arc::new(FileSystemWorkflowPackageStore::new(
            harness.library_root.clone(),
            harness._directory.path().join("workflow-staging"),
        ));
    let runtime_repository: Arc<dyn WorkflowRuntimeRepository> =
        Arc::new(SqliteWorkflowRuntimeRepository::new(harness.pool.clone()));
    let state_repository: Arc<dyn WorkflowRuntimeStateRepository> = Arc::new(
        SqliteWorkflowRuntimeStateRepository::new(harness.pool.clone()),
    );
    let artifact_repository: Arc<dyn WorkflowRuntimeArtifactRepository> = Arc::new(
        SqliteWorkflowRuntimeArtifactRepository::new(harness.pool.clone()),
    );
    WorkflowWorkspaceQueryService::new(
        harness.registry.clone(),
        runtime_repository,
        state_repository,
        artifact_repository,
        package_store,
        onboarding_service_for(harness),
        clock,
    )
}

fn lifecycle_service_for(harness: &RegistryPurgeHarness) -> WorkflowLifecycleService {
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let source: Arc<dyn WorkflowLibrarySource> = Arc::new(FileSystemWorkflowLibrarySource::new(
        harness.library_root.clone(),
    ));
    let package_store: Arc<dyn WorkflowPackageStore> =
        Arc::new(FileSystemWorkflowPackageStore::new(
            harness.library_root.clone(),
            harness._directory.path().join("workflow-staging"),
        ));
    let runtime_repository: Arc<dyn WorkflowRuntimeRepository> =
        Arc::new(SqliteWorkflowRuntimeRepository::new(harness.pool.clone()));
    let state_repository: Arc<dyn WorkflowRuntimeStateRepository> = Arc::new(
        SqliteWorkflowRuntimeStateRepository::new(harness.pool.clone()),
    );
    let library_service = Arc::new(WorkflowLibraryService::new(
        source.clone(),
        Arc::new(SqliteWorkflowLibraryRepository::new(harness.pool.clone())),
        clock.clone(),
    ));
    WorkflowLifecycleService::new(
        source,
        library_service,
        onboarding_service_for(harness),
        runtime_repository,
        state_repository,
        package_store,
        clock,
    )
}

async fn exact_generation_identity(harness: &RegistryPurgeHarness) -> (String, String) {
    sqlx::query_as::<_, (String, String)>(
        "SELECT wv.id, r.id
         FROM workflow_versions wv
         INNER JOIN recipes r ON r.workflow_version_id = wv.id
         WHERE wv.workflow_id = ?
         ORDER BY r.id
         LIMIT 1",
    )
    .bind(&harness.workflow_id)
    .fetch_one(&harness.pool)
    .await
    .expect("exact generation identity should exist")
}

fn generation_request(workflow_version_id: String, recipe_id: String) -> CreateGenerationRequest {
    CreateGenerationRequest {
        project_id: "prj_default".to_owned(),
        workflow_version_id,
        recipe_id,
        values: BTreeMap::new(),
        reference_manifest: None,
        submission_idempotency_key: None,
        submission_attempt: None,
        parent_task_id: None,
    }
}

fn generation_service_for(harness: &RegistryPurgeHarness) -> Arc<GenerationService> {
    let task_repository: Arc<dyn TaskRepository> =
        Arc::new(SqliteTaskRepository::new(harness.pool.clone()));
    let snapshot_repository: Arc<dyn GenerationSnapshotRepository> = Arc::new(
        SqliteGenerationSnapshotRepository::new(harness.pool.clone()),
    );
    let definition_repository: Arc<dyn GenerationDefinitionRepository> = Arc::new(
        SqliteGenerationDefinitionRepository::new(harness.pool.clone()),
    );
    let project_repository: Arc<dyn ProjectRepository> =
        Arc::new(SqliteProjectRepository::new(harness.pool.clone()));
    let asset_repository: Arc<dyn AssetRepository> =
        Arc::new(SqliteAssetRepository::new(harness.pool.clone()));
    let asset_store: Arc<dyn AssetStore> = Arc::new(FileSystemAssetStore::new());
    let comfy_adapter: Arc<dyn ComfyAdapter> = Arc::new(NonSubmittingComfy);

    Arc::new(
        GenerationService::new(
            task_repository,
            snapshot_repository,
            definition_repository,
            comfy_adapter,
            project_repository,
            asset_store,
            asset_repository,
            Arc::new(SystemClock),
        )
        .with_new_generation_admission(harness.registry.clone()),
    )
}

#[tokio::test]
async fn dev084_runtime_artifact_exact_pair_is_unique() {
    let (_directory, pool) = seed_exact_recipe_database().await;
    let repository = SqliteWorkflowRuntimeArtifactRepository::new(pool.clone());
    repository
        .upsert(&artifact(
            "wra_dev084_canonical",
            "dev084-recipe-a",
            "dev084-package-a",
            "workflow-sha",
            "recipe-sha-a",
        ))
        .await
        .expect("canonical artifact should insert");

    // A second package with the same immutable bytes cannot become a second
    // exact artifact. The existing row remains the canonical selection.
    repository
        .upsert(&artifact(
            "wra_dev084_duplicate",
            "dev084-recipe-a",
            "dev084-package-b",
            "workflow-sha",
            "recipe-sha-a",
        ))
        .await
        .expect("same-byte duplicate should be ignored");

    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM workflow_runtime_artifacts
             WHERE workflow_version_id = 'dev084-version' AND recipe_id = 'dev084-recipe-a'",
        )
        .fetch_one(&pool)
        .await
        .expect("artifact count should be readable"),
        1
    );
}

#[tokio::test]
async fn dev084_runtime_artifact_conflict_is_blocked() {
    let (_directory, pool) = seed_exact_recipe_database().await;
    let repository = SqliteWorkflowRuntimeArtifactRepository::new(pool.clone());
    repository
        .upsert(&artifact(
            "wra_dev084_existing",
            "dev084-recipe-a",
            "dev084-package-a",
            "workflow-sha",
            "recipe-sha-a",
        ))
        .await
        .expect("artifact should insert");

    // Simulate a pre-existing corrupt runtime row. The admission hash check
    // must fail closed rather than replacing the exact mapping.
    sqlx::query(
        "UPDATE workflow_runtime_artifacts
         SET recipe_sha256 = 'different-recipe-sha'
         WHERE id = 'wra_dev084_existing'",
    )
    .execute(&pool)
    .await
    .expect("fixture corruption should apply");
    let error = repository
        .upsert(&artifact(
            "wra_dev084_candidate",
            "dev084-recipe-a",
            "dev084-package-b",
            "workflow-sha",
            "recipe-sha-a",
        ))
        .await
        .expect_err("different bytes for one exact pair must be blocked");
    assert!(error.to_string().contains("RUNTIME_ARTIFACT_CONFLICT"));
}

#[tokio::test]
async fn dev084_multi_recipe_migration_reconciles_exact_artifacts() {
    let (_directory, pool) = legacy_pool().await;
    sqlx::query(
        "INSERT INTO workflows
         (id, name, category, mode, current_version_id, created_at, updated_at)
         VALUES ('dev084-workflow', 'DEV-084', 'video', 'text_to_video', NULL, ?, ?)",
    )
    .bind("2026-01-01T00:00:00Z")
    .bind("2026-01-01T00:00:00Z")
    .execute(&pool)
    .await
    .expect("workflow fixture should insert");
    sqlx::query(
        "INSERT INTO workflow_versions
         (id, workflow_id, version, api_workflow_json, workflow_sha256,
          package_name, package_source_path, created_at)
         VALUES ('dev084-version', 'dev084-workflow', '1.0.0', '{}',
                 'workflow-sha', 'legacy-package', 'C:/legacy-package', ?)",
    )
    .bind("2026-01-01T00:00:00Z")
    .execute(&pool)
    .await
    .expect("workflow version fixture should insert");
    for (recipe_id, recipe_sha256) in [
        ("dev084-recipe-a", "recipe-sha-a"),
        ("dev084-recipe-b", "recipe-sha-b"),
    ] {
        sqlx::query(
            "INSERT INTO recipes
             (id, workflow_version_id, version, schema_version, recipe_yaml,
              recipe_sha256, created_at)
             VALUES (?, 'dev084-version', '1.0.0', 1, 'schema_version: 1', ?, ?)",
        )
        .bind(recipe_id)
        .bind(recipe_sha256)
        .bind("2026-01-01T00:00:00Z")
        .execute(&pool)
        .await
        .expect("recipe fixture should insert");
    }

    sqlx::raw_sql(MIGRATIONS_THROUGH_028[27])
        .execute(&pool)
        .await
        .expect("028 should create the legacy provisional artifacts");
    // 028's UNIQUE(package_name) means only one of the two provisional rows
    // can be inserted. DEV-084 reconciles this incomplete state instead of
    // treating the copied package name as an artifact for every recipe.
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflow_runtime_artifacts")
            .fetch_one(&pool)
            .await
            .expect("provisional artifact count should be readable"),
        1
    );

    // Replace the recipe-A provisional row with two real same-SHA candidates
    // and make the Registry-selected package explicit.  029 must keep the
    // selected real row, not select by id or insertion order.
    sqlx::query(
        "DELETE FROM workflow_runtime_artifacts
         WHERE workflow_version_id = 'dev084-version'
           AND recipe_id = 'dev084-recipe-a'",
    )
    .execute(&pool)
    .await
    .expect("recipe-A provisional row should be removed from the fixture");
    sqlx::query(
        "UPDATE workflow_versions
         SET package_name = 'actual-package-a'
         WHERE id = 'dev084-version'",
    )
    .execute(&pool)
    .await
    .expect("Registry-selected package should be recorded");
    sqlx::query(
        "INSERT INTO workflow_runtime_artifacts
         (id, workflow_version_id, recipe_id, package_name, source_kind,
          package_source_path, workflow_sha256, recipe_sha256, created_at)
         VALUES ('wra_00000000-0000-0000-0000-000000000084', 'dev084-version',
                 'dev084-recipe-a', 'actual-package-a', 'USER', 'C:/actual-package-a',
                 'workflow-sha', 'recipe-sha-a', '2026-01-01T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .expect("canonical artifact fixture should insert");
    sqlx::query(
        "INSERT INTO workflow_runtime_artifacts
         (id, workflow_version_id, recipe_id, package_name, source_kind,
          package_source_path, workflow_sha256, recipe_sha256, created_at)
         VALUES ('wra_00000000-0000-0000-0000-000000000085', 'dev084-version',
                 'dev084-recipe-a', 'actual-package-b', 'USER', 'C:/actual-package-b',
                 'workflow-sha', 'recipe-sha-a', '2026-01-01T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .expect("same-SHA duplicate artifact fixture should insert");

    sqlx::raw_sql(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/029_workflow_registry_runtime_artifact_reconciliation.sql"
    )))
    .execute(&pool)
    .await
    .expect("029 should reconcile provisional artifacts");

    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id, recipe_id, package_name
         FROM workflow_runtime_artifacts ORDER BY recipe_id",
    )
    .fetch_all(&pool)
    .await
    .expect("reconciled artifacts should be readable");
    assert_eq!(
        rows,
        vec![(
            "wra_00000000-0000-0000-0000-000000000084".to_owned(),
            "dev084-recipe-a".to_owned(),
            "actual-package-a".to_owned(),
        )]
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'index' AND name = 'idx_workflow_runtime_artifacts_version_recipe'",
        )
        .fetch_one(&pool)
        .await
        .expect("exact pair index should exist"),
        1
    );

    // Actual package sync supplies each manifest's exact recipe identity after
    // the migration; the two recipes are independent exact pairs.
    sqlx::query(
        "INSERT INTO workflow_runtime_artifacts
         (id, workflow_version_id, recipe_id, package_name, source_kind,
          package_source_path, workflow_sha256, recipe_sha256, created_at)
         VALUES ('wra_00000000-0000-0000-0000-000000000086', 'dev084-version',
                 'dev084-recipe-b', 'actual-package-b', 'USER', 'C:/actual-package-b',
                 'workflow-sha', 'recipe-sha-b', '2026-01-01T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .expect("second exact recipe artifact should insert");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflow_runtime_artifacts")
            .fetch_one(&pool)
            .await
            .expect("artifact count should be readable"),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT package_name FROM workflow_versions WHERE id = 'dev084-version'",
        )
        .fetch_one(&pool)
        .await
        .expect("legacy package metadata should remain readable"),
        "actual-package-a"
    );
}

#[tokio::test]
async fn dev084_purge_removes_runtime_package() {
    let harness = registry_purge_harness(
        "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0",
        "dev084_user_purge_package",
        "wfl_aitudou_minimax_h3_lightx2v_8step_fast",
    )
    .await;

    harness
        .registry
        .remove_workflow(&harness.workflow_id)
        .await
        .expect("logical removal should succeed");
    harness
        .registry
        .purge_workflow(&harness.workflow_id)
        .await
        .expect("removed user workflow should purge");

    assert!(!harness.library_root.join(&harness.package_name).exists());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflows WHERE id = ?")
            .bind(&harness.workflow_id)
            .fetch_one(&harness.pool)
            .await
            .expect("workflow count should be readable"),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflow_runtime_artifacts")
            .fetch_one(&harness.pool)
            .await
            .expect("artifact count should be readable"),
        0
    );
}

#[tokio::test]
async fn dev084_purge_recovery_restores_after_crash_before_db_commit() {
    let harness = registry_purge_harness(
        "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0",
        "dev084_user_crash_before_commit",
        "wfl_aitudou_minimax_h3_lightx2v_8step_fast",
    )
    .await;
    harness
        .registry
        .remove_workflow(&harness.workflow_id)
        .await
        .expect("logical removal should succeed");
    let operation_id = "purge_dev084_before_commit";
    prepare_and_quarantine(
        &harness,
        operation_id,
        &[&harness.package_name],
        &[&harness.package_name],
    )
    .await;
    assert!(harness
        .library_root
        .join(".purge")
        .join(operation_id)
        .join("operation.json")
        .is_file());

    let restarted =
        registry_service_with_package_store(&harness, Arc::new(filesystem_package_store(&harness)));
    let recovery = restarted
        .recover_pending_purges()
        .await
        .expect("startup recovery should restore the pre-commit quarantine");
    assert_eq!(recovery.reconciled_operations, 1);
    assert!(harness.library_root.join(&harness.package_name).is_dir());
    assert!(!harness
        .library_root
        .join(".purge")
        .join(operation_id)
        .exists());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflows WHERE id = ?")
            .bind(&harness.workflow_id)
            .fetch_one(&harness.pool)
            .await
            .expect("workflow count should be readable"),
        1
    );

    let sync = WorkflowLibraryService::new(
        Arc::new(FileSystemWorkflowLibrarySource::new(
            harness.library_root.clone(),
        )),
        Arc::new(SqliteWorkflowLibraryRepository::new(harness.pool.clone())),
        Arc::new(SystemClock),
    )
    .sync()
    .await
    .expect("recovered package should synchronize without duplication");
    assert_eq!(sync.packages_found, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflows WHERE id = ?")
            .bind(&harness.workflow_id)
            .fetch_one(&harness.pool)
            .await
            .expect("workflow count after sync should be readable"),
        1
    );
    let registry_record = SqliteWorkflowRegistryRepository::new(harness.pool.clone())
        .get(&harness.workflow_id)
        .await
        .expect("workflow registry row should be readable")
        .expect("workflow registry row should remain present");
    assert_eq!(registry_record.library_state, "REMOVED");
    restarted
        .restore_workflow(&harness.workflow_id)
        .await
        .expect("recovered removed workflow should remain restorable");
}

#[tokio::test]
async fn dev084_purge_recovery_cleans_after_crash_following_db_commit() {
    let harness = registry_purge_harness(
        "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0",
        "dev084_user_crash_after_commit",
        "wfl_aitudou_minimax_h3_lightx2v_8step_fast",
    )
    .await;
    harness
        .registry
        .remove_workflow(&harness.workflow_id)
        .await
        .expect("logical removal should succeed");
    let operation_id = "purge_dev084_after_commit";
    prepare_and_quarantine(
        &harness,
        operation_id,
        &[&harness.package_name],
        &[&harness.package_name],
    )
    .await;
    assert!(SqliteWorkflowRegistryRepository::new(harness.pool.clone())
        .purge(&harness.workflow_id)
        .await
        .expect("database purge should commit"));

    let restarted =
        registry_service_with_package_store(&harness, Arc::new(filesystem_package_store(&harness)));
    restarted
        .recover_pending_purges()
        .await
        .expect("startup recovery should clean the committed quarantine");
    assert!(!harness.library_root.join(&harness.package_name).exists());
    assert!(!harness
        .library_root
        .join(".purge")
        .join(operation_id)
        .exists());
    let sync = WorkflowLibraryService::new(
        Arc::new(FileSystemWorkflowLibrarySource::new(
            harness.library_root.clone(),
        )),
        Arc::new(SqliteWorkflowLibraryRepository::new(harness.pool.clone())),
        Arc::new(SystemClock),
    )
    .sync()
    .await
    .expect("sync should tolerate a committed purge");
    assert_eq!(sync.packages_found, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflows WHERE id = ?")
            .bind(&harness.workflow_id)
            .fetch_one(&harness.pool)
            .await
            .expect("purged workflow count should be readable"),
        0
    );
}

#[tokio::test]
async fn dev084_purge_inspection_reports_exact_references() {
    let harness = registry_purge_harness(
        "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0",
        "dev084_user_purge_inspection",
        "wfl_aitudou_minimax_h3_lightx2v_8step_fast",
    )
    .await;

    let (workflow_version_id, recipe_id) = sqlx::query_as::<_, (String, String)>(
        "SELECT workflow_versions.id, recipes.id
         FROM workflow_versions
         JOIN recipes ON recipes.workflow_version_id = workflow_versions.id
         WHERE workflow_versions.workflow_id = ?
         ORDER BY workflow_versions.id, recipes.id
         LIMIT 1",
    )
    .bind(&harness.workflow_id)
    .fetch_one(&harness.pool)
    .await
    .expect("synced workflow identity should be readable");

    sqlx::query(
        "INSERT INTO tasks
            (id, project_id, workflow_id, workflow_version_id, recipe_id, status, created_at)
         VALUES (?, 'prj_default', ?, ?, ?, 'CREATED', '2026-01-01T00:00:00Z')",
    )
    .bind("dev084-task")
    .bind(&harness.workflow_id)
    .bind(&workflow_version_id)
    .bind(&recipe_id)
    .execute(&harness.pool)
    .await
    .expect("task reference fixture should insert");
    sqlx::query(
        "INSERT INTO production_batches
            (id, project_id, name, status, continue_on_failure, created_at, updated_at)
         VALUES ('dev084-batch', 'prj_default', 'DEV-084 batch', 'DRAFT', 0,
                 '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .execute(&harness.pool)
    .await
    .expect("batch reference fixture should insert");
    sqlx::query(
        "INSERT INTO production_batch_items
            (id, batch_id, ordinal, workflow_version_id, recipe_id, values_json, status,
             created_at, updated_at)
         VALUES ('dev084-batch-item', 'dev084-batch', 0, ?, ?, '{}', 'PENDING',
                 '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .bind(&workflow_version_id)
    .bind(&recipe_id)
    .execute(&harness.pool)
    .await
    .expect("batch item reference fixture should insert");
    sqlx::query(
        "INSERT INTO presets
            (id, project_id, workflow_version_id, recipe_id, name, values_json, created_at, updated_at)
         VALUES ('dev084-preset', 'prj_default', ?, ?, 'DEV-084 preset', '{}',
                 '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .bind(&workflow_version_id)
    .bind(&recipe_id)
    .execute(&harness.pool)
    .await
    .expect("preset reference fixture should insert");
    sqlx::query(
        "INSERT INTO project_templates
            (id, name, normalized_name, description, workflow_version_id, recipe_id, values_json,
             created_at, updated_at)
         VALUES ('dev084-template', 'DEV-084 template', 'dev084-template', NULL, ?, ?, '{}',
                 '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .bind(&workflow_version_id)
    .bind(&recipe_id)
    .execute(&harness.pool)
    .await
    .expect("template reference fixture should insert");
    sqlx::query(
        "INSERT INTO shots
            (id, project_id, ordinal, name, prompt_text, created_at, updated_at)
         VALUES ('dev084-shot', 'prj_default', 0, 'DEV-084 shot', '',
                 '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .execute(&harness.pool)
    .await
    .expect("shot reference fixture should insert");
    sqlx::query(
        "INSERT INTO shot_stage_configs
            (shot_id, stage, workflow_version_id, recipe_id, scalar_values_json, updated_at)
         VALUES ('dev084-shot', 'video', ?, ?, '{}', '2026-01-01T00:00:00Z')",
    )
    .bind(&workflow_version_id)
    .bind(&recipe_id)
    .execute(&harness.pool)
    .await
    .expect("shot config reference fixture should insert");
    sqlx::query(
        "INSERT INTO benchmark_experiments
            (id, project_id, name, media_type, status, base_values_json, asset_ids_json,
             created_at, updated_at)
         VALUES ('dev084-experiment', 'prj_default', 'DEV-084 benchmark', 'VIDEO', 'DRAFT',
                 '{}', '[]', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .execute(&harness.pool)
    .await
    .expect("benchmark reference fixture should insert");
    sqlx::query(
        "INSERT INTO benchmark_candidates
            (id, experiment_id, position, workflow_version_id, recipe_id, label, values_json,
             asset_ids_json, created_at)
         VALUES ('dev084-candidate', 'dev084-experiment', 0, ?, ?, 'DEV-084 candidate', '{}',
                 '[]', '2026-01-01T00:00:00Z')",
    )
    .bind(&workflow_version_id)
    .bind(&recipe_id)
    .execute(&harness.pool)
    .await
    .expect("benchmark candidate reference fixture should insert");
    sqlx::query(
        "INSERT INTO project_workflow_bindings
            (project_id, stage, mode, workflow_version_id, recipe_id, created_at, updated_at)
         VALUES ('prj_default', 'VIDEO', 'FL2VA_TEXT_TO_VIDEO', ?, ?,
                 '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .bind(&workflow_version_id)
    .bind(&recipe_id)
    .execute(&harness.pool)
    .await
    .expect("binding reference fixture should insert");
    sqlx::query(
        "INSERT INTO production_runs
            (id, project_id, name, status, current_stage_ordinal, created_at, updated_at)
         VALUES ('dev084-run', 'prj_default', 'DEV-084 run', 'DRAFT', 0,
                 '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .execute(&harness.pool)
    .await
    .expect("production run reference fixture should insert");
    sqlx::query(
        "INSERT INTO production_stages
            (id, run_id, ordinal, stage_type, status, workflow_version_id, recipe_id,
             frozen_config_json, created_at, updated_at)
         VALUES ('dev084-stage', 'dev084-run', 0, 'H3_VIDEO_GENERATION', 'PENDING', ?, ?, '{}',
                 '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .bind(&workflow_version_id)
    .bind(&recipe_id)
    .execute(&harness.pool)
    .await
    .expect("production stage reference fixture should insert");
    sqlx::query(
        "INSERT INTO production_run_templates
            (id, project_id, name, krea2_workflow_version_id, krea2_recipe_id,
             h3_workflow_version_id, h3_recipe_id, created_at, updated_at)
         VALUES ('dev084-run-template', 'prj_default', 'DEV-084 run template', NULL, NULL, ?, ?,
                 '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .bind(&workflow_version_id)
    .bind(&recipe_id)
    .execute(&harness.pool)
    .await
    .expect("run template reference fixture should insert");

    let inspection = harness
        .registry
        .inspect_purge(&harness.workflow_id)
        .await
        .expect("purge inspection should succeed");
    assert_eq!(inspection.task_count, 1);
    assert_eq!(inspection.batch_item_count, 1);
    assert_eq!(inspection.preset_count, 1);
    assert_eq!(inspection.template_count, 1);
    assert_eq!(inspection.shot_config_count, 1);
    assert_eq!(inspection.benchmark_count, 1);
    assert_eq!(inspection.binding_count, 1);
    assert_eq!(inspection.stage_count, 1);
    assert_eq!(inspection.run_template_count, 1);
    assert_eq!(inspection.package_count, 1);
    assert!(!inspection.can_purge);
    assert!(inspection
        .blocking_reasons
        .iter()
        .any(|reason| reason.contains("任务")));
}

#[tokio::test]
async fn dev084_missing_runtime_package_does_not_block_purge() {
    let harness = registry_purge_harness(
        "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0",
        "dev084_user_missing_package",
        "wfl_aitudou_minimax_h3_lightx2v_8step_fast",
    )
    .await;

    harness
        .registry
        .remove_workflow(&harness.workflow_id)
        .await
        .expect("logical removal should succeed");
    fs::remove_dir_all(harness.library_root.join(&harness.package_name))
        .expect("runtime package should be removable for the fixture");

    let result = harness
        .registry
        .purge_workflow(&harness.workflow_id)
        .await
        .expect("missing runtime package should not block purge");
    assert!(result.committed);
    assert!(!result.cleanup_pending);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflows WHERE id = ?")
            .bind(&harness.workflow_id)
            .fetch_one(&harness.pool)
            .await
            .expect("workflow count should be readable"),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflow_runtime_artifacts")
            .fetch_one(&harness.pool)
            .await
            .expect("artifact count should be readable"),
        0
    );
    let sync = WorkflowLibraryService::new(
        Arc::new(FileSystemWorkflowLibrarySource::new(
            harness.library_root.clone(),
        )),
        Arc::new(SqliteWorkflowLibraryRepository::new(harness.pool.clone())),
        Arc::new(SystemClock),
    )
    .sync()
    .await
    .expect("restart sync should tolerate the missing package");
    assert_eq!(sync.packages_found, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflows WHERE id = ?")
            .bind(&harness.workflow_id)
            .fetch_one(&harness.pool)
            .await
            .expect("workflow count should be readable after sync"),
        0
    );
}

#[tokio::test]
async fn dev084_purge_cleanup_failure_returns_committed_success() {
    let harness = registry_purge_harness(
        "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0",
        "dev084_user_cleanup_pending",
        "wfl_aitudou_minimax_h3_lightx2v_8step_fast",
    )
    .await;

    harness
        .registry
        .remove_workflow(&harness.workflow_id)
        .await
        .expect("logical removal should succeed");
    let package_store = Arc::new(CleanupFailingPackageStore {
        inner: FileSystemWorkflowPackageStore::new(
            harness.library_root.clone(),
            harness._directory.path().join("workflow-staging"),
        ),
    });
    let registry = registry_service_with_package_store(&harness, package_store);
    let result = registry
        .purge_workflow(&harness.workflow_id)
        .await
        .expect("cleanup failure must not undo a committed purge");

    assert!(result.committed);
    assert!(result.cleanup_pending);
    assert_eq!(
        result.warning.as_deref(),
        Some("工作流已永久删除，但临时隔离文件清理未完成。")
    );
    assert!(!harness.library_root.join(&harness.package_name).exists());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflows WHERE id = ?")
            .bind(&harness.workflow_id)
            .fetch_one(&harness.pool)
            .await
            .expect("workflow count should be readable"),
        0
    );
    let restarted =
        registry_service_with_package_store(&harness, Arc::new(filesystem_package_store(&harness)));
    restarted
        .recover_pending_purges()
        .await
        .expect("next startup should clean cleanup-pending quarantine");
    assert!(fs::read_dir(harness.library_root.join(".purge"))
        .expect("purge root should remain readable")
        .next()
        .is_none());
    let sync = WorkflowLibraryService::new(
        Arc::new(FileSystemWorkflowLibrarySource::new(
            harness.library_root.clone(),
        )),
        Arc::new(SqliteWorkflowLibraryRepository::new(harness.pool.clone())),
        Arc::new(SystemClock),
    )
    .sync()
    .await
    .expect("sync should not resurrect cleanup-pending purge");
    assert_eq!(sync.packages_found, 0);
}

#[tokio::test]
async fn dev084_purge_recovery_restores_only_partially_quarantined_packages() {
    let harness = registry_purge_harness(
        "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0",
        "dev084_user_partial_quarantine",
        "wfl_aitudou_minimax_h3_lightx2v_8step_fast",
    )
    .await;
    let second_package = "dev084_partial_package_b";
    let second_root = harness.library_root.join(second_package);
    fs::create_dir_all(&second_root).expect("second package directory should exist");
    let first_root = harness.library_root.join(&harness.package_name);
    for file_name in ["manifest.yaml", "recipe.yaml", "workflow_api.json"] {
        fs::copy(first_root.join(file_name), second_root.join(file_name))
            .expect("second package file should copy");
    }
    harness
        .registry
        .remove_workflow(&harness.workflow_id)
        .await
        .expect("logical removal should succeed");

    let operation_id = "purge_dev084_partial_quarantine";
    prepare_and_quarantine(
        &harness,
        operation_id,
        &[&harness.package_name, second_package],
        &[&harness.package_name],
    )
    .await;
    let restarted =
        registry_service_with_package_store(&harness, Arc::new(filesystem_package_store(&harness)));
    restarted
        .recover_pending_purges()
        .await
        .expect("partial pre-commit quarantine should recover");
    assert!(harness.library_root.join(&harness.package_name).is_dir());
    assert!(second_root.is_dir());
    assert!(!harness
        .library_root
        .join(".purge")
        .join(operation_id)
        .exists());
    let published_packages = fs::read_dir(&harness.library_root)
        .expect("library root should be readable")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.path().is_dir() && !entry.file_name().to_string_lossy().starts_with('.')
        })
        .count();
    assert_eq!(published_packages, 2);
}

#[tokio::test]
async fn dev084_purge_recovery_fails_closed_on_malformed_journal() {
    let harness = registry_purge_harness(
        "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0",
        "dev084_user_malformed_journal",
        "wfl_aitudou_minimax_h3_lightx2v_8step_fast",
    )
    .await;
    let operation_id = "purge_dev084_malformed";
    let operation_root = harness.library_root.join(".purge").join(operation_id);
    let package_root = operation_root.join(&harness.package_name);
    fs::create_dir_all(&package_root).expect("malformed quarantine should exist");
    let published_root = harness.library_root.join(&harness.package_name);
    for file_name in ["manifest.yaml", "recipe.yaml", "workflow_api.json"] {
        fs::copy(published_root.join(file_name), package_root.join(file_name))
            .expect("malformed quarantine package file should copy");
    }
    fs::write(operation_root.join("operation.json"), b"{ not valid json")
        .expect("malformed journal should be written");

    let error =
        registry_service_with_package_store(&harness, Arc::new(filesystem_package_store(&harness)))
            .recover_pending_purges()
            .await
            .expect_err("malformed journal must block recovery");
    assert_eq!(error.code(), "WORKFLOW_PURGE_RECOVERY_BLOCKED");
    assert!(operation_root.is_dir());
    assert!(package_root.is_dir());
    assert!(published_root.is_dir());
}

#[tokio::test]
async fn dev084_legacy_quarantine_recovers_by_manifest_identity() {
    let harness = registry_purge_harness(
        "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0",
        "dev084_user_legacy_quarantine",
        "wfl_aitudou_minimax_h3_lightx2v_8step_fast",
    )
    .await;
    harness
        .registry
        .remove_workflow(&harness.workflow_id)
        .await
        .expect("logical removal should succeed");
    let store = filesystem_package_store(&harness);
    let restore_operation = "purge_dev084_legacy_restore";
    assert_eq!(
        store
            .quarantine_published(restore_operation, &harness.package_name)
            .await
            .expect("legacy package quarantine should succeed"),
        WorkflowPackageQuarantineResult::Quarantined
    );
    assert!(!harness
        .library_root
        .join(".purge")
        .join(restore_operation)
        .join("operation.json")
        .exists());

    let restarted =
        registry_service_with_package_store(&harness, Arc::new(filesystem_package_store(&harness)));
    restarted
        .recover_pending_purges()
        .await
        .expect("legacy pre-commit quarantine should restore by manifest identity");
    assert!(harness.library_root.join(&harness.package_name).is_dir());

    let cleanup_operation = "purge_dev084_legacy_cleanup";
    assert_eq!(
        store
            .quarantine_published(cleanup_operation, &harness.package_name)
            .await
            .expect("second legacy package quarantine should succeed"),
        WorkflowPackageQuarantineResult::Quarantined
    );
    assert!(SqliteWorkflowRegistryRepository::new(harness.pool.clone())
        .purge(&harness.workflow_id)
        .await
        .expect("database purge should commit"));
    restarted
        .recover_pending_purges()
        .await
        .expect("legacy post-commit quarantine should clean by manifest identity");
    assert!(!harness.library_root.join(&harness.package_name).exists());
    assert!(!harness
        .library_root
        .join(".purge")
        .join(cleanup_operation)
        .exists());
}

#[tokio::test]
async fn dev084_concurrent_purge_requests_leave_no_runtime_resurrection() {
    let harness = registry_purge_harness(
        "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0",
        "dev084_user_concurrent_purge",
        "wfl_aitudou_minimax_h3_lightx2v_8step_fast",
    )
    .await;
    harness
        .registry
        .remove_workflow(&harness.workflow_id)
        .await
        .expect("logical removal should succeed");
    let first_registry = harness.registry.clone();
    let second_registry = harness.registry.clone();
    let (first, second) = tokio::join!(
        first_registry.purge_workflow(&harness.workflow_id),
        second_registry.purge_workflow(&harness.workflow_id),
    );
    assert_eq!(first.is_ok() as u8 + second.is_ok() as u8, 1);
    assert_eq!(
        first
            .as_ref()
            .err()
            .or_else(|| second.as_ref().err())
            .map(|error| error.code()),
        Some("WORKFLOW_NOT_FOUND")
    );
    assert!(!harness.library_root.join(&harness.package_name).exists());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflows WHERE id = ?")
            .bind(&harness.workflow_id)
            .fetch_one(&harness.pool)
            .await
            .expect("workflow count should be readable"),
        0
    );
}

#[tokio::test]
async fn dev084_purged_user_workflow_does_not_resurrect_after_library_sync() {
    let harness = registry_purge_harness(
        "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0",
        "dev084_user_restart_package",
        "wfl_aitudou_minimax_h3_lightx2v_8step_fast",
    )
    .await;

    harness
        .registry
        .remove_workflow(&harness.workflow_id)
        .await
        .expect("logical removal should succeed");
    harness
        .registry
        .purge_workflow(&harness.workflow_id)
        .await
        .expect("removed user workflow should purge");

    let restart_service = WorkflowLibraryService::new(
        Arc::new(FileSystemWorkflowLibrarySource::new(
            harness.library_root.clone(),
        )),
        Arc::new(SqliteWorkflowLibraryRepository::new(harness.pool.clone())),
        Arc::new(SystemClock),
    );
    let report = restart_service
        .sync()
        .await
        .expect("restart sync should succeed with the purged package absent");
    assert_eq!(report.packages_found, 0);
    assert!(harness
        .registry
        .list()
        .await
        .expect("registry should load")
        .is_empty());
}

#[tokio::test]
async fn dev084_product_purge_still_blocked() {
    let harness = registry_purge_harness(
        "minimax_h3_fl2va_1_0_0",
        "minimax_h3_fl2va_1_0_0",
        "wfl_minimax_h3_fl2va",
    )
    .await;

    harness
        .registry
        .remove_workflow(&harness.workflow_id)
        .await
        .expect("logical product removal should still be tracked");
    let error = harness
        .registry
        .purge_workflow(&harness.workflow_id)
        .await
        .expect_err("PRODUCT workflow purge must be blocked");
    assert!(error
        .to_string()
        .contains("PRODUCT workflows can never be purged"));
    assert!(harness.library_root.join(&harness.package_name).is_dir());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflows WHERE id = ?")
            .bind(&harness.workflow_id)
            .fetch_one(&harness.pool)
            .await
            .expect("workflow count should be readable"),
        1
    );
}

async fn assert_direct_generation_is_blocked(harness: &RegistryPurgeHarness) {
    let (workflow_version_id, recipe_id) = exact_generation_identity(harness).await;
    let before = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM tasks")
        .fetch_one(&harness.pool)
        .await
        .expect("task count should be readable");
    let error = generation_service_for(harness)
        .start_generation(generation_request(workflow_version_id, recipe_id))
        .await
        .expect_err("unavailable workflow pair must not create a task");
    assert!(matches!(
        error,
        GenerationServiceError::ExecutionFailed { code, .. }
            if code == WORKFLOW_UNAVAILABLE_FOR_NEW_GENERATION
    ));
    let after = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM tasks")
        .fetch_one(&harness.pool)
        .await
        .expect("task count should be readable");
    assert_eq!(after, before);
}

#[tokio::test]
async fn dev084_removed_workflow_cannot_create_direct_generation() {
    let harness = registry_purge_harness(
        "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0",
        "dev084_user_removed_generation",
        "wfl_aitudou_minimax_h3_lightx2v_8step_fast",
    )
    .await;
    harness
        .registry
        .remove_workflow(&harness.workflow_id)
        .await
        .expect("logical removal should succeed");
    assert_direct_generation_is_blocked(&harness).await;
}

#[tokio::test]
async fn dev084_disabled_workflow_cannot_create_direct_generation() {
    let harness = registry_purge_harness(
        "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0",
        "dev084_user_disabled_generation",
        "wfl_aitudou_minimax_h3_lightx2v_8step_fast",
    )
    .await;
    let (workflow_version_id, _) = exact_generation_identity(&harness).await;
    SqliteWorkflowRuntimeStateRepository::new(harness.pool.clone())
        .set_enabled(&workflow_version_id, false, Utc::now())
        .await
        .expect("workflow version should be disabled");
    assert_direct_generation_is_blocked(&harness).await;
}

#[tokio::test]
async fn dev084_archived_version_cannot_create_direct_generation() {
    let harness = registry_purge_harness(
        "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0",
        "dev084_user_archived_generation",
        "wfl_aitudou_minimax_h3_lightx2v_8step_fast",
    )
    .await;
    let (workflow_version_id, _) = exact_generation_identity(&harness).await;
    let now = Utc::now();
    SqliteWorkflowRuntimeStateRepository::new(harness.pool.clone())
        .set_archived(&workflow_version_id, true, false, Some(now), now)
        .await
        .expect("workflow version should be archived");
    assert_direct_generation_is_blocked(&harness).await;
}

#[tokio::test]
async fn dev084_active_exact_recipe_can_create_generation() {
    let harness = registry_purge_harness(
        "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0",
        "dev084_user_active_generation",
        "wfl_aitudou_minimax_h3_lightx2v_8step_fast",
    )
    .await;
    let (workflow_version_id, recipe_id) = exact_generation_identity(&harness).await;
    let task = generation_service_for(&harness)
        .start_generation(generation_request(workflow_version_id, recipe_id))
        .await
        .expect("active exact recipe should create a generation task");
    assert_eq!(task.status, ai_studio_lib::domain::TaskStatus::Created);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM tasks")
            .fetch_one(&harness.pool)
            .await
            .expect("task count should be readable"),
        1
    );
}

#[tokio::test]
async fn dev084_registry_workspace_reports_exact_artifact_missing() {
    let harness = registry_purge_harness(
        "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0",
        "dev084_user_workspace_missing",
        "wfl_aitudou_minimax_h3_lightx2v_8step_fast",
    )
    .await;
    let (workflow_version_id, recipe_id) = exact_generation_identity(&harness).await;
    sqlx::query(
        "DELETE FROM workflow_runtime_artifacts
         WHERE workflow_version_id = ? AND recipe_id = ?",
    )
    .bind(&workflow_version_id)
    .bind(&recipe_id)
    .execute(&harness.pool)
    .await
    .expect("exact artifact should be removable for the missing fixture");

    let response = workspace_query_service_for(&harness)
        .fast()
        .await
        .expect("FAST workspace query should succeed");
    let runtime = response
        .items
        .iter()
        .find(|item| item.registry.workflow_id == harness.workflow_id)
        .and_then(|item| {
            item.runtime.iter().find(|runtime| {
                runtime.workflow_version_id == workflow_version_id && runtime.recipe_id == recipe_id
            })
        })
        .expect("the exact recipe runtime row should remain visible");
    assert_eq!(runtime.artifact_status, "MISSING");
    assert_eq!(runtime.package_status, "MISSING");
    assert_eq!(runtime.package_name, None);
    assert!(runtime
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "RUNTIME_PACKAGE_MISSING"));
}

#[tokio::test]
async fn dev084_refresh_executes_runtime_diagnostics() {
    let harness = registry_purge_harness(
        "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0",
        "dev084_user_workspace_refresh",
        "wfl_aitudou_minimax_h3_lightx2v_8step_fast",
    )
    .await;
    let (workflow_version_id, recipe_id) = exact_generation_identity(&harness).await;
    let recipe_path = harness
        .library_root
        .join(&harness.package_name)
        .join("recipe.yaml");
    let mut recipe_yaml = fs::read(&recipe_path).expect("runtime recipe should be readable");
    recipe_yaml.extend_from_slice(b"\n# DEV-084 runtime diagnostic mutation\n");
    fs::write(&recipe_path, recipe_yaml).expect("runtime recipe mutation should write");

    let response = workspace_query_service_for(&harness)
        .refresh()
        .await
        .expect("REFRESH workspace query should succeed with diagnostics");
    let runtime = response
        .items
        .iter()
        .find(|item| item.registry.workflow_id == harness.workflow_id)
        .and_then(|item| {
            item.runtime.iter().find(|runtime| {
                runtime.workflow_version_id == workflow_version_id && runtime.recipe_id == recipe_id
            })
        })
        .expect("the refreshed exact recipe runtime row should remain visible");
    assert_eq!(runtime.package_status, "INVALID");
    assert_eq!(runtime.artifact_status, "PRESENT");
    assert!(runtime
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "RECIPE_RUNTIME_HASH_MISMATCH"));
}

#[tokio::test]
async fn dev084_recheck_capability_survives_registry_reload() {
    let harness = registry_purge_harness(
        "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0",
        "dev084_user_workspace_cache",
        "wfl_aitudou_minimax_h3_lightx2v_8step_fast",
    )
    .await;
    let (workflow_version_id, recipe_id) = exact_generation_identity(&harness).await;
    let service = workspace_query_service_for(&harness);
    service
        .cache_capability_for_recipe(
            &workflow_version_id,
            &recipe_id,
            CapabilityCheckView {
                state: CapabilityState::Ready,
                checked_at: Some("2026-01-01T00:00:00Z".to_owned()),
                issues: Vec::new(),
            },
        )
        .await
        .expect("explicit recheck result should be cached");

    let response = service
        .fast()
        .await
        .expect("FAST query should reload Registry");
    let runtime = response
        .items
        .iter()
        .find(|item| item.registry.workflow_id == harness.workflow_id)
        .and_then(|item| {
            item.runtime.iter().find(|runtime| {
                runtime.workflow_version_id == workflow_version_id && runtime.recipe_id == recipe_id
            })
        })
        .expect("cached exact recipe runtime row should remain visible");
    assert_eq!(runtime.capability, "READY");
    assert_eq!(
        runtime.live_verified_at.as_deref(),
        Some("2026-01-01T00:00:00Z")
    );
}

#[tokio::test]
async fn dev084_restore_requires_runtime_revalidation() {
    let harness = registry_purge_harness(
        "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0",
        "dev084_user_restore_missing",
        "wfl_aitudou_minimax_h3_lightx2v_8step_fast",
    )
    .await;
    let (workflow_version_id, _) = exact_generation_identity(&harness).await;
    harness
        .registry
        .remove_workflow(&harness.workflow_id)
        .await
        .expect("logical removal should succeed");
    fs::remove_dir_all(harness.library_root.join(&harness.package_name))
        .expect("restore fixture package should be removed");

    let restored = harness
        .registry
        .restore_workflow(&harness.workflow_id)
        .await
        .expect("logical restore should succeed before runtime revalidation");
    assert_eq!(restored.library_state, "ACTIVE");
    let version_restore = lifecycle_service_for(&harness)
        .restore_version(&workflow_version_id)
        .await
        .expect("restore should close with an attention state");
    assert!(!version_restore.enabled);
    assert!(!version_restore.archived);
    assert_eq!(version_restore.readiness, "RESTORED_NEEDS_ATTENTION");
    assert_eq!(
        SqliteWorkflowRuntimeStateRepository::new(harness.pool.clone())
            .find_state(&workflow_version_id)
            .await
            .expect("restored state should be readable")
            .expect("restored state should exist")
            .archived,
        false
    );
}

#[tokio::test]
async fn dev084_project_bindings_not_restored_implicitly() {
    let harness = registry_purge_harness(
        "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0",
        "dev084_user_restore_binding",
        "wfl_aitudou_minimax_h3_lightx2v_8step_fast",
    )
    .await;
    let (workflow_version_id, recipe_id) = exact_generation_identity(&harness).await;
    let now = Utc::now();
    let binding_repository = SqliteProjectWorkflowBindingRepository::new(harness.pool.clone());
    binding_repository
        .replace_for_project(
            "prj_default",
            &[ProjectWorkflowBindingRecord {
                project_id: "prj_default".to_owned(),
                stage: "VIDEO".to_owned(),
                mode: "DEFAULT".to_owned(),
                workflow_version_id,
                recipe_id,
                created_at: now,
                updated_at: now,
            }],
        )
        .await
        .expect("binding fixture should insert");
    harness
        .registry
        .remove_workflow(&harness.workflow_id)
        .await
        .expect("logical removal should clear bindings");
    assert!(binding_repository
        .list_for_project("prj_default")
        .await
        .expect("binding list should be readable")
        .is_empty());
    harness
        .registry
        .restore_workflow(&harness.workflow_id)
        .await
        .expect("logical restore should succeed");
    assert!(binding_repository
        .list_for_project("prj_default")
        .await
        .expect("binding list should remain readable")
        .is_empty());
}
