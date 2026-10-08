//! Deterministic service/repository races: no sleeps or live ComfyUI calls.
#[cfg(test)]
use super::*;
use crate::application::{
    builtin_runtime_packages,
    ports::{WorkflowLibraryRepository, WorkflowPackageRecord},
    workflow_manifest::WorkflowManifest,
};
use crate::infrastructure::database::{
    initialize, repositories::test_support, SqliteProjectRepository,
    SqliteProjectWorkflowBindingRepository, SqliteWorkflowLibraryRepository,
    SqliteWorkflowRegistryRepository, SqliteWorkflowRuntimeArtifactRepository,
    SqliteWorkflowRuntimeRepository, SqliteWorkflowRuntimeStateRepository,
};
use crate::infrastructure::filesystem::FileSystemWorkflowPackageStore;
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::{
    future::Future,
    sync::atomic::{AtomicBool, Ordering},
    task::Poll,
};
use tokio::sync::Notify;

#[derive(Default)]
struct Pause {
    armed: AtomicBool,
    entered: Notify,
    release: Notify,
}

impl Pause {
    async fn wait_if_armed(&self) {
        if self.armed.swap(false, Ordering::SeqCst) {
            self.entered.notify_one();
            self.release.notified().await;
        }
    }
}

struct PausingBindings {
    inner: SqliteProjectWorkflowBindingRepository,
    write: Pause,
    list: Pause,
}

#[async_trait]
impl ProjectWorkflowBindingRepository for PausingBindings {
    async fn list_for_project(
        &self,
        project: &str,
    ) -> Result<Vec<ProjectWorkflowBindingRecord>, RepositoryError> {
        self.inner.list_for_project(project).await
    }
    async fn replace_for_project(
        &self,
        project: &str,
        bindings: &[ProjectWorkflowBindingRecord],
    ) -> Result<(), RepositoryError> {
        self.inner.replace_for_project(project, bindings).await
    }
    async fn insert_slot(
        &self,
        binding: &ProjectWorkflowBindingRecord,
    ) -> Result<(), RepositoryError> {
        self.write.wait_if_armed().await;
        self.inner.insert_slot(binding).await
    }
    async fn update_slot(
        &self,
        project: &str,
        stage: &str,
        mode: &str,
        instance: &str,
        revision: i64,
        version: &str,
        recipe: &str,
        now: DateTime<Utc>,
    ) -> Result<u64, RepositoryError> {
        self.write.wait_if_armed().await;
        self.inner
            .update_slot(
                project, stage, mode, instance, revision, version, recipe, now,
            )
            .await
    }
    async fn delete_slot(
        &self,
        project: &str,
        stage: &str,
        mode: &str,
        instance: &str,
        revision: i64,
    ) -> Result<u64, RepositoryError> {
        self.inner
            .delete_slot(project, stage, mode, instance, revision)
            .await
    }
    async fn list_for_workflow_version(
        &self,
        version: &str,
    ) -> Result<Vec<ProjectWorkflowBindingRecord>, RepositoryError> {
        self.list.wait_if_armed().await;
        self.inner.list_for_workflow_version(version).await
    }
    async fn clear_by_workflow_version(&self, version: &str) -> Result<u64, RepositoryError> {
        self.inner.clear_by_workflow_version(version).await
    }
}

struct RaceFixture {
    _dir: tempfile::TempDir,
    pool: sqlx::SqlitePool,
    bindings: Arc<PausingBindings>,
    registry: Arc<WorkflowRegistryService>,
    service: Arc<ProjectWorkflowBindingService>,
    workflow_id: String,
    version_id: String,
    recipe_id: String,
}

async fn fixture() -> RaceFixture {
    let dir = tempfile::tempdir().unwrap();
    let pool = initialize(&dir.path().join("race.db")).await.unwrap();
    test_support::seed_task_dependencies(&pool).await;
    // The race must reach the same production admission gate as a real binding.
    // A generic image fixture is no longer a new executable product generator.
    let package = "minimax_h3_fl2va_t2v_quality_2_2_0";
    let root = dir.path().join("library");
    builtin_runtime_packages::ensure_installed(&root).unwrap();
    let bytes = builtin_runtime_packages::embedded_package(package).unwrap();
    let manifest =
        WorkflowManifest::parse(std::str::from_utf8(&bytes.manifest_yaml).unwrap()).unwrap();
    let workflow_id = manifest.id.clone();
    SqliteWorkflowLibraryRepository::new(pool.clone())
        .register_package(&WorkflowPackageRecord {
            workflow_id: manifest.id,
            source_kind: "PRODUCT".into(),
            package_name: package.into(),
            package_source_path: None,
            name: manifest.name,
            category: manifest.category,
            mode: manifest.mode,
            workflow_version: manifest.workflow_version,
            workflow_json: serde_json::from_slice(&bytes.workflow_api_json).unwrap(),
            workflow_sha256: format!("{:x}", Sha256::digest(&bytes.workflow_api_json)),
            source_workflow_json: None,
            recognition_metadata_json: None,
            recipe_version: manifest.recipe_version,
            recipe_schema_version: 1,
            recipe_sha256: format!("{:x}", Sha256::digest(&bytes.recipe_yaml)),
            recipe_yaml: String::from_utf8(bytes.recipe_yaml).unwrap(),
            created_at: Utc::now(),
        })
        .await
        .unwrap();
    let bindings = Arc::new(PausingBindings {
        inner: SqliteProjectWorkflowBindingRepository::new(pool.clone()),
        write: Pause::default(),
        list: Pause::default(),
    });
    let runtime = Arc::new(SqliteWorkflowRuntimeRepository::new(pool.clone()));
    let version = runtime
        .list_versions()
        .await
        .unwrap()
        .into_iter()
        .find(|v| v.workflow_id == workflow_id)
        .unwrap();
    let version_id = version.workflow_version_id;
    let recipe_id = version.recipes[0].recipe_id.clone();
    let states = Arc::new(SqliteWorkflowRuntimeStateRepository::new(pool.clone()));
    let clock = Arc::new(crate::infrastructure::time::SystemClock);
    let registry = Arc::new(
        WorkflowRegistryService::new(
            runtime.clone(),
            states.clone(),
            bindings.clone(),
            clock.clone(),
        )
        .with_registry_repository(Arc::new(SqliteWorkflowRegistryRepository::new(
            pool.clone(),
        )))
        .with_runtime_artifact_repository(Arc::new(SqliteWorkflowRuntimeArtifactRepository::new(
            pool.clone(),
        )))
        .with_package_store(Arc::new(FileSystemWorkflowPackageStore::new(
            root,
            dir.path().join("staging"),
        ))),
    );
    assert!(
        registry
            .inspect_new_generation_availability(&version_id, &recipe_id)
            .await
            .unwrap()
            .available
    );
    let service = Arc::new(
        ProjectWorkflowBindingService::new(
            bindings.clone(),
            Arc::new(SqliteProjectRepository::new(pool.clone())),
            runtime,
            states,
            clock,
        )
        .with_registry(registry.clone()),
    );
    RaceFixture {
        _dir: dir,
        pool,
        bindings,
        registry,
        service,
        workflow_id,
        version_id,
        recipe_id,
    }
}

fn create_request(f: &RaceFixture) -> ProjectWorkflowBindingUpsertRequest {
    ProjectWorkflowBindingUpsertRequest {
        stage: "VIDEO".into(),
        mode: "DEFAULT".into(),
        workflow_version_id: f.version_id.clone(),
        recipe_id: f.recipe_id.clone(),
        expected_binding_instance_id: None,
        expected_revision: None,
    }
}

async fn assert_removed_without_binding(f: &RaceFixture) {
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT library_state FROM workflows WHERE id=?")
            .bind(&f.workflow_id)
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        "REMOVED"
    );
    assert!(f
        .bindings
        .list_for_project("project-1")
        .await
        .unwrap()
        .is_empty());
}

async fn write_first(update: bool) {
    let f = fixture().await;
    let mut request = create_request(&f);
    if update {
        let current = f
            .service
            .upsert("project-1", request.clone())
            .await
            .unwrap()
            .video_default
            .unwrap();
        request.expected_binding_instance_id = Some(current.binding_instance_id);
        request.expected_revision = Some(current.revision);
    }
    f.bindings.write.armed.store(true, Ordering::SeqCst);
    let service = f.service.clone();
    let write = tokio::spawn(async move { service.upsert("project-1", request).await });
    f.bindings.write.entered.notified().await; // availability has passed, write has not committed.
    assert!(
        f.registry.lifecycle_gate().try_lock().is_err(),
        "upsert must still own the shared lifecycle gate after availability"
    );
    let remove = f.registry.remove_workflow(&f.workflow_id);
    tokio::pin!(remove);
    std::future::poll_fn(|cx| {
        assert!(
            matches!(remove.as_mut().poll(cx), Poll::Pending),
            "remove must wait for the upsert gate"
        );
        Poll::Ready(())
    })
    .await;
    f.bindings.write.release.notify_one();
    write.await.unwrap().unwrap();
    remove.await.unwrap();
    assert_removed_without_binding(&f).await;
}

#[tokio::test]
async fn lifecycle_race_upsert_first_then_remove_clears_committed_binding() {
    tokio::time::timeout(std::time::Duration::from_secs(10), write_first(false))
        .await
        .unwrap();
}

#[tokio::test]
async fn lifecycle_race_update_first_then_remove_cannot_reuse_old_availability() {
    tokio::time::timeout(std::time::Duration::from_secs(10), write_first(true))
        .await
        .unwrap();
}

#[tokio::test]
async fn lifecycle_race_remove_first_rejects_create_and_update() {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        let f = fixture().await;
        let current = f
            .service
            .upsert("project-1", create_request(&f))
            .await
            .unwrap()
            .video_default
            .unwrap();
        f.bindings.list.armed.store(true, Ordering::SeqCst);
        let registry = f.registry.clone();
        let workflow_id = f.workflow_id.clone();
        let remove = tokio::spawn(async move { registry.remove_workflow(&workflow_id).await });
        f.bindings.list.entered.notified().await; // registry holds its gate before transaction commit.
        let upsert = f.service.upsert("project-1", create_request(&f));
        tokio::pin!(upsert);
        std::future::poll_fn(|cx| {
            assert!(matches!(upsert.as_mut().poll(cx), Poll::Pending));
            Poll::Ready(())
        })
        .await;
        f.bindings.list.release.notify_one();
        remove.await.unwrap().unwrap();
        assert!(upsert
            .await
            .unwrap_err()
            .to_string()
            .contains("WORKFLOW_REMOVED"));
        let mut update = create_request(&f);
        update.expected_binding_instance_id = Some(current.binding_instance_id.clone());
        update.expected_revision = Some(current.revision);
        assert!(f
            .service
            .upsert("project-1", update)
            .await
            .unwrap_err()
            .to_string()
            .contains("WORKFLOW_REMOVED"));
        assert!(matches!(
            f.service
                .remove(
                    "project-1",
                    ProjectWorkflowBindingRemoveRequest {
                        stage: "VIDEO".into(),
                        mode: "DEFAULT".into(),
                        expected_binding_instance_id: Some(current.binding_instance_id),
                        expected_revision: Some(current.revision),
                    }
                )
                .await,
            Err(ProjectWorkflowBindingServiceError::Conflict(_))
        ));
        assert_removed_without_binding(&f).await;
    })
    .await
    .unwrap();
}
