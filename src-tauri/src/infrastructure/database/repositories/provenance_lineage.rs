use super::{
    format_datetime, map_domain_error, map_sqlx_error, parse_datetime, parse_json, serialize_json,
};
use crate::application::ports::{ProvenanceLineageRepository, RepositoryError};
use crate::domain::{
    AssetVersionId, GenerationAssetVersion, GenerationAssetVersionId,
    GenerationAssetVersionRelationType, GenerationToolUsage, GenerationToolUsageId, TaskId,
    ToolInstanceId, ToolVersionId,
};
use async_trait::async_trait;
use sqlx::{FromRow, SqlitePool};

#[derive(Clone)]
pub struct SqliteProvenanceLineageRepository {
    pool: SqlitePool,
}

impl SqliteProvenanceLineageRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProvenanceLineageRepository for SqliteProvenanceLineageRepository {
    async fn insert_tool_usage(&self, usage: &GenerationToolUsage) -> Result<(), RepositoryError> {
        usage
            .validate()
            .map_err(|error| map_domain_error("generation tool usage validation", error))?;
        let metadata_json =
            serialize_json("generation tool usage metadata", Some(&usage.metadata_json))?
                .ok_or_else(|| {
                    RepositoryError::serialization(
                        "generation tool usage metadata",
                        "missing value",
                    )
                })?;
        let tool_version_id = usage.tool_version_id.as_ref().map(ToolVersionId::as_str);
        let result = sqlx::query(
            "INSERT INTO generation_tool_usages
                (id, generation_id, tool_instance_id, tool_version_id, metadata_json, created_at)
             SELECT ?, ?, ?, ?, ?, ?
             WHERE EXISTS (
                 SELECT 1 FROM tasks WHERE id = ?
             )
               AND EXISTS (
                 SELECT 1 FROM tool_instances WHERE id = ?
             )
               AND (
                   ? IS NULL OR EXISTS (
                       SELECT 1 FROM tool_versions version
                       WHERE version.id = ?
                         AND version.tool_id = (
                             SELECT instance.tool_id
                             FROM tool_instances instance
                             WHERE instance.id = ?
                         )
                   )
               )",
        )
        .bind(usage.id.as_str())
        .bind(usage.generation_id.as_str())
        .bind(usage.tool_instance_id.as_str())
        .bind(tool_version_id)
        .bind(metadata_json)
        .bind(format_datetime(usage.created_at))
        .bind(usage.generation_id.as_str())
        .bind(usage.tool_instance_id.as_str())
        .bind(tool_version_id)
        .bind(tool_version_id)
        .bind(usage.tool_instance_id.as_str())
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        if result.rows_affected() != 1 {
            return Err(RepositoryError::not_found(
                "generation tool usage parent",
                usage.generation_id.as_str(),
            ));
        }
        Ok(())
    }

    async fn list_tool_usages(
        &self,
        project_id: &str,
        generation_id: &TaskId,
    ) -> Result<Vec<GenerationToolUsage>, RepositoryError> {
        let rows = sqlx::query_as::<_, GenerationToolUsageRow>(
            "SELECT usage.id, usage.generation_id, usage.tool_instance_id,
                    usage.tool_version_id, usage.metadata_json, usage.created_at
             FROM generation_tool_usages usage
             INNER JOIN tasks task ON task.id = usage.generation_id
             WHERE task.project_id = ? AND usage.generation_id = ?
             ORDER BY usage.created_at ASC, usage.id ASC",
        )
        .bind(project_id)
        .bind(generation_id.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(GenerationToolUsageRow::try_into_domain)
            .collect()
    }

    async fn insert_asset_version_link(
        &self,
        link: &GenerationAssetVersion,
    ) -> Result<(), RepositoryError> {
        link.validate()
            .map_err(|error| map_domain_error("generation asset version validation", error))?;
        let result = sqlx::query(
            "INSERT INTO generation_asset_versions
                (id, generation_id, output_id, ordinal, asset_version_id, relation_type, created_at)
             SELECT ?, ?, ?, ?, ?, ?, ?
             WHERE EXISTS (
                 SELECT 1
                 FROM tasks task
                 INNER JOIN task_output_assets output
                     ON output.task_id = task.id
                    AND output.output_id = ?
                    AND output.ordinal = ?
                 INNER JOIN asset_versions version
                     ON version.id = ?
                    AND version.project_id = task.project_id
                    AND version.asset_id = output.asset_id
                 WHERE task.id = ?
             )",
        )
        .bind(link.id.as_str())
        .bind(link.generation_id.as_str())
        .bind(&link.output_id)
        .bind(i64::from(link.ordinal))
        .bind(link.asset_version_id.as_str())
        .bind(link.relation_type.as_str())
        .bind(format_datetime(link.created_at))
        .bind(&link.output_id)
        .bind(i64::from(link.ordinal))
        .bind(link.asset_version_id.as_str())
        .bind(link.generation_id.as_str())
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        if result.rows_affected() != 1 {
            return Err(RepositoryError::not_found(
                "generation output and matching asset version",
                format!("{}:{}:{}", link.generation_id, link.output_id, link.ordinal),
            ));
        }
        Ok(())
    }

    async fn list_asset_version_links(
        &self,
        project_id: &str,
        generation_id: &TaskId,
    ) -> Result<Vec<GenerationAssetVersion>, RepositoryError> {
        let rows = sqlx::query_as::<_, GenerationAssetVersionRow>(
            "SELECT lineage.id, lineage.generation_id, lineage.output_id,
                    lineage.ordinal, lineage.asset_version_id,
                    lineage.relation_type, lineage.created_at
             FROM generation_asset_versions lineage
             INNER JOIN tasks task ON task.id = lineage.generation_id
             WHERE task.project_id = ? AND lineage.generation_id = ?
             ORDER BY lineage.output_id ASC, lineage.ordinal ASC, lineage.created_at ASC,
                      lineage.id ASC",
        )
        .bind(project_id)
        .bind(generation_id.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(GenerationAssetVersionRow::try_into_domain)
            .collect()
    }
}

#[derive(FromRow)]
struct GenerationToolUsageRow {
    id: String,
    generation_id: String,
    tool_instance_id: String,
    tool_version_id: Option<String>,
    metadata_json: String,
    created_at: String,
}

impl GenerationToolUsageRow {
    fn try_into_domain(self) -> Result<GenerationToolUsage, RepositoryError> {
        let metadata_json = parse_json(
            "generation_tool_usages metadata_json",
            Some(&self.metadata_json),
        )?
        .ok_or_else(|| {
            RepositoryError::serialization("generation_tool_usages metadata_json", "missing value")
        })?;
        GenerationToolUsage::new(
            GenerationToolUsageId::parse(self.id)
                .map_err(|error| map_domain_error("generation_tool_usages id", error))?,
            TaskId::parse(self.generation_id)
                .map_err(|error| map_domain_error("generation_tool_usages generation_id", error))?,
            ToolInstanceId::parse(self.tool_instance_id).map_err(|error| {
                map_domain_error("generation_tool_usages tool_instance_id", error)
            })?,
            self.tool_version_id
                .map(ToolVersionId::parse)
                .transpose()
                .map_err(|error| {
                    map_domain_error("generation_tool_usages tool_version_id", error)
                })?,
            metadata_json,
            parse_datetime("generation_tool_usages created_at", &self.created_at)?,
        )
        .map_err(|error| map_domain_error("generation_tool_usages integrity", error))
    }
}

#[derive(FromRow)]
struct GenerationAssetVersionRow {
    id: String,
    generation_id: String,
    output_id: String,
    ordinal: i64,
    asset_version_id: String,
    relation_type: String,
    created_at: String,
}

impl GenerationAssetVersionRow {
    fn try_into_domain(self) -> Result<GenerationAssetVersion, RepositoryError> {
        GenerationAssetVersion::new(
            GenerationAssetVersionId::parse(self.id)
                .map_err(|error| map_domain_error("generation_asset_versions id", error))?,
            TaskId::parse(self.generation_id).map_err(|error| {
                map_domain_error("generation_asset_versions generation_id", error)
            })?,
            self.output_id,
            u32::try_from(self.ordinal).map_err(|_| {
                RepositoryError::serialization(
                    "generation_asset_versions ordinal",
                    format!("invalid value {}", self.ordinal),
                )
            })?,
            AssetVersionId::parse(self.asset_version_id).map_err(|error| {
                map_domain_error("generation_asset_versions asset_version_id", error)
            })?,
            GenerationAssetVersionRelationType::try_from_db(&self.relation_type).map_err(
                |error| map_domain_error("generation_asset_versions relation_type", error),
            )?,
            parse_datetime("generation_asset_versions created_at", &self.created_at)?,
        )
        .map_err(|error| map_domain_error("generation_asset_versions integrity", error))
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteProvenanceLineageRepository;
    use crate::application::ports::{
        AssetRepository, ProvenanceLineageRepository, TaskOutputAssetMapping, TaskRepository,
    };
    use crate::domain::{
        Asset, AssetId, AssetVersion, AssetVersionId, GenerationAssetVersion,
        GenerationAssetVersionId, GenerationAssetVersionRelationType, GenerationToolUsage,
        GenerationToolUsageId, Task, TaskId, ToolInstanceId, ToolVersionId,
    };
    use crate::infrastructure::database::{
        initialize,
        repositories::{test_support, SqliteAssetRepository, SqliteTaskRepository},
    };
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use sqlx::SqlitePool;
    use tempfile::{tempdir, TempDir};

    async fn setup() -> (TempDir, SqlitePool, Task, SqliteProvenanceLineageRepository) {
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
            now(),
        );
        SqliteTaskRepository::new(pool.clone())
            .create(&task, &task.created_event())
            .await
            .expect("task fixture should persist");
        sqlx::query(
            "INSERT INTO tools (id, name, type, description, metadata_json, created_at)
             VALUES ('tool_test', 'Test Tool', 'image', '', '{}', ?)",
        )
        .bind(now().to_rfc3339())
        .execute(&pool)
        .await
        .expect("tool fixture should persist");
        sqlx::query(
            "INSERT INTO tool_instances (id, tool_id, path, endpoint, status, last_checked)
             VALUES ('tins_test', 'tool_test', NULL, NULL, 'UNKNOWN', NULL)",
        )
        .execute(&pool)
        .await
        .expect("tool instance fixture should persist");
        sqlx::query(
            "INSERT INTO tool_versions
             (id, tool_id, version, observed_at, metadata_json)
             VALUES ('tver_test', 'tool_test', '1.0', ?, '{}')",
        )
        .bind(now().to_rfc3339())
        .execute(&pool)
        .await
        .expect("tool version fixture should persist");
        (
            directory,
            pool.clone(),
            task,
            SqliteProvenanceLineageRepository::new(pool),
        )
    }

    fn now() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
    }

    fn usage(task_id: &TaskId) -> GenerationToolUsage {
        GenerationToolUsage::new(
            GenerationToolUsageId::parse("gtu_test").unwrap(),
            task_id.clone(),
            ToolInstanceId::parse("tins_test").unwrap(),
            Some(ToolVersionId::parse("tver_test").unwrap()),
            json!({"capture": "explicit"}),
            now(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn migration_creates_lineage_tables_and_preserves_old_task_rows() {
        let (_directory, pool, task, repository) = setup().await;
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN
                 ('generation_tool_usages', 'generation_asset_versions')",
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            2
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM tasks WHERE id = ?")
                .bind(task.id.as_str())
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
        assert!(repository
            .list_tool_usages("project-1", &task.id)
            .await
            .unwrap()
            .is_empty());
        assert!(repository
            .list_asset_version_links("project-1", &task.id)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn tool_usage_create_and_query_round_trips_explicit_context() {
        let (_directory, _pool, task, repository) = setup().await;
        let expected = usage(&task.id);
        repository
            .insert_tool_usage(&expected)
            .await
            .expect("tool usage should persist");
        let rows = repository
            .list_tool_usages("project-1", &task.id)
            .await
            .expect("tool usage should query");
        assert_eq!(rows, vec![expected]);
    }

    #[tokio::test]
    async fn asset_version_link_create_and_query_requires_existing_output_key() {
        let (_directory, pool, task, repository) = setup().await;
        let asset_repository = SqliteAssetRepository::new(pool.clone());
        let asset = Asset::new_image(
            AssetId::parse("ast_lineage").unwrap(),
            "project-1",
            "Lineage",
            "lineage.png",
            "C:/project/lineage.png",
            "lineage-sha",
            "image/png",
            2,
            2,
            64,
            task.id.clone(),
            json!({"source": "test"}),
            now(),
        )
        .unwrap();
        let mapping = TaskOutputAssetMapping {
            task_id: task.id.clone(),
            output_id: "output_0".to_owned(),
            ordinal: 0,
            asset_id: asset.id.clone(),
            created_at: now(),
        };
        asset_repository
            .insert_generated_outputs(std::slice::from_ref(&asset), std::slice::from_ref(&mapping))
            .await
            .expect("output fixture should persist");
        let version = AssetVersion::new(
            AssetVersionId::parse("av_lineage").unwrap(),
            "project-1",
            asset.id,
            1,
            json!({"version": 1}),
            "C:/project/lineage.png",
            "lineage-sha",
            now(),
        )
        .unwrap();
        asset_repository
            .insert_asset_version(&version)
            .await
            .expect("asset version fixture should persist");
        let expected = GenerationAssetVersion::new(
            GenerationAssetVersionId::parse("gav_test").unwrap(),
            task.id.clone(),
            "output_0",
            0,
            version.id,
            GenerationAssetVersionRelationType::Output,
            now(),
        )
        .unwrap();
        repository
            .insert_asset_version_link(&expected)
            .await
            .expect("asset version link should persist");
        assert_eq!(
            repository
                .list_asset_version_links("project-1", &task.id)
                .await
                .unwrap(),
            vec![expected]
        );

        let invalid = GenerationAssetVersion::new(
            GenerationAssetVersionId::parse("gav_missing").unwrap(),
            task.id.clone(),
            "missing_output",
            0,
            AssetVersionId::parse("av_lineage").unwrap(),
            GenerationAssetVersionRelationType::Output,
            now(),
        )
        .unwrap();
        assert!(repository
            .insert_asset_version_link(&invalid)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn project_scoped_queries_do_not_expose_another_project_generation() {
        let (_directory, _pool, task, repository) = setup().await;
        repository
            .insert_tool_usage(&usage(&task.id))
            .await
            .unwrap();
        assert!(repository
            .list_tool_usages("project-2", &task.id)
            .await
            .unwrap()
            .is_empty());
    }
}
