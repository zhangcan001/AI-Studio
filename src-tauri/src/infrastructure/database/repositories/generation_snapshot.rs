use super::{
    format_datetime, map_domain_error, map_sqlx_error, parse_datetime, parse_json, serialize_json,
};
use crate::application::ports::{GenerationSnapshotRepository, RepositoryError};
use crate::domain::{GenerationSnapshot, ModelVersionId, SnapshotId, TaskId};
use async_trait::async_trait;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct SqliteGenerationSnapshotRepository {
    pool: SqlitePool,
}

impl SqliteGenerationSnapshotRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl GenerationSnapshotRepository for SqliteGenerationSnapshotRepository {
    async fn insert(&self, snapshot: &GenerationSnapshot) -> Result<(), RepositoryError> {
        snapshot
            .validate()
            .map_err(|error| map_domain_error("generation snapshot validation", error))?;

        let workflow_json =
            serialize_json("snapshot workflow_json", Some(&snapshot.workflow_json))?.ok_or_else(
                || RepositoryError::serialization("snapshot workflow_json", "missing value"),
            )?;
        let user_inputs_json = serialize_json(
            "snapshot user_inputs_json",
            Some(&snapshot.user_inputs_json),
        )?
        .ok_or_else(|| {
            RepositoryError::serialization("snapshot user_inputs_json", "missing value")
        })?;
        let resolved_inputs_json = serialize_json(
            "snapshot resolved_inputs_json",
            Some(&snapshot.resolved_inputs_json),
        )?
        .ok_or_else(|| {
            RepositoryError::serialization("snapshot resolved_inputs_json", "missing value")
        })?;

        sqlx::query(
            "INSERT INTO generation_snapshots (
                id, task_id, workflow_json, recipe_yaml,
                user_inputs_json, resolved_inputs_json, model_version_id,
                prompt_version_id, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(snapshot.id.as_str())
        .bind(snapshot.task_id.as_str())
        .bind(workflow_json)
        .bind(&snapshot.recipe_yaml)
        .bind(user_inputs_json)
        .bind(resolved_inputs_json)
        .bind(
            snapshot
                .model_version_id
                .as_ref()
                .map(ModelVersionId::as_str),
        )
        .bind(&snapshot.prompt_version_id)
        .bind(format_datetime(snapshot.created_at))
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn find_by_task_id(
        &self,
        task_id: &TaskId,
    ) -> Result<Option<GenerationSnapshot>, RepositoryError> {
        let row = sqlx::query_as::<_, SnapshotRow>(
            "SELECT id, task_id, workflow_json, recipe_yaml,
                    user_inputs_json, resolved_inputs_json, model_version_id,
                    prompt_version_id, created_at
             FROM generation_snapshots WHERE task_id = ?",
        )
        .bind(task_id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(SnapshotRow::try_into_domain).transpose()
    }
}

#[derive(sqlx::FromRow)]
struct SnapshotRow {
    id: String,
    task_id: String,
    workflow_json: String,
    recipe_yaml: String,
    user_inputs_json: String,
    resolved_inputs_json: String,
    model_version_id: Option<String>,
    prompt_version_id: Option<String>,
    created_at: String,
}

impl SnapshotRow {
    fn try_into_domain(self) -> Result<GenerationSnapshot, RepositoryError> {
        let workflow_json = parse_json("snapshot workflow_json", Some(&self.workflow_json))?
            .ok_or_else(|| {
                RepositoryError::serialization("snapshot workflow_json", "missing value")
            })?;
        let user_inputs_json =
            parse_json("snapshot user_inputs_json", Some(&self.user_inputs_json))?.ok_or_else(
                || RepositoryError::serialization("snapshot user_inputs_json", "missing value"),
            )?;
        let resolved_inputs_json = parse_json(
            "snapshot resolved_inputs_json",
            Some(&self.resolved_inputs_json),
        )?
        .ok_or_else(|| {
            RepositoryError::serialization("snapshot resolved_inputs_json", "missing value")
        })?;

        let snapshot = GenerationSnapshot {
            id: SnapshotId::parse(self.id)
                .map_err(|error| map_domain_error("snapshot id", error))?,
            task_id: TaskId::parse(self.task_id)
                .map_err(|error| map_domain_error("snapshot task_id", error))?,
            workflow_json,
            recipe_yaml: self.recipe_yaml,
            user_inputs_json,
            resolved_inputs_json,
            model_version_id: self
                .model_version_id
                .map(|id| {
                    ModelVersionId::parse(id)
                        .map_err(|error| map_domain_error("snapshot model_version_id", error))
                })
                .transpose()?,
            prompt_version_id: self.prompt_version_id,
            created_at: parse_datetime("snapshot created_at", &self.created_at)?,
        };
        snapshot
            .validate()
            .map_err(|error| map_domain_error("snapshot integrity", error))?;
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteGenerationSnapshotRepository;
    use crate::application::ports::{GenerationSnapshotRepository, TaskRepository};
    use crate::domain::{GenerationSnapshot, ModelVersionId, Task};
    use crate::infrastructure::database::{
        initialize,
        repositories::{test_support, SqliteTaskRepository},
    };
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use sqlx::SqlitePool;
    use tempfile::{tempdir, TempDir};

    async fn setup() -> (
        TempDir,
        SqlitePool,
        Task,
        SqliteGenerationSnapshotRepository,
    ) {
        let directory = tempdir().expect("temporary directory should exist");
        let pool = initialize(&directory.path().join("app.db"))
            .await
            .expect("database should initialize");
        test_support::seed_task_dependencies(&pool).await;
        let task = Task::new(
            "project-1",
            "workflow-1",
            "workflow-version-1",
            "recipe-1",
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        );
        let task_repository = SqliteTaskRepository::new(pool.clone());
        task_repository
            .create(&task, &task.created_event())
            .await
            .expect("task fixture should insert");
        let snapshot_repository = SqliteGenerationSnapshotRepository::new(pool.clone());
        (directory, pool, task, snapshot_repository)
    }

    fn snapshot(task: &Task) -> GenerationSnapshot {
        GenerationSnapshot::new(
            task.id.clone(),
            json!({"3": {"inputs": {"seed": 123}, "class_type": "KSampler"}}),
            "schema_version: 1\nid: test",
            json!({"seed": "random", "prompt": "hello"}),
            json!({"seed": 123, "prompt": "hello"}),
            task.created_at,
        )
        .expect("snapshot should be valid")
    }

    #[tokio::test]
    async fn insert_and_read_snapshot_round_trip_all_json_fields() {
        let (_directory, _pool, task, repository) = setup().await;
        let snapshot = snapshot(&task);

        repository
            .insert(&snapshot)
            .await
            .expect("snapshot insert should succeed");
        let found = repository
            .find_by_task_id(&task.id)
            .await
            .expect("snapshot lookup should succeed")
            .expect("snapshot should exist");

        assert_eq!(found, snapshot);
        assert_eq!(found.user_inputs_json["seed"], "random");
        assert_eq!(found.resolved_inputs_json["seed"], 123);
    }

    #[tokio::test]
    async fn model_version_provenance_round_trips_with_snapshot() {
        let (_directory, pool, task, repository) = setup().await;
        sqlx::query(
            "INSERT INTO models
             (id, name, provider, type, description, metadata_json, created_at)
             VALUES ('mdl_snapshot', 'H3', 'MiniMax', 'video', '', '{}', ?)",
        )
        .bind(task.created_at.to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO model_versions
             (id, model_id, version, capabilities_json, parameter_schema_json, created_at)
             VALUES ('mdv_snapshot', 'mdl_snapshot', '2026-01', '[]', '{}', ?)",
        )
        .bind(task.created_at.to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
        let snapshot = GenerationSnapshot::new_with_provenance(
            task.id.clone(),
            json!({"3": {"inputs": {}, "class_type": "KSampler"}}),
            "schema_version: 1\nid: test",
            json!({"prompt": "hello"}),
            json!({"prompt": "hello"}),
            Some(ModelVersionId::parse("mdv_snapshot").unwrap()),
            Some("prv_snapshot".to_owned()),
            task.created_at,
        )
        .unwrap();
        repository.insert(&snapshot).await.unwrap();
        let found = repository.find_by_task_id(&task.id).await.unwrap().unwrap();
        assert_eq!(found.model_version_id, snapshot.model_version_id);
        assert_eq!(found.prompt_version_id, snapshot.prompt_version_id);
    }

    #[tokio::test]
    async fn duplicate_snapshot_for_task_is_rejected() {
        let (_directory, _pool, task, repository) = setup().await;
        let snapshot = snapshot(&task);

        repository.insert(&snapshot).await.unwrap();
        let second = GenerationSnapshot::new(
            task.id.clone(),
            snapshot.workflow_json.clone(),
            snapshot.recipe_yaml.clone(),
            snapshot.user_inputs_json.clone(),
            snapshot.resolved_inputs_json.clone(),
            snapshot.created_at,
        )
        .unwrap();

        assert!(repository.insert(&second).await.is_err());
    }

    #[tokio::test]
    async fn invalid_snapshot_json_is_not_replaced_with_empty_json() {
        let (_directory, pool, task, repository) = setup().await;
        let snapshot = snapshot(&task);
        repository.insert(&snapshot).await.unwrap();

        sqlx::query(
            "UPDATE generation_snapshots SET workflow_json = '{not-json' WHERE task_id = ?",
        )
        .bind(task.id.as_str())
        .execute(&pool)
        .await
        .expect("corrupt snapshot should be writable for test");

        assert!(repository.find_by_task_id(&task.id).await.is_err());
    }
}
