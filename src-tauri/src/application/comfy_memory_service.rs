use crate::application::ports::{
    ComfyAdapter, ComfyAdapterError, ProductionQueueRepository, RepositoryError, TaskRepository,
};
use serde::Serialize;
use std::sync::Arc;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComfyMemoryReleaseResult {
    pub unload_models: bool,
    pub free_memory: bool,
}

#[derive(Debug)]
pub enum ComfyMemoryReleaseError {
    Busy {
        active_tasks: usize,
        active_production_items: usize,
        comfy_running: usize,
        comfy_pending: usize,
    },
    TaskRepository(RepositoryError),
    ProductionRepository(RepositoryError),
    Comfy(ComfyAdapterError),
}

impl std::fmt::Display for ComfyMemoryReleaseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy { .. } => formatter.write_str("ComfyUI or AI Studio still has active work"),
            Self::TaskRepository(error) => write!(formatter, "task activity check failed: {error}"),
            Self::ProductionRepository(error) => {
                write!(formatter, "production activity check failed: {error}")
            }
            Self::Comfy(error) => write!(formatter, "ComfyUI memory release failed: {error}"),
        }
    }
}

impl std::error::Error for ComfyMemoryReleaseError {}

pub struct ComfyMemoryService {
    adapter: Arc<dyn ComfyAdapter>,
    task_repository: Arc<dyn TaskRepository>,
    production_queue_repository: Arc<dyn ProductionQueueRepository>,
}

impl ComfyMemoryService {
    pub fn new(
        adapter: Arc<dyn ComfyAdapter>,
        task_repository: Arc<dyn TaskRepository>,
        production_queue_repository: Arc<dyn ProductionQueueRepository>,
    ) -> Self {
        Self {
            adapter,
            task_repository,
            production_queue_repository,
        }
    }

    pub async fn release(&self) -> Result<ComfyMemoryReleaseResult, ComfyMemoryReleaseError> {
        let active_tasks = self
            .task_repository
            .list_active()
            .await
            .map_err(ComfyMemoryReleaseError::TaskRepository)?
            .into_iter()
            .filter(|task| !task.status.is_terminal())
            .count();
        let active_production_items = self
            .production_queue_repository
            .list_non_terminal_items()
            .await
            .map_err(ComfyMemoryReleaseError::ProductionRepository)?
            .len();
        let queue = self
            .adapter
            .get_queue_state()
            .await
            .map_err(ComfyMemoryReleaseError::Comfy)?;
        let comfy_running = queue.running_prompt_ids.len();
        let comfy_pending = queue.pending_prompt_ids.len();
        if active_tasks > 0 || active_production_items > 0 || comfy_running > 0 || comfy_pending > 0
        {
            return Err(ComfyMemoryReleaseError::Busy {
                active_tasks,
                active_production_items,
                comfy_running,
                comfy_pending,
            });
        }
        self.adapter
            .free_memory(true, true)
            .await
            .map_err(ComfyMemoryReleaseError::Comfy)?;
        Ok(ComfyMemoryReleaseResult {
            unload_models: true,
            free_memory: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{ComfyMemoryReleaseError, ComfyMemoryService};
    use crate::application::ports::{
        ComfyAdapter, ComfyAdapterError, ComfyEventSubscription, ComfyHealth, ComfyHistory,
        ComfyOutputData, ComfyOutputFile, ComfyQueueState, ProductionQueueRepository,
        PromptSubmission, SystemStats, TaskRepository,
    };
    use crate::domain::Task;
    use crate::infrastructure::database::{
        initialize, repositories::test_support, SqliteProductionQueueRepository,
        SqliteTaskRepository,
    };
    use async_trait::async_trait;
    use chrono::Utc;
    use serde_json::Value;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tempfile::tempdir;

    struct FakeAdapter {
        releases: Arc<AtomicUsize>,
        queue: ComfyQueueState,
    }

    #[async_trait]
    impl ComfyAdapter for FakeAdapter {
        async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
        }

        async fn get_system_stats(&self) -> Result<SystemStats, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
        }

        async fn get_object_info(&self) -> Result<Value, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
        }

        async fn get_queue_state(&self) -> Result<ComfyQueueState, ComfyAdapterError> {
            Ok(self.queue.clone())
        }

        async fn free_memory(
            &self,
            unload_models: bool,
            free_memory: bool,
        ) -> Result<(), ComfyAdapterError> {
            assert!(unload_models && free_memory);
            self.releases.fetch_add(1, Ordering::SeqCst);
            Ok(())
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

    async fn service_with_queue(
        status_task: bool,
        queue: ComfyQueueState,
    ) -> (ComfyMemoryService, Arc<AtomicUsize>, tempfile::TempDir) {
        let directory = tempdir().expect("temporary directory");
        let pool = initialize(&directory.path().join("app.db"))
            .await
            .expect("database should initialize");
        test_support::seed_task_dependencies(&pool).await;
        if status_task {
            let task = Task::new(
                "project-1",
                "workflow-1",
                "workflow-version-1",
                "recipe-1",
                Utc::now(),
            );
            SqliteTaskRepository::new(pool.clone())
                .create(&task, &task.created_event())
                .await
                .expect("task fixture");
        }
        let releases = Arc::new(AtomicUsize::new(0));
        let adapter = Arc::new(FakeAdapter {
            releases: releases.clone(),
            queue,
        });
        let task_repository: Arc<dyn TaskRepository> =
            Arc::new(SqliteTaskRepository::new(pool.clone()));
        let production_repository: Arc<dyn ProductionQueueRepository> =
            Arc::new(SqliteProductionQueueRepository::new(pool));
        let service = ComfyMemoryService::new(adapter, task_repository, production_repository);
        (service, releases, directory)
    }

    async fn service_with_task(
        status_task: bool,
    ) -> (ComfyMemoryService, Arc<AtomicUsize>, tempfile::TempDir) {
        service_with_queue(
            status_task,
            ComfyQueueState {
                running_prompt_ids: Vec::new(),
                pending_prompt_ids: Vec::new(),
            },
        )
        .await
    }

    #[tokio::test]
    async fn active_task_blocks_release_without_calling_free() {
        let (service, releases, _directory) = service_with_task(true).await;
        let error = service
            .release()
            .await
            .expect_err("active task should block release");
        assert!(matches!(
            error,
            ComfyMemoryReleaseError::Busy {
                active_tasks: 1,
                ..
            }
        ));
        assert_eq!(releases.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn idle_queue_allows_release() {
        let (service, releases, _directory) = service_with_task(false).await;
        service
            .release()
            .await
            .expect("idle queue should release memory");
        assert_eq!(releases.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn running_comfy_queue_blocks_release_without_calling_free() {
        let (service, releases, _directory) = service_with_queue(
            false,
            ComfyQueueState {
                running_prompt_ids: vec!["prompt-running".to_owned()],
                pending_prompt_ids: Vec::new(),
            },
        )
        .await;
        let error = service
            .release()
            .await
            .expect_err("running queue should block release");
        assert!(matches!(
            error,
            ComfyMemoryReleaseError::Busy {
                comfy_running: 1,
                ..
            }
        ));
        assert_eq!(releases.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn pending_comfy_queue_blocks_release_without_calling_free() {
        let (service, releases, _directory) = service_with_queue(
            false,
            ComfyQueueState {
                running_prompt_ids: Vec::new(),
                pending_prompt_ids: vec!["prompt-pending".to_owned()],
            },
        )
        .await;
        let error = service
            .release()
            .await
            .expect_err("pending queue should block release");
        assert!(matches!(
            error,
            ComfyMemoryReleaseError::Busy {
                comfy_pending: 1,
                ..
            }
        ));
        assert_eq!(releases.load(Ordering::SeqCst), 0);
    }
}
