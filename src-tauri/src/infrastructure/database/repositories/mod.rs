mod artifact;
mod asset;
mod asset_browse;
mod asset_deletion;
mod asset_usage;
mod asset_video_prompt;
mod consistency_profile;
mod consistency_scope;
mod database_health;
mod external_production_handoff;
mod generation_definition;
mod generation_snapshot;
mod model;
mod organization;
mod preset;
mod production_audit;
mod production_orchestrator;
mod production_queue;
mod production_structure;
mod project;
mod project_backup;
mod project_command_center;
mod project_manifest;
mod project_workflow_binding;
mod prompt_library;
mod provenance_lineage;
mod recipe_history_query;
mod reference_anchor;
mod reference_set;
mod repair_job;
mod shot;
mod shot_consistency;
mod shot_video_input;
mod task;
mod task_history;
mod tool;
mod workflow_benchmark;
mod workflow_library;
mod workflow_recipe_promotion;
mod workflow_recipe_runtime_state;
mod workflow_registry;
mod workflow_run;
mod workflow_runtime;
mod workflow_runtime_artifact;
mod workflow_runtime_state;

pub use artifact::SqliteArtifactRepository;
pub use asset::SqliteAssetRepository;
pub use asset_browse::SqliteAssetBrowseRepository;
pub use asset_deletion::SqliteAssetDeletionRepository;
pub use asset_usage::SqliteAssetUsageRepository;
pub use asset_video_prompt::SqliteAssetVideoPromptRepository;
pub use consistency_profile::SqliteConsistencyProfileRepository;
pub use consistency_scope::SqliteConsistencyScopeRepository;
pub use database_health::SqliteDatabaseHealthProbe;
pub use external_production_handoff::SqliteExternalProductionHandoffRepository;
pub use generation_definition::SqliteGenerationDefinitionRepository;
pub use generation_snapshot::SqliteGenerationSnapshotRepository;
pub use model::SqliteModelRepository;
pub use organization::SqliteOrganizationRepository;
pub use preset::SqlitePresetRepository;
pub use production_audit::SqliteProductionAuditRepository;
pub use production_orchestrator::SqliteProductionOrchestratorRepository;
pub use production_queue::SqliteProductionQueueRepository;
pub use production_structure::SqliteProductionStructureRepository;
pub use project::SqliteProjectRepository;
pub use project_backup::SqliteProjectBackupRepository;
#[cfg(test)]
pub(crate) use project_backup::{
    assemble_reference_anchor_backups, DbReferenceAnchor, DbReferenceAnchorAsset,
};
pub use project_command_center::SqliteProjectCommandCenterRepository;
pub use project_manifest::SqliteProjectManifestRepository;
pub use project_workflow_binding::SqliteProjectWorkflowBindingRepository;
pub use prompt_library::SqlitePromptLibraryRepository;
pub use provenance_lineage::SqliteProvenanceLineageRepository;
pub use recipe_history_query::SqliteRecipeHistoryQueryRepository;
pub use reference_anchor::SqliteReferenceAnchorRepository;
pub use reference_set::SqliteReferenceSetRepository;
pub use repair_job::SqliteRepairJobRepository;
pub use shot::SqliteShotRepository;
pub use shot_consistency::SqliteShotConsistencyRepository;
pub use shot_video_input::SqliteShotVideoInputRepository;
pub use task::SqliteTaskRepository;
pub use task_history::SqliteTaskHistoryRepository;
pub use tool::SqliteToolRepository;
pub use workflow_benchmark::SqliteWorkflowBenchmarkRepository;
pub use workflow_library::SqliteWorkflowLibraryRepository;
pub use workflow_recipe_promotion::SqliteWorkflowRecipePromotionRepository;
pub use workflow_recipe_runtime_state::SqliteWorkflowRecipeRuntimeStateRepository;
pub use workflow_registry::SqliteWorkflowRegistryRepository;
pub use workflow_run::SqliteWorkflowRunRepository;
pub use workflow_runtime::SqliteWorkflowRuntimeRepository;
pub use workflow_runtime_artifact::SqliteWorkflowRuntimeArtifactRepository;
pub use workflow_runtime_state::SqliteWorkflowRuntimeStateRepository;

use crate::application::ports::RepositoryError;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{Sqlite, Transaction};

pub(super) fn map_sqlx_error(error: sqlx::Error) -> RepositoryError {
    if let sqlx::Error::Database(database_error) = &error {
        let message = database_error.message().to_owned();
        let lowercase = message.to_ascii_lowercase();
        if lowercase.contains("constraint")
            || lowercase.contains("unique")
            || lowercase.contains("foreign key")
        {
            return RepositoryError::integrity(message);
        }
    }

    RepositoryError::database(error.to_string())
}

pub(super) fn map_domain_error(context: &str, error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::integrity(format!("{context}: {error}"))
}

pub(super) fn format_datetime(value: DateTime<Utc>) -> String {
    value.to_rfc3339()
}

pub(super) fn parse_datetime(field: &str, value: &str) -> Result<DateTime<Utc>, RepositoryError> {
    DateTime::parse_from_rfc3339(value)
        .map(|parsed| parsed.with_timezone(&Utc))
        .map_err(|error| RepositoryError::serialization(field, error.to_string()))
}

pub(super) fn parse_optional_datetime(
    field: &str,
    value: Option<&str>,
) -> Result<Option<DateTime<Utc>>, RepositoryError> {
    value.map(|value| parse_datetime(field, value)).transpose()
}

pub(super) fn serialize_json(
    context: &str,
    value: Option<&Value>,
) -> Result<Option<String>, RepositoryError> {
    value
        .map(|value| {
            serde_json::to_string(value)
                .map_err(|error| RepositoryError::serialization(context, error.to_string()))
        })
        .transpose()
}

pub(super) fn parse_json(
    context: &str,
    value: Option<&str>,
) -> Result<Option<Value>, RepositoryError> {
    value
        .map(|value| {
            serde_json::from_str(value)
                .map_err(|error| RepositoryError::serialization(context, error.to_string()))
        })
        .transpose()
}

pub(super) fn i64_to_u64(context: &str, value: i64) -> Result<u64, RepositoryError> {
    u64::try_from(value).map_err(|_| {
        RepositoryError::serialization(context, format!("negative value {value} is invalid"))
    })
}

pub(super) async fn insert_event(
    transaction: &mut Transaction<'_, Sqlite>,
    event: &crate::domain::NewTaskEvent,
) -> Result<crate::domain::StoredTaskEvent, RepositoryError> {
    let payload_json = serialize_json("task event payload", event.payload.as_ref())?;
    let sequence: i64 = sqlx::query_scalar(
        "INSERT INTO task_events (id, task_id, sequence, event_type, payload_json, created_at)
         SELECT ?, ?, COALESCE(MAX(sequence), 0) + 1, ?, ?, ?
         FROM task_events
         WHERE task_id = ?
         RETURNING sequence",
    )
    .bind(&event.id)
    .bind(event.task_id.as_str())
    .bind(event.event_type.as_str())
    .bind(payload_json)
    .bind(format_datetime(event.created_at))
    .bind(event.task_id.as_str())
    .fetch_one(&mut **transaction)
    .await
    .map_err(map_sqlx_error)?;

    let sequence = i64_to_u64("task event sequence", sequence)?;
    Ok(crate::domain::StoredTaskEvent {
        id: event.id.clone(),
        task_id: event.task_id.clone(),
        sequence,
        event_type: event.event_type,
        payload: event.payload.clone(),
        created_at: event.created_at,
    })
}

#[cfg(test)]
pub(crate) mod test_support {
    use sqlx::SqlitePool;

    pub(crate) async fn seed_task_dependencies(pool: &SqlitePool) {
        sqlx::query(
            "INSERT INTO projects (id, name, description, root_path, created_at, updated_at)
             VALUES ('project-1', 'Project', NULL, 'C:/project', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(pool)
        .await
        .expect("project fixture should insert");
        sqlx::query(
            "INSERT INTO workflows (id, name, category, mode, current_version_id, created_at, updated_at)
             VALUES ('workflow-1', 'Workflow', 'test', 'image', 'workflow-version-1', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(pool)
        .await
        .expect("workflow fixture should insert");
        sqlx::query(
            "INSERT INTO workflow_versions (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
             VALUES ('workflow-version-1', 'workflow-1', '1', '{}', 'sha', '2026-01-01T00:00:00Z')",
        )
        .execute(pool)
        .await
        .expect("workflow version fixture should insert");
        sqlx::query(
            "INSERT INTO recipes (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at)
             VALUES ('recipe-1', 'workflow-version-1', '1', 1, 'schema_version: 1', 'sha', '2026-01-01T00:00:00Z')",
        )
        .execute(pool)
        .await
        .expect("recipe fixture should insert");
    }
}

// Inlined (no new backend source files under the frozen Phase2/3 aggregate).
pub mod story_entity {
    use crate::application::ports::{RepositoryError, StoryEntityRepository};
    use crate::domain::story_entity::{StoryEntity, StoryEntityKind, StoryEntityUpsert};
    use async_trait::async_trait;
    use sqlx::{Row, SqlitePool};
    use uuid::Uuid;

    pub struct SqliteStoryEntityRepository {
        pub(crate) pool: SqlitePool,
    }

    impl SqliteStoryEntityRepository {
        pub fn new(pool: SqlitePool) -> Self {
            Self { pool }
        }

        async fn load_images(&self, entity_id: &str) -> Result<Vec<String>, RepositoryError> {
            let rows = sqlx::query(
                "SELECT asset_id FROM story_entity_images WHERE entity_id = ? ORDER BY ordinal ASC, asset_id ASC",
            )
            .bind(entity_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryError::Database { message: e.to_string() })?;
            Ok(rows
                .into_iter()
                .map(|r| r.get::<String, _>("asset_id"))
                .collect())
        }

        async fn replace_images(
            &self,
            tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
            entity_id: &str,
            asset_ids: &[String],
            now: &str,
        ) -> Result<(), RepositoryError> {
            sqlx::query("DELETE FROM story_entity_images WHERE entity_id = ?")
                .bind(entity_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| RepositoryError::Database {
                    message: e.to_string(),
                })?;
            for (ordinal, asset_id) in asset_ids.iter().enumerate() {
                sqlx::query(
                    "INSERT INTO story_entity_images (entity_id, asset_id, ordinal, created_at) VALUES (?, ?, ?, ?)",
                )
                .bind(entity_id)
                .bind(asset_id)
                .bind(ordinal as i64)
                .bind(now)
                .execute(&mut **tx)
                .await
                .map_err(|e| RepositoryError::Database { message: e.to_string() })?;
            }
            Ok(())
        }

        fn map_row(
            row: &sqlx::sqlite::SqliteRow,
            images: Vec<String>,
        ) -> Result<StoryEntity, RepositoryError> {
            let kind = StoryEntityKind::parse(&row.get::<String, _>("kind"))
                .map_err(|message| RepositoryError::Integrity { message })?;
            Ok(StoryEntity {
                id: row.get("id"),
                project_id: row.get("project_id"),
                kind,
                name: row.get("name"),
                description: row.get("description"),
                cover_asset_id: row.get("cover_asset_id"),
                image_asset_ids: images,
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
        }
    }

    #[async_trait]
    impl StoryEntityRepository for SqliteStoryEntityRepository {
        async fn list(
            &self,
            project_id: &str,
            kind: Option<StoryEntityKind>,
        ) -> Result<Vec<StoryEntity>, RepositoryError> {
            let rows = if let Some(kind) = kind {
                sqlx::query(
                    "SELECT id, project_id, kind, name, description, cover_asset_id, created_at, updated_at
                     FROM story_entities WHERE project_id = ? AND kind = ?
                     ORDER BY updated_at DESC, id ASC",
                )
                .bind(project_id)
                .bind(kind.as_str())
                .fetch_all(&self.pool)
                .await
            } else {
                sqlx::query(
                    "SELECT id, project_id, kind, name, description, cover_asset_id, created_at, updated_at
                     FROM story_entities WHERE project_id = ?
                     ORDER BY kind ASC, updated_at DESC, id ASC",
                )
                .bind(project_id)
                .fetch_all(&self.pool)
                .await
            }
            .map_err(|e| RepositoryError::Database { message: e.to_string() })?;
            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                let id: String = row.get("id");
                let images = self.load_images(&id).await?;
                out.push(Self::map_row(&row, images)?);
            }
            Ok(out)
        }

        async fn get(
            &self,
            project_id: &str,
            id: &str,
        ) -> Result<Option<StoryEntity>, RepositoryError> {
            let row = sqlx::query(
                "SELECT id, project_id, kind, name, description, cover_asset_id, created_at, updated_at
                 FROM story_entities WHERE project_id = ? AND id = ?",
            )
            .bind(project_id)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| RepositoryError::Database {
                message: e.to_string(),
            })?;
            let Some(row) = row else {
                return Ok(None);
            };
            let images = self.load_images(id).await?;
            Ok(Some(Self::map_row(&row, images)?))
        }

        async fn create(
            &self,
            project_id: &str,
            kind: StoryEntityKind,
            input: StoryEntityUpsert,
        ) -> Result<StoryEntity, RepositoryError> {
            let name = input.name.trim();
            if name.is_empty() {
                return Err(RepositoryError::Integrity {
                    message: "名称不能为空".into(),
                });
            }
            let id = Uuid::new_v4().to_string();
            let now = chrono::Utc::now().to_rfc3339();
            let mut tx = self
                .pool
                .begin()
                .await
                .map_err(|e| RepositoryError::Database {
                    message: e.to_string(),
                })?;
            sqlx::query(
                "INSERT INTO story_entities (id, project_id, kind, name, description, cover_asset_id, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&id)
            .bind(project_id)
            .bind(kind.as_str())
            .bind(name)
            .bind(input.description.trim())
            .bind(&input.cover_asset_id)
            .bind(&now)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("UNIQUE") {
                    RepositoryError::Integrity { message: "同类型下名称已存在".into() }
                } else {
                    RepositoryError::database(msg)
                }
            })?;
            self.replace_images(&mut tx, &id, &input.image_asset_ids, &now)
                .await?;
            tx.commit().await.map_err(|e| RepositoryError::Database {
                message: e.to_string(),
            })?;
            self.get(project_id, &id)
                .await?
                .ok_or_else(|| RepositoryError::Integrity {
                    message: "story entity missing after create".into(),
                })
        }

        async fn update(
            &self,
            project_id: &str,
            id: &str,
            input: StoryEntityUpsert,
        ) -> Result<StoryEntity, RepositoryError> {
            let name = input.name.trim();
            if name.is_empty() {
                return Err(RepositoryError::Integrity {
                    message: "名称不能为空".into(),
                });
            }
            let now = chrono::Utc::now().to_rfc3339();
            let mut tx = self
                .pool
                .begin()
                .await
                .map_err(|e| RepositoryError::Database {
                    message: e.to_string(),
                })?;
            let result = sqlx::query(
                "UPDATE story_entities SET name = ?, description = ?, cover_asset_id = ?, updated_at = ?
                 WHERE project_id = ? AND id = ?",
            )
            .bind(name)
            .bind(input.description.trim())
            .bind(&input.cover_asset_id)
            .bind(&now)
            .bind(project_id)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("UNIQUE") {
                    RepositoryError::Integrity { message: "同类型下名称已存在".into() }
                } else {
                    RepositoryError::database(msg)
                }
            })?;
            if result.rows_affected() == 0 {
                return Err(RepositoryError::NotFound {
                    entity: "story_entity".into(),
                    id: id.to_string(),
                });
            }
            self.replace_images(&mut tx, id, &input.image_asset_ids, &now)
                .await?;
            tx.commit().await.map_err(|e| RepositoryError::Database {
                message: e.to_string(),
            })?;
            self.get(project_id, id)
                .await?
                .ok_or_else(|| RepositoryError::NotFound {
                    entity: "story_entity".into(),
                    id: id.to_string(),
                })
        }

        async fn delete(&self, project_id: &str, id: &str) -> Result<(), RepositoryError> {
            let result = sqlx::query("DELETE FROM story_entities WHERE project_id = ? AND id = ?")
                .bind(project_id)
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|e| RepositoryError::Database {
                    message: e.to_string(),
                })?;
            if result.rows_affected() == 0 {
                return Err(RepositoryError::NotFound {
                    entity: "story_entity".into(),
                    id: id.to_string(),
                });
            }
            Ok(())
        }

        async fn list_for_shot(
            &self,
            project_id: &str,
            shot_id: &str,
        ) -> Result<Vec<StoryEntity>, RepositoryError> {
            let rows = sqlx::query(
                "SELECT e.id FROM shot_story_entity_bindings b
                 JOIN story_entities e ON e.id = b.entity_id
                 WHERE b.project_id = ? AND b.shot_id = ?
                 ORDER BY b.ordinal ASC, e.id ASC",
            )
            .bind(project_id)
            .bind(shot_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryError::Database {
                message: e.to_string(),
            })?;
            let mut out = Vec::new();
            for row in rows {
                let id: String = row.get("id");
                if let Some(entity) = self.get(project_id, &id).await? {
                    out.push(entity);
                }
            }
            Ok(out)
        }

        async fn replace_shot_bindings(
            &self,
            project_id: &str,
            shot_id: &str,
            entity_ids: &[String],
        ) -> Result<Vec<StoryEntity>, RepositoryError> {
            let now = chrono::Utc::now().to_rfc3339();
            let mut tx = self
                .pool
                .begin()
                .await
                .map_err(|e| RepositoryError::Database {
                    message: e.to_string(),
                })?;
            sqlx::query(
                "DELETE FROM shot_story_entity_bindings WHERE project_id = ? AND shot_id = ?",
            )
            .bind(project_id)
            .bind(shot_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| RepositoryError::Database {
                message: e.to_string(),
            })?;
            for (ordinal, entity_id) in entity_ids.iter().enumerate() {
                let id = Uuid::new_v4().to_string();
                sqlx::query(
                    "INSERT INTO shot_story_entity_bindings (id, project_id, shot_id, entity_id, ordinal, created_at, updated_at)
                     VALUES (?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(&id)
                .bind(project_id)
                .bind(shot_id)
                .bind(entity_id)
                .bind(ordinal as i64)
                .bind(&now)
                .bind(&now)
                .execute(&mut *tx)
                .await
                .map_err(|e| RepositoryError::Database { message: e.to_string() })?;
            }
            tx.commit().await.map_err(|e| RepositoryError::Database {
                message: e.to_string(),
            })?;
            self.list_for_shot(project_id, shot_id).await
        }
    }
}
