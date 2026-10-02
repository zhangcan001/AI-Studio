//! Runs are read-only projections and delegated actions, never persisted entities.
use super::*;
use crate::application::task_cancellation_service::TaskCancellationService;
use std::future::Future;

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunListFilter {
    #[default]
    All,
    Active,
    Failed,
    Completed,
}

impl RunListFilter {
    fn includes(self, status: &str) -> bool {
        match self {
            Self::All => true,
            Self::Active => matches!(status, "QUEUED" | "RUNNING" | "PAUSED"),
            Self::Failed => matches!(status, "FAILED" | "PARTIAL"),
            Self::Completed => matches!(status, "SUCCEEDED" | "PARTIAL" | "CANCELLED"),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunList {
    pub items: Vec<ProductRun>,
    pub next_cursor: Option<String>,
    /// Existing Task/Production authorities expose bounded recent lists, not a merged cursor.
    pub coverage: &'static str,
}

fn rank(source: RunSource) -> u8 {
    match source {
        RunSource::ProductionRun => 0,
        RunSource::QueueBatch => 1,
        RunSource::Task => 2,
    }
}

impl ProductRunFacade {
    pub async fn list(
        &self,
        project_id: &str,
        filter: RunListFilter,
        cursor: Option<&str>,
    ) -> Result<RunList, ProductError> {
        crate::domain::validate_project_id(project_id)
            .map_err(|_| ProductError::new("INVALID_INPUT", "项目标识无效。", None))?;
        if cursor.is_some() {
            return Err(ProductError::new(
                "RUN_CURSOR_UNSUPPORTED",
                "当前运行列表暂不支持历史分页。",
                None,
            ));
        }
        let mut items = Vec::new();
        let mut production_ids = HashSet::new();
        let mut covered_tasks = HashSet::new();
        for run in self
            .production
            .list(project_id, 50)
            .await
            .map_err(production_error)?
        {
            production_ids.insert(run.id.clone());
            let detail = self
                .production
                .get_projection(project_id, &run.id)
                .await
                .map_err(production_error)?;
            covered_tasks.extend(
                detail
                    .stages
                    .iter()
                    .flat_map(|stage| stage.items.iter().filter_map(|item| item.task_id.clone())),
            );
            items.push(
                self.get(
                    project_id,
                    RunRef {
                        source: RunSource::ProductionRun,
                        id: run.id,
                    },
                )
                .await?,
            );
        }
        for batch in self.queue.list(project_id).await.map_err(queue_error)? {
            if batch.archived_at.is_some() {
                continue;
            }
            let detail = self
                .queue
                .get(project_id, batch.id.as_str())
                .await
                .map_err(queue_error)?;
            covered_tasks.extend(detail.items.iter().filter_map(|item| item.task_id.clone()));
            let projected = self
                .get(
                    project_id,
                    RunRef {
                        source: RunSource::QueueBatch,
                        id: batch.id.as_str().to_owned(),
                    },
                )
                .await?;
            if let Some(parent) = &projected.preferred_parent {
                // Lookup uses persisted stage/batch relationships, including older parents.
                if production_ids.insert(parent.id.clone()) {
                    let older = self
                        .production
                        .get_projection(project_id, &parent.id)
                        .await
                        .map_err(production_error)?;
                    covered_tasks.extend(older.stages.iter().flat_map(|stage| {
                        stage.items.iter().filter_map(|item| item.task_id.clone())
                    }));
                    items.push(self.get(project_id, parent.clone()).await?);
                }
            } else {
                items.push(projected);
            }
        }
        for task in self
            .tasks
            .list_recent(project_id, 50)
            .await
            .map_err(ProductError::internal)?
        {
            if covered_tasks.contains(&task.id) {
                continue;
            }
            items.push(
                self.get(
                    project_id,
                    RunRef {
                        source: RunSource::Task,
                        id: task.id,
                    },
                )
                .await?,
            );
        }
        items.retain(|run| run.project_id == project_id && filter.includes(&run.status));
        items.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| rank(a.run_ref.source).cmp(&rank(b.run_ref.source)))
                .then_with(|| a.run_ref.id.cmp(&b.run_ref.id))
        });
        Ok(RunList {
            items,
            next_cursor: None,
            coverage: "RECENT_50_TASKS_AND_PRODUCTION_WITH_UNARCHIVED_QUEUES",
        })
    }

    pub async fn start<F, Fut>(
        &self,
        project_id: &str,
        run_ref: RunRef,
        start_batch: F,
    ) -> Result<ProductRun, ProductError>
    where
        F: FnOnce(String, String) -> Fut,
        Fut: Future<Output = Result<(), ProductError>>,
    {
        let current = self.get(project_id, run_ref.clone()).await?;
        if !current
            .available_actions
            .iter()
            .any(|action| action == "START")
        {
            return Err(ProductError::new(
                "RUN_NOT_STARTABLE",
                "当前运行不能启动，请检查输入或运行状态。",
                Some("REVIEW_RUN"),
            ));
        }
        match run_ref.source {
            RunSource::QueueBatch => start_batch(project_id.to_owned(), run_ref.id.clone()).await?,
            RunSource::ProductionRun => {
                self.production
                    .run_images(project_id, &run_ref.id)
                    .await
                    .map_err(production_error)?;
            }
            RunSource::Task => {
                return Err(ProductError::new(
                    "RUN_NOT_STARTABLE",
                    "历史任务不能直接启动。",
                    None,
                ))
            }
        }
        self.get(project_id, run_ref).await
    }

    pub async fn pause(
        &self,
        project_id: &str,
        run_ref: RunRef,
    ) -> Result<ProductRun, ProductError> {
        let current = self.get(project_id, run_ref.clone()).await?;
        if run_ref.source != RunSource::QueueBatch
            || !current
                .available_actions
                .iter()
                .any(|action| action == "PAUSE")
        {
            return Err(ProductError::new(
                "RUN_PAUSE_UNSUPPORTED",
                "此运行当前不支持暂停。",
                None,
            ));
        }
        self.queue
            .pause(project_id, &run_ref.id)
            .await
            .map_err(queue_error)?;
        self.get(project_id, run_ref).await
    }

    pub async fn cancel(
        &self,
        project_id: &str,
        run_ref: RunRef,
        cancellation: &TaskCancellationService,
    ) -> Result<ProductRun, ProductError> {
        let current = self.get(project_id, run_ref.clone()).await?;
        if !current
            .available_actions
            .iter()
            .any(|action| action == "CANCEL")
        {
            return Err(ProductError::new(
                "RUN_CANCEL_UNSUPPORTED",
                "此运行当前不能取消；已完成结果和历史会保留。",
                None,
            ));
        }
        match run_ref.source {
            RunSource::QueueBatch => {
                self.queue
                    .cancel_pending(project_id, &run_ref.id)
                    .await
                    .map_err(queue_error)?;
            }
            RunSource::ProductionRun => {
                self.production
                    .cancel(project_id, &run_ref.id)
                    .await
                    .map_err(production_error)?;
            }
            RunSource::Task => {
                cancellation
                    .request_cancel(project_id, &run_ref.id)
                    .await
                    .map_err(ProductError::internal)?;
            }
        }
        self.get(project_id, run_ref).await
    }
}
