mod asset;
mod asset_browse;
mod asset_deletion;
mod asset_usage;
mod asset_video_prompt;
mod consistency_profile;
mod consistency_scope;
mod generation_definition;
mod generation_snapshot;
mod organization;
mod preset;
mod production_item_review;
mod production_queue;
mod production_structure;
mod project;
mod prompt_library;
mod reference_anchor;
mod reference_set;
mod script_draft;
mod script_source;
mod shot;
mod shot_consistency;
mod task;
mod task_history;
mod workflow_library;
mod workflow_run;
mod workflow_runtime;
mod workflow_runtime_state;

pub use asset::SqliteAssetRepository;
pub use asset_browse::SqliteAssetBrowseRepository;
pub use asset_deletion::SqliteAssetDeletionRepository;
pub use asset_usage::SqliteAssetUsageRepository;
pub use asset_video_prompt::SqliteAssetVideoPromptRepository;
pub use consistency_profile::SqliteConsistencyProfileRepository;
pub use consistency_scope::SqliteConsistencyScopeRepository;
pub use generation_definition::SqliteGenerationDefinitionRepository;
pub use generation_snapshot::SqliteGenerationSnapshotRepository;
pub use organization::SqliteOrganizationRepository;
pub use preset::SqlitePresetRepository;
pub use production_item_review::SqliteProductionItemReviewRepository;
pub use production_queue::SqliteProductionQueueRepository;
pub use production_structure::SqliteProductionStructureRepository;
pub use project::SqliteProjectRepository;
pub use prompt_library::SqlitePromptLibraryRepository;
pub use reference_anchor::SqliteReferenceAnchorRepository;
pub use reference_set::SqliteReferenceSetRepository;
pub use script_draft::SqliteScriptDraftRepository;
pub use script_source::SqliteScriptSourceRepository;
pub use shot::SqliteShotRepository;
pub use shot_consistency::SqliteShotConsistencyRepository;
pub use task::SqliteTaskRepository;
pub use task_history::SqliteTaskHistoryRepository;
pub use workflow_library::SqliteWorkflowLibraryRepository;
pub use workflow_run::SqliteWorkflowRunRepository;
pub use workflow_runtime::SqliteWorkflowRuntimeRepository;
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
