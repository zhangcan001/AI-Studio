//! Application-level data repair jobs.
//!
//! Recipes and workflow versions are immutable, so a data defect produced by an
//! older recognizer can only be repaired by publishing a new recipe version.
//! Each job is identified by a stable `_vN` id and runs at most once per
//! database: the runner records RUNNING / COMPLETED / FAILED markers in
//! `app_repair_jobs` (migration 040). A failure for one item is recorded in the
//! job summary and never aborts the other items or the other jobs.

use crate::application::ports::{
    Clock, RepairJobRecord, RepairJobRepository, RepositoryError, REPAIR_JOB_COMPLETED,
    REPAIR_JOB_FAILED, REPAIR_JOB_SKIPPED,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub mod recipe_jobs;
pub mod registry_jobs;

/// One unit of work found by a job's read-only planning pass.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairItem {
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub reason: String,
    /// Semantic workflow version (for example `1.2.0`) of the package.
    #[serde(default)]
    pub workflow_version: String,
    /// Recipe version of the package being repaired.
    #[serde(default)]
    pub recipe_version: String,
    #[serde(default)]
    pub package_name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RepairPlan {
    pub items: Vec<RepairItem>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairFailure {
    pub recipe_id: String,
    pub message: String,
}

/// Durable progress for one multi-stage repair item.  A published immutable
/// recipe must not be published again merely because its promotion/binding
/// retarget failed later in the same repair job.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RepairItemCheckpoint {
    #[default]
    Planned,
    Published,
    Retargeted,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairItemProgress {
    pub item: RepairItem,
    pub checkpoint: RepairItemCheckpoint,
    #[serde(default)]
    pub workflow_version_id: Option<String>,
    #[serde(default)]
    pub old_recipe_id: Option<String>,
    #[serde(default)]
    pub new_recipe_id: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairSummary {
    pub planned: usize,
    pub repaired: usize,
    pub skipped: usize,
    /// Items that need a human decision (for example an ambiguous re-mapping).
    #[serde(default)]
    pub needs_review: Vec<RepairItem>,
    #[serde(default)]
    pub failed: Vec<RepairFailure>,
    /// New recipe ids published by this job, in publication order.
    #[serde(default)]
    pub published_recipe_ids: Vec<String>,
    /// Per-item checkpoints used to resume a partially completed repair
    /// without republishing an immutable recipe patch.
    #[serde(default)]
    pub progress: Vec<RepairItemProgress>,
}

#[async_trait]
pub trait RepairJob: Send + Sync {
    /// Stable identifier, always suffixed with `_vN`.
    fn id(&self) -> &'static str;

    /// Read-only planning pass.
    async fn plan(&self) -> Result<RepairPlan, String>;

    /// Apply the plan. Per-item failures belong in the summary; an `Err` means
    /// the whole job could not run and it will be retried on the next start.
    async fn apply(&self, plan: &RepairPlan) -> Result<RepairSummary, String>;

    /// Plan against a previous persisted summary. Existing jobs keep the
    /// simple API; multi-stage jobs can re-add non-terminal items from the
    /// summary instead of relying on a fresh source-package scan.
    async fn plan_with_previous(
        &self,
        _previous: Option<&RepairSummary>,
    ) -> Result<RepairPlan, String> {
        self.plan().await
    }

    /// Apply a plan while recovering item-level checkpoints from a previous
    /// failed run. Existing jobs default to their original implementation.
    async fn apply_with_previous(
        &self,
        plan: &RepairPlan,
        _previous: Option<&RepairSummary>,
    ) -> Result<RepairSummary, String> {
        self.apply(plan).await
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairJobOutcome {
    pub job_id: String,
    pub status: String,
    pub summary: Option<RepairSummary>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairJobStatusView {
    pub job_id: String,
    pub status: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub summary: Option<RepairSummary>,
    pub summary_raw: String,
}

impl From<RepairJobRecord> for RepairJobStatusView {
    fn from(record: RepairJobRecord) -> Self {
        Self {
            summary: serde_json::from_str(&record.summary_json).ok(),
            job_id: record.job_id,
            status: record.status,
            started_at: record.started_at.to_rfc3339(),
            completed_at: record.completed_at.map(|value| value.to_rfc3339()),
            summary_raw: record.summary_json,
        }
    }
}

pub struct RepairJobRunner {
    repository: Arc<dyn RepairJobRepository>,
    clock: Arc<dyn Clock>,
    jobs: Vec<Arc<dyn RepairJob>>,
    run_gate: tokio::sync::Mutex<()>,
}

impl RepairJobRunner {
    pub fn new(
        repository: Arc<dyn RepairJobRepository>,
        clock: Arc<dyn Clock>,
        jobs: Vec<Arc<dyn RepairJob>>,
    ) -> Self {
        Self {
            repository,
            clock,
            jobs,
            run_gate: tokio::sync::Mutex::new(()),
        }
    }

    pub fn job_ids(&self) -> Vec<&'static str> {
        self.jobs.iter().map(|job| job.id()).collect()
    }

    pub async fn status(&self) -> Result<Vec<RepairJobStatusView>, RepositoryError> {
        Ok(self
            .repository
            .list()
            .await?
            .into_iter()
            .map(RepairJobStatusView::from)
            .collect())
    }

    /// Make every registered job eligible to run again (for example after a
    /// package or backup import brought legacy recipes into the library).
    pub async fn mark_all_pending(&self) -> Result<(), RepositoryError> {
        // Resetting a marker while run_pending is planning/applying the same
        // job would let the runner finish with a COMPLETED marker immediately
        // after the reset.  Use the same gate for both operations so an
        // import/reset is ordered before or after a complete run, never in the
        // middle of one.
        let _guard = self.run_gate.lock().await;
        for job in &self.jobs {
            self.repository.reset(job.id()).await?;
        }
        Ok(())
    }

    /// Run every job that has not completed yet, in registration order.
    pub async fn run_pending(&self) -> Vec<RepairJobOutcome> {
        let _guard = self.run_gate.lock().await;
        let mut outcomes = Vec::new();
        for job in &self.jobs {
            let previous_summary = match self.repository.find(job.id()).await {
                Ok(Some(record))
                    if record.status == REPAIR_JOB_COMPLETED
                        || record.status == REPAIR_JOB_SKIPPED =>
                {
                    continue;
                }
                Ok(Some(record)) => serde_json::from_str(&record.summary_json).ok(),
                Ok(None) => None,
                Err(error) => {
                    outcomes.push(RepairJobOutcome {
                        job_id: job.id().to_owned(),
                        status: REPAIR_JOB_FAILED.to_owned(),
                        summary: None,
                        error: Some(error.to_string()),
                    });
                    continue;
                }
            };
            outcomes.push(self.run_one(job.as_ref(), previous_summary.as_ref()).await);
        }
        outcomes
    }

    async fn run_one(
        &self,
        job: &dyn RepairJob,
        previous_summary: Option<&RepairSummary>,
    ) -> RepairJobOutcome {
        let job_id = job.id().to_owned();
        if let Err(error) = self
            .repository
            .mark_started(&job_id, self.clock.now())
            .await
        {
            return RepairJobOutcome {
                job_id,
                status: REPAIR_JOB_FAILED.to_owned(),
                summary: None,
                error: Some(error.to_string()),
            };
        }
        let result = match job.plan_with_previous(previous_summary).await {
            Ok(plan) if plan.items.is_empty() => {
                // A failed run may have only left human-review items. Those
                // are intentionally not re-planned, but their evidence must
                // remain visible when the job becomes COMPLETED.
                let mut summary = previous_summary.cloned().unwrap_or_default();
                summary.planned = 0;
                summary.failed.clear();
                Ok(summary)
            }
            Ok(plan) => {
                job.apply_with_previous(&plan, previous_summary)
                    .await
                    .map(|mut summary| {
                        summary.planned = plan.items.len();
                        summary
                    })
            }
            Err(error) => Err(error),
        };
        let (status, summary, error) = match result {
            Ok(summary) if summary.failed.is_empty() => (REPAIR_JOB_COMPLETED, Some(summary), None),
            Ok(summary) => (
                REPAIR_JOB_FAILED,
                Some(summary.clone()),
                Some(format!(
                    "{} repair item(s) failed; the job remains eligible for retry",
                    summary.failed.len()
                )),
            ),
            Err(error) => (REPAIR_JOB_FAILED, None, Some(error)),
        };
        let summary_json = match (&summary, &error) {
            (Some(summary), _) => serde_json::to_string(summary).unwrap_or_else(|_| "{}".into()),
            (None, Some(error)) => serde_json::json!({ "error": error }).to_string(),
            (None, None) => "{}".to_owned(),
        };
        if let Err(persist_error) = self
            .repository
            .mark_finished(&job_id, status, self.clock.now(), &summary_json)
            .await
        {
            tracing::warn!(job_id = %job_id, error = %persist_error, "repair job marker could not be persisted");
        }
        match status {
            REPAIR_JOB_COMPLETED => tracing::info!(
                job_id = %job_id,
                repaired = summary.as_ref().map_or(0, |summary| summary.repaired),
                failed = summary.as_ref().map_or(0, |summary| summary.failed.len()),
                needs_review = summary.as_ref().map_or(0, |summary| summary.needs_review.len()),
                "repair job completed"
            ),
            _ => tracing::warn!(job_id = %job_id, error = ?error, "repair job failed"),
        }
        RepairJobOutcome {
            job_id,
            status: status.to_owned(),
            summary,
            error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::database::{initialize, SqliteRepairJobRepository};
    use chrono::{DateTime, Utc};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use tempfile::TempDir;
    use tokio::sync::Notify;

    struct FixedClock;
    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            DateTime::parse_from_rfc3339("2026-09-28T08:00:00Z")
                .unwrap()
                .with_timezone(&Utc)
        }
    }

    /// A job that "repairs" `remaining` items once: after apply, plan is empty.
    struct CountingJob {
        id: &'static str,
        remaining: AtomicUsize,
        applied: AtomicUsize,
        fail_item: Option<&'static str>,
        fail_plan: bool,
    }

    impl CountingJob {
        fn new(id: &'static str, items: usize) -> Self {
            Self {
                id,
                remaining: AtomicUsize::new(items),
                applied: AtomicUsize::new(0),
                fail_item: None,
                fail_plan: false,
            }
        }
    }

    #[async_trait]
    impl RepairJob for CountingJob {
        fn id(&self) -> &'static str {
            self.id
        }

        async fn plan(&self) -> Result<RepairPlan, String> {
            if self.fail_plan {
                return Err("plan failed".to_owned());
            }
            Ok(RepairPlan {
                items: (0..self.remaining.load(Ordering::SeqCst))
                    .map(|index| RepairItem {
                        recipe_id: format!("rcp_{index}"),
                        ..RepairItem::default()
                    })
                    .collect(),
            })
        }

        async fn apply(&self, plan: &RepairPlan) -> Result<RepairSummary, String> {
            let mut summary = RepairSummary::default();
            for item in &plan.items {
                if Some(item.recipe_id.as_str()) == self.fail_item {
                    summary.failed.push(RepairFailure {
                        recipe_id: item.recipe_id.clone(),
                        message: "boom".to_owned(),
                    });
                    continue;
                }
                summary.repaired += 1;
                self.applied.fetch_add(1, Ordering::SeqCst);
            }
            self.remaining.store(0, Ordering::SeqCst);
            Ok(summary)
        }
    }

    struct CheckpointJob {
        b_retarget_fail_once: AtomicBool,
        c_apply_fail_once: AtomicBool,
        a_publish_count: AtomicUsize,
        b_publish_count: AtomicUsize,
        c_publish_count: AtomicUsize,
        b_retarget_count: AtomicUsize,
        c_apply_count: AtomicUsize,
    }

    impl CheckpointJob {
        fn new() -> Self {
            Self {
                b_retarget_fail_once: AtomicBool::new(true),
                c_apply_fail_once: AtomicBool::new(true),
                a_publish_count: AtomicUsize::new(0),
                b_publish_count: AtomicUsize::new(0),
                c_publish_count: AtomicUsize::new(0),
                b_retarget_count: AtomicUsize::new(0),
                c_apply_count: AtomicUsize::new(0),
            }
        }
    }

    fn checkpoint_item(recipe_id: &str) -> RepairItem {
        RepairItem {
            recipe_id: recipe_id.to_owned(),
            ..RepairItem::default()
        }
    }

    fn same_checkpoint_item(left: &RepairItem, right: &RepairItem) -> bool {
        left.recipe_id == right.recipe_id
    }

    fn upsert_checkpoint(
        summary: &mut RepairSummary,
        item: &RepairItem,
        checkpoint: RepairItemCheckpoint,
        new_recipe_id: Option<&str>,
        last_error: Option<String>,
    ) {
        let progress = RepairItemProgress {
            item: item.clone(),
            checkpoint,
            workflow_version_id: Some("workflow-version".to_owned()),
            old_recipe_id: Some(item.recipe_id.clone()),
            new_recipe_id: new_recipe_id.map(str::to_owned),
            last_error,
        };
        if let Some(existing) = summary
            .progress
            .iter_mut()
            .find(|existing| same_checkpoint_item(&existing.item, item))
        {
            *existing = progress;
        } else {
            summary.progress.push(progress);
        }
    }

    #[async_trait]
    impl RepairJob for CheckpointJob {
        fn id(&self) -> &'static str {
            "checkpoint_retry_v1"
        }

        async fn plan(&self) -> Result<RepairPlan, String> {
            self.plan_with_previous(None).await
        }

        async fn plan_with_previous(
            &self,
            previous: Option<&RepairSummary>,
        ) -> Result<RepairPlan, String> {
            let mut items = vec![
                checkpoint_item("item_a"),
                checkpoint_item("item_b"),
                checkpoint_item("item_c"),
            ];
            if let Some(previous) = previous {
                items.retain(|item| {
                    !previous.progress.iter().any(|progress| {
                        same_checkpoint_item(&progress.item, item)
                            && progress.checkpoint == RepairItemCheckpoint::Retargeted
                    })
                });
            }
            Ok(RepairPlan { items })
        }

        async fn apply(&self, plan: &RepairPlan) -> Result<RepairSummary, String> {
            self.apply_with_previous(plan, None).await
        }

        async fn apply_with_previous(
            &self,
            plan: &RepairPlan,
            previous: Option<&RepairSummary>,
        ) -> Result<RepairSummary, String> {
            let mut summary = previous.cloned().unwrap_or_default();
            summary.failed.clear();
            for item in &plan.items {
                let previous_checkpoint = summary
                    .progress
                    .iter()
                    .find(|progress| same_checkpoint_item(&progress.item, item))
                    .map(|progress| progress.checkpoint.clone());
                match item.recipe_id.as_str() {
                    "item_a" => {
                        if previous_checkpoint.is_none() {
                            self.a_publish_count.fetch_add(1, Ordering::SeqCst);
                            upsert_checkpoint(
                                &mut summary,
                                item,
                                RepairItemCheckpoint::Published,
                                Some("patch_a"),
                                None,
                            );
                        }
                        upsert_checkpoint(
                            &mut summary,
                            item,
                            RepairItemCheckpoint::Retargeted,
                            Some("patch_a"),
                            None,
                        );
                        summary.repaired += 1;
                    }
                    "item_b" => {
                        if previous_checkpoint != Some(RepairItemCheckpoint::Published) {
                            self.b_publish_count.fetch_add(1, Ordering::SeqCst);
                            upsert_checkpoint(
                                &mut summary,
                                item,
                                RepairItemCheckpoint::Published,
                                Some("patch_b"),
                                None,
                            );
                        }
                        self.b_retarget_count.fetch_add(1, Ordering::SeqCst);
                        if self.b_retarget_fail_once.swap(false, Ordering::SeqCst) {
                            let message = "retarget failed once".to_owned();
                            upsert_checkpoint(
                                &mut summary,
                                item,
                                RepairItemCheckpoint::Published,
                                Some("patch_b"),
                                Some(message.clone()),
                            );
                            summary.failed.push(RepairFailure {
                                recipe_id: item.recipe_id.clone(),
                                message,
                            });
                        } else {
                            upsert_checkpoint(
                                &mut summary,
                                item,
                                RepairItemCheckpoint::Retargeted,
                                Some("patch_b"),
                                None,
                            );
                            summary.repaired += 1;
                        }
                    }
                    "item_c" => {
                        self.c_apply_count.fetch_add(1, Ordering::SeqCst);
                        if self.c_apply_fail_once.swap(false, Ordering::SeqCst) {
                            let message = "apply failed once".to_owned();
                            upsert_checkpoint(
                                &mut summary,
                                item,
                                RepairItemCheckpoint::Planned,
                                None,
                                Some(message.clone()),
                            );
                            summary.failed.push(RepairFailure {
                                recipe_id: item.recipe_id.clone(),
                                message,
                            });
                        } else {
                            self.c_publish_count.fetch_add(1, Ordering::SeqCst);
                            upsert_checkpoint(
                                &mut summary,
                                item,
                                RepairItemCheckpoint::Retargeted,
                                Some("patch_c"),
                                None,
                            );
                            summary.repaired += 1;
                        }
                    }
                    other => panic!("unexpected checkpoint item {other}"),
                }
            }
            Ok(summary)
        }
    }

    struct GateJob {
        started: Notify,
        release: Notify,
        hold: AtomicBool,
    }

    #[async_trait]
    impl RepairJob for GateJob {
        fn id(&self) -> &'static str {
            "gate_v1"
        }

        async fn plan(&self) -> Result<RepairPlan, String> {
            Ok(RepairPlan {
                items: vec![checkpoint_item("gate")],
            })
        }

        async fn apply(&self, _plan: &RepairPlan) -> Result<RepairSummary, String> {
            self.started.notify_waiters();
            while self.hold.load(Ordering::SeqCst) {
                self.release.notified().await;
            }
            Ok(RepairSummary {
                repaired: 1,
                ..RepairSummary::default()
            })
        }
    }

    async fn repository() -> (TempDir, Arc<dyn RepairJobRepository>) {
        let directory = tempfile::tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        (directory, Arc::new(SqliteRepairJobRepository::new(pool)))
    }

    #[tokio::test]
    async fn run_pending_repairs_is_idempotent() {
        let (_directory, repository) = repository().await;
        let job = Arc::new(CountingJob::new("counting_v1", 2));
        let runner =
            RepairJobRunner::new(repository.clone(), Arc::new(FixedClock), vec![job.clone()]);

        let first = runner.run_pending().await;
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].status, REPAIR_JOB_COMPLETED);
        assert_eq!(first[0].summary.as_ref().unwrap().repaired, 2);
        assert_eq!(first[0].summary.as_ref().unwrap().planned, 2);

        let second = runner.run_pending().await;
        assert!(second.is_empty(), "a completed job must not run again");
        assert_eq!(job.applied.load(Ordering::SeqCst), 2);

        let status = runner.status().await.unwrap();
        assert_eq!(status[0].status, REPAIR_JOB_COMPLETED);
        assert_eq!(status[0].summary.as_ref().unwrap().repaired, 2);

        runner.mark_all_pending().await.unwrap();
        let third = runner.run_pending().await;
        assert_eq!(third.len(), 1);
        assert_eq!(third[0].summary.as_ref().unwrap().repaired, 0);
    }

    #[tokio::test]
    async fn repair_job_failure_isolated_per_recipe() {
        let (_directory, repository) = repository().await;
        let mut failing_item = CountingJob::new("item_failure_v1", 3);
        failing_item.fail_item = Some("rcp_1");
        let failing_item = Arc::new(failing_item);
        let mut failing_plan = CountingJob::new("plan_failure_v1", 1);
        failing_plan.fail_plan = true;
        let healthy = Arc::new(CountingJob::new("healthy_v1", 1));
        let runner = RepairJobRunner::new(
            repository.clone(),
            Arc::new(FixedClock),
            vec![
                failing_item.clone(),
                Arc::new(failing_plan),
                healthy.clone(),
            ],
        );

        let outcomes = runner.run_pending().await;
        assert_eq!(outcomes.len(), 3);
        let item = &outcomes[0];
        assert_eq!(item.status, REPAIR_JOB_FAILED);
        assert_eq!(item.summary.as_ref().unwrap().repaired, 2);
        assert_eq!(item.summary.as_ref().unwrap().failed.len(), 1);
        assert!(item.error.is_some());
        assert_eq!(outcomes[1].status, REPAIR_JOB_FAILED);
        assert_eq!(outcomes[2].status, REPAIR_JOB_COMPLETED);
        assert_eq!(healthy.applied.load(Ordering::SeqCst), 1);

        // A failed job stays eligible and is retried on the next pass.  The
        // item-failure job has no remaining work after its first pass, while
        // the plan-failure job still fails at planning.
        let retry = runner.run_pending().await;
        assert_eq!(retry.len(), 2);
        assert_eq!(retry[0].job_id, "item_failure_v1");
        assert_eq!(retry[0].status, REPAIR_JOB_COMPLETED);
        assert_eq!(retry[1].job_id, "plan_failure_v1");
        assert_eq!(retry[1].status, REPAIR_JOB_FAILED);
        assert_eq!(failing_item.applied.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn repair_job_retries_item_checkpoints_without_duplicate_publish() {
        let (_directory, repository) = repository().await;
        let job = Arc::new(CheckpointJob::new());
        let runner =
            RepairJobRunner::new(repository.clone(), Arc::new(FixedClock), vec![job.clone()]);

        let first = runner.run_pending().await;
        assert_eq!(first[0].status, REPAIR_JOB_FAILED);
        let first_summary = first[0].summary.as_ref().unwrap();
        assert_eq!(first_summary.failed.len(), 2);
        assert_eq!(
            first_summary
                .progress
                .iter()
                .find(|progress| progress.item.recipe_id == "item_a")
                .unwrap()
                .checkpoint,
            RepairItemCheckpoint::Retargeted
        );
        assert_eq!(
            first_summary
                .progress
                .iter()
                .find(|progress| progress.item.recipe_id == "item_b")
                .unwrap()
                .checkpoint,
            RepairItemCheckpoint::Published
        );

        let second = runner.run_pending().await;
        assert_eq!(second[0].status, REPAIR_JOB_COMPLETED);
        let second_summary = second[0].summary.as_ref().unwrap();
        assert!(second_summary.failed.is_empty());
        assert_eq!(second_summary.progress.len(), 3);
        assert!(second_summary
            .progress
            .iter()
            .all(|progress| progress.checkpoint == RepairItemCheckpoint::Retargeted));
        assert_eq!(job.a_publish_count.load(Ordering::SeqCst), 1);
        assert_eq!(job.b_publish_count.load(Ordering::SeqCst), 1);
        assert_eq!(job.c_publish_count.load(Ordering::SeqCst), 1);
        assert_eq!(job.b_retarget_count.load(Ordering::SeqCst), 2);
        assert_eq!(job.c_apply_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn mark_all_pending_waits_for_active_apply() {
        let (_directory, repository) = repository().await;
        let job = Arc::new(GateJob {
            started: Notify::new(),
            release: Notify::new(),
            hold: AtomicBool::new(true),
        });
        let runner = Arc::new(RepairJobRunner::new(
            repository,
            Arc::new(FixedClock),
            vec![job.clone()],
        ));
        let run_runner = runner.clone();
        let started = job.started.notified();
        let run = tokio::spawn(async move { run_runner.run_pending().await });
        started.await;

        let reset_runner = runner.clone();
        let mut reset = tokio::spawn(async move { reset_runner.mark_all_pending().await });
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), &mut reset)
                .await
                .is_err(),
            "reset must not interleave with an active apply"
        );

        job.hold.store(false, Ordering::SeqCst);
        job.release.notify_waiters();
        run.await.unwrap();
        reset.await.unwrap().unwrap();
    }
}
