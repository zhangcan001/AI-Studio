use super::{
    i64_to_u64, map_domain_error, map_sqlx_error, parse_datetime, parse_json,
    parse_optional_datetime,
};
use crate::application::pagination::{PageCursor, PageResult};
use crate::application::ports::{
    RepositoryError, TaskHistoryQuery, TaskHistoryRecord, TaskHistoryRepository,
    TaskHistoryTimeFilter, TaskHistoryWorkflowOption,
};
use crate::domain::{Task, TaskError, TaskId, TaskProgress, TaskStatus};
use async_trait::async_trait;
use chrono::{DateTime, Local, LocalResult, NaiveDate, TimeZone, Utc};
use sqlx::{QueryBuilder, Sqlite, SqlitePool};

#[derive(Clone)]
pub struct SqliteTaskHistoryRepository {
    pool: SqlitePool,
}

impl SqliteTaskHistoryRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

const TASK_HISTORY_SELECT: &str = "SELECT
    t.id, t.project_id, t.workflow_id, t.workflow_version_id, t.recipe_id,
    t.status, t.prompt_id, t.queue_number,
    t.progress_mode, t.progress_current, t.progress_total, t.current_node_id,
    t.error_code, t.error_message, t.raw_error_json,
    t.created_at, t.queued_at, t.started_at, t.finished_at,
    w.name AS workflow_name,
    (SELECT COUNT(*) FROM assets a WHERE a.source_task_id = t.id) AS output_count
    FROM tasks t
    INNER JOIN workflows w ON w.id = t.workflow_id";

#[async_trait]
impl TaskHistoryRepository for SqliteTaskHistoryRepository {
    async fn list_page(
        &self,
        request: TaskHistoryQuery,
    ) -> Result<PageResult<TaskHistoryRecord>, RepositoryError> {
        let requested_limit = request.limit.clamp(1, 100);
        let mut query = QueryBuilder::<Sqlite>::new(TASK_HISTORY_SELECT);
        query
            .push(" WHERE t.project_id = ")
            .push_bind(request.project_id);

        if let Some(statuses) = request.filter.statuses() {
            query.push(" AND t.status IN (");
            for (index, status) in statuses.iter().enumerate() {
                if index > 0 {
                    query.push(", ");
                }
                query.push_bind(status.as_str());
            }
            query.push(")");
        }

        if let Some(workflow_id) = request.workflow_id {
            query.push(" AND t.workflow_id = ").push_bind(workflow_id);
        }

        if let Some(keyword) = request.keyword {
            let pattern = format!("%{keyword}%");
            query
                .push(" AND (t.id LIKE ")
                .push_bind(pattern.clone())
                .push(" OR w.name LIKE ")
                .push_bind(pattern)
                .push(")");
        }

        if let Some(cutoff) = time_cutoff(request.time_filter) {
            query
                .push(" AND t.created_at >= ")
                .push_bind(cutoff.to_rfc3339());
        }

        if let Some(cursor) = request.cursor {
            let created_at = cursor.created_at.to_rfc3339();
            query
                .push(" AND (t.created_at < ")
                .push_bind(created_at.clone())
                .push(" OR (t.created_at = ")
                .push_bind(created_at)
                .push(" AND t.id < ")
                .push_bind(cursor.id)
                .push("))");
        }

        query
            .push(" ORDER BY t.created_at DESC, t.id DESC LIMIT ")
            .push_bind(i64::from(requested_limit) + 1);
        let mut rows = query
            .build_query_as::<TaskHistoryRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        let has_more = rows.len() > requested_limit as usize;
        if has_more {
            rows.truncate(requested_limit as usize);
        }
        let next_cursor = has_more
            .then(|| rows.last())
            .flatten()
            .map(|row| PageCursor::for_item(row.created_at_value(), row.id.clone()));
        let items = rows
            .into_iter()
            .map(TaskHistoryRow::try_into_record)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(PageResult { items, next_cursor })
    }

    async fn list_workflow_options(
        &self,
        project_id: &str,
    ) -> Result<Vec<TaskHistoryWorkflowOption>, RepositoryError> {
        #[derive(sqlx::FromRow)]
        struct WorkflowOptionRow {
            workflow_id: String,
            workflow_name: String,
        }

        sqlx::query_as::<_, WorkflowOptionRow>(
            "SELECT DISTINCT t.workflow_id, w.name AS workflow_name
             FROM tasks t
             INNER JOIN workflows w ON w.id = t.workflow_id
             WHERE t.project_id = ?
             ORDER BY w.name ASC, t.workflow_id ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|row| TaskHistoryWorkflowOption {
                    workflow_id: row.workflow_id,
                    workflow_name: row.workflow_name,
                })
                .collect()
        })
        .map_err(map_sqlx_error)
    }

    async fn find_detail(
        &self,
        project_id: &str,
        task_id: &TaskId,
    ) -> Result<Option<TaskHistoryRecord>, RepositoryError> {
        let mut query = QueryBuilder::<Sqlite>::new(TASK_HISTORY_SELECT);
        query
            .push(" WHERE t.project_id = ")
            .push_bind(project_id.to_owned())
            .push(" AND t.id = ")
            .push_bind(task_id.as_str().to_owned());
        query.push(" LIMIT 1");
        query
            .build_query_as::<TaskHistoryRow>()
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx_error)?
            .map(TaskHistoryRow::try_into_record)
            .transpose()
    }
}

fn time_cutoff(filter: TaskHistoryTimeFilter) -> Option<DateTime<Utc>> {
    let now = Utc::now();
    match filter {
        TaskHistoryTimeFilter::All => None,
        TaskHistoryTimeFilter::Today => Some(today_start_utc()),
        TaskHistoryTimeFilter::Last7Days => Some(now - chrono::Duration::days(7)),
        TaskHistoryTimeFilter::Last30Days => Some(now - chrono::Duration::days(30)),
    }
}

fn today_start_utc() -> DateTime<Utc> {
    let local_now = Local::now();
    local_date_start_utc(&Local, local_now.date_naive())
        .unwrap_or_else(|| local_now.with_timezone(&Utc))
}

fn local_date_start_utc<Tz>(timezone: &Tz, date: NaiveDate) -> Option<DateTime<Utc>>
where
    Tz: TimeZone,
{
    let midnight = date.and_hms_opt(0, 0, 0)?;
    if let Some(start) = resolve_local_datetime_utc(timezone, midnight) {
        return Some(start);
    }

    // Some time zones can skip local midnight during an offset transition.
    // Resolve the earliest valid instant on that local calendar date without panicking.
    for seconds_after_midnight in 1..86_400 {
        let Some(candidate) =
            midnight.checked_add_signed(chrono::Duration::seconds(seconds_after_midnight))
        else {
            break;
        };
        if let Some(start) = resolve_local_datetime_utc(timezone, candidate) {
            return Some(start);
        }
    }
    None
}

fn resolve_local_datetime_utc<Tz>(
    timezone: &Tz,
    local_datetime: chrono::NaiveDateTime,
) -> Option<DateTime<Utc>>
where
    Tz: TimeZone,
{
    match timezone.from_local_datetime(&local_datetime) {
        LocalResult::Single(value) => Some(value.with_timezone(&Utc)),
        LocalResult::Ambiguous(first, second) => {
            Some(first.with_timezone(&Utc).min(second.with_timezone(&Utc)))
        }
        LocalResult::None => None,
    }
}

#[derive(sqlx::FromRow)]
struct TaskHistoryRow {
    id: String,
    project_id: String,
    workflow_id: String,
    workflow_version_id: String,
    recipe_id: String,
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
    workflow_name: String,
    output_count: i64,
}

impl TaskHistoryRow {
    fn created_at_value(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(&self.created_at)
            .map(|value| value.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now())
    }

    fn try_into_record(self) -> Result<TaskHistoryRecord, RepositoryError> {
        let status = TaskStatus::try_from_db(&self.status)
            .map_err(|error| map_domain_error("task history status", error))?;
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
                    .map_err(|error| map_domain_error("task history node progress", error))?
            }
            "step" => {
                let current = self.progress_current.ok_or_else(|| {
                    RepositoryError::integrity("step progress requires progress_current")
                })?;
                let total = self.progress_total.ok_or_else(|| {
                    RepositoryError::integrity("step progress requires progress_total")
                })?;
                TaskProgress::step(
                    i64_to_u64("task history progress_current", current)?,
                    i64_to_u64("task history progress_total", total)?,
                    self.current_node_id.clone(),
                )
                .map_err(|error| map_domain_error("task history step progress", error))?
            }
            other => {
                return Err(RepositoryError::integrity(format!(
                    "unknown task history progress_mode \"{other}\""
                )))
            }
        };

        let error = match (&self.error_code, &self.error_message, &self.raw_error_json) {
            (None, None, None) => None,
            (Some(code), Some(message), raw) => Some(TaskError {
                code: code.clone(),
                message: message.clone(),
                raw: parse_json("task history raw error", raw.as_deref())?,
            }),
            _ => {
                return Err(RepositoryError::integrity(
                    "task history error requires both error_code and error_message",
                ))
            }
        };

        let task = Task {
            id: TaskId::parse(self.id)
                .map_err(|error| map_domain_error("task history id", error))?,
            project_id: self.project_id,
            workflow_id: self.workflow_id,
            workflow_version_id: self.workflow_version_id,
            recipe_id: self.recipe_id,
            status,
            prompt_id: self.prompt_id,
            queue_number: self.queue_number,
            progress,
            current_node_id: self.current_node_id,
            error,
            created_at: parse_datetime("task history created_at", &self.created_at)?,
            queued_at: parse_optional_datetime(
                "task history queued_at",
                self.queued_at.as_deref(),
            )?,
            started_at: parse_optional_datetime(
                "task history started_at",
                self.started_at.as_deref(),
            )?,
            finished_at: parse_optional_datetime(
                "task history finished_at",
                self.finished_at.as_deref(),
            )?,
        };
        task.validate()
            .map_err(|error| map_domain_error("task history integrity", error))?;

        Ok(TaskHistoryRecord {
            task,
            workflow_name: self.workflow_name,
            output_count: u32::try_from(self.output_count).map_err(|_| {
                RepositoryError::serialization(
                    "task history output_count",
                    format!("invalid value {}", self.output_count),
                )
            })?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{local_date_start_utc, today_start_utc, SqliteTaskHistoryRepository};
    use crate::application::pagination::PageCursor;
    use crate::application::ports::{
        TaskHistoryFilter, TaskHistoryQuery, TaskHistoryRepository, TaskHistoryTimeFilter,
        TaskRepository,
    };
    use crate::domain::{Task, TaskError, TaskStateMachine, TaskStatus};
    use crate::infrastructure::database::{initialize, repositories::test_support};
    use chrono::{Duration, FixedOffset, TimeZone, Utc};
    use sqlx::SqlitePool;
    use tempfile::tempdir;

    async fn setup() -> (SqlitePool, SqliteTaskHistoryRepository) {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        (pool.clone(), SqliteTaskHistoryRepository::new(pool))
    }

    async fn insert_task(pool: &SqlitePool, task: &Task) {
        crate::infrastructure::database::repositories::SqliteTaskRepository::new(pool.clone())
            .create(task, &task.created_event())
            .await
            .unwrap();
    }

    fn query(
        project_id: &str,
        filter: TaskHistoryFilter,
        cursor: Option<PageCursor>,
        limit: u32,
    ) -> TaskHistoryQuery {
        TaskHistoryQuery {
            project_id: project_id.to_owned(),
            filter,
            workflow_id: None,
            keyword: None,
            time_filter: TaskHistoryTimeFilter::All,
            cursor,
            limit,
        }
    }

    #[tokio::test]
    async fn uses_project_isolation_and_keyset_ordering() {
        let (pool, repository) = setup().await;
        let base = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let first = Task::new(
            "project-1",
            "workflow-1",
            "workflow-version-1",
            "recipe-1",
            base,
        );
        let second = Task::new(
            "project-1",
            "workflow-1",
            "workflow-version-1",
            "recipe-1",
            base,
        );
        let other = Task::new(
            "project-2",
            "workflow-1",
            "workflow-version-1",
            "recipe-1",
            base,
        );
        sqlx::query("INSERT INTO projects (id, name, root_path, created_at, updated_at) VALUES ('project-2', 'Other', 'C:/other', ?, ?)")
            .bind(base.to_rfc3339()).bind(base.to_rfc3339()).execute(&pool).await.unwrap();
        for task in [&first, &second, &other] {
            insert_task(&pool, task).await;
        }

        let page = repository
            .list_page(query("project-1", TaskHistoryFilter::All, None, 1))
            .await
            .unwrap();
        assert_eq!(page.items.len(), 1);
        assert!(page.next_cursor.is_some());
        let next = repository
            .list_page(query(
                "project-1",
                TaskHistoryFilter::All,
                page.next_cursor,
                1,
            ))
            .await
            .unwrap();
        assert_eq!(next.items.len(), 1);
        assert_ne!(page.items[0].task.id, next.items[0].task.id);
        assert!(next.next_cursor.is_none());
        assert_eq!(
            repository
                .list_page(query("project-2", TaskHistoryFilter::All, None, 10))
                .await
                .unwrap()
                .items
                .len(),
            1
        );
        assert!(repository
            .find_detail("project-2", &first.id)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn active_filter_excludes_terminal_statuses() {
        let (pool, repository) = setup().await;
        let task_repository =
            crate::infrastructure::database::repositories::SqliteTaskRepository::new(pool.clone());
        let base = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let mut active = Task::new(
            "project-1",
            "workflow-1",
            "workflow-version-1",
            "recipe-1",
            base,
        );
        task_repository
            .create(&active, &active.created_event())
            .await
            .unwrap();
        for (status, seconds) in [
            (TaskStatus::Validating, 1),
            (TaskStatus::Preparing, 2),
            (TaskStatus::Queued, 3),
            (TaskStatus::Running, 4),
        ] {
            let previous = active.status;
            let event = TaskStateMachine::transition(
                &mut active,
                status,
                base + Duration::seconds(seconds),
            )
            .unwrap();
            task_repository
                .persist_transition(&active, &event, previous)
                .await
                .unwrap();
        }
        let mut failed = Task::new(
            "project-1",
            "workflow-1",
            "workflow-version-1",
            "recipe-1",
            base + Duration::seconds(2),
        );
        task_repository
            .create(&failed, &failed.created_event())
            .await
            .unwrap();
        let previous = failed.status;
        let event = failed
            .fail(
                TaskError {
                    code: "TEST_FAILED".to_owned(),
                    message: "failed".to_owned(),
                    raw: None,
                },
                base + Duration::seconds(3),
            )
            .unwrap();
        task_repository
            .persist_transition(&failed, &event, previous)
            .await
            .unwrap();
        let rows = repository
            .list_page(query("project-1", TaskHistoryFilter::Active, None, 10))
            .await
            .unwrap()
            .items;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].task.status, TaskStatus::Running);
    }

    #[tokio::test]
    async fn applies_workflow_keyword_time_filters_and_returns_project_options() {
        let (pool, repository) = setup().await;
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO workflows (id, name, category, mode, current_version_id, created_at, updated_at)
             VALUES ('workflow-2', '视频工作流', 'test', 'video', 'workflow-version-2', ?, ?)",
        )
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO workflow_versions (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
             VALUES ('workflow-version-2', 'workflow-2', '1', '{}', 'sha-2', ?)",
        )
        .bind(now.to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO recipes (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at)
             VALUES ('recipe-2', 'workflow-version-2', '1', 1, 'schema_version: 1', 'sha-2', ?)",
        )
        .bind(now.to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();

        let recent = Task::new(
            "project-1",
            "workflow-2",
            "workflow-version-2",
            "recipe-2",
            now - Duration::hours(1),
        );
        let old = Task::new(
            "project-1",
            "workflow-2",
            "workflow-version-2",
            "recipe-2",
            now - Duration::days(40),
        );
        let other_workflow = Task::new(
            "project-1",
            "workflow-1",
            "workflow-version-1",
            "recipe-1",
            now - Duration::hours(2),
        );
        insert_task(&pool, &recent).await;
        insert_task(&pool, &old).await;
        insert_task(&pool, &other_workflow).await;

        let filtered = repository
            .list_page(TaskHistoryQuery {
                project_id: "project-1".to_owned(),
                filter: TaskHistoryFilter::All,
                workflow_id: Some("workflow-2".to_owned()),
                keyword: Some("视频".to_owned()),
                time_filter: TaskHistoryTimeFilter::Last30Days,
                cursor: None,
                limit: 10,
            })
            .await
            .unwrap();
        assert_eq!(filtered.items.len(), 1);
        assert_eq!(filtered.items[0].task.id, recent.id);

        for time_filter in [
            TaskHistoryTimeFilter::Today,
            TaskHistoryTimeFilter::Last7Days,
        ] {
            let page = repository
                .list_page(TaskHistoryQuery {
                    project_id: "project-1".to_owned(),
                    filter: TaskHistoryFilter::All,
                    workflow_id: Some("workflow-2".to_owned()),
                    keyword: Some("视频".to_owned()),
                    time_filter,
                    cursor: None,
                    limit: 10,
                })
                .await
                .unwrap();
            assert_eq!(page.items.len(), 1);
        }

        let id_search = repository
            .list_page(TaskHistoryQuery {
                project_id: "project-1".to_owned(),
                filter: TaskHistoryFilter::All,
                workflow_id: None,
                keyword: Some(recent.id.as_str().to_owned()),
                time_filter: TaskHistoryTimeFilter::All,
                cursor: None,
                limit: 10,
            })
            .await
            .unwrap();
        assert_eq!(id_search.items.len(), 1);
        assert_eq!(id_search.items[0].task.id, recent.id);

        let options = repository.list_workflow_options("project-1").await.unwrap();
        assert_eq!(options.len(), 2);
        assert!(options.iter().any(|option| {
            option.workflow_id == "workflow-2" && option.workflow_name == "视频工作流"
        }));
    }

    #[test]
    fn local_today_cutoff_uses_utc_plus_eight_calendar_day() {
        let timezone = FixedOffset::east_opt(8 * 60 * 60).unwrap();
        let local_now = timezone.with_ymd_and_hms(2026, 8, 10, 1, 0, 0).unwrap();

        assert_eq!(
            local_now.with_timezone(&Utc),
            Utc.with_ymd_and_hms(2026, 8, 9, 17, 0, 0).unwrap()
        );
        assert_eq!(
            local_date_start_utc(&timezone, local_now.date_naive()),
            Some(Utc.with_ymd_and_hms(2026, 8, 9, 16, 0, 0).unwrap())
        );
    }

    #[test]
    fn local_today_cutoff_uses_utc_minus_seven_calendar_day() {
        let timezone = FixedOffset::west_opt(7 * 60 * 60).unwrap();
        let local_now = timezone.with_ymd_and_hms(2026, 8, 9, 21, 0, 0).unwrap();

        assert_eq!(
            local_now.with_timezone(&Utc),
            Utc.with_ymd_and_hms(2026, 8, 10, 4, 0, 0).unwrap()
        );
        assert_eq!(
            local_date_start_utc(&timezone, local_now.date_naive()),
            Some(Utc.with_ymd_and_hms(2026, 8, 9, 7, 0, 0).unwrap())
        );
    }

    #[tokio::test]
    async fn today_combines_status_workflow_keyword_project_and_keyset_filters() {
        let (pool, repository) = setup().await;
        let today_start = today_start_utc();
        sqlx::query(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at)
             VALUES ('project-2', 'Other', 'C:/other', ?, ?)",
        )
        .bind(today_start.to_rfc3339())
        .bind(today_start.to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();

        for (id, project_id, created_at) in [
            ("tsk_today_first", "project-1", today_start),
            ("tsk_today_second", "project-1", today_start),
            (
                "tsk_local_yesterday",
                "project-1",
                today_start - Duration::seconds(1),
            ),
            ("tsk_other_project_today", "project-2", today_start),
        ] {
            sqlx::query(
                "INSERT INTO tasks (
                    id, project_id, workflow_id, workflow_version_id, recipe_id,
                    status, progress_mode, created_at, queued_at, started_at, finished_at
                 ) VALUES (?, ?, 'workflow-1', 'workflow-version-1', 'recipe-1',
                    'SUCCEEDED', 'indeterminate', ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(project_id)
            .bind(created_at.to_rfc3339())
            .bind(created_at.to_rfc3339())
            .bind(created_at.to_rfc3339())
            .bind(created_at.to_rfc3339())
            .execute(&pool)
            .await
            .unwrap();
        }

        let first_page = repository
            .list_page(TaskHistoryQuery {
                project_id: "project-1".to_owned(),
                filter: TaskHistoryFilter::Succeeded,
                workflow_id: Some("workflow-1".to_owned()),
                keyword: Some("Workflow".to_owned()),
                time_filter: TaskHistoryTimeFilter::Today,
                cursor: None,
                limit: 1,
            })
            .await
            .unwrap();
        assert_eq!(first_page.items.len(), 1);
        assert_eq!(first_page.items[0].task.id.as_str(), "tsk_today_second");
        assert!(first_page.next_cursor.is_some());

        let second_page = repository
            .list_page(TaskHistoryQuery {
                project_id: "project-1".to_owned(),
                filter: TaskHistoryFilter::Succeeded,
                workflow_id: Some("workflow-1".to_owned()),
                keyword: Some("Workflow".to_owned()),
                time_filter: TaskHistoryTimeFilter::Today,
                cursor: first_page.next_cursor,
                limit: 1,
            })
            .await
            .unwrap();
        assert_eq!(second_page.items.len(), 1);
        assert_eq!(second_page.items[0].task.id.as_str(), "tsk_today_first");
        assert!(second_page.next_cursor.is_none());
    }

    #[tokio::test]
    async fn cursor_serializes_as_created_at_and_id() {
        let cursor = PageCursor::for_item(Utc::now(), "tsk_test");
        let value = serde_json::to_value(cursor).unwrap();
        assert!(value.get("createdAt").is_some());
        assert!(value.get("id").is_some());
    }
}
