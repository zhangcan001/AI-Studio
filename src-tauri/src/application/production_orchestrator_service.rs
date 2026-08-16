use crate::application::generation_input_preparer::GenerationInputValue;
use crate::application::ports::{Clock, GenerationDefinitionRepository};
use crate::application::production_queue_service::{
    generation_values_from_json, generation_values_to_json, CreateProductionBatchItem,
    CreateProductionBatchRequest, ProductionQueueError, ProductionQueueService,
};
use crate::application::task_cancellation_service::TaskCancellationService;
use crate::compiler::RecipeParser;
use crate::domain::InputDefinition;
use crate::domain::{ProductionRunStatus, ProductionStageStatus, ProductionStageType};
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::{FromRow, QueryBuilder, Sqlite, SqlitePool};
use std::{collections::BTreeMap, error::Error, fmt, sync::Arc};
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

const MAX_RUN_IMAGE_COUNT: u32 = 100;

#[derive(Clone, Debug, PartialEq)]
pub struct ProductionRunCreateRequest {
    pub project_id: String,
    pub name: String,
    pub krea2_workflow_version_id: String,
    pub krea2_recipe_id: String,
    pub krea2_preset_id: Option<String>,
    pub krea2_values: BTreeMap<String, GenerationInputValue>,
    pub image_count: u32,
    pub h3_workflow_version_id: Option<String>,
    pub h3_recipe_id: Option<String>,
    pub h3_profile: Option<String>,
    pub h3_values: BTreeMap<String, GenerationInputValue>,
    pub template_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProductionRunTemplateRequest {
    pub project_id: String,
    pub name: String,
    pub krea2_workflow_version_id: Option<String>,
    pub krea2_recipe_id: Option<String>,
    pub krea2_preset_id: Option<String>,
    pub default_image_count: u32,
    pub h3_workflow_version_id: Option<String>,
    pub h3_recipe_id: Option<String>,
    pub h3_profile: Option<String>,
    pub default_duration_seconds: Option<u32>,
    pub default_width: Option<u32>,
    pub default_height: Option<u32>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProductionRunStageItemView {
    pub id: String,
    pub stage_id: String,
    pub ordinal: u32,
    pub status: String,
    pub production_batch_item_id: Option<String>,
    pub task_id: Option<String>,
    pub task_status: Option<String>,
    pub asset_id: Option<String>,
    pub source_asset_id: Option<String>,
    pub reference_index: Option<u32>,
    pub attempt: u32,
    pub submission_idempotency_key: Option<String>,
    pub parent_stage_item_id: Option<String>,
    pub frozen_values: Value,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProductionRunStageView {
    pub id: String,
    pub ordinal: u32,
    pub stage_type: String,
    pub status: String,
    pub workflow_version_id: Option<String>,
    pub recipe_id: Option<String>,
    pub production_batch_id: Option<String>,
    pub frozen_config: Value,
    pub prompt: Option<String>,
    pub items: Vec<ProductionRunStageItemView>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProductionRunView {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub status: String,
    pub current_stage_ordinal: u32,
    pub template_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub stages: Vec<ProductionRunStageView>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProductionRunListItem {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub status: String,
    pub current_stage_ordinal: u32,
    pub template_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProductionRunTemplateView {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub krea2_workflow_version_id: Option<String>,
    pub krea2_recipe_id: Option<String>,
    pub krea2_preset_id: Option<String>,
    pub default_image_count: u32,
    pub h3_workflow_version_id: Option<String>,
    pub h3_recipe_id: Option<String>,
    pub h3_profile: Option<String>,
    pub default_duration_seconds: Option<u32>,
    pub default_width: Option<u32>,
    pub default_height: Option<u32>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug)]
pub enum ProductionOrchestratorError {
    InvalidInput(String),
    InvalidState(String),
    NotFound(String),
    Repository(String),
    Queue(String),
}

impl fmt::Display for ProductionOrchestratorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(formatter, "INVALID_INPUT: {message}"),
            Self::InvalidState(message) => {
                write!(formatter, "PRODUCTION_RUN_INVALID_STATE: {message}")
            }
            Self::NotFound(message) => write!(formatter, "PRODUCTION_RUN_NOT_FOUND: {message}"),
            Self::Repository(message) => write!(formatter, "DATABASE_ERROR: {message}"),
            Self::Queue(message) => write!(formatter, "PRODUCTION_QUEUE_ERROR: {message}"),
        }
    }
}

impl Error for ProductionOrchestratorError {}

impl From<ProductionQueueError> for ProductionOrchestratorError {
    fn from(error: ProductionQueueError) -> Self {
        Self::Queue(error.to_string())
    }
}

#[derive(Clone)]
pub struct ProductionOrchestratorService {
    pool: SqlitePool,
    definition_repository: Arc<dyn GenerationDefinitionRepository>,
    production_queue_service: Arc<ProductionQueueService>,
    task_cancellation_service: Arc<TaskCancellationService>,
    clock: Arc<dyn Clock>,
    stage_trigger_gate: Arc<AsyncMutex<()>>,
}

impl ProductionOrchestratorService {
    pub fn new(
        pool: SqlitePool,
        definition_repository: Arc<dyn GenerationDefinitionRepository>,
        production_queue_service: Arc<ProductionQueueService>,
        task_cancellation_service: Arc<TaskCancellationService>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            pool,
            definition_repository,
            production_queue_service,
            task_cancellation_service,
            clock,
            stage_trigger_gate: Arc::new(AsyncMutex::new(())),
        }
    }

    pub async fn create(
        &self,
        request: ProductionRunCreateRequest,
    ) -> Result<ProductionRunView, ProductionOrchestratorError> {
        validate_project_id(&request.project_id)?;
        let name = validate_name(&request.name)?;
        if request.image_count == 0 || request.image_count > MAX_RUN_IMAGE_COUNT {
            return Err(ProductionOrchestratorError::InvalidInput(
                "图片数量必须在 1–100 之间。".to_owned(),
            ));
        }
        self.ensure_definition(&request.krea2_workflow_version_id, &request.krea2_recipe_id)
            .await?;
        if let (Some(workflow_version_id), Some(recipe_id)) = (
            request.h3_workflow_version_id.as_deref(),
            request.h3_recipe_id.as_deref(),
        ) {
            self.ensure_definition(workflow_version_id, recipe_id)
                .await?;
        }
        if request.template_id.is_some() {
            let exists = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM production_run_templates WHERE id = ? AND project_id = ?",
            )
            .bind(request.template_id.as_deref())
            .bind(&request.project_id)
            .fetch_one(&self.pool)
            .await
            .map_err(db_error)?;
            if exists == 0 {
                return Err(ProductionOrchestratorError::InvalidInput(
                    "Production Run 模板不存在或不属于当前项目。".to_owned(),
                ));
            }
        }
        let id = format!("prun_{}", Uuid::new_v4().simple());
        let krea2_values = generation_values_to_json(&request.krea2_values);
        let h3_values = generation_values_to_json(&request.h3_values);
        let frozen_krea2 = json!({
            "workflowVersionId": request.krea2_workflow_version_id,
            "recipeId": request.krea2_recipe_id,
            "presetId": request.krea2_preset_id,
            "imageCount": request.image_count,
            "values": krea2_values,
        });
        let frozen_h3 = json!({
            "workflowVersionId": request.h3_workflow_version_id,
            "recipeId": request.h3_recipe_id,
            "profile": request.h3_profile,
            "values": h3_values,
        });
        let now = self.clock.now().to_rfc3339();
        let mut transaction = self.pool.begin().await.map_err(db_error)?;
        sqlx::query(
            "INSERT INTO production_runs
             (id, project_id, name, status, current_stage_ordinal, template_id, created_at, updated_at)
             VALUES (?, ?, ?, 'READY', 0, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&request.project_id)
        .bind(name)
        .bind(&request.template_id)
        .bind(&now)
        .bind(&now)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
        let krea2_workflow = frozen_string(&frozen_krea2, "workflowVersionId");
        let krea2_recipe = frozen_string(&frozen_krea2, "recipeId");
        let h3_workflow = frozen_string(&frozen_h3, "workflowVersionId");
        let h3_recipe = frozen_string(&frozen_h3, "recipeId");
        insert_stage(
            &mut transaction,
            &id,
            0,
            ProductionStageType::Krea2ImageGeneration,
            ProductionStageStatus::Ready,
            krea2_workflow.as_deref(),
            krea2_recipe.as_deref(),
            &frozen_krea2,
            None,
            &now,
        )
        .await?;
        insert_stage(
            &mut transaction,
            &id,
            1,
            ProductionStageType::AssetSelection,
            ProductionStageStatus::Pending,
            None,
            None,
            &json!({"selectionMode":"MANUAL","maxAssets":8}),
            None,
            &now,
        )
        .await?;
        insert_stage(
            &mut transaction,
            &id,
            2,
            ProductionStageType::H3VideoGeneration,
            ProductionStageStatus::Pending,
            h3_workflow.as_deref(),
            h3_recipe.as_deref(),
            &frozen_h3,
            None,
            &now,
        )
        .await?;
        transaction.commit().await.map_err(db_error)?;
        self.get(&request.project_id, &id).await
    }

    pub async fn list(
        &self,
        project_id: &str,
        limit: u32,
    ) -> Result<Vec<ProductionRunListItem>, ProductionOrchestratorError> {
        validate_project_id(project_id)?;
        let rows = sqlx::query_as::<_, ProductionRunRow>(
            "SELECT id, project_id, name, status, current_stage_ordinal, template_id,
                    created_at, updated_at, started_at, finished_at
             FROM production_runs WHERE project_id = ?
             ORDER BY created_at DESC, id ASC LIMIT ?",
        )
        .bind(project_id)
        .bind(i64::from(limit.clamp(1, 50)))
        .fetch_all(&self.pool)
        .await
        .map_err(db_error)?;
        Ok(rows.into_iter().map(ProductionRunListItem::from).collect())
    }

    pub async fn get(
        &self,
        project_id: &str,
        run_id: &str,
    ) -> Result<ProductionRunView, ProductionOrchestratorError> {
        validate_project_id(project_id)?;
        let run = self.load_run(project_id, run_id).await?;
        self.sync_run_state(&run.id).await?;
        self.load_view(project_id, run_id).await
    }

    async fn load_run(
        &self,
        project_id: &str,
        run_id: &str,
    ) -> Result<ProductionRunRow, ProductionOrchestratorError> {
        sqlx::query_as::<_, ProductionRunRow>(
            "SELECT id, project_id, name, status, current_stage_ordinal, template_id,
                    created_at, updated_at, started_at, finished_at
             FROM production_runs WHERE project_id = ? AND id = ?",
        )
        .bind(project_id)
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| ProductionOrchestratorError::NotFound(run_id.to_owned()))
    }

    async fn load_view(
        &self,
        project_id: &str,
        run_id: &str,
    ) -> Result<ProductionRunView, ProductionOrchestratorError> {
        let run = self.load_run(project_id, run_id).await?;
        let stage_rows = sqlx::query_as::<_, ProductionStageRow>(
            "SELECT id, run_id, ordinal, stage_type, status, workflow_version_id, recipe_id,
                    production_batch_id, frozen_config_json, prompt
             FROM production_stages WHERE run_id = ? ORDER BY ordinal ASC",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .map_err(db_error)?;
        let item_rows = sqlx::query_as::<_, ProductionStageItemRow>(
            "SELECT si.id, si.stage_id, si.ordinal,
                    CASE
                      WHEN i.status IN ('DISPATCHING', 'DISPATCHED') THEN 'RUNNING'
                      WHEN i.status IS NOT NULL THEN i.status
                      ELSE si.status
                    END AS status,
                    si.production_batch_item_id,
                    COALESCE(i.task_id, si.task_id) AS task_id,
                    t.status AS task_status,
                    COALESCE(si.asset_id, (
                      SELECT MIN(oa.asset_id) FROM task_output_assets oa WHERE oa.task_id = COALESCE(i.task_id, si.task_id)
                    )) AS asset_id,
                    si.source_asset_id, si.reference_index, si.attempt,
                    si.submission_idempotency_key, si.parent_stage_item_id,
                    si.frozen_values_json, si.error_code, si.error_message
             FROM production_stage_items si
             LEFT JOIN production_batch_items i ON i.id = si.production_batch_item_id
             LEFT JOIN tasks t ON t.id = COALESCE(i.task_id, si.task_id)
             INNER JOIN production_stages s ON s.id = si.stage_id
             WHERE s.run_id = ? ORDER BY si.stage_id ASC, si.ordinal ASC",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .map_err(db_error)?;
        let mut items_by_stage: BTreeMap<String, Vec<ProductionRunStageItemView>> = BTreeMap::new();
        for row in item_rows {
            items_by_stage
                .entry(row.stage_id.clone())
                .or_default()
                .push(row.into_view()?);
        }
        let stages = stage_rows
            .into_iter()
            .map(|row| {
                let items = items_by_stage.remove(&row.id).unwrap_or_default();
                row.into_view(items)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ProductionRunView {
            id: run.id,
            project_id: run.project_id,
            name: run.name,
            status: run.status,
            current_stage_ordinal: u32::try_from(run.current_stage_ordinal).unwrap_or_default(),
            template_id: run.template_id,
            created_at: run.created_at,
            updated_at: run.updated_at,
            started_at: run.started_at,
            finished_at: run.finished_at,
            stages,
        })
    }

    pub async fn run_images(
        &self,
        project_id: &str,
        run_id: &str,
    ) -> Result<ProductionRunView, ProductionOrchestratorError> {
        let _trigger_gate = self.stage_trigger_gate.lock().await;
        validate_project_id(project_id)?;
        let run = self.load_run(project_id, run_id).await?;
        let stage = self.load_stage(run_id, 0).await?;
        if let Some(batch_id) = stage.production_batch_id.as_deref() {
            if run.status == ProductionRunStatus::Ready.as_str()
                && stage.status == ProductionStageStatus::Ready.as_str()
            {
                self.production_queue_service
                    .start(project_id, batch_id)
                    .await?;
                self.mark_stage_running(run_id, 0).await?;
                return self.get(project_id, run_id).await;
            }
            return Err(ProductionOrchestratorError::InvalidState(
                "Krea2 Stage 已经创建批次，重复触发不会创建第二个任务。".to_owned(),
            ));
        }
        if run.status != ProductionRunStatus::Ready.as_str()
            || stage.status != ProductionStageStatus::Ready.as_str()
        {
            return Err(ProductionOrchestratorError::InvalidState(
                "Production Run 当前不能启动 Krea2 Stage。".to_owned(),
            ));
        }
        let frozen_config = parse_json(&stage.frozen_config_json)?;
        let values = values_from_config(&stage.frozen_config_json)?;
        let image_count = frozen_config
            .get("imageCount")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(1);
        let workflow_version_id = stage.workflow_version_id.clone().ok_or_else(|| {
            ProductionOrchestratorError::InvalidState("Krea2 Workflow 未冻结。".to_owned())
        })?;
        let recipe_id = stage.recipe_id.clone().ok_or_else(|| {
            ProductionOrchestratorError::InvalidState("Krea2 Recipe 未冻结。".to_owned())
        })?;
        let queue = self
            .production_queue_service
            .create(CreateProductionBatchRequest {
                project_id: project_id.to_owned(),
                name: format!("Production Run · {} · Krea2", run.name),
                continue_on_failure: true,
                items: (0..image_count)
                    .map(|_| CreateProductionBatchItem {
                        workflow_version_id: workflow_version_id.clone(),
                        recipe_id: recipe_id.clone(),
                        values: values.clone(),
                    })
                    .collect(),
            })
            .await?;
        let now = self.clock.now().to_rfc3339();
        let mut transaction = self.pool.begin().await.map_err(db_error)?;
        sqlx::query(
            "UPDATE production_stages SET production_batch_id = ?, status = 'READY', updated_at = ?
             WHERE run_id = ? AND ordinal = 0",
        )
        .bind(queue.batch.id.as_str())
        .bind(&now)
        .bind(run_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
        for item in &queue.items {
            insert_stage_item(
                &mut transaction,
                &stage.id,
                item.ordinal,
                "PENDING",
                Some(item.id.as_str()),
                None,
                None,
                None,
                1,
                None,
                &item.values_json,
                None,
                None,
                &now,
            )
            .await?;
        }
        transaction.commit().await.map_err(db_error)?;
        self.production_queue_service
            .start(project_id, queue.batch.id.as_str())
            .await?;
        self.mark_stage_running(run_id, 0).await?;
        self.get(project_id, run_id).await
    }

    pub async fn select_assets(
        &self,
        project_id: &str,
        run_id: &str,
        asset_ids: Vec<String>,
    ) -> Result<ProductionRunView, ProductionOrchestratorError> {
        validate_project_id(project_id)?;
        let run = self.load_run(project_id, run_id).await?;
        if run.status == ProductionRunStatus::Cancelled.as_str()
            || run.status == ProductionRunStatus::Succeeded.as_str()
        {
            return Err(ProductionOrchestratorError::InvalidState(
                "已结束的 Production Run 不能修改选图。".to_owned(),
            ));
        }
        let stage0 = self.load_stage(run_id, 0).await?;
        let selection = self.load_stage(run_id, 1).await?;
        if stage0.status != ProductionStageStatus::Succeeded.as_str()
            && stage0.status != ProductionStageStatus::Waiting.as_str()
        {
            return Err(ProductionOrchestratorError::InvalidState(
                "Krea2 尚未完成，当前没有可选图片。".to_owned(),
            ));
        }
        let mut unique_assets = asset_ids
            .into_iter()
            .map(|id| id.trim().to_owned())
            .filter(|id| !id.is_empty())
            .collect::<Vec<_>>();
        let mut seen_assets = std::collections::HashSet::new();
        unique_assets.retain(|asset_id| seen_assets.insert(asset_id.clone()));
        if unique_assets.is_empty() || unique_assets.len() > 8 {
            return Err(ProductionOrchestratorError::InvalidInput(
                "至少选择 1 个、最多选择 8 个图片资产。".to_owned(),
            ));
        }
        let mut valid_query = QueryBuilder::<Sqlite>::new(
            "SELECT DISTINCT si.asset_id FROM production_stage_items si
             WHERE si.stage_id = ",
        );
        valid_query
            .push_bind(&stage0.id)
            .push(" AND si.asset_id IN (");
        {
            let mut separated = valid_query.separated(", ");
            for asset_id in &unique_assets {
                separated.push_bind(asset_id);
            }
        }
        valid_query.push(")");
        let valid_assets = valid_query
            .build_query_as::<(Option<String>,)>()
            .fetch_all(&self.pool)
            .await
            .map_err(db_error)?
            .into_iter()
            .filter_map(|(asset_id,)| asset_id)
            .collect::<Vec<_>>();
        if valid_assets.len() != unique_assets.len() {
            return Err(ProductionOrchestratorError::InvalidInput(
                "只能选择本次 Krea2 Stage 实际生成的图片资产。".to_owned(),
            ));
        }
        let now = self.clock.now().to_rfc3339();
        let mut transaction = self.pool.begin().await.map_err(db_error)?;
        sqlx::query("DELETE FROM production_stage_items WHERE stage_id = ?")
            .bind(&selection.id)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
        for (ordinal, asset_id) in unique_assets.iter().enumerate() {
            insert_stage_item(
                &mut transaction,
                &selection.id,
                u32::try_from(ordinal).unwrap_or_default(),
                "SUCCEEDED",
                None,
                None,
                Some(asset_id),
                Some(asset_id),
                1,
                None,
                &json!({"assetId": asset_id, "referenceIndex": ordinal}),
                None,
                None,
                &now,
            )
            .await?;
        }
        sqlx::query(
            "UPDATE production_stages SET status = 'SUCCEEDED', updated_at = ?
             WHERE id = ?",
        )
        .bind(&now)
        .bind(&selection.id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
        sqlx::query(
            "UPDATE production_stages SET status = 'READY', updated_at = ?
             WHERE run_id = ? AND ordinal = 2 AND production_batch_id IS NULL",
        )
        .bind(&now)
        .bind(run_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
        sqlx::query(
            "UPDATE production_runs SET status = 'READY', current_stage_ordinal = 2, updated_at = ?
             WHERE id = ?",
        )
        .bind(&now)
        .bind(run_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
        transaction.commit().await.map_err(db_error)?;
        self.get(project_id, run_id).await
    }

    pub async fn run_video(
        &self,
        project_id: &str,
        run_id: &str,
    ) -> Result<ProductionRunView, ProductionOrchestratorError> {
        self.enqueue_h3(project_id, run_id, false).await
    }

    pub async fn retry_video(
        &self,
        project_id: &str,
        run_id: &str,
    ) -> Result<ProductionRunView, ProductionOrchestratorError> {
        self.enqueue_h3(project_id, run_id, true).await
    }

    async fn enqueue_h3(
        &self,
        project_id: &str,
        run_id: &str,
        retry: bool,
    ) -> Result<ProductionRunView, ProductionOrchestratorError> {
        let _trigger_gate = self.stage_trigger_gate.lock().await;
        validate_project_id(project_id)?;
        self.sync_run_state(run_id).await?;
        let run = self.load_run(project_id, run_id).await?;
        let stage = self.load_stage(run_id, 2).await?;
        if let Some(batch_id) = stage.production_batch_id.as_deref() {
            if !retry && stage.status == ProductionStageStatus::Ready.as_str() {
                self.production_queue_service
                    .start(project_id, batch_id)
                    .await?;
                self.mark_stage_running(run_id, 2).await?;
                return self.get(project_id, run_id).await;
            }
            if retry && stage.status == ProductionStageStatus::Failed.as_str() {
                // Keep the failed batch and its Task lineage.  A retry below
                // creates a new normal ProductionBatch and a new attempt.
            } else {
                return Err(ProductionOrchestratorError::InvalidState(
                    "H3 Stage 已经创建批次，重复触发不会创建第二个任务。".to_owned(),
                ));
            }
        }
        if retry {
            if !matches!(run.status.as_str(), "FAILED" | "PARTIAL_FAILED")
                && stage.status != ProductionStageStatus::Failed.as_str()
            {
                return Err(ProductionOrchestratorError::InvalidState(
                    "只有失败的 H3 Stage 才能重试。".to_owned(),
                ));
            }
        } else if stage.status != ProductionStageStatus::Ready.as_str()
            || run.status == ProductionRunStatus::Cancelled.as_str()
        {
            return Err(ProductionOrchestratorError::InvalidState(
                "请先完成 Krea2 并选择图片，再启动 H3。".to_owned(),
            ));
        }
        let selection = self.load_stage(run_id, 1).await?;
        let references = self.load_selected_assets(&selection.id).await?;
        if references.is_empty() {
            return Err(ProductionOrchestratorError::InvalidState(
                "H3 Stage 没有选中的图片资产。".to_owned(),
            ));
        }
        let workflow_version_id = stage.workflow_version_id.clone().ok_or_else(|| {
            ProductionOrchestratorError::InvalidState("H3 Workflow 未冻结。".to_owned())
        })?;
        let recipe_id = stage.recipe_id.clone().ok_or_else(|| {
            ProductionOrchestratorError::InvalidState("H3 Recipe 未冻结。".to_owned())
        })?;
        let mut values = values_from_config(&stage.frozen_config_json)?;
        let definition = self
            .definition_repository
            .find(&workflow_version_id, &recipe_id)
            .await
            .map_err(|error| ProductionOrchestratorError::Repository(error.to_string()))?
            .ok_or_else(|| {
                ProductionOrchestratorError::InvalidInput(
                    "H3 Workflow / Recipe 不可用。".to_owned(),
                )
            })?;
        let recipe = RecipeParser::parse(&definition.recipe_yaml)
            .map_err(|error| ProductionOrchestratorError::InvalidInput(error.to_string()))?;
        inject_reference_values(&mut values, &references, &recipe.inputs);
        let queue = self
            .production_queue_service
            .create(CreateProductionBatchRequest {
                project_id: project_id.to_owned(),
                name: format!("Production Run · {} · H3", run.name),
                continue_on_failure: true,
                items: vec![CreateProductionBatchItem {
                    workflow_version_id,
                    recipe_id,
                    values,
                }],
            })
            .await?;
        let now = self.clock.now().to_rfc3339();
        let next_ordinal = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT MAX(ordinal) FROM production_stage_items WHERE stage_id = ?",
        )
        .bind(&stage.id)
        .fetch_one(&self.pool)
        .await
        .map_err(db_error)?
        .unwrap_or(-1)
        .saturating_add(1);
        let previous_parent = if retry {
            sqlx::query_scalar::<_, Option<String>>(
                "SELECT id FROM production_stage_items WHERE stage_id = ? ORDER BY ordinal ASC LIMIT 1",
            )
            .bind(&stage.id)
            .fetch_optional(&self.pool)
            .await
            .map_err(db_error)?
            .flatten()
        } else {
            None
        };
        let attempt = if retry {
            sqlx::query_scalar::<_, Option<i64>>(
                "SELECT MAX(attempt) FROM production_stage_items WHERE stage_id = ?",
            )
            .bind(&stage.id)
            .fetch_one(&self.pool)
            .await
            .map_err(db_error)?
            .unwrap_or(0)
            .saturating_add(1)
        } else {
            1
        };
        let mut transaction = self.pool.begin().await.map_err(db_error)?;
        sqlx::query(
            "UPDATE production_stages SET production_batch_id = ?, status = 'READY', updated_at = ?
             WHERE id = ?",
        )
        .bind(queue.batch.id.as_str())
        .bind(&now)
        .bind(&stage.id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
        for (reference_offset, reference) in references.iter().enumerate() {
            let item_id = format!("prsi_{}", Uuid::new_v4().simple());
            let key = format!("production-stage-item:{item_id}:attempt:{attempt}");
            sqlx::query(
                "INSERT INTO production_stage_items
                 (id, stage_id, ordinal, status, production_batch_item_id, task_id, asset_id,
                  source_asset_id, reference_index, attempt, submission_idempotency_key,
                  parent_stage_item_id, frozen_values_json, created_at, updated_at)
                 VALUES (?, ?, ?, 'PENDING', ?, NULL, NULL, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&item_id)
            .bind(&stage.id)
            .bind(next_ordinal + i64::try_from(reference_offset).unwrap_or_default())
            .bind(queue.items[0].id.as_str())
            .bind(&reference.asset_id)
            .bind(i64::try_from(reference.reference_index).unwrap_or_default())
            .bind(attempt)
            .bind(&key)
            .bind(&previous_parent)
            .bind(queue.items[0].values_json.to_string())
            .bind(&now)
            .bind(&now)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
        }
        sqlx::query(
            "UPDATE production_runs SET status = 'RUNNING', current_stage_ordinal = 2,
                    started_at = COALESCE(started_at, ?), updated_at = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(&now)
        .bind(run_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
        transaction.commit().await.map_err(db_error)?;
        if let Err(error) = self
            .production_queue_service
            .start(project_id, queue.batch.id.as_str())
            .await
        {
            sqlx::query(
                "UPDATE production_runs SET status = 'WAITING_FOR_SELECTION', updated_at = ? WHERE id = ?",
            )
            .bind(self.clock.now().to_rfc3339())
            .bind(run_id)
            .execute(&self.pool)
            .await
            .map_err(db_error)?;
            return Err(error.into());
        }
        self.mark_stage_running(run_id, 2).await?;
        self.get(project_id, run_id).await
    }

    pub async fn cancel(
        &self,
        project_id: &str,
        run_id: &str,
    ) -> Result<ProductionRunView, ProductionOrchestratorError> {
        validate_project_id(project_id)?;
        let run = self.load_run(project_id, run_id).await?;
        if matches!(run.status.as_str(), "SUCCEEDED" | "CANCELLED") {
            return Err(ProductionOrchestratorError::InvalidState(
                "已结束的 Production Run 不能取消。".to_owned(),
            ));
        }
        let task_ids = sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT COALESCE(i.task_id, si.task_id)
             FROM production_stage_items si
             LEFT JOIN production_batch_items i ON i.id = si.production_batch_item_id
             INNER JOIN production_stages s ON s.id = si.stage_id
             WHERE s.run_id = ? AND COALESCE(i.task_id, si.task_id) IS NOT NULL",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .map_err(db_error)?;
        for task_id in task_ids {
            let _ = self
                .task_cancellation_service
                .request_cancel(project_id, &task_id)
                .await;
        }
        let batches = sqlx::query_as::<_, (String, String)>(
            "SELECT DISTINCT s.production_batch_id, b.status
             FROM production_stages s INNER JOIN production_batches b ON b.id = s.production_batch_id
             WHERE s.run_id = ? AND s.production_batch_id IS NOT NULL",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .map_err(db_error)?;
        for (batch_id, status) in batches {
            if status == "READY" {
                let _ = self
                    .production_queue_service
                    .cancel_pending(project_id, &batch_id)
                    .await;
            }
        }
        let now = self.clock.now().to_rfc3339();
        let mut transaction = self.pool.begin().await.map_err(db_error)?;
        sqlx::query(
            "UPDATE production_stages SET status = CASE WHEN status IN ('SUCCEEDED', 'SKIPPED') THEN status ELSE 'CANCELLED' END,
                    updated_at = ? WHERE run_id = ?",
        )
        .bind(&now)
        .bind(run_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
        sqlx::query(
            "UPDATE production_stage_items SET status = CASE WHEN status IN ('SUCCEEDED', 'SKIPPED') THEN status ELSE 'CANCELLED' END,
                    updated_at = ? WHERE stage_id IN (SELECT id FROM production_stages WHERE run_id = ?)
                    AND status NOT IN ('SUCCEEDED', 'SKIPPED')",
        )
        .bind(&now)
        .bind(run_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
        sqlx::query(
            "UPDATE production_runs SET status = 'CANCELLED', finished_at = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(&now)
        .bind(run_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
        transaction.commit().await.map_err(db_error)?;
        self.get(project_id, run_id).await
    }

    pub async fn refresh(
        &self,
        project_id: &str,
        run_id: &str,
    ) -> Result<ProductionRunView, ProductionOrchestratorError> {
        self.get(project_id, run_id).await
    }

    pub async fn save_template(
        &self,
        request: ProductionRunTemplateRequest,
    ) -> Result<ProductionRunTemplateView, ProductionOrchestratorError> {
        validate_project_id(&request.project_id)?;
        let name = validate_name(&request.name)?;
        if request.default_image_count == 0 || request.default_image_count > MAX_RUN_IMAGE_COUNT {
            return Err(ProductionOrchestratorError::InvalidInput(
                "模板默认图片数量必须在 1–100 之间。".to_owned(),
            ));
        }
        if let Some(duration) = request.default_duration_seconds {
            if !(1..=15).contains(&duration) {
                return Err(ProductionOrchestratorError::InvalidInput(
                    "模板默认视频时长必须在 1–15 秒之间。".to_owned(),
                ));
            }
        }
        let id = format!("prt_{}", Uuid::new_v4().simple());
        let now = self.clock.now().to_rfc3339();
        sqlx::query(
            "INSERT INTO production_run_templates
             (id, project_id, name, krea2_workflow_version_id, krea2_recipe_id, krea2_preset_id,
              default_image_count, h3_workflow_version_id, h3_recipe_id, h3_profile,
              default_duration_seconds, default_width, default_height, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&request.project_id)
        .bind(name)
        .bind(&request.krea2_workflow_version_id)
        .bind(&request.krea2_recipe_id)
        .bind(&request.krea2_preset_id)
        .bind(i64::from(request.default_image_count))
        .bind(&request.h3_workflow_version_id)
        .bind(&request.h3_recipe_id)
        .bind(&request.h3_profile)
        .bind(request.default_duration_seconds.map(i64::from))
        .bind(request.default_width.map(i64::from))
        .bind(request.default_height.map(i64::from))
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(db_error)?;
        self.get_template(&request.project_id, &id).await
    }

    pub async fn list_templates(
        &self,
        project_id: &str,
    ) -> Result<Vec<ProductionRunTemplateView>, ProductionOrchestratorError> {
        validate_project_id(project_id)?;
        sqlx::query_as::<_, ProductionRunTemplateRow>(
            "SELECT id, project_id, name, krea2_workflow_version_id, krea2_recipe_id,
                    krea2_preset_id, default_image_count, h3_workflow_version_id, h3_recipe_id,
                    h3_profile, default_duration_seconds, default_width, default_height,
                    created_at, updated_at
             FROM production_run_templates WHERE project_id = ?
             ORDER BY updated_at DESC, id ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(db_error)
        .map(|rows| {
            rows.into_iter()
                .map(ProductionRunTemplateView::from)
                .collect()
        })
    }

    async fn get_template(
        &self,
        project_id: &str,
        template_id: &str,
    ) -> Result<ProductionRunTemplateView, ProductionOrchestratorError> {
        sqlx::query_as::<_, ProductionRunTemplateRow>(
            "SELECT id, project_id, name, krea2_workflow_version_id, krea2_recipe_id,
                    krea2_preset_id, default_image_count, h3_workflow_version_id, h3_recipe_id,
                    h3_profile, default_duration_seconds, default_width, default_height,
                    created_at, updated_at
             FROM production_run_templates WHERE project_id = ? AND id = ?",
        )
        .bind(project_id)
        .bind(template_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_error)?
        .map(ProductionRunTemplateView::from)
        .ok_or_else(|| ProductionOrchestratorError::NotFound(template_id.to_owned()))
    }

    async fn ensure_definition(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
    ) -> Result<(), ProductionOrchestratorError> {
        if self
            .definition_repository
            .find(workflow_version_id, recipe_id)
            .await
            .map_err(|error| ProductionOrchestratorError::Repository(error.to_string()))?
            .is_none()
        {
            return Err(ProductionOrchestratorError::InvalidInput(format!(
                "Workflow / Recipe 不可用：{workflow_version_id} / {recipe_id}"
            )));
        }
        Ok(())
    }

    async fn load_stage(
        &self,
        run_id: &str,
        ordinal: u32,
    ) -> Result<ProductionStageRow, ProductionOrchestratorError> {
        sqlx::query_as::<_, ProductionStageRow>(
            "SELECT id, run_id, ordinal, stage_type, status, workflow_version_id, recipe_id,
                    production_batch_id, frozen_config_json, prompt
             FROM production_stages WHERE run_id = ? AND ordinal = ?",
        )
        .bind(run_id)
        .bind(i64::from(ordinal))
        .fetch_optional(&self.pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| ProductionOrchestratorError::NotFound(format!("{run_id}:{ordinal}")))
    }

    async fn load_selected_assets(
        &self,
        selection_stage_id: &str,
    ) -> Result<Vec<SelectedReference>, ProductionOrchestratorError> {
        sqlx::query_as::<_, SelectedReferenceRow>(
            "SELECT asset_id, COALESCE(reference_index, ordinal) AS reference_index
             FROM production_stage_items WHERE stage_id = ? AND status = 'SUCCEEDED'
             AND asset_id IS NOT NULL ORDER BY COALESCE(reference_index, ordinal), ordinal",
        )
        .bind(selection_stage_id)
        .fetch_all(&self.pool)
        .await
        .map_err(db_error)
        .map(|rows| {
            rows.into_iter()
                .filter_map(|row| {
                    row.asset_id.map(|asset_id| SelectedReference {
                        asset_id,
                        reference_index: u32::try_from(row.reference_index).unwrap_or_default(),
                    })
                })
                .collect()
        })
    }

    async fn mark_stage_running(
        &self,
        run_id: &str,
        ordinal: u32,
    ) -> Result<(), ProductionOrchestratorError> {
        let now = self.clock.now().to_rfc3339();
        sqlx::query(
            "UPDATE production_stages SET status = 'RUNNING', started_at = COALESCE(started_at, ?), updated_at = ?
             WHERE run_id = ? AND ordinal = ?",
        )
        .bind(&now)
        .bind(&now)
        .bind(run_id)
        .bind(i64::from(ordinal))
        .execute(&self.pool)
        .await
        .map_err(db_error)?;
        sqlx::query(
            "UPDATE production_runs SET status = 'RUNNING', current_stage_ordinal = ?, started_at = COALESCE(started_at, ?), updated_at = ?
             WHERE id = ?",
        )
        .bind(i64::from(ordinal))
        .bind(&now)
        .bind(&now)
        .bind(run_id)
        .execute(&self.pool)
        .await
        .map_err(db_error)?;
        Ok(())
    }

    async fn sync_run_state(&self, run_id: &str) -> Result<(), ProductionOrchestratorError> {
        sqlx::query(
            "UPDATE production_stage_items
             SET task_id = COALESCE((SELECT i.task_id FROM production_batch_items i WHERE i.id = production_stage_items.production_batch_item_id), task_id),
                 asset_id = COALESCE(asset_id, (SELECT MIN(oa.asset_id) FROM task_output_assets oa
                     WHERE oa.task_id = COALESCE(production_stage_items.task_id,
                         (SELECT i.task_id FROM production_batch_items i WHERE i.id = production_stage_items.production_batch_item_id)))),
                 status = CASE
                    WHEN (SELECT i.status FROM production_batch_items i WHERE i.id = production_stage_items.production_batch_item_id) IN ('DISPATCHING', 'DISPATCHED') THEN 'RUNNING'
                    WHEN (SELECT i.status FROM production_batch_items i WHERE i.id = production_stage_items.production_batch_item_id) IS NOT NULL THEN (SELECT i.status FROM production_batch_items i WHERE i.id = production_stage_items.production_batch_item_id)
                    ELSE status
                 END,
                 updated_at = ?
             WHERE stage_id IN (SELECT id FROM production_stages WHERE run_id = ?)
               AND production_batch_item_id IS NOT NULL",
        )
        .bind(self.clock.now().to_rfc3339())
        .bind(run_id)
        .execute(&self.pool)
        .await
        .map_err(db_error)?;

        let run = self.load_run_by_id(run_id).await?;
        if run.status == ProductionRunStatus::Cancelled.as_str()
            || run.status == ProductionRunStatus::Succeeded.as_str()
        {
            return Ok(());
        }
        let stage0 = self.load_stage(run_id, 0).await?;
        let selection = self.load_stage(run_id, 1).await?;
        let h3 = self.load_stage(run_id, 2).await?;
        if let Some(batch_id) = stage0.production_batch_id.as_deref() {
            let batch_status = sqlx::query_scalar::<_, String>(
                "SELECT status FROM production_batches WHERE id = ?",
            )
            .bind(batch_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(db_error)?;
            let stats = stage_stats(&self.pool, &stage0.id).await?;
            if batch_status.as_deref() == Some("READY") && stats.active == 0 && stats.terminal == 0
            {
                self.set_stage_status(run_id, 0, ProductionStageStatus::Ready)
                    .await?;
                return Ok(());
            }
            if stats.pending > 0 || stats.active > 0 {
                self.set_stage_status(run_id, 0, ProductionStageStatus::Running)
                    .await?;
                if selection.status != ProductionStageStatus::Succeeded.as_str() {
                    self.set_run_status(run_id, ProductionRunStatus::Running, 0, None)
                        .await?;
                }
                return Ok(());
            }
            if stats.succeeded > 0 {
                self.set_stage_status(run_id, 0, ProductionStageStatus::Succeeded)
                    .await?;
                if selection.status != ProductionStageStatus::Succeeded.as_str() {
                    self.set_stage_status(run_id, 1, ProductionStageStatus::Waiting)
                        .await?;
                    self.set_run_status(run_id, ProductionRunStatus::WaitingForSelection, 1, None)
                        .await?;
                    return Ok(());
                }
            } else if stats.terminal > 0 {
                self.set_stage_status(run_id, 0, ProductionStageStatus::Failed)
                    .await?;
                self.set_stage_status(run_id, 1, ProductionStageStatus::Skipped)
                    .await?;
                self.set_stage_status(run_id, 2, ProductionStageStatus::Skipped)
                    .await?;
                self.set_run_status(
                    run_id,
                    ProductionRunStatus::Failed,
                    0,
                    Some(self.clock.now().to_rfc3339()),
                )
                .await?;
                return Ok(());
            }
        }
        if let Some(_batch_id) = h3.production_batch_id.as_deref() {
            let stats = stage_stats(&self.pool, &h3.id).await?;
            if stats.pending > 0 || stats.active > 0 {
                self.set_stage_status(run_id, 2, ProductionStageStatus::Running)
                    .await?;
                self.set_run_status(run_id, ProductionRunStatus::Running, 2, None)
                    .await?;
            } else if stats.succeeded == stats.total && stats.total > 0 {
                self.set_stage_status(run_id, 2, ProductionStageStatus::Succeeded)
                    .await?;
                self.set_run_status(
                    run_id,
                    ProductionRunStatus::Succeeded,
                    2,
                    Some(self.clock.now().to_rfc3339()),
                )
                .await?;
            } else if stats.terminal > 0 {
                self.set_stage_status(run_id, 2, ProductionStageStatus::Failed)
                    .await?;
                let status = if stats.succeeded > 0 {
                    ProductionRunStatus::PartialFailed
                } else {
                    ProductionRunStatus::Failed
                };
                self.set_run_status(run_id, status, 2, Some(self.clock.now().to_rfc3339()))
                    .await?;
            }
        }
        Ok(())
    }

    async fn load_run_by_id(
        &self,
        run_id: &str,
    ) -> Result<ProductionRunRow, ProductionOrchestratorError> {
        sqlx::query_as::<_, ProductionRunRow>(
            "SELECT id, project_id, name, status, current_stage_ordinal, template_id,
                    created_at, updated_at, started_at, finished_at
             FROM production_runs WHERE id = ?",
        )
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| ProductionOrchestratorError::NotFound(run_id.to_owned()))
    }

    async fn set_stage_status(
        &self,
        run_id: &str,
        ordinal: u32,
        status: ProductionStageStatus,
    ) -> Result<(), ProductionOrchestratorError> {
        sqlx::query(
            "UPDATE production_stages SET status = ?, finished_at = CASE WHEN ? IN ('SUCCEEDED', 'FAILED', 'SKIPPED', 'CANCELLED') THEN COALESCE(finished_at, ?) ELSE finished_at END, updated_at = ?
             WHERE run_id = ? AND ordinal = ?",
        )
        .bind(status.as_str())
        .bind(status.as_str())
        .bind(self.clock.now().to_rfc3339())
        .bind(self.clock.now().to_rfc3339())
        .bind(run_id)
        .bind(i64::from(ordinal))
        .execute(&self.pool)
        .await
        .map_err(db_error)?;
        Ok(())
    }

    async fn set_run_status(
        &self,
        run_id: &str,
        status: ProductionRunStatus,
        ordinal: u32,
        finished_at: Option<String>,
    ) -> Result<(), ProductionOrchestratorError> {
        sqlx::query(
            "UPDATE production_runs SET status = ?, current_stage_ordinal = ?, finished_at = COALESCE(?, finished_at), updated_at = ? WHERE id = ?",
        )
        .bind(status.as_str())
        .bind(i64::from(ordinal))
        .bind(finished_at)
        .bind(self.clock.now().to_rfc3339())
        .bind(run_id)
        .execute(&self.pool)
        .await
        .map_err(db_error)?;
        Ok(())
    }
}

#[derive(Debug, FromRow)]
struct ProductionRunRow {
    id: String,
    project_id: String,
    name: String,
    status: String,
    current_stage_ordinal: i64,
    template_id: Option<String>,
    created_at: String,
    updated_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
}

impl From<ProductionRunRow> for ProductionRunListItem {
    fn from(row: ProductionRunRow) -> Self {
        Self {
            id: row.id,
            project_id: row.project_id,
            name: row.name,
            status: row.status,
            current_stage_ordinal: u32::try_from(row.current_stage_ordinal).unwrap_or_default(),
            template_id: row.template_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, FromRow)]
struct ProductionStageRow {
    id: String,
    run_id: String,
    ordinal: i64,
    stage_type: String,
    status: String,
    workflow_version_id: Option<String>,
    recipe_id: Option<String>,
    production_batch_id: Option<String>,
    frozen_config_json: String,
    prompt: Option<String>,
}

impl ProductionStageRow {
    fn into_view(
        self,
        items: Vec<ProductionRunStageItemView>,
    ) -> Result<ProductionRunStageView, ProductionOrchestratorError> {
        Ok(ProductionRunStageView {
            id: self.id,
            ordinal: u32::try_from(self.ordinal).unwrap_or_default(),
            stage_type: self.stage_type,
            status: self.status,
            workflow_version_id: self.workflow_version_id,
            recipe_id: self.recipe_id,
            production_batch_id: self.production_batch_id,
            frozen_config: parse_json(&self.frozen_config_json)?,
            prompt: self.prompt,
            items,
        })
    }
}

#[derive(Debug, FromRow)]
struct ProductionStageItemRow {
    id: String,
    stage_id: String,
    ordinal: i64,
    status: String,
    production_batch_item_id: Option<String>,
    task_id: Option<String>,
    task_status: Option<String>,
    asset_id: Option<String>,
    source_asset_id: Option<String>,
    reference_index: Option<i64>,
    attempt: i64,
    submission_idempotency_key: Option<String>,
    parent_stage_item_id: Option<String>,
    frozen_values_json: String,
    error_code: Option<String>,
    error_message: Option<String>,
}

impl ProductionStageItemRow {
    fn into_view(self) -> Result<ProductionRunStageItemView, ProductionOrchestratorError> {
        Ok(ProductionRunStageItemView {
            id: self.id,
            stage_id: self.stage_id,
            ordinal: u32::try_from(self.ordinal).unwrap_or_default(),
            status: self.status,
            production_batch_item_id: self.production_batch_item_id,
            task_id: self.task_id,
            task_status: self.task_status,
            asset_id: self.asset_id,
            source_asset_id: self.source_asset_id,
            reference_index: self
                .reference_index
                .and_then(|value| u32::try_from(value).ok()),
            attempt: u32::try_from(self.attempt).unwrap_or(1),
            submission_idempotency_key: self.submission_idempotency_key,
            parent_stage_item_id: self.parent_stage_item_id,
            frozen_values: parse_json(&self.frozen_values_json)?,
            error_code: self.error_code,
            error_message: self.error_message,
        })
    }
}

#[derive(Debug, FromRow)]
struct SelectedReferenceRow {
    asset_id: Option<String>,
    reference_index: i64,
}

#[derive(Clone, Debug)]
struct SelectedReference {
    asset_id: String,
    reference_index: u32,
}

#[derive(Debug, FromRow)]
struct ProductionRunTemplateRow {
    id: String,
    project_id: String,
    name: String,
    krea2_workflow_version_id: Option<String>,
    krea2_recipe_id: Option<String>,
    krea2_preset_id: Option<String>,
    default_image_count: i64,
    h3_workflow_version_id: Option<String>,
    h3_recipe_id: Option<String>,
    h3_profile: Option<String>,
    default_duration_seconds: Option<i64>,
    default_width: Option<i64>,
    default_height: Option<i64>,
    created_at: String,
    updated_at: String,
}

impl From<ProductionRunTemplateRow> for ProductionRunTemplateView {
    fn from(row: ProductionRunTemplateRow) -> Self {
        Self {
            id: row.id,
            project_id: row.project_id,
            name: row.name,
            krea2_workflow_version_id: row.krea2_workflow_version_id,
            krea2_recipe_id: row.krea2_recipe_id,
            krea2_preset_id: row.krea2_preset_id,
            default_image_count: u32::try_from(row.default_image_count).unwrap_or(1),
            h3_workflow_version_id: row.h3_workflow_version_id,
            h3_recipe_id: row.h3_recipe_id,
            h3_profile: row.h3_profile,
            default_duration_seconds: row
                .default_duration_seconds
                .and_then(|value| u32::try_from(value).ok()),
            default_width: row
                .default_width
                .and_then(|value| u32::try_from(value).ok()),
            default_height: row
                .default_height
                .and_then(|value| u32::try_from(value).ok()),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Default)]
struct StageStats {
    total: i64,
    succeeded: i64,
    terminal: i64,
    pending: i64,
    active: i64,
}

async fn stage_stats(
    pool: &SqlitePool,
    stage_id: &str,
) -> Result<StageStats, ProductionOrchestratorError> {
    sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
        "SELECT COUNT(*),
                COALESCE(SUM(CASE WHEN status = 'SUCCEEDED' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status IN ('SUCCEEDED', 'FAILED', 'CANCELLED', 'SKIPPED') THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status IN ('PENDING', 'READY') THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status IN ('RUNNING', 'DISPATCHING', 'DISPATCHED') THEN 1 ELSE 0 END), 0)
         FROM production_stage_items WHERE stage_id = ?",
    )
    .bind(stage_id)
    .fetch_one(pool)
    .await
    .map(|(total, succeeded, terminal, pending, active)| StageStats {
        total,
        succeeded,
        terminal,
        pending,
        active,
    })
    .map_err(db_error)
}

async fn insert_stage(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    run_id: &str,
    ordinal: u32,
    stage_type: ProductionStageType,
    status: ProductionStageStatus,
    workflow_version_id: Option<&str>,
    recipe_id: Option<&str>,
    frozen_config: &Value,
    prompt: Option<&str>,
    now: &str,
) -> Result<(), ProductionOrchestratorError> {
    let id = format!("prst_{}", Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO production_stages
         (id, run_id, ordinal, stage_type, status, workflow_version_id, recipe_id,
          frozen_config_json, prompt, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(run_id)
    .bind(i64::from(ordinal))
    .bind(stage_type.as_str())
    .bind(status.as_str())
    .bind(workflow_version_id)
    .bind(recipe_id)
    .bind(frozen_config.to_string())
    .bind(prompt)
    .bind(now)
    .bind(now)
    .execute(&mut **transaction)
    .await
    .map_err(db_error)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_stage_item(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    stage_id: &str,
    ordinal: u32,
    status: &str,
    production_batch_item_id: Option<&str>,
    task_id: Option<&str>,
    asset_id: Option<&str>,
    source_asset_id: Option<&str>,
    attempt: u32,
    parent_stage_item_id: Option<&str>,
    frozen_values: &Value,
    error_code: Option<&str>,
    error_message: Option<&str>,
    now: &str,
) -> Result<(), ProductionOrchestratorError> {
    let id = format!("prsi_{}", Uuid::new_v4().simple());
    let key = format!("production-stage-item:{id}:attempt:{attempt}");
    sqlx::query(
        "INSERT INTO production_stage_items
         (id, stage_id, ordinal, status, production_batch_item_id, task_id, asset_id,
          source_asset_id, attempt, submission_idempotency_key, parent_stage_item_id,
          frozen_values_json, error_code, error_message, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(stage_id)
    .bind(i64::from(ordinal))
    .bind(status)
    .bind(production_batch_item_id)
    .bind(task_id)
    .bind(asset_id)
    .bind(source_asset_id)
    .bind(i64::from(attempt))
    .bind(Some(key))
    .bind(parent_stage_item_id)
    .bind(frozen_values.to_string())
    .bind(error_code)
    .bind(error_message)
    .bind(now)
    .bind(now)
    .execute(&mut **transaction)
    .await
    .map_err(db_error)?;
    Ok(())
}

fn values_from_config(
    config_json: &str,
) -> Result<BTreeMap<String, GenerationInputValue>, ProductionOrchestratorError> {
    let config = parse_json(config_json)?;
    let values = config.get("values").cloned().unwrap_or_else(|| json!({}));
    generation_values_from_json(&values).map_err(ProductionOrchestratorError::InvalidInput)
}

fn inject_reference_values(
    values: &mut BTreeMap<String, GenerationInputValue>,
    references: &[SelectedReference],
    inputs: &BTreeMap<String, InputDefinition>,
) {
    let asset_ids = references
        .iter()
        .map(|reference| crate::domain::AssetId::parse(reference.asset_id.clone()).ok())
        .flatten()
        .collect::<Vec<_>>();
    let normalized_key = |key: &str| key.to_ascii_lowercase().replace('-', "_");
    let mut image_inputs = Vec::new();
    let mut reference_images = None;
    let mut first_frame = None;
    let mut last_frame = None;
    for (key, definition) in inputs {
        if !matches!(
            definition,
            InputDefinition::Image { .. } | InputDefinition::Images { .. }
        ) {
            continue;
        }
        image_inputs.push((key, definition));
        match normalized_key(key).as_str() {
            "reference_images" | "references" => reference_images = Some((key, definition)),
            "first_frame" => first_frame = Some(key),
            "last_frame" => last_frame = Some(key),
            _ => {}
        }
    }
    if let Some((key, definition)) = reference_images {
        if matches!(definition, InputDefinition::Images { .. }) {
            values.insert(
                key.clone(),
                GenerationInputValue::ImageAssets(asset_ids.clone()),
            );
            return;
        }
    }
    if let (Some(first_key), Some(first)) = (first_frame, asset_ids.first()) {
        values.insert(
            first_key.clone(),
            GenerationInputValue::ImageAsset(first.clone()),
        );
        if let (Some(last_key), Some(last)) = (last_frame, asset_ids.get(1)) {
            values.insert(
                last_key.clone(),
                GenerationInputValue::ImageAsset(last.clone()),
            );
        }
        return;
    }
    if let Some((key, definition)) = image_inputs.first() {
        if matches!(definition, InputDefinition::Images { .. }) {
            values.insert((*key).clone(), GenerationInputValue::ImageAssets(asset_ids));
        } else if let Some(first) = asset_ids.first() {
            values.insert(
                (*key).clone(),
                GenerationInputValue::ImageAsset(first.clone()),
            );
        }
    }
}

fn frozen_string(config: &Value, key: &str) -> Option<String> {
    config
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn parse_json(value: &str) -> Result<Value, ProductionOrchestratorError> {
    serde_json::from_str(value)
        .map_err(|error| ProductionOrchestratorError::Repository(error.to_string()))
}

fn validate_project_id(project_id: &str) -> Result<(), ProductionOrchestratorError> {
    crate::domain::validate_project_id(project_id)
        .map_err(|error| ProductionOrchestratorError::InvalidInput(error.to_string()))
}

fn validate_name(name: &str) -> Result<&str, ProductionOrchestratorError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 120 {
        return Err(ProductionOrchestratorError::InvalidInput(
            "名称必须为 1–120 个字符。".to_owned(),
        ));
    }
    Ok(name)
}

fn db_error(error: sqlx::Error) -> ProductionOrchestratorError {
    ProductionOrchestratorError::Repository(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{inject_reference_values, SelectedReference};
    use crate::application::generation_input_preparer::GenerationInputValue;
    use crate::domain::{AssetId, InputDefinition};
    use std::collections::{BTreeMap, HashMap};

    fn asset(id: &str) -> AssetId {
        AssetId::parse(id.to_owned()).expect("test asset id should be valid")
    }

    #[test]
    fn ref2va_keeps_selected_reference_order() {
        let mut values = BTreeMap::new();
        let inputs = BTreeMap::from([(
            "reference_images".to_owned(),
            InputDefinition::Images {
                label: "References".to_owned(),
                required: false,
                min_items: 0,
                max_items: 9,
            },
        )]);
        let references = vec![
            SelectedReference {
                asset_id: "ast_second".to_owned(),
                reference_index: 0,
            },
            SelectedReference {
                asset_id: "ast_first".to_owned(),
                reference_index: 1,
            },
        ];

        inject_reference_values(&mut values, &references, &inputs);

        assert_eq!(
            values.get("reference_images"),
            Some(&GenerationInputValue::ImageAssets(vec![
                asset("ast_second"),
                asset("ast_first"),
            ]))
        );
    }

    #[test]
    fn fl2va_maps_first_and_last_frame_without_reordering() {
        let mut values = BTreeMap::new();
        let inputs = BTreeMap::from([
            (
                "first_frame".to_owned(),
                InputDefinition::Image {
                    label: "First".to_owned(),
                    required: false,
                },
            ),
            (
                "last_frame".to_owned(),
                InputDefinition::Image {
                    label: "Last".to_owned(),
                    required: false,
                },
            ),
        ]);
        let references = vec![
            SelectedReference {
                asset_id: "ast_first".to_owned(),
                reference_index: 0,
            },
            SelectedReference {
                asset_id: "ast_last".to_owned(),
                reference_index: 1,
            },
        ];

        inject_reference_values(&mut values, &references, &inputs);

        assert_eq!(
            values.get("first_frame"),
            Some(&GenerationInputValue::ImageAsset(asset("ast_first")))
        );
        assert_eq!(
            values.get("last_frame"),
            Some(&GenerationInputValue::ImageAsset(asset("ast_last")))
        );
    }

    #[test]
    fn a_missing_optional_last_frame_is_not_fabricated() {
        let mut values = BTreeMap::new();
        let inputs = HashMap::from([
            (
                "first_frame".to_owned(),
                InputDefinition::Image {
                    label: "First".to_owned(),
                    required: false,
                },
            ),
            (
                "last_frame".to_owned(),
                InputDefinition::Image {
                    label: "Last".to_owned(),
                    required: false,
                },
            ),
        ])
        .into_iter()
        .collect();
        let references = vec![SelectedReference {
            asset_id: "ast_first".to_owned(),
            reference_index: 0,
        }];

        inject_reference_values(&mut values, &references, &inputs);

        assert!(values.contains_key("first_frame"));
        assert!(!values.contains_key("last_frame"));
    }
}
