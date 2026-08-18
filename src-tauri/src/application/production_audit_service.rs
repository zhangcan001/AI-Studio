//! Read-only, set-based views over the existing production data model.
//!
//! Production Audit deliberately has no persistence of its own.  It loads the
//! existing run, queue, task, snapshot, asset, and shot rows in batches and
//! joins them in memory.  This keeps audit useful for large projects without
//! turning the audit page into an N+1 query surface.

use crate::application::production_queue_service::{
    build_retry_lineages_from_edges, RetryLineage, RetryLineageEdge,
};
use crate::domain::validate_project_id;
use chrono::Utc;
use serde::Serialize;
use sqlx::{FromRow, SqlitePool};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
};

pub const DEFAULT_ACTIVITY_LIMIT: u32 = 50;
pub const MAX_ACTIVITY_LIMIT: u32 = 200;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum ProductionAuditSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum ProductionAuditHealth {
    Healthy,
    Warning,
    Blocked,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProductionAuditIssue {
    pub severity: ProductionAuditSeverity,
    pub code: String,
    pub message: String,
    pub entity_type: String,
    pub entity_id: String,
    pub related_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProductionAuditSummary {
    pub project_id: String,
    pub active_runs: u64,
    pub completed_runs: u64,
    pub failed_runs: u64,
    pub active_batches: u64,
    pub paused_batches: u64,
    pub failed_batches: u64,
    pub logical_items: u64,
    pub attempts: u64,
    pub succeeded_items: u64,
    pub failed_items: u64,
    pub review_required_items: u64,
    pub tasks: u64,
    pub succeeded_tasks: u64,
    pub failed_tasks: u64,
    pub assets: u64,
    pub unassigned_shots: u64,
    pub checked_at: String,
    pub health: ProductionAuditHealth,
    pub issues: Vec<ProductionAuditIssue>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProductionAuditActivity {
    pub kind: String,
    pub timestamp: String,
    pub severity: ProductionAuditSeverity,
    pub title: String,
    pub detail: String,
    pub run_id: Option<String>,
    pub batch_id: Option<String>,
    pub item_id: Option<String>,
    pub task_id: Option<String>,
    pub shot_id: Option<String>,
    pub asset_id: Option<String>,
    pub error_code: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProductionAuditLineage {
    pub root_type: String,
    pub root_id: String,
    pub nodes: Vec<ProductionAuditLineageNode>,
    pub issues: Vec<ProductionAuditIssue>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProductionAuditLineageNode {
    pub entity_type: String,
    pub id: String,
    pub label: String,
    pub status: Option<String>,
    pub parent_id: Option<String>,
    pub run_id: Option<String>,
    pub batch_id: Option<String>,
    pub item_id: Option<String>,
    pub task_id: Option<String>,
    pub shot_id: Option<String>,
    pub asset_id: Option<String>,
    pub stage: Option<String>,
    pub error_code: Option<String>,
    pub related_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProductionAuditIntegrity {
    pub project_id: String,
    pub health: ProductionAuditHealth,
    pub checked_at: String,
    pub issues: Vec<ProductionAuditIssue>,
}

#[derive(Debug)]
pub enum ProductionAuditError {
    InvalidInput(String),
    NotFound(String),
    Database(sqlx::Error),
}

impl fmt::Display for ProductionAuditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) | Self::NotFound(message) => formatter.write_str(message),
            Self::Database(error) => write!(formatter, "production audit database error: {error}"),
        }
    }
}

impl Error for ProductionAuditError {}

impl From<sqlx::Error> for ProductionAuditError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

pub struct ProductionAuditService {
    pool: SqlitePool,
}

impl ProductionAuditService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn project_summary(
        &self,
        project_id: &str,
    ) -> Result<ProductionAuditSummary, ProductionAuditError> {
        let graph = self.load_graph(project_id).await?;
        let issues = integrity_issues(&graph);
        let health = health_for(&issues);
        let mut logical_items = 0_u64;
        let mut attempts = 0_u64;
        let mut succeeded_items = 0_u64;
        let mut failed_items = 0_u64;
        let mut review_required_items = 0_u64;
        let mut failed_batches = 0_u64;

        for batch in &graph.batches {
            let items = graph
                .batch_items
                .iter()
                .filter(|item| item.batch_id == batch.id)
                .collect::<Vec<_>>();
            attempts += items.len() as u64;
            logical_items += items
                .iter()
                .filter(|item| item.retry_of_item_id.is_none())
                .count() as u64;

            let lineages = retry_lineages(&items);
            let mut batch_failed = false;
            if let Ok(lineages) = lineages {
                for lineage in lineages {
                    let Some(leaf) = items.iter().find(|item| item.id == lineage.leaf_item_id)
                    else {
                        continue;
                    };
                    match leaf.status.as_str() {
                        "SUCCEEDED" => succeeded_items += 1,
                        "FAILED" => {
                            failed_items += 1;
                            review_required_items += 1;
                            batch_failed = true;
                        }
                        "CANCELLED" => review_required_items += 1,
                        _ => {}
                    }
                }
            } else {
                batch_failed = items.iter().any(|item| item.status == "FAILED");
                failed_items += items.iter().filter(|item| item.status == "FAILED").count() as u64;
                review_required_items += items
                    .iter()
                    .filter(|item| matches!(item.status.as_str(), "FAILED" | "CANCELLED"))
                    .count() as u64;
            }
            if batch_failed {
                failed_batches += 1;
            }
        }

        let active_runs = graph
            .runs
            .iter()
            .filter(|run| {
                !matches!(
                    run.status.as_str(),
                    "SUCCEEDED" | "PARTIAL_FAILED" | "FAILED" | "CANCELLED"
                )
            })
            .count() as u64;
        let completed_runs = graph
            .runs
            .iter()
            .filter(|run| run.status == "SUCCEEDED")
            .count() as u64;
        let failed_runs = graph
            .runs
            .iter()
            .filter(|run| matches!(run.status.as_str(), "PARTIAL_FAILED" | "FAILED"))
            .count() as u64;
        let active_batches = graph
            .batches
            .iter()
            .filter(|batch| matches!(batch.status.as_str(), "READY" | "RUNNING"))
            .count() as u64;
        let paused_batches = graph
            .batches
            .iter()
            .filter(|batch| batch.status == "PAUSED")
            .count() as u64;
        let succeeded_tasks = graph
            .tasks
            .iter()
            .filter(|task| task.status == "SUCCEEDED")
            .count() as u64;
        let failed_tasks = graph
            .tasks
            .iter()
            .filter(|task| task.status == "FAILED")
            .count() as u64;
        let unassigned_shots = graph
            .shots
            .iter()
            .filter(|shot| {
                shot.selected_image_asset_id.is_none() && shot.selected_video_asset_id.is_none()
            })
            .count() as u64;

        Ok(ProductionAuditSummary {
            project_id: project_id.to_owned(),
            active_runs,
            completed_runs,
            failed_runs,
            active_batches,
            paused_batches,
            failed_batches,
            logical_items,
            attempts,
            succeeded_items,
            failed_items,
            review_required_items,
            tasks: graph.tasks.len() as u64,
            succeeded_tasks,
            failed_tasks,
            assets: graph.assets.len() as u64,
            unassigned_shots,
            checked_at: now_string(),
            health,
            issues,
        })
    }

    pub async fn summary(
        &self,
        project_id: &str,
    ) -> Result<ProductionAuditSummary, ProductionAuditError> {
        self.project_summary(project_id).await
    }

    pub async fn recent_activity(
        &self,
        project_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<ProductionAuditActivity>, ProductionAuditError> {
        let graph = self.load_graph(project_id).await?;
        let limit = limit
            .unwrap_or(DEFAULT_ACTIVITY_LIMIT)
            .clamp(1, MAX_ACTIVITY_LIMIT) as usize;
        let mut activities = Vec::new();

        for run in &graph.runs {
            activities.push(activity(
                "RUN_CREATED",
                &run.created_at,
                ProductionAuditSeverity::Info,
                "生产运行已创建",
                &run.name,
                Some(run.id.clone()),
                None,
                None,
                None,
                None,
                None,
                None,
            ));
            if let Some(timestamp) = run.finished_at.as_deref() {
                let (kind, severity, title) = if run.status == "SUCCEEDED" {
                    (
                        "RUN_COMPLETED",
                        ProductionAuditSeverity::Info,
                        "生产运行已完成",
                    )
                } else {
                    ("RUN_FAILED", ProductionAuditSeverity::Error, "生产运行失败")
                };
                activities.push(activity(
                    kind,
                    timestamp,
                    severity,
                    title,
                    &run.name,
                    Some(run.id.clone()),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                ));
            }
        }
        for batch in &graph.batches {
            activities.push(activity(
                "BATCH_CREATED",
                &batch.created_at,
                ProductionAuditSeverity::Info,
                "生产批次已创建",
                &batch.name,
                None,
                Some(batch.id.clone()),
                None,
                None,
                None,
                None,
                None,
            ));
            if batch.status == "PAUSED" {
                activities.push(activity(
                    "BATCH_PAUSED",
                    &batch.updated_at,
                    ProductionAuditSeverity::Warning,
                    "生产批次已暂停",
                    &batch.name,
                    None,
                    Some(batch.id.clone()),
                    None,
                    None,
                    None,
                    None,
                    None,
                ));
            } else if batch.status == "COMPLETED" {
                activities.push(activity(
                    "BATCH_COMPLETED",
                    &batch.updated_at,
                    ProductionAuditSeverity::Info,
                    "生产批次已完成",
                    &batch.name,
                    None,
                    Some(batch.id.clone()),
                    None,
                    None,
                    None,
                    None,
                    None,
                ));
            }
        }
        for item in &graph.batch_items {
            if item.status == "FAILED" {
                activities.push(activity(
                    "ITEM_FAILED",
                    &item.updated_at,
                    ProductionAuditSeverity::Error,
                    "生产项失败",
                    &item.id,
                    None,
                    Some(item.batch_id.clone()),
                    Some(item.id.clone()),
                    item.task_id.clone(),
                    None,
                    None,
                    item.error_code.clone(),
                ));
            }
            if item.retry_of_item_id.is_some() {
                activities.push(activity(
                    "ITEM_RETRIED",
                    &item.created_at,
                    ProductionAuditSeverity::Info,
                    "生产项已重试",
                    &item.id,
                    None,
                    Some(item.batch_id.clone()),
                    Some(item.id.clone()),
                    item.task_id.clone(),
                    None,
                    None,
                    None,
                ));
            }
        }
        for task in &graph.tasks {
            if matches!(task.status.as_str(), "SUCCEEDED" | "FAILED") {
                activities.push(activity(
                    if task.status == "SUCCEEDED" {
                        "TASK_SUCCEEDED"
                    } else {
                        "TASK_FAILED"
                    },
                    task.finished_at.as_deref().unwrap_or(&task.updated_at),
                    if task.status == "SUCCEEDED" {
                        ProductionAuditSeverity::Info
                    } else {
                        ProductionAuditSeverity::Error
                    },
                    if task.status == "SUCCEEDED" {
                        "任务已成功"
                    } else {
                        "任务失败"
                    },
                    &task.id,
                    None,
                    None,
                    None,
                    Some(task.id.clone()),
                    None,
                    None,
                    task.error_code.clone(),
                ));
            }
        }
        for asset in &graph.assets {
            activities.push(activity(
                "ASSET_CREATED",
                &asset.created_at,
                ProductionAuditSeverity::Info,
                "资产已创建",
                &asset.name,
                None,
                None,
                None,
                asset.source_task_id.clone(),
                None,
                Some(asset.id.clone()),
                None,
            ));
        }
        for shot in &graph.shots {
            if let Some(asset_id) = shot.selected_image_asset_id.as_deref() {
                activities.push(activity(
                    "SHOT_IMAGE_SELECTED",
                    &shot.updated_at,
                    ProductionAuditSeverity::Info,
                    "镜头已选择图片",
                    &shot.name,
                    None,
                    None,
                    None,
                    None,
                    Some(shot.id.clone()),
                    Some(asset_id.to_owned()),
                    None,
                ));
            }
            if let Some(asset_id) = shot.selected_video_asset_id.as_deref() {
                activities.push(activity(
                    "SHOT_VIDEO_SELECTED",
                    &shot.updated_at,
                    ProductionAuditSeverity::Info,
                    "镜头已选择视频",
                    &shot.name,
                    None,
                    None,
                    None,
                    None,
                    Some(shot.id.clone()),
                    Some(asset_id.to_owned()),
                    None,
                ));
            }
        }

        activities.sort_by(|left, right| {
            right
                .timestamp
                .cmp(&left.timestamp)
                .then_with(|| left.kind.cmp(&right.kind))
                .then_with(|| left.detail.cmp(&right.detail))
        });
        activities.truncate(limit);
        Ok(activities)
    }

    pub async fn production_lineage(
        &self,
        project_id: &str,
        root_type: &str,
        root_id: &str,
    ) -> Result<ProductionAuditLineage, ProductionAuditError> {
        let graph = self.load_graph(project_id).await?;
        let normalized_type = root_type.trim().to_ascii_uppercase();
        if !matches!(normalized_type.as_str(), "RUN" | "BATCH" | "SHOT" | "TASK") {
            return Err(ProductionAuditError::InvalidInput(format!(
                "unsupported production lineage root type: {root_type}"
            )));
        }
        let mut builder = LineageBuilder::default();
        match normalized_type.as_str() {
            "RUN" => {
                let run = graph
                    .runs
                    .iter()
                    .find(|run| run.id == root_id)
                    .ok_or_else(|| {
                        ProductionAuditError::NotFound(format!("run not found: {root_id}"))
                    })?;
                builder.add_run(&graph, run, None);
            }
            "BATCH" => {
                let batch = graph
                    .batches
                    .iter()
                    .find(|batch| batch.id == root_id)
                    .ok_or_else(|| {
                        ProductionAuditError::NotFound(format!("batch not found: {root_id}"))
                    })?;
                builder.add_batch(&graph, batch, None);
            }
            "SHOT" => {
                let shot = graph
                    .shots
                    .iter()
                    .find(|shot| shot.id == root_id)
                    .ok_or_else(|| {
                        ProductionAuditError::NotFound(format!("shot not found: {root_id}"))
                    })?;
                builder.add_shot(&graph, shot, None);
            }
            "TASK" => {
                let task = graph
                    .tasks
                    .iter()
                    .find(|task| task.id == root_id)
                    .ok_or_else(|| {
                        ProductionAuditError::NotFound(format!("task not found: {root_id}"))
                    })?;
                builder.add_task(&graph, task, None);
            }
            _ => unreachable!(),
        }
        Ok(ProductionAuditLineage {
            root_type: normalized_type,
            root_id: root_id.to_owned(),
            nodes: builder.nodes,
            issues: integrity_issues(&graph),
        })
    }

    pub async fn lineage(
        &self,
        project_id: &str,
        root_type: &str,
        root_id: &str,
    ) -> Result<ProductionAuditLineage, ProductionAuditError> {
        self.production_lineage(project_id, root_type, root_id)
            .await
    }

    pub async fn audit_integrity(
        &self,
        project_id: &str,
    ) -> Result<ProductionAuditIntegrity, ProductionAuditError> {
        let graph = self.load_graph(project_id).await?;
        let issues = integrity_issues(&graph);
        Ok(ProductionAuditIntegrity {
            project_id: project_id.to_owned(),
            health: health_for(&issues),
            checked_at: now_string(),
            issues,
        })
    }

    pub async fn integrity(
        &self,
        project_id: &str,
    ) -> Result<ProductionAuditIntegrity, ProductionAuditError> {
        self.audit_integrity(project_id).await
    }

    async fn load_graph(&self, project_id: &str) -> Result<AuditGraph, ProductionAuditError> {
        validate_project_id(project_id)
            .map_err(|error| ProductionAuditError::InvalidInput(error.to_string()))?;
        let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM projects WHERE id = ?")
            .bind(project_id)
            .fetch_one(&self.pool)
            .await?;
        if exists == 0 {
            return Err(ProductionAuditError::NotFound(format!(
                "project not found: {project_id}"
            )));
        }

        let runs = sqlx::query_as::<_, RunRow>(
            "SELECT id, project_id, name, status, created_at, updated_at, started_at, finished_at
             FROM production_runs WHERE project_id = ?",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let stages = sqlx::query_as::<_, StageRow>(
            "SELECT s.id, s.run_id, s.ordinal, s.stage_type, s.status, s.production_batch_id,
                    s.created_at, s.updated_at, s.started_at, s.finished_at
             FROM production_stages s JOIN production_runs r ON r.id = s.run_id
             WHERE r.project_id = ? ORDER BY s.run_id, s.ordinal, s.id",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let batches = sqlx::query_as::<_, BatchRow>(
            "SELECT id, project_id, name, status, created_at, updated_at
             FROM production_batches WHERE project_id = ?",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let batch_items = sqlx::query_as::<_, BatchItemRow>(
            "SELECT i.id, i.batch_id, i.ordinal, i.status, i.task_id, i.retry_of_item_id,
                    i.error_code, i.error_message, i.created_at, i.updated_at
             FROM production_batch_items i JOIN production_batches b ON b.id = i.batch_id
             WHERE b.project_id = ? ORDER BY i.batch_id, i.ordinal, i.id",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let stage_items = sqlx::query_as::<_, StageItemRow>(
            "SELECT i.id, i.stage_id, i.ordinal, i.status, i.production_batch_item_id,
                    i.task_id, i.asset_id, i.parent_stage_item_id, i.error_code
             FROM production_stage_items i
             JOIN production_stages s ON s.id = i.stage_id
             JOIN production_runs r ON r.id = s.run_id
             WHERE r.project_id = ? ORDER BY i.stage_id, i.ordinal, i.id",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let tasks = sqlx::query_as::<_, TaskRow>(
            "SELECT id, project_id, status, error_code, created_at,
                    COALESCE(finished_at, started_at, queued_at, created_at) AS updated_at,
                    finished_at
             FROM tasks WHERE project_id = ?",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let snapshots = sqlx::query_as::<_, SnapshotRow>(
            "SELECT s.id, s.task_id, s.created_at
             FROM generation_snapshots s JOIN tasks t ON t.id = s.task_id
             WHERE t.project_id = ?",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let assets = sqlx::query_as::<_, AssetRow>(
            "SELECT id, project_id, name, source_task_id, created_at, updated_at
             FROM assets WHERE project_id = ?",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let task_outputs = sqlx::query_as::<_, TaskOutputRow>(
            "SELECT o.task_id, o.output_id, o.ordinal, o.asset_id, o.created_at
             FROM task_output_assets o JOIN tasks t ON t.id = o.task_id
             WHERE t.project_id = ? ORDER BY o.task_id, o.output_id, o.ordinal",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let shots = sqlx::query_as::<_, ShotRow>(
            "SELECT id, project_id, name, selected_image_asset_id, selected_video_asset_id,
                    created_at, updated_at
             FROM shots WHERE project_id = ? ORDER BY ordinal, id",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let shot_links = sqlx::query_as::<_, ShotLinkRow>(
            "SELECT l.id, l.shot_id, l.stage, l.task_id, l.production_batch_item_id, l.created_at
             FROM shot_generation_links l JOIN shots s ON s.id = l.shot_id
             WHERE s.project_id = ? ORDER BY l.shot_id, l.created_at, l.id",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(AuditGraph {
            runs,
            stages,
            batches,
            batch_items,
            stage_items,
            tasks,
            snapshots,
            assets,
            task_outputs,
            shots,
            shot_links,
        })
    }
}

#[derive(Debug, FromRow)]
struct RunRow {
    id: String,
    project_id: String,
    name: String,
    status: String,
    created_at: String,
    updated_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
}

#[derive(Debug, FromRow)]
struct StageRow {
    id: String,
    run_id: String,
    ordinal: i64,
    stage_type: String,
    status: String,
    production_batch_id: Option<String>,
    created_at: String,
    updated_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
}

#[derive(Debug, FromRow)]
struct BatchRow {
    id: String,
    project_id: String,
    name: String,
    status: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, FromRow)]
struct BatchItemRow {
    id: String,
    batch_id: String,
    ordinal: i64,
    status: String,
    task_id: Option<String>,
    retry_of_item_id: Option<String>,
    error_code: Option<String>,
    error_message: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, FromRow)]
struct StageItemRow {
    id: String,
    stage_id: String,
    ordinal: i64,
    status: String,
    production_batch_item_id: Option<String>,
    task_id: Option<String>,
    asset_id: Option<String>,
    parent_stage_item_id: Option<String>,
    error_code: Option<String>,
}

#[derive(Debug, FromRow)]
struct TaskRow {
    id: String,
    project_id: String,
    status: String,
    error_code: Option<String>,
    created_at: String,
    updated_at: String,
    finished_at: Option<String>,
}

#[derive(Debug, FromRow)]
struct SnapshotRow {
    id: String,
    task_id: String,
    created_at: String,
}

#[derive(Debug, FromRow)]
struct AssetRow {
    id: String,
    project_id: String,
    name: String,
    source_task_id: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, FromRow)]
struct TaskOutputRow {
    task_id: String,
    output_id: String,
    ordinal: i64,
    asset_id: String,
    created_at: String,
}

#[derive(Debug, FromRow)]
struct ShotRow {
    id: String,
    project_id: String,
    name: String,
    selected_image_asset_id: Option<String>,
    selected_video_asset_id: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, FromRow)]
struct ShotLinkRow {
    id: String,
    shot_id: String,
    stage: String,
    task_id: Option<String>,
    production_batch_item_id: Option<String>,
    created_at: String,
}

struct AuditGraph {
    runs: Vec<RunRow>,
    stages: Vec<StageRow>,
    batches: Vec<BatchRow>,
    batch_items: Vec<BatchItemRow>,
    stage_items: Vec<StageItemRow>,
    tasks: Vec<TaskRow>,
    snapshots: Vec<SnapshotRow>,
    assets: Vec<AssetRow>,
    task_outputs: Vec<TaskOutputRow>,
    shots: Vec<ShotRow>,
    shot_links: Vec<ShotLinkRow>,
}

fn now_string() -> String {
    Utc::now().to_rfc3339()
}

fn retry_lineages(items: &[&BatchItemRow]) -> Result<Vec<RetryLineage>, String> {
    let edges = items
        .iter()
        .map(|item| RetryLineageEdge {
            item_id: item.id.clone(),
            parent_item_id: item.retry_of_item_id.clone(),
            ordinal: item.ordinal,
        })
        .collect::<Vec<_>>();
    build_retry_lineages_from_edges(&edges)
}

fn health_for(issues: &[ProductionAuditIssue]) -> ProductionAuditHealth {
    if issues
        .iter()
        .any(|issue| issue.severity == ProductionAuditSeverity::Error)
    {
        ProductionAuditHealth::Blocked
    } else if issues
        .iter()
        .any(|issue| issue.severity == ProductionAuditSeverity::Warning)
    {
        ProductionAuditHealth::Warning
    } else {
        ProductionAuditHealth::Healthy
    }
}

fn issue(
    severity: ProductionAuditSeverity,
    code: &str,
    message: impl Into<String>,
    entity_type: &str,
    entity_id: &str,
    related_ids: Vec<String>,
) -> ProductionAuditIssue {
    ProductionAuditIssue {
        severity,
        code: code.to_owned(),
        message: message.into(),
        entity_type: entity_type.to_owned(),
        entity_id: entity_id.to_owned(),
        related_ids,
    }
}

fn integrity_issues(graph: &AuditGraph) -> Vec<ProductionAuditIssue> {
    let task_ids = graph
        .tasks
        .iter()
        .map(|task| task.id.as_str())
        .collect::<HashSet<_>>();
    let asset_ids = graph
        .assets
        .iter()
        .map(|asset| asset.id.as_str())
        .collect::<HashSet<_>>();
    let batch_item_ids = graph
        .batch_items
        .iter()
        .map(|item| item.id.as_str())
        .collect::<HashSet<_>>();
    let mut issues = Vec::new();

    for item in &graph.stage_items {
        if let Some(task_id) = item.task_id.as_deref() {
            if !task_ids.contains(task_id) {
                issues.push(issue(
                    ProductionAuditSeverity::Error,
                    "PRODUCTION_STAGE_ITEM_TASK_MISSING",
                    format!("stage item {} references missing task {task_id}", item.id),
                    "PRODUCTION_STAGE_ITEM",
                    &item.id,
                    vec![task_id.to_owned(), item.stage_id.clone()],
                ));
            }
        }
    }
    for item in &graph.batch_items {
        if let Some(task_id) = item.task_id.as_deref() {
            if !task_ids.contains(task_id) {
                issues.push(issue(
                    ProductionAuditSeverity::Error,
                    "PRODUCTION_BATCH_ITEM_TASK_MISSING",
                    format!("batch item {} references missing task {task_id}", item.id),
                    "PRODUCTION_BATCH_ITEM",
                    &item.id,
                    vec![task_id.to_owned(), item.batch_id.clone()],
                ));
            }
        }
    }
    for output in &graph.task_outputs {
        if let Some(task) = graph.tasks.iter().find(|task| task.id == output.task_id) {
            if task.status == "SUCCEEDED" && !asset_ids.contains(output.asset_id.as_str()) {
                issues.push(issue(
                    ProductionAuditSeverity::Error,
                    "TASK_OUTPUT_ASSET_MISSING",
                    format!(
                        "succeeded task {} maps to missing asset {}",
                        output.task_id, output.asset_id
                    ),
                    "TASK",
                    &output.task_id,
                    vec![output.asset_id.clone(), output.output_id.clone()],
                ));
            }
        }
    }
    for link in &graph.shot_links {
        if let Some(task_id) = link.task_id.as_deref() {
            if !task_ids.contains(task_id) {
                issues.push(issue(
                    ProductionAuditSeverity::Error,
                    "SHOT_GENERATION_TASK_MISSING",
                    format!(
                        "shot generation link {} references missing task {task_id}",
                        link.id
                    ),
                    "SHOT_GENERATION_LINK",
                    &link.id,
                    vec![link.shot_id.clone(), task_id.to_owned()],
                ));
            }
        }
        if let Some(item_id) = link.production_batch_item_id.as_deref() {
            if !batch_item_ids.contains(item_id) {
                issues.push(issue(
                    ProductionAuditSeverity::Error,
                    "SHOT_GENERATION_ITEM_MISSING",
                    format!(
                        "shot generation link {} references missing item {item_id}",
                        link.id
                    ),
                    "SHOT_GENERATION_LINK",
                    &link.id,
                    vec![link.shot_id.clone(), item_id.to_owned()],
                ));
            }
        }
    }
    for shot in &graph.shots {
        for (stage, asset_id) in [
            ("image", shot.selected_image_asset_id.as_deref()),
            ("video", shot.selected_video_asset_id.as_deref()),
        ] {
            if let Some(asset_id) = asset_id {
                if !asset_ids.contains(asset_id) {
                    issues.push(issue(
                        ProductionAuditSeverity::Error,
                        "SHOT_SELECTED_ASSET_MISSING",
                        format!(
                            "shot {} selected {stage} asset {asset_id}, but the asset is missing",
                            shot.id
                        ),
                        "SHOT",
                        &shot.id,
                        vec![stage.to_owned(), asset_id.to_owned()],
                    ));
                }
            }
        }
    }

    let mut items_by_batch = HashMap::<&str, Vec<&BatchItemRow>>::new();
    for item in &graph.batch_items {
        items_by_batch
            .entry(item.batch_id.as_str())
            .or_default()
            .push(item);
    }
    for (batch_id, items) in items_by_batch {
        if let Err(error) = retry_lineages(&items) {
            let (code, message) = if error.contains("missing parent") {
                ("PRODUCTION_RETRY_PARENT_MISSING", error)
            } else if error.contains("multiple children") {
                ("PRODUCTION_RETRY_DUPLICATE_CHILD", error)
            } else if error.contains("cycle") || error.contains("no root item") {
                ("PRODUCTION_RETRY_CYCLE", error)
            } else {
                ("PRODUCTION_RETRY_LINEAGE_INVALID", error)
            };
            issues.push(issue(
                ProductionAuditSeverity::Error,
                code,
                message,
                "PRODUCTION_BATCH",
                batch_id,
                vec![batch_id.to_owned()],
            ));
        }
    }

    let failed_tasks = graph
        .tasks
        .iter()
        .filter(|task| task.status == "FAILED")
        .count();
    if failed_tasks > 0 {
        issues.push(issue(
            ProductionAuditSeverity::Warning,
            "FAILED_TASKS_PRESENT",
            format!("{failed_tasks} task(s) failed and require review"),
            "PROJECT",
            &graph
                .runs
                .first()
                .map(|run| run.project_id.clone())
                .or_else(|| graph.batches.first().map(|batch| batch.project_id.clone()))
                .or_else(|| graph.tasks.first().map(|task| task.project_id.clone()))
                .unwrap_or_default(),
            Vec::new(),
        ));
    }
    let paused_batches = graph
        .batches
        .iter()
        .filter(|batch| batch.status == "PAUSED")
        .count();
    if paused_batches > 0 {
        issues.push(issue(
            ProductionAuditSeverity::Warning,
            "PAUSED_BATCHES_PRESENT",
            format!("{paused_batches} batch(es) are paused"),
            "PROJECT",
            &graph
                .batches
                .first()
                .map(|batch| batch.project_id.clone())
                .unwrap_or_default(),
            Vec::new(),
        ));
    }
    issues
}

#[allow(clippy::too_many_arguments)]
fn activity(
    kind: &str,
    timestamp: &str,
    severity: ProductionAuditSeverity,
    title: &str,
    detail: &str,
    run_id: Option<String>,
    batch_id: Option<String>,
    item_id: Option<String>,
    task_id: Option<String>,
    shot_id: Option<String>,
    asset_id: Option<String>,
    error_code: Option<String>,
) -> ProductionAuditActivity {
    ProductionAuditActivity {
        kind: kind.to_owned(),
        timestamp: timestamp.to_owned(),
        severity,
        title: title.to_owned(),
        detail: detail.to_owned(),
        run_id,
        batch_id,
        item_id,
        task_id,
        shot_id,
        asset_id,
        error_code,
    }
}

#[derive(Default)]
struct LineageBuilder {
    nodes: Vec<ProductionAuditLineageNode>,
    seen: HashSet<String>,
}

impl LineageBuilder {
    fn push(&mut self, node: ProductionAuditLineageNode) {
        let key = format!("{}:{}", node.entity_type, node.id);
        if self.seen.insert(key) {
            self.nodes.push(node);
        }
    }

    fn has(&self, entity_type: &str, id: &str) -> bool {
        self.seen.contains(&format!("{entity_type}:{id}"))
    }

    fn add_run(&mut self, graph: &AuditGraph, run: &RunRow, parent_id: Option<String>) {
        self.push(node(
            "RUN",
            &run.id,
            &run.name,
            Some(&run.status),
            parent_id,
            Some(run.id.clone()),
            None,
            None,
            None,
            None,
            None,
            None,
            Vec::new(),
        ));
        for stage in graph.stages.iter().filter(|stage| stage.run_id == run.id) {
            self.add_stage(graph, stage, Some(run.id.clone()));
        }
    }

    fn add_stage(&mut self, graph: &AuditGraph, stage: &StageRow, parent_id: Option<String>) {
        if self.has("STAGE", &stage.id) {
            return;
        }
        self.push(node(
            "STAGE",
            &stage.id,
            &stage.stage_type,
            Some(&stage.status),
            parent_id,
            Some(stage.run_id.clone()),
            stage.production_batch_id.clone(),
            None,
            None,
            None,
            None,
            None,
            Vec::new(),
        ));
        if let Some(batch_id) = stage.production_batch_id.as_deref() {
            if let Some(batch) = graph.batches.iter().find(|batch| batch.id == batch_id) {
                self.add_batch(graph, batch, Some(stage.id.clone()));
            }
        }
        for item in graph
            .stage_items
            .iter()
            .filter(|item| item.stage_id == stage.id)
        {
            self.add_stage_item(graph, item, Some(stage.id.clone()));
        }
    }

    fn add_batch(&mut self, graph: &AuditGraph, batch: &BatchRow, parent_id: Option<String>) {
        if self.has("BATCH", &batch.id) {
            return;
        }
        self.push(node(
            "BATCH",
            &batch.id,
            &batch.name,
            Some(&batch.status),
            parent_id,
            None,
            Some(batch.id.clone()),
            None,
            None,
            None,
            None,
            None,
            Vec::new(),
        ));
        for stage in graph
            .stages
            .iter()
            .filter(|stage| stage.production_batch_id.as_deref() == Some(batch.id.as_str()))
        {
            self.add_stage(graph, stage, Some(batch.id.clone()));
        }
        let items = graph
            .batch_items
            .iter()
            .filter(|item| item.batch_id == batch.id)
            .collect::<Vec<_>>();
        let ids = items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<HashSet<_>>();
        for item in items {
            let parent = item
                .retry_of_item_id
                .as_ref()
                .filter(|id| ids.contains(id.as_str()))
                .cloned()
                .or_else(|| Some(batch.id.clone()));
            self.add_batch_item(graph, item, parent);
        }
    }

    fn add_batch_item(
        &mut self,
        graph: &AuditGraph,
        item: &BatchItemRow,
        parent_id: Option<String>,
    ) {
        if self.has("BATCH_ITEM", &item.id) {
            return;
        }
        self.push(node(
            "BATCH_ITEM",
            &item.id,
            &format!("Item {}", item.ordinal + 1),
            Some(&item.status),
            parent_id,
            None,
            Some(item.batch_id.clone()),
            Some(item.id.clone()),
            item.task_id.clone(),
            None,
            None,
            None,
            item.error_code.clone().into_iter().collect(),
        ));
        if let Some(task_id) = item.task_id.as_deref() {
            if let Some(task) = graph.tasks.iter().find(|task| task.id == task_id) {
                self.add_task(graph, task, Some(item.id.clone()));
            }
        }
    }

    fn add_stage_item(
        &mut self,
        graph: &AuditGraph,
        item: &StageItemRow,
        parent_id: Option<String>,
    ) {
        if self.has("STAGE_ITEM", &item.id) {
            return;
        }
        self.push(node(
            "STAGE_ITEM",
            &item.id,
            &format!("Stage item {}", item.ordinal + 1),
            Some(&item.status),
            parent_id,
            None,
            None,
            item.production_batch_item_id.clone(),
            item.task_id.clone(),
            None,
            item.asset_id.clone(),
            None,
            item.error_code.clone().into_iter().collect(),
        ));
        if let Some(task_id) = item.task_id.as_deref() {
            if let Some(task) = graph.tasks.iter().find(|task| task.id == task_id) {
                self.add_task(graph, task, Some(item.id.clone()));
            }
        }
        if let Some(asset_id) = item.asset_id.as_deref() {
            if let Some(asset) = graph.assets.iter().find(|asset| asset.id == asset_id) {
                self.add_asset(asset, Some(item.id.clone()));
            }
        }
    }

    fn add_task(&mut self, graph: &AuditGraph, task: &TaskRow, parent_id: Option<String>) {
        if self.has("TASK", &task.id) {
            return;
        }
        self.push(node(
            "TASK",
            &task.id,
            &task.id,
            Some(&task.status),
            parent_id,
            None,
            None,
            None,
            Some(task.id.clone()),
            None,
            None,
            None,
            task.error_code.clone().into_iter().collect(),
        ));
        if let Some(snapshot) = graph
            .snapshots
            .iter()
            .find(|snapshot| snapshot.task_id == task.id)
        {
            self.push(node(
                "SNAPSHOT",
                &snapshot.id,
                &snapshot.id,
                None,
                Some(task.id.clone()),
                None,
                None,
                None,
                Some(task.id.clone()),
                None,
                None,
                None,
                Vec::new(),
            ));
        }
        for output in graph
            .task_outputs
            .iter()
            .filter(|output| output.task_id == task.id)
        {
            if let Some(asset) = graph
                .assets
                .iter()
                .find(|asset| asset.id == output.asset_id)
            {
                self.add_asset(asset, Some(task.id.clone()));
            } else {
                self.push(node(
                    "ASSET",
                    &output.asset_id,
                    &output.asset_id,
                    None,
                    Some(task.id.clone()),
                    None,
                    None,
                    None,
                    Some(task.id.clone()),
                    None,
                    Some(output.asset_id.clone()),
                    None,
                    Vec::new(),
                ));
            }
        }
        for asset in graph
            .assets
            .iter()
            .filter(|asset| asset.source_task_id.as_deref() == Some(task.id.as_str()))
        {
            self.add_asset(asset, Some(task.id.clone()));
        }
        for link in graph
            .shot_links
            .iter()
            .filter(|link| link.task_id.as_deref() == Some(task.id.as_str()))
        {
            self.push(node(
                "SHOT_GENERATION_LINK",
                &link.id,
                &format!("{} generation", link.stage),
                None,
                Some(task.id.clone()),
                None,
                None,
                link.production_batch_item_id.clone(),
                Some(task.id.clone()),
                Some(link.shot_id.clone()),
                None,
                Some(link.stage.clone()),
                Vec::new(),
            ));
        }
    }

    fn add_shot(&mut self, graph: &AuditGraph, shot: &ShotRow, parent_id: Option<String>) {
        if self.has("SHOT", &shot.id) {
            return;
        }
        self.push(node(
            "SHOT",
            &shot.id,
            &shot.name,
            None,
            parent_id,
            None,
            None,
            None,
            None,
            Some(shot.id.clone()),
            None,
            None,
            Vec::new(),
        ));
        for (stage, asset_id) in [
            ("image", shot.selected_image_asset_id.as_deref()),
            ("video", shot.selected_video_asset_id.as_deref()),
        ] {
            if let Some(asset_id) = asset_id {
                if let Some(asset) = graph.assets.iter().find(|asset| asset.id == asset_id) {
                    self.push(node(
                        "ASSET",
                        &asset.id,
                        &asset.name,
                        None,
                        Some(shot.id.clone()),
                        None,
                        None,
                        None,
                        asset.source_task_id.clone(),
                        Some(shot.id.clone()),
                        Some(asset.id.clone()),
                        Some(stage.to_owned()),
                        Vec::new(),
                    ));
                }
            }
        }
        for link in graph
            .shot_links
            .iter()
            .filter(|link| link.shot_id == shot.id)
        {
            self.push(node(
                "SHOT_GENERATION_LINK",
                &link.id,
                &format!("{} generation", link.stage),
                None,
                Some(shot.id.clone()),
                None,
                None,
                link.production_batch_item_id.clone(),
                link.task_id.clone(),
                Some(shot.id.clone()),
                None,
                Some(link.stage.clone()),
                Vec::new(),
            ));
            if let Some(task_id) = link.task_id.as_deref() {
                if let Some(task) = graph.tasks.iter().find(|task| task.id == task_id) {
                    self.add_task(graph, task, Some(link.id.clone()));
                }
            }
            if let Some(item_id) = link.production_batch_item_id.as_deref() {
                if let Some(item) = graph.batch_items.iter().find(|item| item.id == item_id) {
                    self.add_batch_item(graph, item, Some(link.id.clone()));
                }
            }
        }
    }

    fn add_asset(&mut self, asset: &AssetRow, parent_id: Option<String>) {
        self.push(node(
            "ASSET",
            &asset.id,
            &asset.name,
            None,
            parent_id,
            None,
            None,
            None,
            asset.source_task_id.clone(),
            None,
            Some(asset.id.clone()),
            None,
            Vec::new(),
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn node(
    entity_type: &str,
    id: &str,
    label: &str,
    status: Option<&str>,
    parent_id: Option<String>,
    run_id: Option<String>,
    batch_id: Option<String>,
    item_id: Option<String>,
    task_id: Option<String>,
    shot_id: Option<String>,
    asset_id: Option<String>,
    stage: Option<String>,
    related_ids: Vec<String>,
) -> ProductionAuditLineageNode {
    ProductionAuditLineageNode {
        entity_type: entity_type.to_owned(),
        id: id.to_owned(),
        label: label.to_owned(),
        status: status.map(str::to_owned),
        parent_id,
        run_id,
        batch_id,
        item_id,
        task_id,
        shot_id,
        asset_id,
        stage,
        error_code: None,
        related_ids,
    }
}

#[cfg(test)]
mod tests {
    use super::{ProductionAuditHealth, ProductionAuditService};
    use crate::infrastructure::database::initialize;
    use sqlx::SqlitePool;
    use tempfile::tempdir;

    const PROJECT: &str = "prj_default";
    const NOW: &str = "2026-08-18T00:00:00Z";

    async fn fixture() -> (tempfile::TempDir, SqlitePool) {
        let directory = tempdir().expect("temporary database directory should exist");
        let pool = initialize(&directory.path().join("audit.db"))
            .await
            .expect("database should migrate");
        sqlx::query(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at)
             VALUES (?, 'Audit project', ?, ?, ?)",
        )
        .bind(PROJECT)
        .bind(
            directory
                .path()
                .join("project")
                .to_string_lossy()
                .to_string(),
        )
        .bind(NOW)
        .bind(NOW)
        .execute(&pool)
        .await
        .expect("project should insert");
        sqlx::query(
            "INSERT INTO workflows (id, name, category, mode, created_at, updated_at)
             VALUES ('wf_audit', 'Audit workflow', 'image', 'T2I', ?, ?)",
        )
        .bind(NOW)
        .bind(NOW)
        .execute(&pool)
        .await
        .expect("workflow should insert");
        sqlx::query(
            "INSERT INTO workflow_versions
             (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
             VALUES ('wv_audit', 'wf_audit', '1', '{}', 'sha', ?)",
        )
        .bind(NOW)
        .execute(&pool)
        .await
        .expect("workflow version should insert");
        sqlx::query(
            "INSERT INTO recipes
             (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at)
             VALUES ('recipe_audit', 'wv_audit', '1', 1, 'inputs: {}', 'sha', ?)",
        )
        .bind(NOW)
        .execute(&pool)
        .await
        .expect("recipe should insert");
        (directory, pool)
    }

    async fn insert_task(pool: &SqlitePool, id: &str, status: &str) {
        sqlx::query(
            "INSERT INTO tasks
             (id, project_id, workflow_id, workflow_version_id, recipe_id, status, created_at, finished_at)
             VALUES (?, ?, 'wf_audit', 'wv_audit', 'recipe_audit', ?, ?, ?)",
        )
        .bind(id)
        .bind(PROJECT)
        .bind(status)
        .bind(NOW)
        .bind(if matches!(status, "SUCCEEDED" | "FAILED") { Some(NOW) } else { None::<&str> })
        .execute(pool)
        .await
        .expect("task should insert");
    }

    async fn insert_asset(pool: &SqlitePool, id: &str, task_id: &str) {
        sqlx::query(
            "INSERT INTO assets
             (id, project_id, type, category, name, storage_path, sha256, source_task_id, created_at, updated_at)
             VALUES (?, ?, 'IMAGE', 'GENERATED_IMAGE', ?, ?, 'sha', ?, ?, ?)",
        )
        .bind(id)
        .bind(PROJECT)
        .bind(id)
        .bind(format!("assets/{id}.png"))
        .bind(task_id)
        .bind(NOW)
        .bind(NOW)
        .execute(pool)
        .await
        .expect("asset should insert");
    }

    #[tokio::test]
    async fn retry_summary_activity_and_all_lineage_roots_are_set_based() {
        let (_directory, pool) = fixture().await;
        for (id, status) in [
            ("task-a", "SUCCEEDED"),
            ("task-b", "FAILED"),
            ("task-b2", "SUCCEEDED"),
            ("task-c", "FAILED"),
        ] {
            insert_task(&pool, id, status).await;
        }
        insert_asset(&pool, "asset-a", "task-a").await;
        insert_asset(&pool, "asset-b2", "task-b2").await;
        for (task_id, asset_id) in [("task-a", "asset-a"), ("task-b2", "asset-b2")] {
            sqlx::query(
                "INSERT INTO task_output_assets (task_id, output_id, ordinal, asset_id, created_at)
                 VALUES (?, 'output', 0, ?, ?)",
            )
            .bind(task_id)
            .bind(asset_id)
            .bind(NOW)
            .execute(&pool)
            .await
            .expect("task output should insert");
        }
        sqlx::query(
            "INSERT INTO generation_snapshots
             (id, task_id, workflow_json, recipe_yaml, user_inputs_json, resolved_inputs_json, created_at)
             VALUES ('snapshot-a', 'task-a', '{}', 'inputs: {}', '{}', '{}', ?)",
        )
        .bind(NOW)
        .execute(&pool)
        .await
        .expect("snapshot should insert");
        sqlx::query(
            "INSERT INTO production_batches
             (id, project_id, name, status, continue_on_failure, created_at, updated_at)
             VALUES ('batch-audit', ?, 'Audit batch', 'COMPLETED', 1, ?, ?)",
        )
        .bind(PROJECT)
        .bind(NOW)
        .bind(NOW)
        .execute(&pool)
        .await
        .expect("batch should insert");
        for (id, status, task_id, retry_of) in [
            ("item-a", "SUCCEEDED", Some("task-a"), None),
            ("item-b", "FAILED", Some("task-b"), None),
            ("item-b2", "SUCCEEDED", Some("task-b2"), Some("item-b")),
            ("item-c", "FAILED", Some("task-c"), None),
        ] {
            sqlx::query(
                "INSERT INTO production_batch_items
                 (id, batch_id, ordinal, workflow_version_id, recipe_id, values_json, status, task_id, retry_of_item_id, created_at, updated_at)
                 VALUES (?, 'batch-audit', ?, 'wv_audit', 'recipe_audit', '{}', ?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(match id { "item-a" => 0_i64, "item-b" => 1, "item-b2" => 2, _ => 3 })
            .bind(status)
            .bind(task_id)
            .bind(retry_of)
            .bind(NOW)
            .bind(NOW)
            .execute(&pool)
            .await
            .expect("batch item should insert");
        }
        sqlx::query(
            "INSERT INTO production_runs
             (id, project_id, name, status, created_at, updated_at, finished_at)
             VALUES ('run-audit', ?, 'Audit run', 'SUCCEEDED', ?, ?, ?)",
        )
        .bind(PROJECT)
        .bind(NOW)
        .bind(NOW)
        .bind(NOW)
        .execute(&pool)
        .await
        .expect("run should insert");
        sqlx::query(
            "INSERT INTO production_stages
             (id, run_id, ordinal, stage_type, status, production_batch_id, frozen_config_json, created_at, updated_at)
             VALUES ('stage-audit', 'run-audit', 0, 'KREA2_IMAGE_GENERATION', 'SUCCEEDED', 'batch-audit', '{}', ?, ?)",
        )
        .bind(NOW)
        .bind(NOW)
        .execute(&pool)
        .await
        .expect("stage should insert");
        sqlx::query(
            "INSERT INTO production_stage_items
             (id, stage_id, ordinal, status, production_batch_item_id, task_id, asset_id, attempt, frozen_values_json, created_at, updated_at)
             VALUES ('stage-item-a', 'stage-audit', 0, 'SUCCEEDED', 'item-a', 'task-a', 'asset-a', 1, '{}', ?, ?)",
        )
        .bind(NOW)
        .bind(NOW)
        .execute(&pool)
        .await
        .expect("stage item should insert");
        sqlx::query(
            "INSERT INTO shots
             (id, project_id, ordinal, name, prompt_text, selected_image_asset_id, created_at, updated_at)
             VALUES ('shot-audit', ?, 0, 'Audit shot', 'prompt', 'asset-a', ?, ?)",
        )
        .bind(PROJECT)
        .bind(NOW)
        .bind(NOW)
        .execute(&pool)
        .await
        .expect("shot should insert");
        sqlx::query(
            "INSERT INTO shot_generation_links
             (id, shot_id, stage, task_id, production_batch_item_id, created_at)
             VALUES ('link-audit', 'shot-audit', 'image', 'task-a', 'item-a', ?)",
        )
        .bind(NOW)
        .execute(&pool)
        .await
        .expect("shot link should insert");

        let service = ProductionAuditService::new(pool.clone());
        let summary = service
            .project_summary(PROJECT)
            .await
            .expect("summary should load");
        assert_eq!(summary.logical_items, 3);
        assert_eq!(summary.attempts, 4);
        assert_eq!(summary.succeeded_items, 2);
        assert_eq!(summary.failed_items, 1);
        assert_eq!(summary.review_required_items, 1);
        assert_eq!(summary.tasks, 4);
        assert_eq!(summary.assets, 2);
        assert_eq!(summary.health, ProductionAuditHealth::Warning);

        let activity = service
            .recent_activity(PROJECT, Some(200))
            .await
            .expect("activity should load");
        assert!(activity.iter().any(
            |entry| entry.kind == "ITEM_RETRIED" && entry.item_id.as_deref() == Some("item-b2")
        ));

        for (root_type, root_id, expected) in [
            ("RUN", "run-audit", ["RUN", "STAGE", "BATCH"]),
            ("BATCH", "batch-audit", ["BATCH", "BATCH_ITEM", "TASK"]),
            ("SHOT", "shot-audit", ["SHOT", "TASK", "ASSET"]),
            ("TASK", "task-a", ["TASK", "SNAPSHOT", "ASSET"]),
        ] {
            let lineage = service
                .production_lineage(PROJECT, root_type, root_id)
                .await
                .expect("lineage should load");
            for entity_type in expected {
                assert!(
                    lineage
                        .nodes
                        .iter()
                        .any(|node| node.entity_type == entity_type),
                    "missing {entity_type} in {root_type} lineage"
                );
            }
        }
        let batch_lineage = service
            .production_lineage(PROJECT, "BATCH", "batch-audit")
            .await
            .expect("batch lineage should load");
        assert!(batch_lineage
            .nodes
            .iter()
            .any(|node| node.id == "item-b2" && node.parent_id.as_deref() == Some("item-b")));
    }

    #[tokio::test]
    async fn healthy_and_corrupt_retry_fixtures_have_explicit_health() {
        let (_directory, pool) = fixture().await;
        let service = ProductionAuditService::new(pool.clone());
        assert_eq!(
            service.audit_integrity(PROJECT).await.unwrap().health,
            ProductionAuditHealth::Healthy
        );

        sqlx::query(
            "INSERT INTO production_batches
             (id, project_id, name, status, continue_on_failure, created_at, updated_at)
             VALUES ('batch-corrupt', ?, 'Corrupt batch', 'READY', 0, ?, ?)",
        )
        .bind(PROJECT)
        .bind(NOW)
        .bind(NOW)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO production_batch_items
             (id, batch_id, ordinal, workflow_version_id, recipe_id, values_json, status, retry_of_item_id, created_at, updated_at)
             VALUES ('item-orphan', 'batch-corrupt', 0, 'wv_audit', 'recipe_audit', '{}', 'FAILED', 'missing-parent', ?, ?)",
        )
        .bind(NOW)
        .bind(NOW)
        .execute(&pool)
        .await
        .unwrap();
        let integrity = service.audit_integrity(PROJECT).await.unwrap();
        assert_eq!(integrity.health, ProductionAuditHealth::Blocked);
        assert!(integrity
            .issues
            .iter()
            .any(|issue| issue.code == "PRODUCTION_RETRY_PARENT_MISSING"));

        let mut connection = pool.acquire().await.unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *connection)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO production_batch_items
             (id, batch_id, ordinal, workflow_version_id, recipe_id, values_json, status, task_id, created_at, updated_at)
             VALUES ('item-task-orphan', 'batch-corrupt', 1, 'wv_audit', 'recipe_audit', '{}', 'FAILED', 'missing-task', ?, ?)",
        )
        .bind(NOW)
        .bind(NOW)
        .execute(&mut *connection)
        .await
        .unwrap();
        drop(connection);
        let integrity = service.audit_integrity(PROJECT).await.unwrap();
        assert!(integrity
            .issues
            .iter()
            .any(|issue| issue.code == "PRODUCTION_BATCH_ITEM_TASK_MISSING"));
    }

    #[tokio::test]
    async fn five_hundred_shots_and_one_thousand_tasks_load_without_per_row_queries() {
        let (_directory, pool) = fixture().await;
        let mut transaction = pool.begin().await.unwrap();
        for index in 0..1000 {
            let id = format!("task-scale-{index:04}");
            sqlx::query(
                "INSERT INTO tasks
                 (id, project_id, workflow_id, workflow_version_id, recipe_id, status, created_at, finished_at)
                 VALUES (?, ?, 'wf_audit', 'wv_audit', 'recipe_audit', ?, ?, ?)",
            )
            .bind(id)
            .bind(PROJECT)
            .bind(if index % 2 == 0 { "SUCCEEDED" } else { "RUNNING" })
            .bind(NOW)
            .bind(if index % 2 == 0 { Some(NOW) } else { None::<&str> })
            .execute(&mut *transaction)
            .await
            .unwrap();
        }
        for index in 0..500 {
            sqlx::query(
                "INSERT INTO shots
                 (id, project_id, ordinal, name, prompt_text, created_at, updated_at)
                 VALUES (?, ?, ?, ?, 'prompt', ?, ?)",
            )
            .bind(format!("shot-scale-{index:04}"))
            .bind(PROJECT)
            .bind(index)
            .bind(format!("Shot {index}"))
            .bind(NOW)
            .bind(NOW)
            .execute(&mut *transaction)
            .await
            .unwrap();
        }
        transaction.commit().await.unwrap();

        let service = ProductionAuditService::new(pool);
        let summary = service.project_summary(PROJECT).await.unwrap();
        assert_eq!(summary.tasks, 1000);
        assert_eq!(summary.unassigned_shots, 500);
        assert_eq!(
            service.recent_activity(PROJECT, None).await.unwrap().len(),
            50
        );
    }
}
