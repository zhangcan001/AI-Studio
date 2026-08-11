use super::{map_domain_error, map_sqlx_error, parse_json};
use crate::application::ports::{
    AssetDeletionReferences, AssetDeletionRepository, RepositoryError,
};
use crate::domain::{AssetId, ProductionBatchItemStatus, TaskId, TaskStatus};
use async_trait::async_trait;
use serde_json::Value;
use sqlx::{FromRow, SqlitePool};
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
pub struct SqliteAssetDeletionRepository {
    pool: SqlitePool,
}

impl SqliteAssetDeletionRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct QueueReferenceRow {
    item_id: String,
    status: String,
    values_json: String,
    task_id: Option<String>,
}

#[derive(FromRow)]
struct TaskReferenceRow {
    task_id: String,
    status: String,
    user_inputs_json: Option<String>,
    resolved_inputs_json: Option<String>,
}

#[derive(FromRow)]
struct SourceTaskRow {
    asset_id: String,
    task_id: String,
    status: String,
}

#[async_trait]
impl AssetDeletionRepository for SqliteAssetDeletionRepository {
    async fn references_for(
        &self,
        project_id: &str,
        asset_ids: &[AssetId],
    ) -> Result<Vec<AssetDeletionReferences>, RepositoryError> {
        let selected = asset_ids
            .iter()
            .map(|asset_id| asset_id.as_str().to_owned())
            .collect::<HashSet<_>>();
        let mut references = asset_ids
            .iter()
            .cloned()
            .map(|asset_id| {
                (
                    asset_id.as_str().to_owned(),
                    AssetDeletionReferences {
                        asset_id,
                        ..Default::default()
                    },
                )
            })
            .collect::<HashMap<_, _>>();

        let queue_rows = sqlx::query_as::<_, QueueReferenceRow>(
            "SELECT i.id AS item_id, i.status, i.values_json, i.task_id
             FROM production_batch_items i
             INNER JOIN production_batches b ON b.id = i.batch_id
             WHERE b.project_id = ?",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        for row in queue_rows {
            let values: Value = serde_json::from_str(&row.values_json).map_err(|error| {
                RepositoryError::serialization("production values_json", error.to_string())
            })?;
            let status = ProductionBatchItemStatus::parse(&row.status)
                .map_err(|error| map_domain_error("production batch item status", error))?;
            for asset_id in extract_asset_ids(&values) {
                if !selected.contains(&asset_id) {
                    continue;
                }
                let reference = references
                    .get_mut(&asset_id)
                    .expect("selected asset reference");
                if status.is_terminal() {
                    if let Some(task_id) = row.task_id.as_deref().and_then(parse_task_id) {
                        push_unique(&mut reference.historical_task_ids, task_id);
                    }
                } else if !reference.active_production_item_ids.contains(&row.item_id) {
                    reference
                        .active_production_item_ids
                        .push(row.item_id.clone());
                }
            }
        }

        let task_rows = sqlx::query_as::<_, TaskReferenceRow>(
            "SELECT t.id AS task_id, t.status,
                    s.user_inputs_json, s.resolved_inputs_json
             FROM tasks t
             LEFT JOIN generation_snapshots s ON s.task_id = t.id
             WHERE t.project_id = ?",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        for row in task_rows {
            let status = TaskStatus::try_from_db(&row.status)
                .map_err(|error| map_domain_error("task status", error))?;
            let mut task_assets = HashSet::new();
            if let Some(json) = row.user_inputs_json.as_deref() {
                let value =
                    parse_json("snapshot user_inputs_json", Some(json))?.ok_or_else(|| {
                        RepositoryError::serialization("snapshot user_inputs_json", "missing value")
                    })?;
                task_assets.extend(extract_asset_ids(&value));
            }
            if let Some(json) = row.resolved_inputs_json.as_deref() {
                let value =
                    parse_json("snapshot resolved_inputs_json", Some(json))?.ok_or_else(|| {
                        RepositoryError::serialization(
                            "snapshot resolved_inputs_json",
                            "missing value",
                        )
                    })?;
                task_assets.extend(extract_asset_ids(&value));
            }
            for asset_id in task_assets {
                if !selected.contains(&asset_id) {
                    continue;
                }
                let reference = references
                    .get_mut(&asset_id)
                    .expect("selected asset reference");
                let task_id = TaskId::parse(row.task_id.clone())
                    .map_err(|error| map_domain_error("task id", error))?;
                if status.is_terminal() {
                    push_unique(&mut reference.historical_task_ids, task_id);
                } else {
                    push_unique(&mut reference.active_task_ids, task_id);
                }
            }
        }

        let source_rows = sqlx::query_as::<_, SourceTaskRow>(
            "SELECT a.id AS asset_id, t.id AS task_id, t.status
             FROM assets a
             INNER JOIN tasks t ON t.id = a.source_task_id
             WHERE a.project_id = ?",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        for row in source_rows {
            if !selected.contains(&row.asset_id) {
                continue;
            }
            let status = TaskStatus::try_from_db(&row.status)
                .map_err(|error| map_domain_error("source task status", error))?;
            let task_id = TaskId::parse(row.task_id)
                .map_err(|error| map_domain_error("source task id", error))?;
            let reference = references
                .get_mut(&row.asset_id)
                .expect("selected asset reference");
            if status.is_terminal() {
                push_unique(&mut reference.historical_task_ids, task_id);
            } else {
                push_unique(&mut reference.active_task_ids, task_id);
            }
        }

        Ok(asset_ids
            .iter()
            .filter_map(|asset_id| references.remove(asset_id.as_str()))
            .collect())
    }
}

fn push_unique<T: PartialEq>(items: &mut Vec<T>, value: T) {
    if !items.contains(&value) {
        items.push(value);
    }
}

fn parse_task_id(value: &str) -> Option<TaskId> {
    TaskId::parse(value.to_owned()).ok()
}

fn extract_asset_ids(value: &Value) -> HashSet<String> {
    let mut output = HashSet::new();
    collect_asset_ids(value, &mut output);
    output
}

fn collect_asset_ids(value: &Value, output: &mut HashSet<String>) {
    match value {
        Value::Array(items) => items
            .iter()
            .for_each(|item| collect_asset_ids(item, output)),
        Value::Object(object) => {
            if let Some(kind) = object.get("type").and_then(Value::as_str) {
                let is_asset = matches!(
                    kind,
                    "image_asset"
                        | "video_asset"
                        | "audio_asset"
                        | "image_assets"
                        | "video_assets"
                        | "audio_assets"
                );
                if is_asset {
                    if let Some(asset_id) = object.get("assetId").and_then(Value::as_str) {
                        output.insert(asset_id.to_owned());
                    }
                    if let Some(asset_ids) = object.get("assetIds").and_then(Value::as_array) {
                        output.extend(
                            asset_ids
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::to_owned),
                        );
                    }
                }
            }
            object
                .values()
                .for_each(|item| collect_asset_ids(item, output));
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{extract_asset_ids, SqliteAssetDeletionRepository};
    use crate::application::ports::AssetDeletionRepository;
    use crate::domain::AssetId;
    use crate::infrastructure::database::{initialize, repositories::test_support};
    use chrono::Utc;
    use serde_json::json;
    use sqlx::SqlitePool;
    use tempfile::tempdir;

    #[test]
    fn extracts_single_and_plural_asset_input_values_recursively() {
        let ids = extract_asset_ids(&json!({
            "prompt": {"type": "image_asset", "assetId": "ast_one"},
            "references": {"type": "image_assets", "assetIds": ["ast_two", "ast_three"]},
            "nested": [{"type": "audio_asset", "assetId": "ast_audio"}]
        }));
        assert_eq!(ids.len(), 4);
        assert!(ids.contains("ast_one"));
        assert!(ids.contains("ast_three"));
    }

    async fn setup_queue() -> (tempfile::TempDir, SqlitePool) {
        let directory = tempdir().expect("temporary directory");
        let pool = initialize(&directory.path().join("app.db"))
            .await
            .expect("database should initialize");
        test_support::seed_task_dependencies(&pool).await;
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO production_batches
             (id, project_id, name, status, continue_on_failure, created_at, updated_at)
             VALUES ('pbt_delete_test', 'project-1', 'Delete Test', 'READY', 0, ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("batch fixture");
        sqlx::query(
            "INSERT INTO production_batch_items
             (id, batch_id, ordinal, workflow_version_id, recipe_id, values_json, status,
              task_id, error_code, error_message, created_at, updated_at)
             VALUES ('pbi_delete_test', 'pbt_delete_test', 0, 'workflow-version-1', 'recipe-1', ?, 'PENDING', NULL, NULL, NULL, ?, ?)",
        )
        .bind(serde_json::to_string(&json!({
            "image": {"type": "image_asset", "assetId": "ast_active"}
        }))
        .expect("values json"))
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("item fixture");
        (directory, pool)
    }

    #[tokio::test]
    async fn reports_non_terminal_production_item_as_active_reference() {
        let (_directory, pool) = setup_queue().await;
        let repository = SqliteAssetDeletionRepository::new(pool);
        let references = repository
            .references_for(
                "project-1",
                &[AssetId::parse("ast_active").expect("asset id")],
            )
            .await
            .expect("references should load");
        assert_eq!(references.len(), 1);
        assert_eq!(
            references[0].active_production_item_ids,
            vec!["pbi_delete_test"]
        );
        assert!(references[0].active_task_ids.is_empty());
    }
}
