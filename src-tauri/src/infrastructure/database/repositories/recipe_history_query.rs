use super::{map_sqlx_error, parse_datetime, parse_optional_datetime};
use crate::application::pagination::{PageCursor, PageResult};
use crate::application::ports::{
    RecipeHistoryBindingRecord, RecipeHistoryCounts, RecipeHistoryDefinitionRecord,
    RecipeHistoryExperimentRecord, RecipeHistoryPresetRecord,
    RecipeHistoryProductionRunTemplateRecord, RecipeHistoryProjectTemplateRecord,
    RecipeHistoryQuery, RecipeHistoryQueryRecord, RecipeHistoryQueryRepository,
    RecipeHistoryQueueRecord, RecipeHistoryShotRecord, RecipeHistoryTaskRecord, RepositoryError,
};
use async_trait::async_trait;
use sqlx::{QueryBuilder, Sqlite, SqlitePool};

const RELATED_LIMIT: i64 = 50;

#[derive(Clone)]
pub struct SqliteRecipeHistoryQueryRepository {
    pool: SqlitePool,
}

impl SqliteRecipeHistoryQueryRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RecipeHistoryQueryRepository for SqliteRecipeHistoryQueryRepository {
    async fn get_exact_pair(
        &self,
        request: RecipeHistoryQuery,
    ) -> Result<Option<RecipeHistoryQueryRecord>, RepositoryError> {
        let definition = sqlx::query_as::<_, DefinitionRow>(
            "SELECT
                wv.workflow_id,
                r.workflow_version_id,
                r.id AS recipe_id,
                wv.version AS workflow_version,
                r.version AS recipe_version,
                r.schema_version,
                r.recipe_sha256,
                r.created_at,
                CASE WHEN w.current_version_id = wv.id THEN 1 ELSE 0 END AS is_current_version,
                CASE WHEN p.recipe_id IS NOT NULL THEN 1 ELSE 0 END AS is_promoted,
                p.promoted_at,
                COALESCE(rs.archived, 0) AS archived,
                rs.archived_at,
                w.library_state AS workflow_library_state
             FROM recipes r
             INNER JOIN workflow_versions wv ON wv.id = r.workflow_version_id
             INNER JOIN workflows w ON w.id = wv.workflow_id
             LEFT JOIN workflow_recipe_promotions p
                ON p.workflow_version_id = r.workflow_version_id
               AND p.recipe_id = r.id
             LEFT JOIN workflow_recipe_runtime_states rs
                ON rs.workflow_version_id = r.workflow_version_id
               AND rs.recipe_id = r.id
             WHERE r.workflow_version_id = ? AND r.id = ?
             LIMIT 1",
        )
        .bind(&request.workflow_version_id)
        .bind(&request.recipe_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        let Some(definition) = definition else {
            return Ok(None);
        };

        let counts = sqlx::query_as::<_, CountsRow>(
            "SELECT
                (SELECT COUNT(*) FROM tasks t
                 WHERE t.workflow_version_id = ? AND t.recipe_id = ?) AS task_count,
                (SELECT COUNT(*) FROM tasks t
                 WHERE t.workflow_version_id = ? AND t.recipe_id = ?
                   AND t.status NOT IN ('SUCCEEDED', 'FAILED', 'CANCELLED')) AS active_task_count,
                (SELECT COUNT(*) FROM tasks t
                 WHERE t.workflow_version_id = ? AND t.recipe_id = ? AND t.status = 'SUCCEEDED') AS succeeded_task_count,
                (SELECT COUNT(*) FROM tasks t
                 WHERE t.workflow_version_id = ? AND t.recipe_id = ? AND t.status = 'FAILED') AS failed_task_count,
                (SELECT COUNT(*) FROM tasks t
                 WHERE t.workflow_version_id = ? AND t.recipe_id = ? AND t.status = 'CANCELLED') AS cancelled_task_count,
                (SELECT MAX(t.finished_at) FROM tasks t
                 WHERE t.workflow_version_id = ? AND t.recipe_id = ?) AS last_finished_at,
                (SELECT MAX(t.created_at) FROM tasks t
                 WHERE t.workflow_version_id = ? AND t.recipe_id = ?) AS last_attempt_at,
                (SELECT COUNT(DISTINCT t.project_id) FROM tasks t
                 WHERE t.workflow_version_id = ? AND t.recipe_id = ?) AS executed_project_count,
                (SELECT COUNT(*) FROM (
                    SELECT t.project_id FROM tasks t
                     WHERE t.workflow_version_id = ? AND t.recipe_id = ?
                    UNION
                    SELECT b.project_id FROM production_batch_items i
                    INNER JOIN production_batches b ON b.id = i.batch_id
                     WHERE i.workflow_version_id = ? AND i.recipe_id = ?
                    UNION
                    SELECT pb.project_id FROM project_workflow_bindings pb
                     WHERE pb.workflow_version_id = ? AND pb.recipe_id = ?
                    UNION
                    SELECT pr.project_id FROM production_stages ps
                    INNER JOIN production_runs pr ON pr.id = ps.run_id
                     WHERE ps.workflow_version_id = ? AND ps.recipe_id = ?
                    UNION
                    SELECT prt.project_id FROM production_run_templates prt
                     WHERE (prt.krea2_workflow_version_id = ? AND prt.krea2_recipe_id = ?)
                        OR (prt.h3_workflow_version_id = ? AND prt.h3_recipe_id = ?)
                )) AS referenced_project_count,
                (SELECT COUNT(*) FROM presets p
                 WHERE p.workflow_version_id = ? AND p.recipe_id = ?) AS preset_count,
                (SELECT COUNT(*) FROM project_templates pt
                 WHERE pt.workflow_version_id = ? AND pt.recipe_id = ?) AS project_template_count,
                (SELECT COUNT(*) FROM production_run_templates prt
                 WHERE (prt.krea2_workflow_version_id = ? AND prt.krea2_recipe_id = ?)
                    OR (prt.h3_workflow_version_id = ? AND prt.h3_recipe_id = ?)) AS production_run_template_count,
                (SELECT COUNT(*) FROM production_batch_items i
                 WHERE i.workflow_version_id = ? AND i.recipe_id = ?) AS queue_item_count,
                (SELECT COUNT(*) FROM shot_stage_configs ssc
                 WHERE ssc.workflow_version_id = ? AND ssc.recipe_id = ?) AS shot_stage_count,
                (SELECT COUNT(DISTINCT bc.experiment_id) FROM benchmark_candidates bc
                 WHERE bc.workflow_version_id = ? AND bc.recipe_id = ?) AS experiment_count,
                (SELECT COUNT(*) FROM benchmark_runs br
                 INNER JOIN benchmark_candidates bc ON bc.id = br.candidate_id
                 WHERE bc.workflow_version_id = ? AND bc.recipe_id = ?) AS benchmark_run_count,
                (SELECT COUNT(*) FROM project_workflow_bindings pb
                 WHERE pb.workflow_version_id = ? AND pb.recipe_id = ?) AS project_binding_count",
        )
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .bind(&request.workflow_version_id).bind(&request.recipe_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        let tasks = self.load_tasks(&request).await?;
        let queue_items = self.load_queue_items(&request).await?;
        let presets = self.load_presets(&request).await?;
        let project_templates = self.load_project_templates(&request).await?;
        let production_run_templates = self.load_production_run_templates(&request).await?;
        let project_bindings = self.load_project_bindings(&request).await?;
        let shots = self.load_shots(&request).await?;
        let experiments = self.load_experiments(&request).await?;

        Ok(Some(RecipeHistoryQueryRecord {
            definition: definition.try_into_record()?,
            counts: counts.try_into_counts()?,
            tasks,
            queue_items,
            presets,
            project_templates,
            production_run_templates,
            project_bindings,
            shots,
            experiments,
        }))
    }
}

impl SqliteRecipeHistoryQueryRepository {
    async fn load_tasks(
        &self,
        request: &RecipeHistoryQuery,
    ) -> Result<PageResult<RecipeHistoryTaskRecord>, RepositoryError> {
        #[derive(sqlx::FromRow)]
        struct Row {
            id: String,
            project_id: String,
            project_name: String,
            status: String,
            created_at: String,
            queued_at: Option<String>,
            started_at: Option<String>,
            finished_at: Option<String>,
        }

        let limit = i64::from(request.task_limit.clamp(1, 100));
        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT t.id, t.project_id, p.name AS project_name, t.status,
                    t.created_at, t.queued_at, t.started_at, t.finished_at
             FROM tasks t
             INNER JOIN projects p ON p.id = t.project_id
             WHERE t.workflow_version_id = ",
        );
        query
            .push_bind(&request.workflow_version_id)
            .push(" AND t.recipe_id = ")
            .push_bind(&request.recipe_id);
        if let Some(cursor) = &request.task_cursor {
            let created_at = cursor.created_at.to_rfc3339();
            query
                .push(" AND (t.created_at < ")
                .push_bind(created_at.clone())
                .push(" OR (t.created_at = ")
                .push_bind(created_at)
                .push(" AND t.id < ")
                .push_bind(&cursor.id)
                .push("))");
        }
        query
            .push(" ORDER BY t.created_at DESC, t.id DESC LIMIT ")
            .push_bind(limit + 1);
        let mut rows = query
            .build_query_as::<Row>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        let has_more = rows.len() > limit as usize;
        if has_more {
            rows.truncate(limit as usize);
        }
        let next_cursor = has_more
            .then(|| rows.last())
            .flatten()
            .map(|row| parse_datetime("recipe history task created_at", &row.created_at))
            .transpose()?
            .map(|created_at| {
                PageCursor::for_item(created_at, rows.last().expect("row exists").id.clone())
            });
        let items = rows
            .into_iter()
            .map(|row| {
                Ok(RecipeHistoryTaskRecord {
                    id: row.id,
                    project_id: row.project_id,
                    project_name: row.project_name,
                    status: row.status,
                    created_at: parse_datetime("recipe history task created_at", &row.created_at)?,
                    queued_at: parse_optional_datetime(
                        "recipe history task queued_at",
                        row.queued_at.as_deref(),
                    )?,
                    started_at: parse_optional_datetime(
                        "recipe history task started_at",
                        row.started_at.as_deref(),
                    )?,
                    finished_at: parse_optional_datetime(
                        "recipe history task finished_at",
                        row.finished_at.as_deref(),
                    )?,
                })
            })
            .collect::<Result<Vec<_>, RepositoryError>>()?;
        Ok(PageResult { items, next_cursor })
    }

    async fn load_queue_items(
        &self,
        request: &RecipeHistoryQuery,
    ) -> Result<Vec<RecipeHistoryQueueRecord>, RepositoryError> {
        #[derive(sqlx::FromRow)]
        struct Row {
            batch_id: String,
            batch_name: String,
            project_id: String,
            project_name: String,
            batch_status: String,
            item_id: String,
            item_status: String,
            task_id: Option<String>,
            created_at: String,
            updated_at: String,
        }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT b.id AS batch_id, b.name AS batch_name, b.project_id, p.name AS project_name,
                    b.status AS batch_status, i.id AS item_id, i.status AS item_status, i.task_id,
                    i.created_at, i.updated_at
             FROM production_batch_items i
             INNER JOIN production_batches b ON b.id = i.batch_id
             INNER JOIN projects p ON p.id = b.project_id
             WHERE i.workflow_version_id = ? AND i.recipe_id = ?
             ORDER BY i.updated_at DESC, i.id DESC LIMIT ?",
        )
        .bind(&request.workflow_version_id)
        .bind(&request.recipe_id)
        .bind(RELATED_LIMIT)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(|row| {
                Ok(RecipeHistoryQueueRecord {
                    batch_id: row.batch_id,
                    batch_name: row.batch_name,
                    project_id: row.project_id,
                    project_name: row.project_name,
                    batch_status: row.batch_status,
                    item_id: row.item_id,
                    item_status: row.item_status,
                    task_id: row.task_id,
                    created_at: parse_datetime("recipe history queue created_at", &row.created_at)?,
                    updated_at: parse_datetime("recipe history queue updated_at", &row.updated_at)?,
                })
            })
            .collect()
    }

    async fn load_presets(
        &self,
        request: &RecipeHistoryQuery,
    ) -> Result<Vec<RecipeHistoryPresetRecord>, RepositoryError> {
        #[derive(sqlx::FromRow)]
        struct Row {
            id: String,
            name: String,
            project_id: String,
            project_name: String,
            updated_at: String,
        }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT pr.id, pr.name, pr.project_id, p.name AS project_name, pr.updated_at
             FROM presets pr INNER JOIN projects p ON p.id = pr.project_id
             WHERE pr.workflow_version_id = ? AND pr.recipe_id = ?
             ORDER BY pr.updated_at DESC, pr.id DESC LIMIT ?",
        )
        .bind(&request.workflow_version_id)
        .bind(&request.recipe_id)
        .bind(RELATED_LIMIT)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(|row| {
                Ok(RecipeHistoryPresetRecord {
                    id: row.id,
                    name: row.name,
                    project_id: row.project_id,
                    project_name: row.project_name,
                    updated_at: parse_datetime(
                        "recipe history preset updated_at",
                        &row.updated_at,
                    )?,
                })
            })
            .collect()
    }

    async fn load_project_templates(
        &self,
        request: &RecipeHistoryQuery,
    ) -> Result<Vec<RecipeHistoryProjectTemplateRecord>, RepositoryError> {
        #[derive(sqlx::FromRow)]
        struct Row {
            id: String,
            name: String,
            updated_at: String,
        }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT id, name, updated_at FROM project_templates
             WHERE workflow_version_id = ? AND recipe_id = ?
             ORDER BY updated_at DESC, id DESC LIMIT ?",
        )
        .bind(&request.workflow_version_id)
        .bind(&request.recipe_id)
        .bind(RELATED_LIMIT)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(|row| {
                Ok(RecipeHistoryProjectTemplateRecord {
                    id: row.id,
                    name: row.name,
                    updated_at: parse_datetime(
                        "recipe history project template updated_at",
                        &row.updated_at,
                    )?,
                })
            })
            .collect()
    }

    async fn load_production_run_templates(
        &self,
        request: &RecipeHistoryQuery,
    ) -> Result<Vec<RecipeHistoryProductionRunTemplateRecord>, RepositoryError> {
        #[derive(sqlx::FromRow)]
        struct Row {
            id: String,
            name: String,
            project_id: String,
            project_name: String,
            recipe_role: String,
            updated_at: String,
        }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT prt.id, prt.name, prt.project_id, p.name AS project_name,
                    CASE WHEN prt.krea2_workflow_version_id = ? AND prt.krea2_recipe_id = ?
                         AND prt.h3_workflow_version_id = ? AND prt.h3_recipe_id = ? THEN 'KREA2_AND_H3'
                         WHEN prt.krea2_workflow_version_id = ? AND prt.krea2_recipe_id = ? THEN 'KREA2'
                         ELSE 'H3' END AS recipe_role,
                    prt.updated_at
             FROM production_run_templates prt INNER JOIN projects p ON p.id = prt.project_id
             WHERE (prt.krea2_workflow_version_id = ? AND prt.krea2_recipe_id = ?)
                OR (prt.h3_workflow_version_id = ? AND prt.h3_recipe_id = ?)
             ORDER BY prt.updated_at DESC, prt.id DESC LIMIT ?",
        ).bind(&request.workflow_version_id).bind(&request.recipe_id)
         .bind(&request.workflow_version_id).bind(&request.recipe_id)
         .bind(&request.workflow_version_id).bind(&request.recipe_id)
         .bind(&request.workflow_version_id).bind(&request.recipe_id)
         .bind(&request.workflow_version_id).bind(&request.recipe_id).bind(RELATED_LIMIT)
         .fetch_all(&self.pool).await.map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(|row| {
                Ok(RecipeHistoryProductionRunTemplateRecord {
                    id: row.id,
                    name: row.name,
                    project_id: row.project_id,
                    project_name: row.project_name,
                    recipe_role: row.recipe_role,
                    updated_at: parse_datetime(
                        "recipe history production run template updated_at",
                        &row.updated_at,
                    )?,
                })
            })
            .collect()
    }

    async fn load_project_bindings(
        &self,
        request: &RecipeHistoryQuery,
    ) -> Result<Vec<RecipeHistoryBindingRecord>, RepositoryError> {
        #[derive(sqlx::FromRow)]
        struct Row {
            project_id: String,
            project_name: String,
            stage: String,
            mode: String,
            updated_at: String,
        }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT b.project_id, p.name AS project_name, b.stage, b.mode, b.updated_at
             FROM project_workflow_bindings b INNER JOIN projects p ON p.id = b.project_id
             WHERE b.workflow_version_id = ? AND b.recipe_id = ?
             ORDER BY b.updated_at DESC, b.project_id ASC LIMIT ?",
        )
        .bind(&request.workflow_version_id)
        .bind(&request.recipe_id)
        .bind(RELATED_LIMIT)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(|row| {
                Ok(RecipeHistoryBindingRecord {
                    project_id: row.project_id,
                    project_name: row.project_name,
                    stage: row.stage,
                    mode: row.mode,
                    updated_at: parse_datetime(
                        "recipe history project binding updated_at",
                        &row.updated_at,
                    )?,
                })
            })
            .collect()
    }

    async fn load_shots(
        &self,
        request: &RecipeHistoryQuery,
    ) -> Result<Vec<RecipeHistoryShotRecord>, RepositoryError> {
        #[derive(sqlx::FromRow)]
        struct Row {
            shot_id: String,
            project_id: String,
            project_name: String,
            stage: String,
            updated_at: String,
            task_id: Option<String>,
            production_batch_item_id: Option<String>,
        }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT c.shot_id, s.project_id, p.name AS project_name, c.stage, c.updated_at,
                    l.task_id, l.production_batch_item_id
             FROM shot_stage_configs c
             INNER JOIN shots s ON s.id = c.shot_id
             INNER JOIN projects p ON p.id = s.project_id
             LEFT JOIN shot_generation_links l ON l.shot_id = c.shot_id AND l.stage = c.stage
             WHERE c.workflow_version_id = ? AND c.recipe_id = ?
             ORDER BY c.updated_at DESC, c.shot_id ASC, l.created_at DESC LIMIT ?",
        )
        .bind(&request.workflow_version_id)
        .bind(&request.recipe_id)
        .bind(RELATED_LIMIT)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(|row| {
                Ok(RecipeHistoryShotRecord {
                    shot_id: row.shot_id,
                    project_id: row.project_id,
                    project_name: row.project_name,
                    stage: row.stage,
                    updated_at: parse_datetime("recipe history shot updated_at", &row.updated_at)?,
                    task_id: row.task_id,
                    production_batch_item_id: row.production_batch_item_id,
                })
            })
            .collect()
    }

    async fn load_experiments(
        &self,
        request: &RecipeHistoryQuery,
    ) -> Result<Vec<RecipeHistoryExperimentRecord>, RepositoryError> {
        #[derive(sqlx::FromRow)]
        struct Row {
            experiment_id: String,
            experiment_name: String,
            project_id: String,
            project_name: String,
            candidate_id: String,
            run_count: i64,
            created_at: String,
        }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT e.id AS experiment_id, e.name AS experiment_name, e.project_id, p.name AS project_name,
                    c.id AS candidate_id, COUNT(br.id) AS run_count, c.created_at
             FROM benchmark_candidates c
             INNER JOIN benchmark_experiments e ON e.id = c.experiment_id
             INNER JOIN projects p ON p.id = e.project_id
             LEFT JOIN benchmark_runs br ON br.candidate_id = c.id
             WHERE c.workflow_version_id = ? AND c.recipe_id = ?
             GROUP BY e.id, e.name, e.project_id, p.name, c.id, c.created_at
             ORDER BY c.created_at DESC, c.id DESC LIMIT ?",
        ).bind(&request.workflow_version_id).bind(&request.recipe_id).bind(RELATED_LIMIT)
         .fetch_all(&self.pool).await.map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(|row| {
                Ok(RecipeHistoryExperimentRecord {
                    experiment_id: row.experiment_id,
                    experiment_name: row.experiment_name,
                    project_id: row.project_id,
                    project_name: row.project_name,
                    candidate_id: row.candidate_id,
                    run_count: u64::try_from(row.run_count).map_err(|_| {
                        RepositoryError::serialization("recipe history run count", "negative count")
                    })?,
                    created_at: parse_datetime(
                        "recipe history experiment created_at",
                        &row.created_at,
                    )?,
                })
            })
            .collect()
    }
}

#[derive(sqlx::FromRow)]
struct DefinitionRow {
    workflow_id: String,
    workflow_version_id: String,
    recipe_id: String,
    workflow_version: String,
    recipe_version: String,
    schema_version: i64,
    recipe_sha256: String,
    created_at: String,
    is_current_version: i64,
    is_promoted: i64,
    promoted_at: Option<String>,
    archived: i64,
    archived_at: Option<String>,
    workflow_library_state: String,
}

impl DefinitionRow {
    fn try_into_record(self) -> Result<RecipeHistoryDefinitionRecord, RepositoryError> {
        Ok(RecipeHistoryDefinitionRecord {
            workflow_id: self.workflow_id,
            workflow_version_id: self.workflow_version_id,
            recipe_id: self.recipe_id,
            workflow_version: self.workflow_version,
            recipe_version: self.recipe_version,
            schema_version: u32::try_from(self.schema_version).map_err(|_| {
                RepositoryError::serialization("recipe schema version", "invalid value")
            })?,
            recipe_sha256: self.recipe_sha256,
            created_at: parse_datetime("recipe history recipe created_at", &self.created_at)?,
            is_current_version: self.is_current_version != 0,
            is_promoted: self.is_promoted != 0,
            promoted_at: parse_optional_datetime(
                "recipe history promoted_at",
                self.promoted_at.as_deref(),
            )?,
            archived: self.archived != 0,
            archived_at: parse_optional_datetime(
                "recipe history archived_at",
                self.archived_at.as_deref(),
            )?,
            workflow_library_state: self.workflow_library_state,
        })
    }
}

#[derive(sqlx::FromRow)]
struct CountsRow {
    task_count: i64,
    active_task_count: i64,
    succeeded_task_count: i64,
    failed_task_count: i64,
    cancelled_task_count: i64,
    last_finished_at: Option<String>,
    last_attempt_at: Option<String>,
    executed_project_count: i64,
    referenced_project_count: i64,
    preset_count: i64,
    project_template_count: i64,
    production_run_template_count: i64,
    queue_item_count: i64,
    shot_stage_count: i64,
    experiment_count: i64,
    benchmark_run_count: i64,
    project_binding_count: i64,
}

impl CountsRow {
    fn try_into_counts(self) -> Result<RecipeHistoryCounts, RepositoryError> {
        let count = |name: &'static str, value: i64| {
            u64::try_from(value).map_err(|_| RepositoryError::serialization(name, "negative count"))
        };
        Ok(RecipeHistoryCounts {
            task_count: count("recipe history task count", self.task_count)?,
            active_task_count: count("recipe history active task count", self.active_task_count)?,
            succeeded_task_count: count(
                "recipe history succeeded task count",
                self.succeeded_task_count,
            )?,
            failed_task_count: count("recipe history failed task count", self.failed_task_count)?,
            cancelled_task_count: count(
                "recipe history cancelled task count",
                self.cancelled_task_count,
            )?,
            last_finished_at: parse_optional_datetime(
                "recipe history last_finished_at",
                self.last_finished_at.as_deref(),
            )?,
            last_attempt_at: parse_optional_datetime(
                "recipe history last_attempt_at",
                self.last_attempt_at.as_deref(),
            )?,
            executed_project_count: count(
                "recipe history executed project count",
                self.executed_project_count,
            )?,
            referenced_project_count: count(
                "recipe history referenced project count",
                self.referenced_project_count,
            )?,
            preset_count: count("recipe history preset count", self.preset_count)?,
            project_template_count: count(
                "recipe history project template count",
                self.project_template_count,
            )?,
            production_run_template_count: count(
                "recipe history production run template count",
                self.production_run_template_count,
            )?,
            queue_item_count: count("recipe history queue item count", self.queue_item_count)?,
            shot_stage_count: count("recipe history shot stage count", self.shot_stage_count)?,
            experiment_count: count("recipe history experiment count", self.experiment_count)?,
            benchmark_run_count: count(
                "recipe history benchmark run count",
                self.benchmark_run_count,
            )?,
            project_binding_count: count(
                "recipe history project binding count",
                self.project_binding_count,
            )?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteRecipeHistoryQueryRepository;
    use crate::application::ports::{RecipeHistoryQuery, RecipeHistoryQueryRepository};
    use crate::infrastructure::database::{initialize, repositories::test_support};
    use tempfile::tempdir;

    #[tokio::test]
    async fn exact_pair_query_is_read_only_and_excludes_other_recipe_tasks() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        sqlx::query(
            "INSERT INTO recipes (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at)
             VALUES ('recipe-2', 'workflow-version-1', '2', 1, 'schema_version: 1', 'sha-2', '2026-01-02T00:00:00Z')",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO tasks (id, project_id, workflow_id, workflow_version_id, recipe_id, status, created_at)
             VALUES ('task-2', 'project-1', 'workflow-1', 'workflow-version-1', 'recipe-2', 'SUCCEEDED', '2026-01-02T00:00:00Z')",
        ).execute(&pool).await.unwrap();
        let repository = SqliteRecipeHistoryQueryRepository::new(pool);
        let result = repository
            .get_exact_pair(RecipeHistoryQuery {
                workflow_version_id: "workflow-version-1".to_owned(),
                recipe_id: "recipe-2".to_owned(),
                task_cursor: None,
                task_limit: 20,
            })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.counts.task_count, 1);
        assert_eq!(result.counts.succeeded_task_count, 1);
        assert!(repository
            .get_exact_pair(RecipeHistoryQuery {
                workflow_version_id: "workflow-version-1".to_owned(),
                recipe_id: "missing".to_owned(),
                task_cursor: None,
                task_limit: 20,
            })
            .await
            .unwrap()
            .is_none());
    }
}
