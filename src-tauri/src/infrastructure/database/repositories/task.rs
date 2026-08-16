use super::{
    format_datetime, i64_to_u64, insert_event, map_domain_error, map_sqlx_error, parse_datetime,
    parse_json, parse_optional_datetime, serialize_json,
};
use crate::application::ports::{RepositoryError, TaskRepository};
use crate::domain::{
    NewTaskEvent, RuntimeProvenance, StoredTaskEvent, Task, TaskError, TaskEventType, TaskId,
    TaskProgress, TaskStatus,
};
use async_trait::async_trait;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct SqliteTaskRepository {
    pool: SqlitePool,
}

impl SqliteTaskRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TaskRepository for SqliteTaskRepository {
    async fn create(
        &self,
        task: &Task,
        created_event: &NewTaskEvent,
    ) -> Result<StoredTaskEvent, RepositoryError> {
        if task.status != TaskStatus::Created {
            return Err(RepositoryError::integrity(
                "task create requires status CREATED",
            ));
        }
        validate_event(task, created_event, TaskEventType::TaskCreated)?;
        let values = TaskDbValues::from_task(task)?;
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;

        sqlx::query(
            "INSERT INTO tasks (
                id, project_id, workflow_id, workflow_version_id, recipe_id,
                app_version, build_commit, workflow_version, workflow_sha256,
                recipe_version, recipe_sha256, package_name, package_source_path,
                dynamic_binding_targets_json,
                status, prompt_id, queue_number,
                progress_mode, progress_current, progress_total, current_node_id,
                error_code, error_message, raw_error_json,
                created_at, queued_at, started_at, finished_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(task.id.as_str())
        .bind(&task.project_id)
        .bind(&task.workflow_id)
        .bind(&task.workflow_version_id)
        .bind(&task.recipe_id)
        .bind(values.app_version)
        .bind(values.build_commit)
        .bind(values.workflow_version)
        .bind(values.workflow_sha256)
        .bind(values.recipe_version)
        .bind(values.recipe_sha256)
        .bind(values.package_name)
        .bind(values.package_source_path)
        .bind(values.dynamic_binding_targets_json)
        .bind(values.status)
        .bind(values.prompt_id)
        .bind(values.queue_number)
        .bind(values.progress_mode)
        .bind(values.progress_current)
        .bind(values.progress_total)
        .bind(values.current_node_id)
        .bind(values.error_code)
        .bind(values.error_message)
        .bind(values.raw_error_json)
        .bind(values.created_at)
        .bind(values.queued_at)
        .bind(values.started_at)
        .bind(values.finished_at)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;

        let stored_event = insert_event(&mut transaction, created_event).await?;
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(stored_event)
    }

    async fn persist_transition(
        &self,
        task: &Task,
        event: &NewTaskEvent,
        expected_previous_status: TaskStatus,
    ) -> Result<StoredTaskEvent, RepositoryError> {
        if task.status == TaskStatus::Created {
            return Err(RepositoryError::integrity(
                "persist_transition cannot persist status CREATED",
            ));
        }
        if expected_previous_status == task.status || expected_previous_status.is_terminal() {
            return Err(RepositoryError::integrity(format!(
                "invalid expected previous status {} for current status {}",
                expected_previous_status.as_str(),
                task.status.as_str()
            )));
        }
        let expected_event = task.status.event_type().ok_or_else(|| {
            RepositoryError::integrity("task status does not have a transition event")
        })?;
        validate_event(task, event, expected_event)?;
        let values = TaskDbValues::from_task(task)?;
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;

        let result = sqlx::query(
            "UPDATE tasks SET
                project_id = ?, workflow_id = ?, workflow_version_id = ?, recipe_id = ?,
                app_version = ?, build_commit = ?, workflow_version = ?, workflow_sha256 = ?,
                recipe_version = ?, recipe_sha256 = ?, package_name = ?, package_source_path = ?,
                dynamic_binding_targets_json = ?,
                status = ?, prompt_id = ?, queue_number = ?,
                progress_mode = ?, progress_current = ?, progress_total = ?, current_node_id = ?,
                error_code = ?, error_message = ?, raw_error_json = ?,
                created_at = ?, queued_at = ?, started_at = ?, finished_at = ?
             WHERE id = ? AND status = ?",
        )
        .bind(&task.project_id)
        .bind(&task.workflow_id)
        .bind(&task.workflow_version_id)
        .bind(&task.recipe_id)
        .bind(values.app_version)
        .bind(values.build_commit)
        .bind(values.workflow_version)
        .bind(values.workflow_sha256)
        .bind(values.recipe_version)
        .bind(values.recipe_sha256)
        .bind(values.package_name)
        .bind(values.package_source_path)
        .bind(values.dynamic_binding_targets_json)
        .bind(values.status)
        .bind(values.prompt_id)
        .bind(values.queue_number)
        .bind(values.progress_mode)
        .bind(values.progress_current)
        .bind(values.progress_total)
        .bind(values.current_node_id)
        .bind(values.error_code)
        .bind(values.error_message)
        .bind(values.raw_error_json)
        .bind(values.created_at)
        .bind(values.queued_at)
        .bind(values.started_at)
        .bind(values.finished_at)
        .bind(task.id.as_str())
        .bind(expected_previous_status.as_str())
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::integrity(
                "stale task transition or task does not exist",
            ));
        }

        let stored_event = insert_event(&mut transaction, event).await?;
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(stored_event)
    }

    async fn persist_runtime_update(
        &self,
        task: &Task,
        event: &NewTaskEvent,
    ) -> Result<StoredTaskEvent, RepositoryError> {
        validate_runtime_event(task, event)?;
        let values = TaskDbValues::from_task(task)?;
        let mut transaction = self.pool.begin().await.map_err(map_sqlx_error)?;

        let result = sqlx::query(
            "UPDATE tasks SET
                project_id = ?, workflow_id = ?, workflow_version_id = ?, recipe_id = ?,
                app_version = ?, build_commit = ?, workflow_version = ?, workflow_sha256 = ?,
                recipe_version = ?, recipe_sha256 = ?, package_name = ?, package_source_path = ?,
                dynamic_binding_targets_json = ?,
                status = ?, prompt_id = ?, queue_number = ?,
                progress_mode = ?, progress_current = ?, progress_total = ?, current_node_id = ?,
                error_code = ?, error_message = ?, raw_error_json = ?,
                created_at = ?, queued_at = ?, started_at = ?, finished_at = ?
             WHERE id = ? AND status = ?",
        )
        .bind(&task.project_id)
        .bind(&task.workflow_id)
        .bind(&task.workflow_version_id)
        .bind(&task.recipe_id)
        .bind(values.app_version)
        .bind(values.build_commit)
        .bind(values.workflow_version)
        .bind(values.workflow_sha256)
        .bind(values.recipe_version)
        .bind(values.recipe_sha256)
        .bind(values.package_name)
        .bind(values.package_source_path)
        .bind(values.dynamic_binding_targets_json)
        .bind(values.status)
        .bind(values.prompt_id)
        .bind(values.queue_number)
        .bind(values.progress_mode)
        .bind(values.progress_current)
        .bind(values.progress_total)
        .bind(values.current_node_id)
        .bind(values.error_code)
        .bind(values.error_message)
        .bind(values.raw_error_json)
        .bind(values.created_at)
        .bind(values.queued_at)
        .bind(values.started_at)
        .bind(values.finished_at)
        .bind(task.id.as_str())
        .bind(task.status.as_str())
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx_error)?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::integrity(
                "stale task runtime update or task does not exist",
            ));
        }

        let stored_event = insert_event(&mut transaction, event).await?;
        transaction.commit().await.map_err(map_sqlx_error)?;
        Ok(stored_event)
    }

    async fn find_by_id(&self, task_id: &TaskId) -> Result<Option<Task>, RepositoryError> {
        let row = sqlx::query_as::<_, TaskRow>(
            "SELECT
                id, project_id, workflow_id, workflow_version_id, recipe_id,
                app_version, build_commit, workflow_version, workflow_sha256,
                recipe_version, recipe_sha256, package_name, package_source_path,
                dynamic_binding_targets_json,
                status, prompt_id, queue_number,
                progress_mode, progress_current, progress_total, current_node_id,
                error_code, error_message, raw_error_json,
                created_at, queued_at, started_at, finished_at
             FROM tasks WHERE id = ?",
        )
        .bind(task_id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(TaskRow::try_into_domain).transpose()
    }

    async fn list_recent(
        &self,
        project_id: &str,
        limit: u32,
    ) -> Result<Vec<Task>, RepositoryError> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT
                id, project_id, workflow_id, workflow_version_id, recipe_id,
                app_version, build_commit, workflow_version, workflow_sha256,
                recipe_version, recipe_sha256, package_name, package_source_path,
                dynamic_binding_targets_json,
                status, prompt_id, queue_number,
                progress_mode, progress_current, progress_total, current_node_id,
                error_code, error_message, raw_error_json,
                created_at, queued_at, started_at, finished_at
             FROM tasks
             WHERE project_id = ?
             ORDER BY created_at DESC, id DESC LIMIT ?",
        )
        .bind(project_id)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(TaskRow::try_into_domain).collect()
    }

    async fn list_active(&self) -> Result<Vec<Task>, RepositoryError> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT
                id, project_id, workflow_id, workflow_version_id, recipe_id,
                app_version, build_commit, workflow_version, workflow_sha256,
                recipe_version, recipe_sha256, package_name, package_source_path,
                dynamic_binding_targets_json,
                status, prompt_id, queue_number,
                progress_mode, progress_current, progress_total, current_node_id,
                error_code, error_message, raw_error_json,
                created_at, queued_at, started_at, finished_at
             FROM tasks
             WHERE status IN (
                'CREATED', 'VALIDATING', 'PREPARING', 'QUEUED',
                'RUNNING', 'COLLECTING', 'CANCEL_REQUESTED'
             )
             ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(TaskRow::try_into_domain).collect()
    }

    async fn list_events(&self, task_id: &TaskId) -> Result<Vec<StoredTaskEvent>, RepositoryError> {
        let rows = sqlx::query_as::<_, EventRow>(
            "SELECT id, task_id, sequence, event_type, payload_json, created_at
             FROM task_events WHERE task_id = ? ORDER BY sequence ASC",
        )
        .bind(task_id.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(EventRow::try_into_domain).collect()
    }
}

fn validate_event(
    task: &Task,
    event: &NewTaskEvent,
    expected: TaskEventType,
) -> Result<(), RepositoryError> {
    if event.id.trim().is_empty() {
        return Err(RepositoryError::integrity(
            "task event id must not be empty",
        ));
    }
    if event.task_id != task.id {
        return Err(RepositoryError::integrity(
            "task event task_id does not match task",
        ));
    }
    if event.event_type != expected {
        return Err(RepositoryError::integrity(format!(
            "task status {} requires event {}, received {}",
            task.status.as_str(),
            expected.as_str(),
            event.event_type.as_str()
        )));
    }
    if event.created_at < task.created_at {
        return Err(RepositoryError::integrity(
            "task event created_at must not precede task created_at",
        ));
    }
    Ok(())
}

fn validate_runtime_event(task: &Task, event: &NewTaskEvent) -> Result<(), RepositoryError> {
    let allowed = match event.event_type {
        TaskEventType::TaskSubmissionPrepared => task.status == TaskStatus::Preparing,
        TaskEventType::TaskNodeStarted | TaskEventType::TaskProgressUpdated => {
            task.status == TaskStatus::Running
        }
        TaskEventType::TaskStreamDisconnected => matches!(
            task.status,
            TaskStatus::Queued | TaskStatus::Running | TaskStatus::Collecting
        ),
        TaskEventType::TaskCancelNotEffective => task.status == TaskStatus::CancelRequested,
        TaskEventType::TaskRecoveryStarted
        | TaskEventType::TaskRecoverySucceeded
        | TaskEventType::TaskRecoveryDeferred
        | TaskEventType::TaskRecoveryUnresolved => true,
        _ => false,
    };
    if !allowed {
        return Err(RepositoryError::integrity(format!(
            "runtime event {} is not allowed while task is {}",
            event.event_type.as_str(),
            task.status.as_str()
        )));
    }

    if event.id.trim().is_empty() {
        return Err(RepositoryError::integrity(
            "task event id must not be empty",
        ));
    }
    if event.task_id != task.id {
        return Err(RepositoryError::integrity(
            "task event task_id does not match task",
        ));
    }
    if event.created_at < task.created_at {
        return Err(RepositoryError::integrity(
            "task event created_at must not precede task created_at",
        ));
    }
    Ok(())
}

struct TaskDbValues {
    app_version: Option<String>,
    build_commit: Option<String>,
    workflow_version: Option<String>,
    workflow_sha256: Option<String>,
    recipe_version: Option<String>,
    recipe_sha256: Option<String>,
    package_name: Option<String>,
    package_source_path: Option<String>,
    dynamic_binding_targets_json: Option<String>,
    status: &'static str,
    prompt_id: Option<String>,
    queue_number: Option<i64>,
    progress_mode: &'static str,
    progress_current: Option<i64>,
    progress_total: Option<i64>,
    current_node_id: Option<String>,
    error_code: Option<String>,
    error_message: Option<String>,
    raw_error_json: Option<String>,
    created_at: String,
    queued_at: Option<String>,
    started_at: Option<String>,
    finished_at: Option<String>,
}

impl TaskDbValues {
    fn from_task(task: &Task) -> Result<Self, RepositoryError> {
        task.validate()
            .map_err(|error| map_domain_error("task validation", error))?;

        let (progress_mode, progress_current, progress_total, progress_node_id) =
            match &task.progress {
                TaskProgress::Indeterminate => {
                    if task.current_node_id.is_some() {
                        return Err(RepositoryError::integrity(
                            "indeterminate progress cannot have current_node_id",
                        ));
                    }
                    ("indeterminate", None, None, None)
                }
                TaskProgress::Node { node_id } => {
                    if task.current_node_id.as_deref() != Some(node_id.as_str()) {
                        return Err(RepositoryError::integrity(
                            "node progress and current_node_id must match",
                        ));
                    }
                    ("node", None, None, Some(node_id.clone()))
                }
                TaskProgress::Step {
                    current,
                    total,
                    node_id,
                } => {
                    if task.current_node_id.as_deref() != node_id.as_deref() {
                        return Err(RepositoryError::integrity(
                            "step progress and current_node_id must match",
                        ));
                    }
                    let current = i64::try_from(*current).map_err(|_| {
                        RepositoryError::serialization(
                            "progress_current",
                            "value exceeds SQLite INTEGER range",
                        )
                    })?;
                    let total = i64::try_from(*total).map_err(|_| {
                        RepositoryError::serialization(
                            "progress_total",
                            "value exceeds SQLite INTEGER range",
                        )
                    })?;
                    ("step", Some(current), Some(total), node_id.clone())
                }
            };

        let (error_code, error_message, raw_error_json) = match &task.error {
            None => (None, None, None),
            Some(error) => (
                Some(error.code.clone()),
                Some(error.message.clone()),
                serialize_json("task error raw", error.raw.as_ref())?,
            ),
        };

        let provenance = task.runtime_provenance.as_ref();
        let dynamic_binding_targets_json = provenance
            .map(|value| {
                serde_json::to_string(&value.dynamic_binding_targets).map_err(|error| {
                    RepositoryError::serialization(
                        "task dynamic binding targets",
                        error.to_string(),
                    )
                })
            })
            .transpose()?;

        Ok(Self {
            app_version: provenance.map(|value| value.app_version.clone()),
            build_commit: provenance.map(|value| value.build_commit.clone()),
            workflow_version: provenance.map(|value| value.workflow_version.clone()),
            workflow_sha256: provenance.map(|value| value.workflow_sha256.clone()),
            recipe_version: provenance.map(|value| value.recipe_version.clone()),
            recipe_sha256: provenance.map(|value| value.recipe_sha256.clone()),
            package_name: provenance.and_then(|value| value.package_name.clone()),
            package_source_path: provenance.and_then(|value| value.package_source_path.clone()),
            dynamic_binding_targets_json,
            status: task.status.as_str(),
            prompt_id: task.prompt_id.clone(),
            queue_number: task.queue_number,
            progress_mode,
            progress_current,
            progress_total,
            current_node_id: progress_node_id,
            error_code,
            error_message,
            raw_error_json,
            created_at: format_datetime(task.created_at),
            queued_at: task.queued_at.map(format_datetime),
            started_at: task.started_at.map(format_datetime),
            finished_at: task.finished_at.map(format_datetime),
        })
    }
}

#[derive(sqlx::FromRow)]
struct TaskRow {
    id: String,
    project_id: String,
    workflow_id: String,
    workflow_version_id: String,
    recipe_id: String,
    app_version: Option<String>,
    build_commit: Option<String>,
    workflow_version: Option<String>,
    workflow_sha256: Option<String>,
    recipe_version: Option<String>,
    recipe_sha256: Option<String>,
    package_name: Option<String>,
    package_source_path: Option<String>,
    dynamic_binding_targets_json: Option<String>,
    status: String,
    prompt_id: Option<String>,
    queue_number: Option<i64>,
    progress_mode: String,
    progress_current: Option<i64>,
    progress_total: Option<i64>,
    current_node_id: Option<String>,
    error_code: Option<String>,
    error_message: Option<String>,
    raw_error_json: Option<String>,
    created_at: String,
    queued_at: Option<String>,
    started_at: Option<String>,
    finished_at: Option<String>,
}

impl TaskRow {
    fn try_into_domain(self) -> Result<Task, RepositoryError> {
        let status = TaskStatus::try_from_db(&self.status)
            .map_err(|error| map_domain_error("task status", error))?;
        let progress = match self.progress_mode.as_str() {
            "indeterminate" => {
                if self.progress_current.is_some()
                    || self.progress_total.is_some()
                    || self.current_node_id.is_some()
                {
                    return Err(RepositoryError::integrity(
                        "indeterminate progress must not have progress values or current_node_id",
                    ));
                }
                TaskProgress::Indeterminate
            }
            "node" => {
                if self.progress_current.is_some() || self.progress_total.is_some() {
                    return Err(RepositoryError::integrity(
                        "node progress must not have step values",
                    ));
                }
                let node_id = self.current_node_id.clone().ok_or_else(|| {
                    RepositoryError::integrity("node progress requires current_node_id")
                })?;
                TaskProgress::node(node_id)
                    .map_err(|error| map_domain_error("node progress", error))?
            }
            "step" => {
                let current = self.progress_current.ok_or_else(|| {
                    RepositoryError::integrity("step progress requires progress_current")
                })?;
                let total = self.progress_total.ok_or_else(|| {
                    RepositoryError::integrity("step progress requires progress_total")
                })?;
                let current = i64_to_u64("progress_current", current)?;
                let total = i64_to_u64("progress_total", total)?;
                TaskProgress::step(current, total, self.current_node_id.clone())
                    .map_err(|error| map_domain_error("step progress", error))?
            }
            other => {
                return Err(RepositoryError::integrity(format!(
                    "unknown progress_mode \"{other}\""
                )))
            }
        };

        let error = match (&self.error_code, &self.error_message, &self.raw_error_json) {
            (None, None, None) => None,
            (Some(code), Some(message), raw) => Some(TaskError {
                code: code.clone(),
                message: message.clone(),
                raw: parse_json("task error raw", raw.as_deref())?,
            }),
            _ => {
                return Err(RepositoryError::integrity(
                    "task error requires both error_code and error_message",
                ))
            }
        };

        let runtime_provenance = match (
            self.app_version,
            self.build_commit,
            self.workflow_version,
            self.workflow_sha256,
            self.recipe_version,
            self.recipe_sha256,
            self.package_name,
            self.package_source_path,
            self.dynamic_binding_targets_json,
        ) {
            (None, None, None, None, None, None, None, None, None) => None,
            (
                Some(app_version),
                Some(build_commit),
                Some(workflow_version),
                Some(workflow_sha256),
                Some(recipe_version),
                Some(recipe_sha256),
                package_name,
                package_source_path,
                Some(dynamic_binding_targets_json),
            ) => Some(RuntimeProvenance {
                app_version,
                build_commit,
                workflow_id: self.workflow_id.clone(),
                workflow_version_id: self.workflow_version_id.clone(),
                workflow_version,
                workflow_sha256,
                recipe_id: self.recipe_id.clone(),
                recipe_version,
                recipe_sha256,
                package_name,
                package_source_path,
                dynamic_binding_targets: serde_json::from_value(
                    parse_json(
                        "task dynamic binding targets",
                        Some(&dynamic_binding_targets_json),
                    )?
                    .ok_or_else(|| {
                        RepositoryError::integrity(
                            "task dynamic binding targets must contain a JSON array",
                        )
                    })?,
                )
                .map_err(|error| {
                    RepositoryError::serialization(
                        "task dynamic binding targets",
                        error.to_string(),
                    )
                })?,
            }),
            _ => {
                return Err(RepositoryError::integrity(
                    "task runtime provenance columns must be complete",
                ))
            }
        };

        let task = Task {
            id: TaskId::parse(self.id).map_err(|error| map_domain_error("task id", error))?,
            project_id: self.project_id,
            workflow_id: self.workflow_id,
            workflow_version_id: self.workflow_version_id,
            recipe_id: self.recipe_id,
            runtime_provenance,
            status,
            prompt_id: self.prompt_id,
            queue_number: self.queue_number,
            progress,
            current_node_id: self.current_node_id,
            error,
            created_at: parse_datetime("created_at", &self.created_at)?,
            queued_at: parse_optional_datetime("queued_at", self.queued_at.as_deref())?,
            started_at: parse_optional_datetime("started_at", self.started_at.as_deref())?,
            finished_at: parse_optional_datetime("finished_at", self.finished_at.as_deref())?,
        };
        task.validate()
            .map_err(|error| map_domain_error("task integrity", error))?;
        Ok(task)
    }
}

#[derive(sqlx::FromRow)]
struct EventRow {
    id: String,
    task_id: String,
    sequence: i64,
    event_type: String,
    payload_json: Option<String>,
    created_at: String,
}

impl EventRow {
    fn try_into_domain(self) -> Result<StoredTaskEvent, RepositoryError> {
        Ok(StoredTaskEvent {
            id: self.id,
            task_id: TaskId::parse(self.task_id)
                .map_err(|error| map_domain_error("task event task_id", error))?,
            sequence: i64_to_u64("task event sequence", self.sequence)?,
            event_type: TaskEventType::try_from_db(&self.event_type)
                .map_err(|error| map_domain_error("task event type", error))?,
            payload: parse_json("task event payload", self.payload_json.as_deref())?,
            created_at: parse_datetime("task event created_at", &self.created_at)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteTaskRepository;
    use crate::application::ports::TaskRepository;
    use crate::domain::{
        RuntimeProvenance, Task, TaskError, TaskEventType, TaskProgress, TaskStateMachine,
        TaskStatus,
    };
    use crate::infrastructure::database::{initialize, repositories::test_support};
    use chrono::{Duration, TimeZone, Utc};
    use serde_json::json;
    use sqlx::SqlitePool;
    use tempfile::{tempdir, TempDir};

    async fn setup() -> (TempDir, SqlitePool, SqliteTaskRepository) {
        let directory = tempdir().expect("temporary directory should exist");
        let pool = initialize(&directory.path().join("app.db"))
            .await
            .expect("database should initialize");
        test_support::seed_task_dependencies(&pool).await;
        let repository = SqliteTaskRepository::new(pool.clone());
        (directory, pool, repository)
    }

    fn new_task() -> Task {
        Task::new(
            "project-1",
            "workflow-1",
            "workflow-version-1",
            "recipe-1",
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        )
    }

    #[tokio::test]
    async fn create_find_and_created_event_are_consistent() {
        let (_directory, _pool, repository) = setup().await;
        let task = new_task();
        let mut event = task.created_event();
        event.payload = Some(json!({"source": "test"}));

        let stored_event = repository
            .create(&task, &event)
            .await
            .expect("task and event should commit");
        let found = repository
            .find_by_id(&task.id)
            .await
            .expect("task lookup should succeed")
            .expect("task should exist");
        let events = repository
            .list_events(&task.id)
            .await
            .expect("event lookup should succeed");

        assert_eq!(found, task);
        assert_eq!(stored_event.sequence, 1);
        assert_eq!(stored_event.event_type, TaskEventType::TaskCreated);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sequence, 1);
        assert_eq!(events[0].payload, event.payload);
    }

    #[tokio::test]
    async fn runtime_provenance_roundtrips_with_dynamic_binding_targets() {
        let (_directory, _pool, repository) = setup().await;
        let mut task = new_task();
        task.runtime_provenance = Some(RuntimeProvenance {
            app_version: "0.3.0".to_owned(),
            build_commit: "abc123".to_owned(),
            workflow_id: task.workflow_id.clone(),
            workflow_version_id: task.workflow_version_id.clone(),
            workflow_version: "2.0.0".to_owned(),
            workflow_sha256: "workflow-sha".to_owned(),
            recipe_id: task.recipe_id.clone(),
            recipe_version: "1.2.0".to_owned(),
            recipe_sha256: "recipe-sha".to_owned(),
            package_name: Some("runtime-package".to_owned()),
            package_source_path: Some("C:/runtime-package".to_owned()),
            dynamic_binding_targets: vec!["14.first_frame".to_owned(), "24.image".to_owned()],
        });
        repository
            .create(&task, &task.created_event())
            .await
            .expect("task should commit");

        let found = repository
            .find_by_id(&task.id)
            .await
            .expect("task lookup should succeed")
            .expect("task should exist");
        assert_eq!(found.runtime_provenance, task.runtime_provenance);
    }

    #[tokio::test]
    async fn list_active_returns_only_non_terminal_recovery_states() {
        let (_directory, _pool, repository) = setup().await;
        let statuses = [
            TaskStatus::Created,
            TaskStatus::Validating,
            TaskStatus::Preparing,
            TaskStatus::Queued,
            TaskStatus::Running,
            TaskStatus::Collecting,
            TaskStatus::CancelRequested,
        ];

        for (index, status) in statuses.into_iter().enumerate() {
            let mut task = new_task();
            task.id = crate::domain::TaskId::parse(format!("tsk_active_{index}")).unwrap();
            repository
                .create(&task, &task.created_event())
                .await
                .expect("active task should create");
            let base = task.created_at + Duration::seconds((index * 10) as i64);
            if status != TaskStatus::Created {
                let event = TaskStateMachine::transition(
                    &mut task,
                    TaskStatus::Validating,
                    base + Duration::seconds(1),
                )
                .unwrap();
                repository
                    .persist_transition(&task, &event, TaskStatus::Created)
                    .await
                    .unwrap();
            }
            if matches!(
                status,
                TaskStatus::Preparing
                    | TaskStatus::Queued
                    | TaskStatus::Running
                    | TaskStatus::Collecting
                    | TaskStatus::CancelRequested
            ) {
                let event = TaskStateMachine::transition(
                    &mut task,
                    TaskStatus::Preparing,
                    base + Duration::seconds(2),
                )
                .unwrap();
                repository
                    .persist_transition(&task, &event, TaskStatus::Validating)
                    .await
                    .unwrap();
            }
            if matches!(
                status,
                TaskStatus::Queued
                    | TaskStatus::Running
                    | TaskStatus::Collecting
                    | TaskStatus::CancelRequested
            ) {
                let prepared = task
                    .prepare_submission(
                        format!("prompt-{index}"),
                        format!("client-{index}"),
                        base + Duration::seconds(3),
                    )
                    .unwrap();
                repository
                    .persist_runtime_update(&task, &prepared)
                    .await
                    .unwrap();
                task.set_queue_number(Some(index as i64)).unwrap();
                let event = TaskStateMachine::transition(
                    &mut task,
                    TaskStatus::Queued,
                    base + Duration::seconds(4),
                )
                .unwrap();
                repository
                    .persist_transition(&task, &event, TaskStatus::Preparing)
                    .await
                    .unwrap();
            }
            if matches!(status, TaskStatus::Running | TaskStatus::Collecting) {
                let event = TaskStateMachine::transition(
                    &mut task,
                    TaskStatus::Running,
                    base + Duration::seconds(5),
                )
                .unwrap();
                repository
                    .persist_transition(&task, &event, TaskStatus::Queued)
                    .await
                    .unwrap();
            }
            if status == TaskStatus::Collecting {
                let event = TaskStateMachine::transition(
                    &mut task,
                    TaskStatus::Collecting,
                    base + Duration::seconds(6),
                )
                .unwrap();
                repository
                    .persist_transition(&task, &event, TaskStatus::Running)
                    .await
                    .unwrap();
            }
            if status == TaskStatus::CancelRequested {
                let event = task.request_cancel(base + Duration::seconds(5)).unwrap();
                repository
                    .persist_transition(&task, &event, TaskStatus::Queued)
                    .await
                    .unwrap();
            }
            assert_eq!(task.status, status);
        }

        let mut succeeded = new_task();
        succeeded.id = crate::domain::TaskId::parse("tsk_succeeded").unwrap();
        repository
            .create(&succeeded, &succeeded.created_event())
            .await
            .unwrap();
        let event = succeeded
            .fail(
                TaskError {
                    code: "TEST".to_owned(),
                    message: "terminal".to_owned(),
                    raw: None,
                },
                succeeded.created_at + Duration::seconds(1),
            )
            .unwrap();
        repository
            .persist_transition(&succeeded, &event, TaskStatus::Created)
            .await
            .unwrap();

        let mut active = repository.list_active().await.unwrap();
        active.sort_by_key(|task| task.id.as_str().to_owned());
        assert_eq!(
            active.iter().map(|task| task.status).collect::<Vec<_>>(),
            statuses
        );
    }

    #[tokio::test]
    async fn transition_persists_task_and_second_event_atomically() {
        let (_directory, _pool, repository) = setup().await;
        let mut task = new_task();
        repository
            .create(&task, &task.created_event())
            .await
            .expect("create should succeed");

        let at = task.created_at + Duration::seconds(1);
        let event = TaskStateMachine::transition(&mut task, TaskStatus::Validating, at)
            .expect("transition should succeed");
        let stored_event = repository
            .persist_transition(&task, &event, TaskStatus::Created)
            .await
            .expect("transition should commit");
        let found = repository
            .find_by_id(&task.id)
            .await
            .expect("lookup should succeed")
            .expect("task should exist");
        let events = repository
            .list_events(&task.id)
            .await
            .expect("events should load");

        assert_eq!(found.status, TaskStatus::Validating);
        assert_eq!(stored_event.sequence, 2);
        assert_eq!(
            events
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            [1, 2]
        );
        assert_eq!(
            repository.list_recent("project-1", 1).await.unwrap().len(),
            1
        );
    }

    #[tokio::test]
    async fn list_recent_is_scoped_to_project() {
        let (_directory, pool, repository) = setup().await;
        sqlx::query(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at)
             VALUES ('project-2', 'Other', 'C:/other', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let first = new_task();
        let second = Task::new(
            "project-2",
            "workflow-1",
            "workflow-version-1",
            "recipe-1",
            first.created_at + Duration::seconds(1),
        );
        repository
            .create(&first, &first.created_event())
            .await
            .unwrap();
        repository
            .create(&second, &second.created_event())
            .await
            .unwrap();

        assert_eq!(
            repository.list_recent("project-1", 10).await.unwrap().len(),
            1
        );
        assert_eq!(
            repository.list_recent("project-2", 10).await.unwrap().len(),
            1
        );
    }

    #[tokio::test]
    async fn persists_created_to_failed_transition_and_failed_event() {
        let (_directory, _pool, repository) = setup().await;
        let mut task = new_task();
        repository
            .create(&task, &task.created_event())
            .await
            .expect("create should succeed");

        let failed_at = task.created_at + Duration::seconds(1);
        let event = task
            .fail(
                TaskError {
                    code: "TEST_FAILURE".to_owned(),
                    message: "expected failure".to_owned(),
                    raw: None,
                },
                failed_at,
            )
            .expect("failure transition should succeed");
        repository
            .persist_transition(&task, &event, TaskStatus::Created)
            .await
            .expect("terminal failure transition should persist");

        let found = repository.find_by_id(&task.id).await.unwrap().unwrap();
        let events = repository.list_events(&task.id).await.unwrap();
        assert_eq!(found.status, TaskStatus::Failed);
        assert_eq!(found.finished_at, Some(failed_at));
        assert_eq!(
            events
                .iter()
                .map(|event| event.event_type)
                .collect::<Vec<_>>(),
            vec![TaskEventType::TaskCreated, TaskEventType::TaskFailed]
        );
    }

    #[tokio::test]
    async fn persists_complete_lifecycle_through_succeeded() {
        let (_directory, _pool, repository) = setup().await;
        let mut task = new_task();
        repository
            .create(&task, &task.created_event())
            .await
            .expect("create should succeed");

        let previous = task.status;
        let at = task.created_at + Duration::seconds(1);
        let event = TaskStateMachine::transition(&mut task, TaskStatus::Validating, at)
            .expect("transition should succeed");
        repository
            .persist_transition(&task, &event, previous)
            .await
            .unwrap();
        let previous = task.status;
        let at = task.created_at + Duration::seconds(2);
        let event = TaskStateMachine::transition(&mut task, TaskStatus::Preparing, at)
            .expect("transition should succeed");
        repository
            .persist_transition(&task, &event, previous)
            .await
            .unwrap();
        task.set_queue_number(Some(1)).unwrap();
        let previous = task.status;
        let at = task.created_at + Duration::seconds(3);
        let event = TaskStateMachine::transition(&mut task, TaskStatus::Queued, at)
            .expect("transition should succeed");
        repository
            .persist_transition(&task, &event, previous)
            .await
            .unwrap();
        let previous = task.status;
        let at = task.created_at + Duration::seconds(4);
        let event = TaskStateMachine::transition(&mut task, TaskStatus::Running, at)
            .expect("transition should succeed");
        repository
            .persist_transition(&task, &event, previous)
            .await
            .unwrap();
        let previous = task.status;
        let at = task.created_at + Duration::seconds(5);
        let event = TaskStateMachine::transition(&mut task, TaskStatus::Collecting, at)
            .expect("transition should succeed");
        repository
            .persist_transition(&task, &event, previous)
            .await
            .unwrap();
        let previous = task.status;
        let at = task.created_at + Duration::seconds(6);
        let event = task.succeed(at).expect("success transition should succeed");
        repository
            .persist_transition(&task, &event, previous)
            .await
            .expect("terminal success transition should persist");

        let found = repository.find_by_id(&task.id).await.unwrap().unwrap();
        let events = repository.list_events(&task.id).await.unwrap();
        assert_eq!(found.status, TaskStatus::Succeeded);
        assert_eq!(found.current_node_id, None);
        assert_eq!(
            events
                .iter()
                .map(|event| event.event_type)
                .collect::<Vec<_>>(),
            vec![
                TaskEventType::TaskCreated,
                TaskEventType::TaskValidating,
                TaskEventType::TaskPreparing,
                TaskEventType::TaskQueued,
                TaskEventType::TaskRunning,
                TaskEventType::TaskCollecting,
                TaskEventType::TaskSucceeded,
            ]
        );
    }

    #[tokio::test]
    async fn event_failure_rolls_back_task_update() {
        let (_directory, _pool, repository) = setup().await;
        let mut task = new_task();
        repository
            .create(&task, &task.created_event())
            .await
            .expect("create should succeed");
        let created_event_id = repository.list_events(&task.id).await.unwrap()[0]
            .id
            .clone();

        let transition_at = task.created_at + Duration::seconds(1);
        let event = TaskStateMachine::transition(&mut task, TaskStatus::Validating, transition_at)
            .expect("transition should succeed");
        let mut duplicate_event = event;
        duplicate_event.id = created_event_id;

        assert!(repository
            .persist_transition(&task, &duplicate_event, TaskStatus::Created)
            .await
            .is_err());
        let found = repository
            .find_by_id(&task.id)
            .await
            .unwrap()
            .expect("task should remain");
        let events = repository.list_events(&task.id).await.unwrap();

        assert_eq!(found.status, TaskStatus::Created);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sequence, 1);
    }

    #[tokio::test]
    async fn progress_modes_round_trip_and_corrupt_progress_is_rejected() {
        let (_directory, pool, repository) = setup().await;
        let mut task = new_task();
        task.progress = TaskProgress::node("3").unwrap();
        task.current_node_id = Some("3".to_owned());
        repository
            .create(&task, &task.created_event())
            .await
            .expect("node progress task should create");
        assert_eq!(
            repository.find_by_id(&task.id).await.unwrap(),
            Some(task.clone())
        );

        sqlx::query("UPDATE tasks SET progress_mode = 'step', progress_current = 2, progress_total = NULL WHERE id = ?")
            .bind(task.id.as_str())
            .execute(&pool)
            .await
            .expect("corrupt progress should be writable for test");
        assert!(repository.find_by_id(&task.id).await.is_err());
    }

    #[tokio::test]
    async fn unknown_database_status_is_not_silently_converted() {
        let (_directory, pool, repository) = setup().await;
        let task = new_task();
        repository
            .create(&task, &task.created_event())
            .await
            .expect("create should succeed");
        sqlx::query("UPDATE tasks SET status = 'FOOBAR' WHERE id = ?")
            .bind(task.id.as_str())
            .execute(&pool)
            .await
            .expect("corrupt status should be writable for test");

        assert!(repository.find_by_id(&task.id).await.is_err());
    }

    #[tokio::test]
    async fn runtime_updates_round_trip_and_stale_status_is_rejected() {
        let (_directory, _pool, repository) = setup().await;
        let mut task = new_task();
        repository
            .create(&task, &task.created_event())
            .await
            .expect("create should succeed");

        let validating_at = task.created_at + Duration::seconds(1);
        let validating_event =
            TaskStateMachine::transition(&mut task, TaskStatus::Validating, validating_at)
                .expect("validating transition should succeed");
        repository
            .persist_transition(&task, &validating_event, TaskStatus::Created)
            .await
            .expect("validating transition should persist");

        let preparing_at = task.created_at + Duration::seconds(2);
        let preparing_event =
            TaskStateMachine::transition(&mut task, TaskStatus::Preparing, preparing_at)
                .expect("preparing transition should succeed");
        repository
            .persist_transition(&task, &preparing_event, TaskStatus::Validating)
            .await
            .expect("preparing transition should persist");

        let prepared_event = task
            .prepare_submission(
                "550e8400-e29b-41d4-a716-446655440000",
                "client-1",
                task.created_at + Duration::seconds(3),
            )
            .expect("submission should prepare");
        repository
            .persist_runtime_update(&task, &prepared_event)
            .await
            .expect("submission preparation should persist");
        let stale_task = task.clone();

        task.set_queue_number(Some(4))
            .expect("queue number should be set");
        let queued_at = task.created_at + Duration::seconds(4);
        let queued_event = TaskStateMachine::transition(&mut task, TaskStatus::Queued, queued_at)
            .expect("queued transition should succeed");
        repository
            .persist_transition(&task, &queued_event, TaskStatus::Preparing)
            .await
            .expect("queued transition should persist");

        assert!(repository
            .persist_runtime_update(&stale_task, &prepared_event)
            .await
            .is_err());
        let found = repository
            .find_by_id(&task.id)
            .await
            .expect("task lookup should succeed")
            .expect("task should exist");
        assert_eq!(found.status, TaskStatus::Queued);
        assert_eq!(
            found.prompt_id.as_deref(),
            Some("550e8400-e29b-41d4-a716-446655440000")
        );
        assert_eq!(repository.list_events(&task.id).await.unwrap().len(), 5);
    }
}
