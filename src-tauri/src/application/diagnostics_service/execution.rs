//! Read-only projections of existing task facts. No storage, events or execution decisions.
use crate::application::ports::TaskDiagnosticFacts;
use crate::domain::TaskStatus;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::BTreeMap;

pub const RECENT_TASK_LIMIT: u32 = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TelemetryCompleteness {
    Complete,
    Partial,
    LegacyUnavailable,
    Invalid,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseDurations {
    pub prepare_ms: Option<i64>,
    pub submit_ms: Option<i64>,
    pub queue_wait_ms: Option<i64>,
    pub execution_ms: Option<i64>,
    pub collection_ms: Option<i64>,
    pub total_ms: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskTimeline {
    pub task_id: String,
    pub project_id: String,
    pub status: String,
    pub completeness: TelemetryCompleteness,
    pub durations: PhaseDurations,
    pub created_at: DateTime<Utc>,
    pub queued_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub prepare_started_at: Option<DateTime<Utc>>,
    pub prepared_at: Option<DateTime<Utc>>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub execution_started_at: Option<DateTime<Utc>>,
    pub execution_finished_at: Option<DateTime<Utc>>,
    pub collection_finished_at: Option<DateTime<Utc>>,
    pub generation_execution_id: Option<String>,
    pub runtime_profile: Option<String>,
    pub concurrency_class: Option<String>,
}

impl TaskTimeline {
    pub fn from_facts(task: &TaskDiagnosticFacts) -> Self {
        let telemetry = &task.telemetry;
        let observed = [
            telemetry.prepare_started_at,
            telemetry.prepared_at,
            telemetry.submitted_at,
            telemetry.execution_started_at,
            telemetry.execution_finished_at,
            telemetry.collection_finished_at,
        ];
        // Compare successive *observed* stages too: missing intermediate stages
        // must not hide an impossible persisted ordering. Never rewrite facts.
        let ordered: Vec<_> = observed.iter().flatten().copied().collect();
        let invalid = telemetry.validate(task.created_at, task.queued_at).is_err()
            || ordered.windows(2).any(|pair| pair[1] < pair[0])
            || task.queued_at.is_some_and(|at| at < task.created_at)
            || task.started_at.is_some_and(|at| at < task.created_at)
            || task.finished_at.is_some_and(|at| {
                at < task.created_at
                    || task.started_at.is_some_and(|start| at < start)
                    || ordered.last().is_some_and(|last| at < *last)
            });
        let any_metadata = telemetry.generation_execution_id.is_some()
            || telemetry.compiled_workflow_sha256.is_some()
            || telemetry.runtime_profile.is_some()
            || telemetry.concurrency_class.is_some();
        let completeness = if invalid {
            TelemetryCompleteness::Invalid
        } else if observed.iter().all(Option::is_none) && !any_metadata && task.status.is_terminal()
        {
            // A terminal record with no collected metadata is unavailable.
            // A newly-created/current task is PARTIAL, not falsely labelled old.
            // There is no invented task creation date cutoff.
            TelemetryCompleteness::LegacyUnavailable
        } else if observed.iter().all(Option::is_some) && task.queued_at.is_some() {
            TelemetryCompleteness::Complete
        } else {
            TelemetryCompleteness::Partial
        };
        let values = telemetry.durations(task.created_at, task.queued_at);
        let durations = if invalid {
            PhaseDurations::default()
        } else {
            PhaseDurations {
                prepare_ms: values.prepare_ms,
                submit_ms: values.submit_ms,
                queue_wait_ms: values.queue_wait_ms,
                execution_ms: values.comfy_execution_ms,
                collection_ms: values.collection_ms,
                total_ms: values.total_ms,
            }
        };
        Self {
            task_id: task.id.to_string(),
            project_id: task.project_id.clone(),
            status: task.status.as_str().to_owned(),
            completeness,
            durations,
            created_at: task.created_at,
            queued_at: task.queued_at,
            started_at: task.started_at,
            finished_at: task.finished_at,
            prepare_started_at: telemetry.prepare_started_at,
            prepared_at: telemetry.prepared_at,
            submitted_at: telemetry.submitted_at,
            execution_started_at: telemetry.execution_started_at,
            execution_finished_at: telemetry.execution_finished_at,
            collection_finished_at: telemetry.collection_finished_at,
            generation_execution_id: telemetry.generation_execution_id.clone(),
            runtime_profile: telemetry.runtime_profile.clone(),
            concurrency_class: telemetry.concurrency_class.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DurationSample {
    pub median_ms: Option<f64>,
    pub sample_count: usize,
}

fn sample(
    tasks: &[TaskTimeline],
    value: impl Fn(&PhaseDurations) -> Option<i64>,
) -> DurationSample {
    let mut values: Vec<_> = tasks
        .iter()
        .filter_map(|task| value(&task.durations))
        .collect();
    values.sort_unstable();
    let len = values.len();
    let median_ms = (len > 0).then(|| {
        if len % 2 == 0 {
            values[len / 2 - 1] as f64 / 2.0 + values[len / 2] as f64 / 2.0
        } else {
            values[len / 2] as f64
        }
    });
    DurationSample {
        median_ms,
        sample_count: len,
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionHealth {
    pub project_id: String,
    pub window_limit: u32,
    pub sample_count: usize,
    pub active: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub telemetry_complete: usize,
    pub telemetry_partial: usize,
    pub telemetry_unavailable: usize,
    pub telemetry_invalid: usize,
    pub prepare: DurationSample,
    pub submit: DurationSample,
    pub queue_wait: DurationSample,
    pub execution: DurationSample,
    pub collection: DurationSample,
    pub total: DurationSample,
}

impl ExecutionHealth {
    pub fn from_recent(project_id: &str, tasks: &[TaskDiagnosticFacts]) -> Self {
        let tasks: Vec<_> = tasks
            .iter()
            .filter(|task| task.project_id == project_id)
            .take(RECENT_TASK_LIMIT as usize)
            .map(TaskTimeline::from_facts)
            .collect();
        let status_count = |status: &str| tasks.iter().filter(|task| task.status == status).count();
        let completeness_count = |value| {
            tasks
                .iter()
                .filter(|task| task.completeness == value)
                .count()
        };
        Self {
            project_id: project_id.to_owned(),
            window_limit: RECENT_TASK_LIMIT,
            sample_count: tasks.len(),
            active: tasks
                .iter()
                .filter(|task| {
                    !matches!(task.status.as_str(), "SUCCEEDED" | "FAILED" | "CANCELLED")
                })
                .count(),
            succeeded: status_count("SUCCEEDED"),
            failed: status_count("FAILED"),
            cancelled: status_count("CANCELLED"),
            telemetry_complete: completeness_count(TelemetryCompleteness::Complete),
            telemetry_partial: completeness_count(TelemetryCompleteness::Partial),
            telemetry_unavailable: completeness_count(TelemetryCompleteness::LegacyUnavailable),
            telemetry_invalid: completeness_count(TelemetryCompleteness::Invalid),
            prepare: sample(&tasks, |d| d.prepare_ms),
            submit: sample(&tasks, |d| d.submit_ms),
            queue_wait: sample(&tasks, |d| d.queue_wait_ms),
            execution: sample(&tasks, |d| d.execution_ms),
            collection: sample(&tasks, |d| d.collection_ms),
            total: sample(&tasks, |d| d.total_ms),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentFailure {
    pub code: String,
    pub status: String,
    pub count: usize,
    pub affected_task_count: usize,
    pub latest_at: Option<DateTime<Utc>>,
    pub project_id: String,
    pub example_task_id: String,
}

pub fn recent_failures(project_id: &str, tasks: &[TaskDiagnosticFacts]) -> Vec<RecentFailure> {
    let mut groups = BTreeMap::<String, RecentFailure>::new();
    for task in tasks
        .iter()
        .filter(|task| task.project_id == project_id)
        .take(RECENT_TASK_LIMIT as usize)
    {
        if task.status != TaskStatus::Failed {
            continue;
        }
        // Only the persisted machine-code field participates, never Display/message/raw JSON.
        let code = task
            .error_code
            .as_deref()
            .filter(|code| {
                code.len() <= 80
                    && code.len() >= 2
                    && code.bytes().next().is_some_and(|c| c.is_ascii_uppercase())
                    && code
                        .bytes()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
            })
            .unwrap_or("UNKNOWN");
        let at = task.finished_at;
        let group = groups
            .entry(code.to_owned())
            .or_insert_with(|| RecentFailure {
                code: code.to_owned(),
                status: "FAILED".to_owned(),
                count: 0,
                affected_task_count: 0,
                latest_at: at,
                project_id: project_id.to_owned(),
                example_task_id: task.id.to_string(),
            });
        group.count += 1;
        group.affected_task_count += 1;
        if at > group.latest_at {
            group.latest_at = at;
            if at.is_some() {
                group.example_task_id = task.id.to_string();
            }
        }
    }
    let mut groups: Vec<_> = groups.into_values().collect();
    groups.sort_by(|a, b| {
        b.latest_at
            .cmp(&a.latest_at)
            .then_with(|| a.code.cmp(&b.code))
    });
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Task, TaskTelemetry};
    use chrono::TimeZone;

    fn fixture() -> TaskDiagnosticFacts {
        let mut task = Task::new(
            "project-a",
            "workflow",
            "version",
            "recipe",
            Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        );
        task.status = TaskStatus::Succeeded;
        TaskDiagnosticFacts::from(&task)
    }

    #[test]
    fn phase13_case2_timeline_complete_partial_legacy_invalid() {
        let mut task = fixture();
        let at = |ms| task.created_at + chrono::Duration::milliseconds(ms);
        task.queued_at = Some(at(1));
        task.telemetry = TaskTelemetry {
            prepare_started_at: Some(at(2)),
            prepared_at: Some(at(12)),
            submitted_at: Some(at(20)),
            execution_started_at: Some(at(30)),
            execution_finished_at: Some(at(80)),
            collection_finished_at: Some(at(90)),
            ..Default::default()
        };
        let complete = TaskTimeline::from_facts(&task);
        assert_eq!(complete.completeness, TelemetryCompleteness::Complete);
        assert_eq!(
            complete.durations,
            PhaseDurations {
                prepare_ms: Some(10),
                submit_ms: Some(8),
                queue_wait_ms: Some(29),
                execution_ms: Some(50),
                collection_ms: Some(10),
                total_ms: Some(90),
            }
        );
        task.telemetry.collection_finished_at = None;
        assert_eq!(
            TaskTimeline::from_facts(&task).completeness,
            TelemetryCompleteness::Partial
        );
        task.telemetry.prepared_at = Some(task.created_at + chrono::Duration::milliseconds(50));
        let before = task.clone();
        assert_eq!(
            TaskTimeline::from_facts(&task).completeness,
            TelemetryCompleteness::Invalid
        );
        assert_eq!(
            TaskTimeline::from_facts(&task).durations,
            PhaseDurations::default()
        );
        assert_eq!(before, task);
        task.telemetry = TaskTelemetry::default();
        assert_eq!(
            TaskTimeline::from_facts(&task).completeness,
            TelemetryCompleteness::LegacyUnavailable
        );
        task.status = TaskStatus::Created;
        assert_eq!(
            TaskTimeline::from_facts(&task).completeness,
            TelemetryCompleteness::Partial
        );
        task.telemetry.generation_execution_id = Some("execution".to_owned());
        assert_eq!(
            TaskTimeline::from_facts(&task).completeness,
            TelemetryCompleteness::Partial
        );
    }

    #[test]
    fn phase13_case3_missing_is_not_zero_in_timeline_or_aggregation() {
        let legacy = fixture();
        assert_eq!(
            TaskTimeline::from_facts(&legacy).durations,
            PhaseDurations::default()
        );
        let mut measured = fixture();
        measured.telemetry.execution_started_at = Some(measured.created_at);
        measured.telemetry.execution_finished_at =
            Some(measured.created_at + chrono::Duration::milliseconds(40));
        let health = ExecutionHealth::from_recent("project-a", &[legacy.clone(), measured]);
        assert_eq!(health.execution.sample_count, 1);
        assert_eq!(health.execution.median_ms, Some(40.0));
        assert_eq!(health.prepare.sample_count, 0);
        assert_eq!(health.prepare.median_ms, None);
        assert_eq!(
            ExecutionHealth::from_recent("project-b", &[legacy]).sample_count,
            0
        );
    }

    #[test]
    fn phase13_undated_failure_is_null_not_creation_time_and_dated_sample_wins() {
        let mut undated = fixture();
        undated.status = TaskStatus::Failed;
        let groups = recent_failures("project-a", &[undated.clone()]);
        assert_eq!(groups[0].latest_at, None);
        assert!(serde_json::to_value(&groups).unwrap()[0]["latestAt"].is_null());
        let mut dated = undated.clone();
        dated.id = fixture().id;
        dated.finished_at = Some(dated.created_at + chrono::Duration::seconds(7));
        let groups = recent_failures("project-a", &[undated.clone(), dated.clone()]);
        assert_eq!(groups[0].latest_at, dated.finished_at);
        assert_eq!(groups[0].count, 2);
        let reverse = recent_failures("project-a", &[dated.clone(), undated]);
        assert_eq!(reverse[0].latest_at, dated.finished_at);
    }

    #[test]
    fn phase13_case4_failures_bounded_scoped_and_no_message_inference() {
        let mut failed = fixture();
        failed.status = TaskStatus::Failed;
        failed.error_code = Some("COMFY_OFFLINE".to_owned());
        let mut unknown = failed.clone();
        unknown.error_code = None;
        let mut wrong_project = failed.clone();
        wrong_project.project_id = "project-b".to_owned();
        let groups = recent_failures("project-a", &[failed.clone(), unknown, wrong_project]);
        assert_eq!(groups.len(), 2);
        assert!(groups.iter().any(|group| group.code == "UNKNOWN"));
        let json = serde_json::to_string(&groups).unwrap();
        assert!(!json.contains("PRIVATE_PROMPT"));
        assert!(!json.contains("secret"));
        let bounded = recent_failures("project-a", &vec![failed; 60]);
        assert_eq!(bounded[0].count, 50);
    }
}
