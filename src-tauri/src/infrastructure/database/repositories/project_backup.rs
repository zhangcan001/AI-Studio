use crate::application::ports::{
    ProjectBackupAssetSource, ProjectBackupRepository, ProjectBackupRepositorySource,
    ProjectBackupRestorePlan, ProjectBackupSnapshot, ProjectRecord, RepositoryError,
};
use crate::application::project_backup_service::*;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, Row, Sqlite, SqlitePool, Transaction};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

pub struct SqliteProjectBackupRepository {
    pool: SqlitePool,
}
impl SqliteProjectBackupRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl ProjectBackupRepositorySource for SqlitePool {
    fn into_repository(self) -> Arc<dyn ProjectBackupRepository> {
        Arc::new(SqliteProjectBackupRepository::new(self))
    }
}
#[async_trait]
impl ProjectBackupRepository for SqliteProjectBackupRepository {
    async fn load_export_snapshot(
        &self,
        project_id: &str,
    ) -> Result<ProjectBackupSnapshot, RepositoryError> {
        // Keep the metadata snapshot short: no filesystem reads or ZIP writes
        // happen while this SQLite read transaction is open.
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| RepositoryError::database(error.to_string()))?;
        let project = query_project(&mut transaction, project_id)
            .await?
            .ok_or_else(|| RepositoryError::not_found("project", project_id))?;
        let db_tasks = sqlx::query_as::<_, DbTask>(
            "SELECT id, project_id, workflow_id, workflow_version_id, recipe_id, status,
             app_version, build_commit, workflow_version, workflow_sha256, recipe_version,
             recipe_sha256, package_name, package_source_path, dynamic_binding_targets_json,
             generation_execution_id, compiled_workflow_sha256, runtime_profile,
             concurrency_class, prepare_started_at, prepared_at, submitted_at,
             execution_started_at, execution_finished_at, collection_finished_at,
             prompt_id, queue_number, progress_mode, progress_current, progress_total,
             current_node_id, error_code, error_message, raw_error_json, created_at,
             queued_at, started_at, finished_at FROM tasks WHERE project_id = ? ORDER BY created_at, id",
        )
        .bind(project_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
        let db_assets = sqlx::query_as::<_, DbAsset>(
            "SELECT id, project_id, type, category, name, original_name, storage_path,
             thumbnail_path, sha256, mime_type, width, height, duration_ms, file_size,
             source_task_id, metadata_json, created_at, updated_at FROM assets WHERE project_id = ? ORDER BY created_at, id",
        )
        .bind(project_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
        let active_ids = db_tasks
            .iter()
            .filter(|task| !is_terminal_task(&task.status))
            .map(|task| task.id.clone())
            .collect::<HashSet<_>>();
        let excluded_tasks = active_ids.clone();
        let included_task_ids = db_tasks
            .iter()
            .filter(|task| !excluded_tasks.contains(&task.id))
            .map(|task| task.id.clone())
            .collect::<HashSet<_>>();
        let included_asset_ids = db_assets
            .iter()
            .filter(|asset| {
                asset
                    .source_task_id
                    .as_ref()
                    .is_none_or(|task_id| !excluded_tasks.contains(task_id))
            })
            .map(|asset| asset.id.as_str())
            .collect::<HashSet<_>>();
        let tasks = db_tasks
            .into_iter()
            .filter(|task| included_task_ids.contains(&task.id))
            .map(BackupTask::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let task_events = query_task_events(&mut transaction, &included_task_ids).await?;
        let snapshots = query_snapshots(&mut transaction, &included_task_ids).await?;
        let mappings = query_mappings(&mut transaction, &included_task_ids).await?;
        let presets = query_presets(&mut transaction, project_id).await?;
        let prompt_entries = query_prompt_entries(&mut transaction, project_id).await?;
        let prompt_versions = query_prompt_versions(&mut transaction, project_id).await?;
        let batches = query_batches(&mut transaction, project_id).await?;
        let items = query_batch_items(&mut transaction, &batches).await?;
        let preparation_snapshots =
            query_production_preparation_snapshots(&mut transaction, project_id).await?;
        let benchmark_experiments =
            query_benchmark_experiments(&mut transaction, project_id).await?;
        let mut benchmark_candidates =
            query_benchmark_candidates(&mut transaction, project_id).await?;
        for candidate in &mut benchmark_candidates {
            if candidate
                .task_id
                .as_ref()
                .is_some_and(|task_id| !included_task_ids.contains(task_id))
            {
                candidate.task_id = None;
            }
        }
        let production_runs = query_production_runs(&mut transaction, project_id).await?;
        let production_stages = query_production_stages(&mut transaction, &production_runs).await?;
        let mut production_stage_items =
            query_production_stage_items(&mut transaction, &production_stages).await?;
        for item in &mut production_stage_items {
            if item
                .task_id
                .as_ref()
                .is_some_and(|task_id| !included_task_ids.contains(task_id))
            {
                item.task_id = None;
            }
            if item
                .asset_id
                .as_ref()
                .is_some_and(|asset_id| !included_asset_ids.contains(asset_id.as_str()))
            {
                item.asset_id = None;
            }
            if item
                .source_asset_id
                .as_ref()
                .is_some_and(|asset_id| !included_asset_ids.contains(asset_id.as_str()))
            {
                item.source_asset_id = None;
            }
        }
        let production_run_templates =
            query_production_run_templates(&mut transaction, project_id).await?;
        let mut benchmark_runs = query_benchmark_runs(&mut transaction, project_id).await?;
        for run in &mut benchmark_runs {
            if run
                .task_id
                .as_ref()
                .is_some_and(|task_id| !included_task_ids.contains(task_id))
            {
                run.task_id = None;
            }
            if run.snapshot_id.as_ref().is_some_and(|snapshot_id| {
                !snapshots.iter().any(|snapshot| snapshot.id == *snapshot_id)
            }) {
                run.snapshot_id = None;
            }
            if run
                .output_asset_id
                .as_ref()
                .is_some_and(|asset_id| !included_asset_ids.contains(asset_id.as_str()))
            {
                run.output_asset_id = None;
            }
        }
        let benchmark_quality_scores =
            query_benchmark_quality_scores(&mut transaction, project_id).await?;
        let mut production_item_reviews =
            query_production_item_reviews(&mut transaction, project_id).await?;
        let mut shots = query_shots(&mut transaction, project_id).await?;
        let mut shot_stage_configs = query_shot_stage_configs(&mut transaction).await?;
        let mut shot_stage_prompts = query_shot_stage_prompts(&mut transaction, project_id).await?;
        let mut shot_reference_assets = query_shot_reference_assets(&mut transaction).await?;
        let mut shot_generation_links = query_shot_generation_links(&mut transaction).await?;
        let asset_tags = sqlx::query_as::<_, BackupAssetTag>(
            "SELECT id, project_id, name, normalized_name, created_at, updated_at FROM asset_tags WHERE project_id = ? ORDER BY created_at, id",
        ).bind(project_id).fetch_all(&mut *transaction).await.map_err(|error| RepositoryError::database(error.to_string()))?;
        let mut asset_tag_links = sqlx::query_as::<_, BackupAssetTagLink>(
            "SELECT asset_id, tag_id, project_id, created_at FROM asset_tag_links WHERE project_id = ? ORDER BY created_at, asset_id, tag_id",
        ).bind(project_id).fetch_all(&mut *transaction).await.map_err(|error| RepositoryError::database(error.to_string()))?;
        let mut asset_favorites = sqlx::query_as::<_, BackupAssetFavorite>(
            "SELECT asset_id, project_id, created_at FROM asset_favorites WHERE project_id = ? ORDER BY created_at, asset_id",
        ).bind(project_id).fetch_all(&mut *transaction).await.map_err(|error| RepositoryError::database(error.to_string()))?;
        let mut asset_video_prompts = sqlx::query_as::<_, DbAssetVideoPrompt>(
            "SELECT asset_id, project_id, prompt_text, updated_at
             FROM asset_video_prompts WHERE project_id = ? ORDER BY asset_id",
        )
        .bind(project_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?
        .into_iter()
        .map(|row| BackupAssetVideoPrompt {
            asset_id: row.asset_id,
            project_id: row.project_id,
            prompt_text: row.prompt_text,
            updated_at: row.updated_at,
        })
        .collect::<Vec<_>>();
        let reference_anchor_rows = sqlx::query_as::<_, DbReferenceAnchor>(
            "SELECT id, project_id, kind, name, normalized_name, description, created_at, updated_at
             FROM reference_anchors WHERE project_id = ? ORDER BY created_at, id",
        )
        .bind(project_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
        let reference_anchor_asset_rows = sqlx::query_as::<_, DbReferenceAnchorAsset>(
            "SELECT m.anchor_id, m.asset_id, m.ordinal, m.created_at
             FROM reference_anchor_assets m
             JOIN reference_anchors a ON a.id = m.anchor_id
             WHERE a.project_id = ? ORDER BY m.anchor_id, m.ordinal, m.asset_id",
        )
        .bind(project_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
        let (production_series, production_episodes, production_scenes, shot_scene_assignments) =
            query_production_structure(&mut transaction, project_id).await?;
        let script_sources = query_script_sources(&mut transaction, project_id).await?;
        let script_draft_revisions =
            query_script_draft_revisions(&mut transaction, project_id).await?;
        let character_profiles = query_character_profiles(&mut transaction, project_id).await?;
        let scene_profiles = query_scene_profiles(&mut transaction, project_id).await?;
        let prop_profiles = query_prop_profiles(&mut transaction, project_id).await?;
        let style_profiles = query_style_profiles(&mut transaction, project_id).await?;
        let costume_variants = query_costume_variants(&mut transaction, project_id).await?;
        let profile_revisions = query_profile_revisions(&mut transaction, project_id).await?;
        let reference_sets = query_reference_sets(&mut transaction, project_id).await?;
        let reference_set_items = query_reference_set_items(&mut transaction, project_id).await?;
        let shot_profile_bindings =
            query_shot_profile_bindings(&mut transaction, project_id).await?;
        let shot_reference_set_bindings =
            query_shot_reference_set_bindings(&mut transaction, project_id).await?;
        let scope_profile_bindings =
            query_scope_profile_bindings(&mut transaction, project_id).await?;
        let scope_reference_set_bindings =
            query_scope_reference_set_bindings(&mut transaction, project_id).await?;
        let included_asset_ids = db_assets
            .iter()
            .filter(|asset| {
                asset
                    .source_task_id
                    .as_ref()
                    .is_none_or(|task_id| !excluded_tasks.contains(task_id))
            })
            .map(|asset| asset.id.as_str())
            .collect::<HashSet<_>>();
        let included_batch_item_ids = items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<HashSet<_>>();
        production_item_reviews.retain(|review| {
            included_batch_item_ids.contains(review.production_batch_item_id.as_str())
                && review
                    .task_id
                    .as_ref()
                    .is_none_or(|task_id| included_task_ids.contains(task_id))
        });
        let included_shot_ids = shots
            .iter()
            .map(|shot| shot.id.clone())
            .collect::<HashSet<_>>();
        for shot in &mut shots {
            if shot
                .selected_image_asset_id
                .as_ref()
                .is_some_and(|asset_id| !included_asset_ids.contains(asset_id.as_str()))
            {
                shot.selected_image_asset_id = None;
            }
            if shot
                .selected_video_asset_id
                .as_ref()
                .is_some_and(|asset_id| !included_asset_ids.contains(asset_id.as_str()))
            {
                shot.selected_video_asset_id = None;
            }
        }
        shot_stage_configs.retain(|config| included_shot_ids.contains(config.shot_id.as_str()));
        shot_stage_prompts.retain(|prompt| included_shot_ids.contains(prompt.shot_id.as_str()));
        shot_reference_assets.retain(|reference| {
            included_shot_ids.contains(reference.shot_id.as_str())
                && included_asset_ids.contains(reference.asset_id.as_str())
        });
        shot_generation_links.retain(|link| {
            included_shot_ids.contains(link.shot_id.as_str())
                && link
                    .task_id
                    .as_ref()
                    .is_none_or(|task_id| included_task_ids.contains(task_id))
                && link
                    .production_batch_item_id
                    .as_ref()
                    .is_none_or(|item_id| included_batch_item_ids.contains(item_id.as_str()))
        });
        let mut workflow_refs = collect_workflow_refs(&tasks);
        for reference in query_benchmark_workflow_refs(&mut transaction, project_id).await? {
            if !workflow_refs.iter().any(|item| {
                item.workflow_version_id == reference.workflow_version_id
                    && item.recipe_id == reference.recipe_id
            }) {
                workflow_refs.push(reference);
            }
        }
        for config in &shot_stage_configs {
            let reference = WorkflowReference {
                workflow_id: config.workflow_id.clone(),
                workflow_version_id: config.workflow_version_id.clone(),
                recipe_id: config.recipe_id.clone(),
            };
            if !workflow_refs.iter().any(|item| {
                item.workflow_version_id == reference.workflow_version_id
                    && item.recipe_id == reference.recipe_id
            }) {
                workflow_refs.push(reference);
            }
        }
        let project_workflow_bindings =
            query_project_workflow_bindings(&mut transaction, project_id).await?;
        let workflow_registry = query_workflow_registry_snapshot(
            &mut transaction,
            &workflow_refs,
            &project_workflow_bindings,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(|error| RepositoryError::database(error.to_string()))?;

        // Only after the short metadata transaction commits do we inspect and
        // stream potentially large asset files.

        let asset_sources = db_assets
            .into_iter()
            .filter(|asset| {
                asset
                    .source_task_id
                    .as_ref()
                    .is_none_or(|task_id| !excluded_tasks.contains(task_id))
            })
            .map(|asset| ProjectBackupAssetSource {
                id: asset.id,
                asset_type: asset.r#type,
                category: asset.category,
                name: asset.name,
                original_name: asset.original_name,
                sha256: asset.sha256,
                mime_type: asset.mime_type,
                width: asset.width,
                height: asset.height,
                duration_ms: asset.duration_ms,
                file_size: asset.file_size,
                source_task_id: asset.source_task_id,
                metadata_json: asset.metadata_json,
                created_at: asset.created_at,
                updated_at: asset.updated_at,
                storage_path: asset.storage_path,
                thumbnail_path: asset.thumbnail_path,
            })
            .collect::<Vec<_>>();
        let included_asset_ids = asset_sources
            .iter()
            .map(|asset| asset.id.clone())
            .collect::<HashSet<_>>();
        asset_tag_links.retain(|link| included_asset_ids.contains(link.asset_id.as_str()));
        asset_favorites.retain(|favorite| included_asset_ids.contains(favorite.asset_id.as_str()));
        asset_video_prompts.retain(|prompt| included_asset_ids.contains(prompt.asset_id.as_str()));
        let reference_anchors = assemble_reference_anchor_backups(
            reference_anchor_rows,
            reference_anchor_asset_rows,
            &included_asset_ids,
        );
        let shot_scene_assignments = shot_scene_assignments
            .into_iter()
            .filter(|assignment| included_shot_ids.contains(assignment.shot_id.as_str()))
            .collect::<Vec<_>>();
        let document = BackupDocument {
            project: BackupProject {
                id: project.id,
                name: project.name,
            },
            description: project.description,
            created_at: project.created_at.to_rfc3339(),
            updated_at: project.updated_at.to_rfc3339(),
            active_tasks_excluded: active_ids.len(),
            incomplete_tasks_excluded: 0,
            tasks,
            task_events,
            assets: Vec::new(),
            mappings,
            snapshots,
            presets,
            prompt_entries,
            prompt_versions,
            batches,
            items,
            preparation_snapshots,
            workflow_refs,
            project_workflow_bindings,
            workflow_registry: Some(workflow_registry),
            asset_tags,
            asset_tag_links,
            asset_favorites,
            asset_video_prompts,
            reference_anchors,
            production_series,
            production_episodes,
            production_scenes,
            shot_scene_assignments,
            script_sources,
            script_draft_revisions,
            production_item_reviews,
            benchmark_experiments,
            benchmark_candidates,
            production_runs,
            production_stages,
            production_stage_items,
            production_run_templates,
            benchmark_runs,
            benchmark_quality_scores,
            shots,
            shot_stage_configs,
            shot_stage_prompts,
            shot_reference_assets,
            shot_generation_links,
            character_profiles,
            scene_profiles,
            prop_profiles,
            style_profiles,
            costume_variants,
            profile_revisions,
            reference_sets,
            reference_set_items,
            shot_profile_bindings,
            shot_reference_set_bindings,
            scope_profile_bindings,
            scope_reference_set_bindings,
        };
        Ok(ProjectBackupSnapshot {
            document,
            assets: asset_sources,
        })
    }

    async fn find_missing_workflows(
        &self,
        document: &BackupDocument,
    ) -> Result<Vec<String>, RepositoryError> {
        let mut missing = Vec::new();
        for reference in &document.workflow_refs {
            let exists =
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflows WHERE id = ?")
                    .bind(&reference.workflow_id)
                    .fetch_one(&self.pool)
                    .await
                    .map_err(|error| RepositoryError::database(error.to_string()))?
                    > 0;
            if !exists && !missing.contains(&reference.workflow_id) {
                missing.push(reference.workflow_id.clone());
            }
        }
        for binding in &document.project_workflow_bindings {
            if let Some(workflow_id) = &binding.workflow_id {
                let exists =
                    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflows WHERE id = ?")
                        .bind(workflow_id)
                        .fetch_one(&self.pool)
                        .await
                        .map_err(|error| RepositoryError::database(error.to_string()))?
                        > 0;
                if !exists && !missing.contains(workflow_id) {
                    missing.push(workflow_id.clone());
                }
            }
            let version_exists =
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflow_versions WHERE id = ?")
                    .bind(&binding.workflow_version_id)
                    .fetch_one(&self.pool)
                    .await
                    .map_err(|error| RepositoryError::database(error.to_string()))?
                    > 0;
            if !version_exists && !missing.contains(&binding.workflow_version_id) {
                missing.push(binding.workflow_version_id.clone());
            }
            let recipe_exists = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM recipes WHERE id = ? AND workflow_version_id = ?",
            )
            .bind(&binding.recipe_id)
            .bind(&binding.workflow_version_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|error| RepositoryError::database(error.to_string()))?
                > 0;
            if !recipe_exists && !missing.contains(&binding.recipe_id) {
                missing.push(binding.recipe_id.clone());
            }
        }
        Ok(missing)
    }

    async fn restore_atomic(&self, plan: ProjectBackupRestorePlan) -> Result<(), RepositoryError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| RepositoryError::database(error.to_string()))?;
        let result = restore_rows_in_transaction(
            &mut transaction,
            &plan.project,
            &plan.document,
            &plan.task_ids,
            &plan.asset_ids,
            &plan.snapshot_ids,
            &plan.preset_ids,
            &plan.prompt_ids,
            &plan.prompt_version_ids,
            &plan.batch_ids,
            &plan.item_ids,
            &plan.preparation_snapshot_ids,
            &plan.benchmark_experiment_ids,
            &plan.benchmark_candidate_ids,
            &plan.production_run_ids,
            &plan.production_stage_ids,
            &plan.production_stage_item_ids,
            &plan.production_run_template_ids,
            &plan.benchmark_run_ids,
            &plan.benchmark_quality_score_ids,
            &plan.tag_ids,
            &plan.reference_anchor_ids,
            &plan.production_structure_ids,
            &plan.script_source_ids,
            &plan.script_draft_ids,
            &plan.script_revision_ids,
            &plan.consistency_ids,
            &plan.shot_ids,
            &plan.shot_generation_link_ids,
            &plan.restored_assets,
            &plan.restored_snapshots,
        )
        .await;
        match result {
            Ok(()) => transaction
                .commit()
                .await
                .map_err(|error| RepositoryError::database(error.to_string())),
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error)
            }
        }
    }
}

#[derive(FromRow)]
struct DbProject {
    id: String,
    name: String,
    description: Option<String>,
    root_path: String,
    created_at: String,
    updated_at: String,
}

#[derive(FromRow)]
struct DbTask {
    id: String,
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
    generation_execution_id: Option<String>,
    compiled_workflow_sha256: Option<String>,
    runtime_profile: Option<String>,
    concurrency_class: Option<String>,
    prepare_started_at: Option<String>,
    prepared_at: Option<String>,
    submitted_at: Option<String>,
    execution_started_at: Option<String>,
    execution_finished_at: Option<String>,
    collection_finished_at: Option<String>,
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

impl TryFrom<DbTask> for BackupTask {
    type Error = RepositoryError;

    fn try_from(task: DbTask) -> Result<Self, Self::Error> {
        Ok(Self {
            id: task.id,
            workflow_id: task.workflow_id,
            workflow_version_id: task.workflow_version_id,
            recipe_id: task.recipe_id,
            app_version: task.app_version,
            build_commit: task.build_commit,
            workflow_version: task.workflow_version,
            workflow_sha256: task.workflow_sha256,
            recipe_version: task.recipe_version,
            recipe_sha256: task.recipe_sha256,
            package_name: task.package_name,
            package_source_path: task.package_source_path,
            dynamic_binding_targets: parse_optional_value(
                task.dynamic_binding_targets_json.as_deref(),
                "task dynamic binding targets",
            )?,
            generation_execution_id: task.generation_execution_id,
            compiled_workflow_sha256: task.compiled_workflow_sha256,
            runtime_profile: task.runtime_profile,
            concurrency_class: task.concurrency_class,
            prepare_started_at: task.prepare_started_at,
            prepared_at: task.prepared_at,
            submitted_at: task.submitted_at,
            execution_started_at: task.execution_started_at,
            execution_finished_at: task.execution_finished_at,
            collection_finished_at: task.collection_finished_at,
            status: task.status,
            prompt_id: task.prompt_id,
            queue_number: task.queue_number,
            progress_mode: task.progress_mode,
            progress_current: task.progress_current,
            progress_total: task.progress_total,
            current_node_id: task.current_node_id,
            error_code: task.error_code,
            error_message: task.error_message,
            raw_error: parse_optional_value(task.raw_error_json.as_deref(), "task error")?,
            created_at: task.created_at,
            queued_at: task.queued_at,
            started_at: task.started_at,
            finished_at: task.finished_at,
        })
    }
}

#[derive(FromRow)]
struct DbAsset {
    id: String,
    r#type: String,
    category: Option<String>,
    name: String,
    original_name: Option<String>,
    storage_path: String,
    thumbnail_path: Option<String>,
    sha256: String,
    mime_type: Option<String>,
    width: Option<i64>,
    height: Option<i64>,
    duration_ms: Option<i64>,
    file_size: Option<i64>,
    source_task_id: Option<String>,
    metadata_json: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(FromRow)]
struct DbAssetVideoPrompt {
    asset_id: String,
    project_id: String,
    prompt_text: String,
    updated_at: String,
}

#[derive(FromRow)]
pub(crate) struct DbReferenceAnchor {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) kind: String,
    pub(crate) name: String,
    pub(crate) normalized_name: String,
    pub(crate) description: String,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(FromRow)]
pub(crate) struct DbReferenceAnchorAsset {
    pub(crate) anchor_id: String,
    pub(crate) asset_id: String,
    pub(crate) ordinal: i64,
    pub(crate) created_at: String,
}

#[derive(FromRow)]
struct DbProductionSeries {
    id: String,
    project_id: String,
    ordinal: i64,
    name: String,
    description: String,
    created_at: String,
    updated_at: String,
}

#[derive(FromRow)]
struct DbProductionEpisode {
    id: String,
    series_id: String,
    ordinal: i64,
    name: String,
    description: String,
    created_at: String,
    updated_at: String,
}

#[derive(FromRow)]
struct DbProductionScene {
    id: String,
    episode_id: String,
    ordinal: i64,
    name: String,
    description: String,
    created_at: String,
    updated_at: String,
}

#[derive(FromRow)]
struct DbShotSceneAssignment {
    shot_id: String,
    scene_id: String,
    ordinal: i64,
    created_at: String,
    updated_at: String,
}

#[derive(FromRow)]
struct DbTaskEvent {
    id: String,
    task_id: String,
    sequence: i64,
    event_type: String,
    payload_json: Option<String>,
    created_at: String,
}

#[derive(FromRow)]
struct DbSnapshot {
    id: String,
    task_id: String,
    workflow_json: String,
    recipe_yaml: String,
    user_inputs_json: String,
    resolved_inputs_json: String,
    created_at: String,
}

#[derive(FromRow)]
struct DbPreset {
    id: String,
    workflow_version_id: String,
    recipe_id: String,
    name: String,
    values_json: String,
    created_at: String,
    updated_at: String,
}

#[derive(FromRow)]
struct DbPromptEntry {
    id: String,
    project_id: String,
    kind: String,
    name: String,
    normalized_name: String,
    tags_json: String,
    created_at: String,
    updated_at: String,
}

#[derive(FromRow)]
struct DbPromptVersion {
    id: String,
    prompt_id: String,
    version: i64,
    text: String,
    created_at: String,
}

#[derive(FromRow)]
struct DbBatch {
    id: String,
    name: String,
    status: String,
    continue_on_failure: i64,
    archived_at: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(FromRow)]
struct DbBatchItem {
    id: String,
    batch_id: String,
    ordinal: i64,
    workflow_version_id: String,
    recipe_id: String,
    values_json: String,
    status: String,
    task_id: Option<String>,
    retry_of_item_id: Option<String>,
    error_code: Option<String>,
    error_message: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(FromRow)]
struct DbProductionPreparationSnapshot {
    id: String,
    project_id: String,
    shot_id: String,
    stage: String,
    context_hash: String,
    production_batch_id: String,
    production_batch_item_id: String,
    snapshot_json: String,
    created_at: String,
}

#[derive(FromRow)]
struct DbBenchmarkExperiment {
    id: String,
    name: String,
    media_type: String,
    status: String,
    base_values_json: String,
    asset_ids_json: String,
    winner_candidate_id: Option<String>,
    production_batch_id: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(FromRow)]
struct DbBenchmarkCandidate {
    id: String,
    experiment_id: String,
    position: i64,
    workflow_version_id: String,
    recipe_id: String,
    preset_id: Option<String>,
    preset_name: Option<String>,
    label: String,
    values_json: String,
    asset_ids_json: String,
    production_batch_item_id: Option<String>,
    task_id: Option<String>,
    created_at: String,
}

#[derive(FromRow)]
struct DbProductionRun {
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

#[derive(FromRow)]
struct DbProductionStage {
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
    created_at: String,
    updated_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
}

#[derive(FromRow)]
struct DbProductionStageItem {
    id: String,
    stage_id: String,
    ordinal: i64,
    status: String,
    production_batch_item_id: Option<String>,
    task_id: Option<String>,
    asset_id: Option<String>,
    source_asset_id: Option<String>,
    reference_index: Option<i64>,
    attempt: i64,
    submission_idempotency_key: Option<String>,
    parent_stage_item_id: Option<String>,
    frozen_values_json: String,
    error_code: Option<String>,
    error_message: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(FromRow)]
struct DbProductionRunTemplate {
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

#[derive(FromRow)]
struct DbBenchmarkRun {
    id: String,
    experiment_id: String,
    candidate_id: String,
    run_number: i64,
    production_batch_item_id: Option<String>,
    task_id: Option<String>,
    snapshot_id: Option<String>,
    output_asset_id: Option<String>,
    generation_execution_id: Option<String>,
    compiled_workflow_sha256: Option<String>,
    runtime_profile: Option<String>,
    concurrency_class: Option<String>,
    queue_wait_ms: Option<i64>,
    prepare_ms: Option<i64>,
    submit_ms: Option<i64>,
    comfy_execution_ms: Option<i64>,
    collect_ms: Option<i64>,
    total_ms: Option<i64>,
    status: Option<String>,
    error_code: Option<String>,
    output_file_size: Option<i64>,
    created_at: String,
    updated_at: String,
}

#[derive(FromRow)]
struct DbBenchmarkQualityScore {
    id: String,
    candidate_id: String,
    prompt_adherence: Option<i64>,
    visual_quality: Option<i64>,
    motion_quality: Option<i64>,
    reference_consistency: Option<i64>,
    overall: Option<i64>,
    note: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(FromRow)]
struct DbBenchmarkWorkflowRef {
    workflow_id: String,
    workflow_version_id: String,
    recipe_id: String,
}

#[derive(FromRow)]
struct DbProductionItemReview {
    id: String,
    project_id: String,
    production_batch_id: String,
    production_batch_item_id: String,
    task_id: Option<String>,
    result_asset_id: Option<String>,
    review_status: String,
    review_note: String,
    version: i64,
    lineage_key: String,
    parent_batch_id: Option<String>,
    parent_item_id: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(FromRow)]
struct DbMapping {
    task_id: String,
    output_id: String,
    ordinal: i64,
    asset_id: String,
    created_at: String,
}

pub(crate) fn assemble_reference_anchor_backups(
    anchors: Vec<DbReferenceAnchor>,
    memberships: Vec<DbReferenceAnchorAsset>,
    included_asset_ids: &HashSet<String>,
) -> Vec<BackupReferenceAnchor> {
    let mut assets_by_anchor = HashMap::<String, Vec<BackupReferenceAnchorAsset>>::new();
    for membership in memberships {
        if included_asset_ids.contains(&membership.asset_id) {
            assets_by_anchor
                .entry(membership.anchor_id)
                .or_default()
                .push(BackupReferenceAnchorAsset {
                    asset_id: membership.asset_id,
                    ordinal: membership.ordinal,
                    created_at: membership.created_at,
                });
        }
    }
    for assets in assets_by_anchor.values_mut() {
        assets.sort_by(|left, right| {
            left.ordinal
                .cmp(&right.ordinal)
                .then_with(|| left.asset_id.cmp(&right.asset_id))
        });
    }
    anchors
        .into_iter()
        .map(|anchor| BackupReferenceAnchor {
            assets: assets_by_anchor.remove(&anchor.id).unwrap_or_default(),
            id: anchor.id,
            project_id: anchor.project_id,
            kind: anchor.kind,
            name: anchor.name,
            normalized_name: anchor.normalized_name,
            description: anchor.description,
            created_at: anchor.created_at,
            updated_at: anchor.updated_at,
        })
        .collect()
}

fn is_terminal_task(status: &str) -> bool {
    matches!(status, "SUCCEEDED" | "FAILED" | "CANCELLED")
}
fn parse_value(value: Option<&str>, label: &str) -> Result<Value, RepositoryError> {
    value
        .ok_or_else(|| RepositoryError::integrity(format!("{label} 缺失")))
        .and_then(|value| {
            serde_json::from_str(value)
                .map_err(|error| RepositoryError::integrity(format!("{label} JSON 无效：{error}")))
        })
}
fn parse_optional_value(
    value: Option<&str>,
    label: &str,
) -> Result<Option<Value>, RepositoryError> {
    value
        .map(|value| parse_value(Some(value), label))
        .transpose()
}
fn parse_string_array(value: Option<&str>, label: &str) -> Result<Vec<String>, RepositoryError> {
    let value = parse_value(value, label)?;
    let Some(values) = value.as_array() else {
        return Err(RepositoryError::integrity(format!(
            "{label} 必须是字符串数组"
        )));
    };
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| RepositoryError::integrity(format!("{label} 必须只包含字符串")))
        })
        .collect()
}
fn remap_snapshot_asset_references(value: &mut Value, asset_ids: &HashMap<String, String>) {
    match value {
        Value::Array(values) => values
            .iter_mut()
            .for_each(|value| remap_snapshot_asset_references(value, asset_ids)),
        Value::Object(values) => values
            .values_mut()
            .for_each(|value| remap_snapshot_asset_references(value, asset_ids)),
        Value::String(value) => {
            if let Some(remapped) = asset_ids.get(value) {
                *value = remapped.clone();
            }
        }
        Value::Bool(_) | Value::Number(_) | Value::Null => {}
    }
}
fn collect_exact_asset_id_references(value: &Value, known: &HashSet<String>) -> Vec<String> {
    match value {
        Value::Array(values) => values
            .iter()
            .flat_map(|value| collect_exact_asset_id_references(value, known))
            .collect(),
        Value::Object(values) => values
            .values()
            .flat_map(|value| collect_exact_asset_id_references(value, known))
            .collect(),
        Value::String(value) if known.contains(value) => vec![value.clone()],
        Value::Bool(_) | Value::Number(_) | Value::Null | Value::String(_) => Vec::new(),
    }
}
fn remap_exact_string_ids(value: &mut Value, ids: &HashMap<String, String>) {
    match value {
        Value::String(text) => {
            if let Some(restored) = ids.get(text) {
                *text = restored.clone();
            }
        }
        Value::Array(values) => values
            .iter_mut()
            .for_each(|value| remap_exact_string_ids(value, ids)),
        Value::Object(values) => values
            .values_mut()
            .for_each(|value| remap_exact_string_ids(value, ids)),
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}
fn consistency_required_id<'a>(
    ids: &'a HashMap<String, String>,
    old_id: &str,
    label: &str,
) -> Result<&'a str, RepositoryError> {
    ids.get(old_id)
        .map(String::as_str)
        .ok_or_else(|| RepositoryError::integrity(format!("{} ID 映射缺失", label)))
}
fn consistency_optional_id(
    ids: &HashMap<String, String>,
    old_id: Option<&String>,
) -> Option<String> {
    old_id.map(|id| ids.get(id).cloned().unwrap_or_else(|| id.clone()))
}
fn remap_consistency_scope_id(
    scope_type: &str,
    scope_id: &str,
    source_project_id: &str,
    restored_project_id: &str,
    structure_ids: &ProductionStructureIds,
) -> Result<String, RepositoryError> {
    match scope_type {
        "PROJECT" if scope_id == source_project_id => Ok(restored_project_id.to_owned()),
        "PROJECT" => Err(RepositoryError::integrity("一致性 Scope 项目 ID 不匹配")),
        "SERIES" => consistency_required_id(&structure_ids.series, scope_id, "Scope Series")
            .map(str::to_owned),
        "EPISODE" => consistency_required_id(&structure_ids.episodes, scope_id, "Scope Episode")
            .map(str::to_owned),
        "SCENE" => consistency_required_id(&structure_ids.scenes, scope_id, "Scope Scene")
            .map(str::to_owned),
        _ => Err(RepositoryError::integrity("一致性 Scope 类型无效")),
    }
}
fn remap_reference_anchor_assets(
    anchor: &BackupReferenceAnchor,
    asset_ids: &HashMap<String, String>,
) -> Result<Vec<BackupReferenceAnchorAsset>, RepositoryError> {
    anchor
        .assets
        .iter()
        .map(|asset| {
            Ok(BackupReferenceAnchorAsset {
                asset_id: asset_ids
                    .get(&asset.asset_id)
                    .cloned()
                    .ok_or_else(|| RepositoryError::integrity("参考锚点素材缺少映射"))?,
                ordinal: asset.ordinal,
                created_at: asset.created_at.clone(),
            })
        })
        .collect()
}
fn hash_bytes(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn validate_production_preparation_snapshot_json(
    snapshot_json: &str,
) -> Result<(), RepositoryError> {
    let snapshot = serde_json::from_str::<ProductionPreparationSnapshotV1>(snapshot_json).map_err(
        |error| {
            RepositoryError::integrity(format!(
                "Production Preparation Snapshot JSON 无效：{error}"
            ))
        },
    )?;
    if snapshot.schema_version != 1
        || snapshot.project_id.trim().is_empty()
        || snapshot.shot_id.trim().is_empty()
        || !matches!(snapshot.stage.as_str(), "image" | "video")
        || snapshot.context_hash.trim().is_empty()
        || snapshot.resolved_at.trim().is_empty()
        || snapshot.prepared_at.trim().is_empty()
        || !snapshot.reference_assets.is_array()
    {
        return Err(RepositoryError::integrity(
            "Production Preparation Snapshot V1 字段无效",
        ));
    }

    let _ = (
        &snapshot.structure,
        &snapshot.profiles,
        &snapshot.reference_sets,
        &snapshot.prompt,
        &snapshot.workflow,
        &snapshot.output_spec,
        &snapshot.stage_input,
        &snapshot.frozen_generation_values,
        &snapshot.readiness,
        &snapshot.comfy_capability_evidence,
    );
    for reference in snapshot
        .reference_assets
        .as_array()
        .expect("reference_assets was checked to be an array")
    {
        let Some(reference) = reference.as_object() else {
            return Err(RepositoryError::integrity(
                "Production Preparation Snapshot referenceAssets 必须是对象数组",
            ));
        };
        let valid_string = |key: &str| {
            reference
                .get(key)
                .and_then(Value::as_str)
                .is_some_and(|value| !value.trim().is_empty())
        };
        let valid_ordinal = reference
            .get("ordinal")
            .and_then(Value::as_i64)
            .is_some_and(|ordinal| ordinal >= 0);
        if !valid_string("assetId")
            || !valid_string("sha256")
            || !valid_string("role")
            || !valid_ordinal
        {
            return Err(RepositoryError::integrity(
                "Production Preparation Snapshot referenceAsset 字段无效",
            ));
        }
    }
    Ok(())
}

async fn query_production_structure(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<
    (
        Vec<BackupProductionSeries>,
        Vec<BackupProductionEpisode>,
        Vec<BackupProductionScene>,
        Vec<BackupShotSceneAssignment>,
    ),
    RepositoryError,
> {
    let table_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sqlite_master
         WHERE type = 'table' AND name IN
           ('production_series', 'production_episodes', 'production_scenes',
            'shot_scene_assignments')",
    )
    .fetch_one(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?;
    if table_count == 0 {
        return Ok((Vec::new(), Vec::new(), Vec::new(), Vec::new()));
    }
    if table_count != 4 {
        return Err(RepositoryError::database(
            "生产结构表不完整，请先应用 migration 021",
        ));
    }

    let series = sqlx::query_as::<_, DbProductionSeries>(
        "SELECT id, project_id, ordinal, name, description, created_at, updated_at
         FROM production_series WHERE project_id = ? ORDER BY ordinal, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?
    .into_iter()
    .map(|row| BackupProductionSeries {
        id: row.id,
        project_id: row.project_id,
        ordinal: row.ordinal,
        name: row.name,
        description: row.description,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
    .collect();
    let episodes = sqlx::query_as::<_, DbProductionEpisode>(
        "SELECT e.id, e.series_id, e.ordinal, e.name, e.description, e.created_at, e.updated_at
         FROM production_episodes e
         JOIN production_series s ON s.id = e.series_id
         WHERE s.project_id = ? ORDER BY e.series_id, e.ordinal, e.id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?
    .into_iter()
    .map(|row| BackupProductionEpisode {
        id: row.id,
        series_id: row.series_id,
        ordinal: row.ordinal,
        name: row.name,
        description: row.description,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
    .collect();
    let scenes = sqlx::query_as::<_, DbProductionScene>(
        "SELECT c.id, c.episode_id, c.ordinal, c.name, c.description, c.created_at, c.updated_at
         FROM production_scenes c
         JOIN production_episodes e ON e.id = c.episode_id
         JOIN production_series s ON s.id = e.series_id
         WHERE s.project_id = ? ORDER BY c.episode_id, c.ordinal, c.id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?
    .into_iter()
    .map(|row| BackupProductionScene {
        id: row.id,
        episode_id: row.episode_id,
        ordinal: row.ordinal,
        name: row.name,
        description: row.description,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
    .collect();
    let assignments = sqlx::query_as::<_, DbShotSceneAssignment>(
        "SELECT a.shot_id, a.scene_id, a.ordinal, a.created_at, a.updated_at
         FROM shot_scene_assignments a
         JOIN production_scenes c ON c.id = a.scene_id
         JOIN production_episodes e ON e.id = c.episode_id
         JOIN production_series s ON s.id = e.series_id
         JOIN shots h ON h.id = a.shot_id
         WHERE s.project_id = ? AND h.project_id = ?
         ORDER BY a.scene_id, a.ordinal, a.shot_id",
    )
    .bind(project_id)
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?
    .into_iter()
    .map(|row| BackupShotSceneAssignment {
        shot_id: row.shot_id,
        scene_id: row.scene_id,
        ordinal: row.ordinal,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
    .collect();
    Ok((series, episodes, scenes, assignments))
}

async fn query_project(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Option<ProjectRecord>, RepositoryError> {
    let row = sqlx::query_as::<_, DbProject>(
        "SELECT id, name, description, root_path, created_at, updated_at FROM projects WHERE id = ?",
    )
    .bind(project_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?;
    row.map(|row| {
        if row.root_path.trim().is_empty() {
            return Err(RepositoryError::database("项目 root_path 不能为空"));
        }
        let created_at = DateTime::parse_from_rfc3339(&row.created_at)
            .map(|value| value.with_timezone(&Utc))
            .map_err(|error| RepositoryError::database(format!("项目 created_at 无效：{error}")))?;
        let updated_at = DateTime::parse_from_rfc3339(&row.updated_at)
            .map(|value| value.with_timezone(&Utc))
            .map_err(|error| RepositoryError::database(format!("项目 updated_at 无效：{error}")))?;
        Ok(ProjectRecord {
            id: row.id,
            name: row.name,
            description: row.description,
            root_path: PathBuf::from(row.root_path),
            created_at,
            updated_at,
        })
    })
    .transpose()
}

async fn query_project_workflow_bindings(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupProjectWorkflowBinding>, RepositoryError> {
    sqlx::query_as::<_, BackupProjectWorkflowBinding>(
        "SELECT b.stage, b.mode, b.workflow_version_id, b.recipe_id,
                b.created_at, b.updated_at, wv.workflow_id
         FROM project_workflow_bindings b
         LEFT JOIN workflow_versions wv ON wv.id = b.workflow_version_id
         WHERE b.project_id = ?
         ORDER BY b.stage ASC, b.mode ASC",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))
}

async fn query_workflow_registry_snapshot(
    transaction: &mut Transaction<'_, Sqlite>,
    workflow_refs: &[WorkflowReference],
    bindings: &[BackupProjectWorkflowBinding],
) -> Result<BackupWorkflowRegistry, RepositoryError> {
    let mut workflow_ids = workflow_refs
        .iter()
        .map(|reference| reference.workflow_id.clone())
        .filter(|id| !id.trim().is_empty())
        .collect::<HashSet<_>>();
    let version_ids = workflow_refs
        .iter()
        .map(|reference| reference.workflow_version_id.clone())
        .chain(
            bindings
                .iter()
                .map(|binding| binding.workflow_version_id.clone()),
        )
        .collect::<HashSet<_>>();
    workflow_ids.extend(
        bindings
            .iter()
            .filter_map(|binding| binding.workflow_id.clone())
            .filter(|id| !id.trim().is_empty()),
    );

    let all_versions = sqlx::query_as::<_, BackupWorkflowVersion>(
        "SELECT id, workflow_id, version, api_workflow_json, workflow_sha256,
                package_name, package_source_path, created_at
         FROM workflow_versions ORDER BY workflow_id, version, id",
    )
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?;
    let versions = all_versions
        .into_iter()
        .filter(|version| {
            workflow_ids.contains(&version.workflow_id) || version_ids.contains(&version.id)
        })
        .collect::<Vec<_>>();
    workflow_ids.extend(versions.iter().map(|version| version.workflow_id.clone()));
    let selected_version_ids = versions
        .iter()
        .map(|version| version.id.clone())
        .collect::<HashSet<_>>();

    let workflows = sqlx::query_as::<_, BackupWorkflow>(
        "SELECT id, name, category, mode, source_kind, library_state,
                current_version_id, removed_at, created_at, updated_at
         FROM workflows ORDER BY id",
    )
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?
    .into_iter()
    .filter(|workflow| workflow_ids.contains(&workflow.id))
    .collect::<Vec<_>>();

    let recipes = sqlx::query_as::<_, BackupWorkflowRecipe>(
        "SELECT id, workflow_version_id, version, schema_version, recipe_yaml,
                recipe_sha256, created_at
         FROM recipes ORDER BY workflow_version_id, version, id",
    )
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?
    .into_iter()
    .filter(|recipe| selected_version_ids.contains(&recipe.workflow_version_id))
    .collect::<Vec<_>>();
    let recipe_ids = recipes
        .iter()
        .map(|recipe| recipe.id.clone())
        .collect::<HashSet<_>>();
    let runtime_artifacts = sqlx::query_as::<_, BackupWorkflowRuntimeArtifact>(
        "SELECT id, workflow_version_id, recipe_id, package_name, source_kind,
                package_source_path, workflow_sha256, recipe_sha256, created_at
         FROM workflow_runtime_artifacts ORDER BY workflow_version_id, recipe_id, package_name, id",
    )
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?
    .into_iter()
    .filter(|artifact| {
        selected_version_ids.contains(&artifact.workflow_version_id)
            && recipe_ids.contains(&artifact.recipe_id)
    })
    .collect();

    Ok(BackupWorkflowRegistry {
        workflows,
        versions,
        recipes,
        runtime_artifacts,
    })
}

fn collect_workflow_refs(tasks: &[BackupTask]) -> Vec<WorkflowReference> {
    let mut refs = tasks
        .iter()
        .map(|task| WorkflowReference {
            workflow_id: task.workflow_id.clone(),
            workflow_version_id: task.workflow_version_id.clone(),
            recipe_id: task.recipe_id.clone(),
        })
        .collect::<Vec<_>>();
    refs.sort_by(|a, b| a.workflow_id.cmp(&b.workflow_id));
    refs.dedup();
    refs
}

async fn query_task_events(
    transaction: &mut Transaction<'_, Sqlite>,
    task_ids: &HashSet<String>,
) -> Result<Vec<BackupTaskEvent>, RepositoryError> {
    let mut result = Vec::new();
    for task_id in task_ids {
        let rows = sqlx::query_as::<_, DbTaskEvent>(
            "SELECT id, task_id, sequence, event_type, payload_json, created_at FROM task_events WHERE task_id = ? ORDER BY sequence",
        )
        .bind(task_id)
        .fetch_all(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
        for row in rows {
            result.push(BackupTaskEvent {
                id: row.id,
                task_id: row.task_id,
                sequence: row.sequence,
                event_type: row.event_type,
                payload: parse_optional_value(row.payload_json.as_deref(), "任务事件")?,
                created_at: row.created_at,
            });
        }
    }
    Ok(result)
}

async fn query_snapshots(
    transaction: &mut Transaction<'_, Sqlite>,
    task_ids: &HashSet<String>,
) -> Result<Vec<BackupSnapshot>, RepositoryError> {
    let mut result = Vec::new();
    for task_id in task_ids {
        let row = sqlx::query_as::<_, DbSnapshot>(
            "SELECT id, task_id, workflow_json, recipe_yaml, user_inputs_json, resolved_inputs_json, created_at FROM generation_snapshots WHERE task_id = ?",
        )
        .bind(task_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
        if let Some(row) = row {
            result.push(BackupSnapshot {
                id: row.id,
                task_id: row.task_id,
                workflow: parse_value(Some(&row.workflow_json), "工作流快照")?,
                recipe_yaml: row.recipe_yaml,
                user_inputs: parse_value(Some(&row.user_inputs_json), "用户输入快照")?,
                resolved_inputs: parse_value(Some(&row.resolved_inputs_json), "解析输入快照")?,
                created_at: row.created_at,
            });
        }
    }
    Ok(result)
}

async fn query_mappings(
    transaction: &mut Transaction<'_, Sqlite>,
    task_ids: &HashSet<String>,
) -> Result<Vec<BackupMapping>, RepositoryError> {
    let mut result = Vec::new();
    for task_id in task_ids {
        let rows = sqlx::query_as::<_, DbMapping>(
            "SELECT task_id, output_id, ordinal, asset_id, created_at FROM task_output_assets WHERE task_id = ? ORDER BY output_id, ordinal",
        )
        .bind(task_id)
        .fetch_all(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
        result.extend(rows.into_iter().map(|row| BackupMapping {
            task_id: row.task_id,
            output_id: row.output_id,
            ordinal: row.ordinal,
            asset_id: row.asset_id,
            created_at: row.created_at,
        }));
    }
    Ok(result)
}

async fn query_presets(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupPreset>, RepositoryError> {
    let rows = sqlx::query_as::<_, DbPreset>(
        "SELECT id, project_id, workflow_version_id, recipe_id, name, values_json, created_at, updated_at FROM presets WHERE project_id = ? ORDER BY updated_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?;
    rows.into_iter()
        .map(|row| {
            Ok(BackupPreset {
                id: row.id,
                workflow_version_id: row.workflow_version_id,
                recipe_id: row.recipe_id,
                name: row.name,
                values: parse_value(Some(&row.values_json), "预设")?,
                created_at: row.created_at,
                updated_at: row.updated_at,
            })
        })
        .collect()
}

async fn query_prompt_entries(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupPromptEntry>, RepositoryError> {
    let rows = sqlx::query_as::<_, DbPromptEntry>(
        "SELECT id, project_id, kind, name, normalized_name, tags_json, created_at, updated_at
         FROM prompt_entries WHERE project_id = ? ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?;
    rows.into_iter()
        .map(|row| {
            let tags = serde_json::from_str::<Vec<String>>(&row.tags_json).map_err(|error| {
                RepositoryError::database(format!("提示词标签 JSON 无效：{error}"))
            })?;
            Ok(BackupPromptEntry {
                id: row.id,
                project_id: row.project_id,
                kind: row.kind,
                name: row.name,
                normalized_name: row.normalized_name,
                tags,
                created_at: row.created_at,
                updated_at: row.updated_at,
            })
        })
        .collect()
}

async fn query_prompt_versions(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupPromptVersion>, RepositoryError> {
    let rows = sqlx::query_as::<_, DbPromptVersion>(
        "SELECT v.id, v.prompt_id, v.version, v.text, v.created_at
         FROM prompt_versions v
         JOIN prompt_entries e ON e.id = v.prompt_id
         WHERE e.project_id = ? ORDER BY v.prompt_id, v.version",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|row| BackupPromptVersion {
            id: row.id,
            project_id: project_id.to_owned(),
            prompt_id: row.prompt_id,
            version: row.version,
            text: row.text,
            created_at: row.created_at,
        })
        .collect())
}

async fn query_batches(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupBatch>, RepositoryError> {
    let rows = sqlx::query_as::<_, DbBatch>(
        "SELECT id, project_id, name, status, continue_on_failure, archived_at, created_at, updated_at FROM production_batches WHERE project_id = ? ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|row| BackupBatch {
            id: row.id,
            name: row.name,
            status: row.status,
            continue_on_failure: row.continue_on_failure,
            archived_at: row.archived_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
        .collect())
}

async fn query_batch_items(
    transaction: &mut Transaction<'_, Sqlite>,
    batches: &[BackupBatch],
) -> Result<Vec<BackupBatchItem>, RepositoryError> {
    let mut result = Vec::new();
    for batch in batches {
        let rows = sqlx::query_as::<_, DbBatchItem>(
            "SELECT id, batch_id, ordinal, workflow_version_id, recipe_id, values_json, status, task_id, retry_of_item_id, error_code, error_message, created_at, updated_at FROM production_batch_items WHERE batch_id = ? ORDER BY ordinal",
        )
        .bind(&batch.id)
        .fetch_all(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
        for row in rows {
            result.push(BackupBatchItem {
                id: row.id,
                batch_id: row.batch_id,
                ordinal: row.ordinal,
                workflow_version_id: row.workflow_version_id,
                recipe_id: row.recipe_id,
                values: parse_value(Some(&row.values_json), "生产队列输入")?,
                status: row.status,
                task_id: row.task_id,
                retry_of_item_id: row.retry_of_item_id,
                error_code: row.error_code,
                error_message: row.error_message,
                created_at: row.created_at,
                updated_at: row.updated_at,
            });
        }
    }
    Ok(result)
}

async fn query_production_preparation_snapshots(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupProductionPreparationSnapshot>, RepositoryError> {
    let rows = sqlx::query_as::<_, DbProductionPreparationSnapshot>(
        "SELECT p.id, p.project_id, p.shot_id, p.stage, p.context_hash,
                p.production_batch_id, p.production_batch_item_id, p.snapshot_json,
                p.created_at
         FROM production_preparation_snapshots p
         JOIN projects pr ON pr.id = p.project_id
         JOIN shots s ON s.id = p.shot_id AND s.project_id = p.project_id
         JOIN production_batches b
           ON b.id = p.production_batch_id AND b.project_id = p.project_id
         JOIN production_batch_items i
           ON i.id = p.production_batch_item_id AND i.batch_id = p.production_batch_id
         WHERE pr.id = ?
         ORDER BY p.created_at, p.id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?;
    rows.into_iter()
        .map(|row| {
            validate_production_preparation_snapshot_json(&row.snapshot_json)?;
            Ok(BackupProductionPreparationSnapshot {
                id: row.id,
                project_id: row.project_id,
                shot_id: row.shot_id,
                stage: row.stage,
                context_hash: row.context_hash,
                production_batch_id: row.production_batch_id,
                production_batch_item_id: row.production_batch_item_id,
                snapshot_json: row.snapshot_json,
                created_at: row.created_at,
            })
        })
        .collect()
}

async fn query_benchmark_experiments(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupBenchmarkExperiment>, RepositoryError> {
    let rows = sqlx::query_as::<_, DbBenchmarkExperiment>(
        "SELECT id, name, media_type, status, base_values_json, asset_ids_json,
                winner_candidate_id, production_batch_id, created_at, updated_at
         FROM benchmark_experiments WHERE project_id = ? ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?;
    rows.into_iter()
        .map(|row| {
            Ok(BackupBenchmarkExperiment {
                id: row.id,
                name: row.name,
                media_type: row.media_type,
                status: row.status,
                base_values: parse_value(Some(&row.base_values_json), "Benchmark 基准输入")?,
                asset_ids: parse_string_array(Some(&row.asset_ids_json), "Benchmark 素材")?,
                winner_candidate_id: row.winner_candidate_id,
                production_batch_id: row.production_batch_id,
                created_at: row.created_at,
                updated_at: row.updated_at,
            })
        })
        .collect()
}

async fn query_benchmark_candidates(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupBenchmarkCandidate>, RepositoryError> {
    let rows = sqlx::query_as::<_, DbBenchmarkCandidate>(
        "SELECT c.id, c.experiment_id, c.position, c.workflow_version_id, c.recipe_id,
                c.preset_id, c.preset_name, c.label, c.values_json, c.asset_ids_json,
                c.production_batch_item_id, c.task_id, c.created_at
         FROM benchmark_candidates c
         JOIN benchmark_experiments e ON e.id = c.experiment_id
         WHERE e.project_id = ? ORDER BY c.experiment_id, c.position, c.id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?;
    rows.into_iter()
        .map(|row| {
            Ok(BackupBenchmarkCandidate {
                id: row.id,
                experiment_id: row.experiment_id,
                position: row.position,
                workflow_version_id: row.workflow_version_id,
                recipe_id: row.recipe_id,
                preset_id: row.preset_id,
                preset_name: row.preset_name,
                label: row.label,
                values: parse_value(Some(&row.values_json), "Benchmark 候选输入")?,
                asset_ids: parse_string_array(Some(&row.asset_ids_json), "Benchmark 候选素材")?,
                production_batch_item_id: row.production_batch_item_id,
                task_id: row.task_id,
                created_at: row.created_at,
            })
        })
        .collect()
}

async fn query_production_runs(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupProductionRun>, RepositoryError> {
    let rows = sqlx::query_as::<_, DbProductionRun>(
        "SELECT id, project_id, name, status, current_stage_ordinal, template_id,
                created_at, updated_at, started_at, finished_at
         FROM production_runs WHERE project_id = ? ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|row| BackupProductionRun {
            id: row.id,
            project_id: row.project_id,
            name: row.name,
            status: row.status,
            current_stage_ordinal: row.current_stage_ordinal,
            template_id: row.template_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
            started_at: row.started_at,
            finished_at: row.finished_at,
        })
        .collect())
}

async fn query_production_stages(
    transaction: &mut Transaction<'_, Sqlite>,
    runs: &[BackupProductionRun],
) -> Result<Vec<BackupProductionStage>, RepositoryError> {
    let mut result = Vec::new();
    for run in runs {
        let rows = sqlx::query_as::<_, DbProductionStage>(
            "SELECT id, run_id, ordinal, stage_type, status, workflow_version_id, recipe_id,
                    production_batch_id, frozen_config_json, prompt, created_at, updated_at,
                    started_at, finished_at
             FROM production_stages WHERE run_id = ? ORDER BY ordinal, id",
        )
        .bind(&run.id)
        .fetch_all(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
        result.extend(rows.into_iter().map(|row| {
            Ok(BackupProductionStage {
                id: row.id,
                run_id: row.run_id,
                ordinal: row.ordinal,
                stage_type: row.stage_type,
                status: row.status,
                workflow_version_id: row.workflow_version_id,
                recipe_id: row.recipe_id,
                production_batch_id: row.production_batch_id,
                frozen_config: parse_value(Some(&row.frozen_config_json), "Production Stage 配置")?,
                prompt: row.prompt,
                created_at: row.created_at,
                updated_at: row.updated_at,
                started_at: row.started_at,
                finished_at: row.finished_at,
            })
        }));
    }
    result.into_iter().collect()
}

async fn query_production_stage_items(
    transaction: &mut Transaction<'_, Sqlite>,
    stages: &[BackupProductionStage],
) -> Result<Vec<BackupProductionStageItem>, RepositoryError> {
    let mut result = Vec::new();
    for stage in stages {
        let rows = sqlx::query_as::<_, DbProductionStageItem>(
            "SELECT id, stage_id, ordinal, status, production_batch_item_id, task_id,
                    asset_id, source_asset_id, reference_index, attempt,
                    submission_idempotency_key, parent_stage_item_id, frozen_values_json,
                    error_code, error_message, created_at, updated_at
             FROM production_stage_items WHERE stage_id = ? ORDER BY ordinal, id",
        )
        .bind(&stage.id)
        .fetch_all(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
        result.extend(rows.into_iter().map(|row| {
            Ok(BackupProductionStageItem {
                id: row.id,
                stage_id: row.stage_id,
                ordinal: row.ordinal,
                status: row.status,
                production_batch_item_id: row.production_batch_item_id,
                task_id: row.task_id,
                asset_id: row.asset_id,
                source_asset_id: row.source_asset_id,
                reference_index: row.reference_index,
                attempt: row.attempt,
                submission_idempotency_key: row.submission_idempotency_key,
                parent_stage_item_id: row.parent_stage_item_id,
                frozen_values: parse_value(
                    Some(&row.frozen_values_json),
                    "Production Stage Item 输入",
                )?,
                error_code: row.error_code,
                error_message: row.error_message,
                created_at: row.created_at,
                updated_at: row.updated_at,
            })
        }));
    }
    result.into_iter().collect()
}

async fn query_production_run_templates(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupProductionRunTemplate>, RepositoryError> {
    let rows = sqlx::query_as::<_, DbProductionRunTemplate>(
        "SELECT id, project_id, name, krea2_workflow_version_id, krea2_recipe_id,
                krea2_preset_id, default_image_count, h3_workflow_version_id, h3_recipe_id,
                h3_profile, default_duration_seconds, default_width, default_height,
                created_at, updated_at
         FROM production_run_templates WHERE project_id = ? ORDER BY updated_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|row| BackupProductionRunTemplate {
            id: row.id,
            project_id: row.project_id,
            name: row.name,
            krea2_workflow_version_id: row.krea2_workflow_version_id,
            krea2_recipe_id: row.krea2_recipe_id,
            krea2_preset_id: row.krea2_preset_id,
            default_image_count: row.default_image_count,
            h3_workflow_version_id: row.h3_workflow_version_id,
            h3_recipe_id: row.h3_recipe_id,
            h3_profile: row.h3_profile,
            default_duration_seconds: row.default_duration_seconds,
            default_width: row.default_width,
            default_height: row.default_height,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
        .collect())
}

async fn query_benchmark_runs(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupBenchmarkRun>, RepositoryError> {
    let rows = sqlx::query_as::<_, DbBenchmarkRun>(
        "SELECT r.id, r.experiment_id, r.candidate_id, r.run_number,
                r.production_batch_item_id, r.task_id, r.snapshot_id, r.output_asset_id,
                r.generation_execution_id, r.compiled_workflow_sha256, r.runtime_profile,
                r.concurrency_class, r.queue_wait_ms, r.prepare_ms, r.submit_ms,
                r.comfy_execution_ms, r.collect_ms, r.total_ms, r.status, r.error_code,
                r.output_file_size, r.created_at, r.updated_at
         FROM benchmark_runs r
         JOIN benchmark_experiments e ON e.id = r.experiment_id
         WHERE e.project_id = ? ORDER BY r.experiment_id, r.candidate_id, r.run_number, r.id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|row| BackupBenchmarkRun {
            id: row.id,
            experiment_id: row.experiment_id,
            candidate_id: row.candidate_id,
            run_number: row.run_number,
            production_batch_item_id: row.production_batch_item_id,
            task_id: row.task_id,
            snapshot_id: row.snapshot_id,
            output_asset_id: row.output_asset_id,
            generation_execution_id: row.generation_execution_id,
            compiled_workflow_sha256: row.compiled_workflow_sha256,
            runtime_profile: row.runtime_profile,
            concurrency_class: row.concurrency_class,
            queue_wait_ms: row.queue_wait_ms,
            prepare_ms: row.prepare_ms,
            submit_ms: row.submit_ms,
            comfy_execution_ms: row.comfy_execution_ms,
            collect_ms: row.collect_ms,
            total_ms: row.total_ms,
            status: row.status,
            error_code: row.error_code,
            output_file_size: row.output_file_size,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
        .collect())
}

async fn query_benchmark_quality_scores(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupBenchmarkQualityScore>, RepositoryError> {
    let rows = sqlx::query_as::<_, DbBenchmarkQualityScore>(
        "SELECT q.id, q.candidate_id, q.prompt_adherence, q.visual_quality,
                q.motion_quality, q.reference_consistency, q.overall, q.note,
                q.created_at, q.updated_at
         FROM benchmark_quality_scores q
         JOIN benchmark_candidates c ON c.id = q.candidate_id
         JOIN benchmark_experiments e ON e.id = c.experiment_id
         WHERE e.project_id = ? ORDER BY q.candidate_id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|row| BackupBenchmarkQualityScore {
            id: row.id,
            candidate_id: row.candidate_id,
            prompt_adherence: row.prompt_adherence,
            visual_quality: row.visual_quality,
            motion_quality: row.motion_quality,
            reference_consistency: row.reference_consistency,
            overall: row.overall,
            note: row.note,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
        .collect())
}

async fn query_benchmark_workflow_refs(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<WorkflowReference>, RepositoryError> {
    let rows = sqlx::query_as::<_, DbBenchmarkWorkflowRef>(
        "SELECT DISTINCT wv.workflow_id, c.workflow_version_id, c.recipe_id
         FROM benchmark_candidates c
         JOIN benchmark_experiments e ON e.id = c.experiment_id
         JOIN workflow_versions wv ON wv.id = c.workflow_version_id
         WHERE e.project_id = ? ORDER BY wv.workflow_id, c.workflow_version_id, c.recipe_id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|row| WorkflowReference {
            workflow_id: row.workflow_id,
            workflow_version_id: row.workflow_version_id,
            recipe_id: row.recipe_id,
        })
        .collect())
}

async fn query_production_item_reviews(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupProductionItemReview>, RepositoryError> {
    let rows = sqlx::query_as::<_, DbProductionItemReview>(
        "SELECT id, project_id, production_batch_id, production_batch_item_id,
                task_id, result_asset_id, review_status, review_note, version,
                lineage_key, parent_batch_id, parent_item_id, created_at, updated_at
         FROM production_item_reviews
         WHERE project_id = ?
         ORDER BY lineage_key, version, production_batch_item_id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|row| BackupProductionItemReview {
            id: row.id,
            project_id: row.project_id,
            production_batch_id: row.production_batch_id,
            production_batch_item_id: row.production_batch_item_id,
            task_id: row.task_id,
            result_asset_id: row.result_asset_id,
            review_status: row.review_status,
            review_note: row.review_note,
            version: row.version,
            lineage_key: row.lineage_key,
            parent_batch_id: row.parent_batch_id,
            parent_item_id: row.parent_item_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
        .collect())
}

#[derive(FromRow)]
struct DbShot {
    id: String,
    project_id: String,
    ordinal: i64,
    name: String,
    prompt_text: String,
    prompt_entry_id: Option<String>,
    prompt_version_id: Option<String>,
    selected_image_asset_id: Option<String>,
    selected_video_asset_id: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(FromRow)]
struct DbShotStageConfig {
    shot_id: String,
    stage: String,
    workflow_id: String,
    workflow_version_id: String,
    recipe_id: String,
    scalar_values_json: String,
    updated_at: String,
}

#[derive(FromRow)]
struct DbShotStagePrompt {
    shot_id: String,
    stage: String,
    prompt_text: String,
    prompt_entry_id: Option<String>,
    prompt_version_id: Option<String>,
    updated_at: String,
}

#[derive(FromRow)]
struct DbShotReferenceAsset {
    shot_id: String,
    stage: String,
    asset_id: String,
    ordinal: i64,
}

#[derive(FromRow)]
struct DbShotGenerationLink {
    id: String,
    shot_id: String,
    stage: String,
    task_id: Option<String>,
    production_batch_item_id: Option<String>,
    created_at: String,
}

async fn query_shots(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupShot>, RepositoryError> {
    let rows = sqlx::query_as::<_, DbShot>(
        "SELECT id, project_id, ordinal, name, prompt_text, prompt_entry_id, prompt_version_id,
                selected_image_asset_id, selected_video_asset_id, created_at, updated_at
         FROM shots WHERE project_id = ? ORDER BY ordinal, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|row| BackupShot {
            id: row.id,
            project_id: row.project_id,
            ordinal: row.ordinal,
            name: row.name,
            prompt_text: row.prompt_text,
            prompt_entry_id: row.prompt_entry_id,
            prompt_version_id: row.prompt_version_id,
            selected_image_asset_id: row.selected_image_asset_id,
            selected_video_asset_id: row.selected_video_asset_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
        .collect())
}

async fn query_shot_stage_configs(
    transaction: &mut Transaction<'_, Sqlite>,
) -> Result<Vec<BackupShotStageConfig>, RepositoryError> {
    let rows = sqlx::query_as::<_, DbShotStageConfig>(
        "SELECT c.shot_id, c.stage, v.workflow_id, c.workflow_version_id, c.recipe_id,
                c.scalar_values_json, c.updated_at
         FROM shot_stage_configs c
         JOIN workflow_versions v ON v.id = c.workflow_version_id
         ORDER BY c.shot_id, c.stage",
    )
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?;
    rows.into_iter()
        .map(|row| {
            Ok(BackupShotStageConfig {
                shot_id: row.shot_id,
                stage: row.stage,
                workflow_id: row.workflow_id,
                workflow_version_id: row.workflow_version_id,
                recipe_id: row.recipe_id,
                scalar_values: parse_value(Some(&row.scalar_values_json), "镜头阶段参数")?,
                updated_at: row.updated_at,
            })
        })
        .collect()
}

async fn query_shot_stage_prompts(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupShotStagePrompt>, RepositoryError> {
    let rows = sqlx::query_as::<_, DbShotStagePrompt>(
        "SELECT p.shot_id, p.stage, p.prompt_text, p.prompt_entry_id,
                p.prompt_version_id, p.updated_at
         FROM shot_stage_prompts p
         JOIN shots s ON s.id = p.shot_id
         WHERE s.project_id = ?
         ORDER BY p.shot_id, p.stage",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|row| BackupShotStagePrompt {
            shot_id: row.shot_id,
            stage: row.stage,
            prompt_text: row.prompt_text,
            prompt_entry_id: row.prompt_entry_id,
            prompt_version_id: row.prompt_version_id,
            updated_at: row.updated_at,
        })
        .collect())
}

async fn query_shot_reference_assets(
    transaction: &mut Transaction<'_, Sqlite>,
) -> Result<Vec<BackupShotReferenceAsset>, RepositoryError> {
    let rows = sqlx::query_as::<_, DbShotReferenceAsset>(
        "SELECT shot_id, stage, asset_id, ordinal
         FROM shot_reference_assets ORDER BY shot_id, stage, ordinal, asset_id",
    )
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|row| BackupShotReferenceAsset {
            shot_id: row.shot_id,
            stage: row.stage,
            asset_id: row.asset_id,
            ordinal: row.ordinal,
        })
        .collect())
}

async fn query_shot_generation_links(
    transaction: &mut Transaction<'_, Sqlite>,
) -> Result<Vec<BackupShotGenerationLink>, RepositoryError> {
    let rows = sqlx::query_as::<_, DbShotGenerationLink>(
        "SELECT id, shot_id, stage, task_id, production_batch_item_id, created_at
         FROM shot_generation_links ORDER BY shot_id, created_at, id",
    )
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|row| BackupShotGenerationLink {
            id: row.id,
            shot_id: row.shot_id,
            stage: row.stage,
            task_id: row.task_id,
            production_batch_item_id: row.production_batch_item_id,
            created_at: row.created_at,
        })
        .collect())
}

async fn query_character_profiles(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupCharacterProfile>, RepositoryError> {
    sqlx::query_as::<_, BackupCharacterProfile>(
        "SELECT id, project_id, name, description, canonical_prompt, negative_prompt,
                default_style_profile_id, default_reference_set_id, active_revision_id,
                metadata_json, created_at, updated_at
         FROM character_profiles
         WHERE project_id = ?
         ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))
}

async fn query_scene_profiles(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupSceneProfile>, RepositoryError> {
    sqlx::query_as::<_, BackupSceneProfile>(
        "SELECT id, project_id, name, description, environment_prompt, lighting_prompt,
                negative_prompt, default_style_profile_id, default_reference_set_id,
                active_revision_id, created_at, updated_at
         FROM scene_profiles
         WHERE project_id = ?
         ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))
}

async fn query_prop_profiles(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupPropProfile>, RepositoryError> {
    sqlx::query_as::<_, BackupPropProfile>(
        "SELECT id, project_id, name, description, canonical_prompt, material_prompt,
                scale_prompt, default_reference_set_id, active_revision_id, created_at,
                updated_at
         FROM prop_profiles
         WHERE project_id = ?
         ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))
}

async fn query_style_profiles(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupStyleProfile>, RepositoryError> {
    sqlx::query_as::<_, BackupStyleProfile>(
        "SELECT id, project_id, name, style_prompt, color_prompt, line_prompt,
                negative_prompt, output_notes, active_revision_id, created_at, updated_at
         FROM style_profiles
         WHERE project_id = ?
         ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))
}

async fn query_costume_variants(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupCostumeVariant>, RepositoryError> {
    sqlx::query_as::<_, BackupCostumeVariant>(
        "SELECT v.id, v.character_profile_id, v.name, v.prompt_fragment,
                v.reference_set_id, v.is_default, v.ordinal, v.active_revision_id,
                v.created_at, v.updated_at
         FROM costume_variants v
         JOIN character_profiles p ON p.id = v.character_profile_id
         WHERE p.project_id = ?
         ORDER BY v.character_profile_id, v.ordinal, v.id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))
}

async fn query_profile_revisions(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupProfileRevision>, RepositoryError> {
    sqlx::query_as::<_, BackupProfileRevision>(
        "SELECT id, profile_type, profile_id, revision_number, content_json,
                content_sha256, status, created_at, created_by
         FROM profile_revisions
         WHERE (profile_type = 'CHARACTER' AND profile_id IN
                    (SELECT id FROM character_profiles WHERE project_id = ?))
            OR (profile_type = 'SCENE' AND profile_id IN
                    (SELECT id FROM scene_profiles WHERE project_id = ?))
            OR (profile_type = 'PROP' AND profile_id IN
                    (SELECT id FROM prop_profiles WHERE project_id = ?))
            OR (profile_type = 'STYLE' AND profile_id IN
                    (SELECT id FROM style_profiles WHERE project_id = ?))
         ORDER BY profile_type, profile_id, revision_number, id",
    )
    .bind(project_id)
    .bind(project_id)
    .bind(project_id)
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))
}

async fn query_reference_sets(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupReferenceSet>, RepositoryError> {
    sqlx::query_as::<_, BackupReferenceSet>(
        "SELECT id, project_id, name, purpose, description, owner_profile_type,
                owner_profile_id, active_revision_id, created_at, updated_at
         FROM reference_sets
         WHERE project_id = ?
         ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))
}

async fn query_reference_set_items(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupReferenceSetItem>, RepositoryError> {
    sqlx::query_as::<_, BackupReferenceSetItem>(
        "SELECT i.reference_set_id, i.asset_id, i.ordinal, i.role, i.is_primary,
                i.created_at
         FROM reference_set_items i
         JOIN reference_sets r ON r.id = i.reference_set_id
         WHERE r.project_id = ?
         ORDER BY i.reference_set_id, i.ordinal, i.asset_id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))
}

async fn query_shot_profile_bindings(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupShotProfileBinding>, RepositoryError> {
    sqlx::query_as::<_, BackupShotProfileBinding>(
        "SELECT b.id, b.shot_id, b.role, b.profile_type, b.profile_id,
                b.costume_variant_id, b.ordinal, b.inheritance_mode,
                b.created_at, b.updated_at
         FROM shot_profile_bindings b
         JOIN shots s ON s.id = b.shot_id
         WHERE s.project_id = ?
         ORDER BY b.shot_id, b.role, b.ordinal, b.id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))
}

async fn query_shot_reference_set_bindings(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupShotReferenceSetBinding>, RepositoryError> {
    sqlx::query_as::<_, BackupShotReferenceSetBinding>(
        "SELECT b.id, b.shot_id, b.role, b.reference_set_id, b.ordinal,
                b.required, b.inheritance_mode, b.created_at, b.updated_at
         FROM shot_reference_set_bindings b
         JOIN shots s ON s.id = b.shot_id
         WHERE s.project_id = ?
         ORDER BY b.shot_id, b.role, b.ordinal, b.id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))
}

async fn query_scope_profile_bindings(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupScopeProfileBinding>, RepositoryError> {
    sqlx::query_as::<_, BackupScopeProfileBinding>(
        "SELECT id, project_id, scope_type, scope_id, role, profile_type,
                profile_id, costume_variant_id, ordinal, inheritance_mode,
                created_at, updated_at
         FROM consistency_scope_profile_bindings
         WHERE project_id = ?
         ORDER BY scope_type, scope_id, role, ordinal, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))
}

async fn query_scope_reference_set_bindings(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupScopeReferenceSetBinding>, RepositoryError> {
    sqlx::query_as::<_, BackupScopeReferenceSetBinding>(
        "SELECT id, project_id, scope_type, scope_id, role, reference_set_id,
                ordinal, required, inheritance_mode, created_at, updated_at
         FROM consistency_scope_reference_set_bindings
         WHERE project_id = ?
         ORDER BY scope_type, scope_id, role, ordinal, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))
}

async fn query_script_sources(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupScriptSource>, RepositoryError> {
    sqlx::query_as::<_, BackupScriptSource>(
        "SELECT id, project_id, format, original_filename, source_checksum, source_bytes, source_text,
                schema_version, created_at
         FROM script_sources
         WHERE project_id = ?
         ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))
}

async fn query_script_draft_revisions(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupScriptDraftRevision>, RepositoryError> {
    sqlx::query_as::<_, BackupScriptDraftRevision>(
        "SELECT id, draft_id, project_id, source_id, revision, previous_revision_id,
                schema_version, revision_kind, parser_version, contract_version,
                provider_kind, provider_model, provider_metadata_json,
                payload_checksum, summary_json, payload_json,
                created_at
         FROM script_import_drafts
         WHERE project_id = ?
         ORDER BY draft_id, revision, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))
}

async fn restore_rows_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    project: &ProjectRecord,
    document: &BackupDocument,
    task_ids: &HashMap<String, String>,
    asset_ids: &HashMap<String, String>,
    snapshot_ids: &HashMap<String, String>,
    preset_ids: &HashMap<String, String>,
    prompt_ids: &HashMap<String, String>,
    prompt_version_ids: &HashMap<String, String>,
    batch_ids: &HashMap<String, String>,
    item_ids: &HashMap<String, String>,
    preparation_snapshot_ids: &HashMap<String, String>,
    benchmark_experiment_ids: &HashMap<String, String>,
    benchmark_candidate_ids: &HashMap<String, String>,
    production_run_ids: &HashMap<String, String>,
    production_stage_ids: &HashMap<String, String>,
    production_stage_item_ids: &HashMap<String, String>,
    production_run_template_ids: &HashMap<String, String>,
    benchmark_run_ids: &HashMap<String, String>,
    benchmark_quality_score_ids: &HashMap<String, String>,
    tag_ids: &HashMap<String, String>,
    reference_anchor_ids: &HashMap<String, String>,
    production_structure_ids: &ProductionStructureIds,
    script_source_ids: &HashMap<String, String>,
    script_draft_ids: &HashMap<String, String>,
    script_revision_ids: &HashMap<String, String>,
    consistency_ids: &ConsistencyRestoreIds,
    shot_ids: &HashMap<String, String>,
    shot_generation_link_ids: &HashMap<String, String>,
    restored_assets: &[RestoredAsset],
    restored_snapshots: &[BackupSnapshot],
) -> Result<(), RepositoryError> {
    sqlx::query(
        "INSERT INTO projects (id, name, description, root_path, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&project.id)
    .bind(&project.name)
    .bind(&project.description)
    .bind(project.root_path.to_string_lossy().to_string())
    .bind(project.created_at.to_rfc3339())
    .bind(project.updated_at.to_rfc3339())
    .execute(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?;

    if let Some(registry) = &document.workflow_registry {
        restore_workflow_registry(transaction, registry).await?;
    }

    for binding in &document.project_workflow_bindings {
        sqlx::query(
            "INSERT INTO project_workflow_bindings
             (project_id, stage, mode, workflow_version_id, recipe_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&project.id)
        .bind(&binding.stage)
        .bind(&binding.mode)
        .bind(&binding.workflow_version_id)
        .bind(&binding.recipe_id)
        .bind(&binding.created_at)
        .bind(&binding.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }

    for entry in &document.prompt_entries {
        let prompt_id = prompt_ids
            .get(&entry.id)
            .ok_or_else(|| RepositoryError::integrity("提示词 ID 映射缺失"))?;
        let tags_json = serde_json::to_string(&entry.tags).map_err(|error| {
            RepositoryError::integrity(format!("提示词标签序列化失败：{error}"))
        })?;
        sqlx::query(
            "INSERT INTO prompt_entries
             (id, project_id, kind, name, normalized_name, tags_json, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(prompt_id)
        .bind(&project.id)
        .bind(&entry.kind)
        .bind(&entry.name)
        .bind(&entry.normalized_name)
        .bind(tags_json)
        .bind(&entry.created_at)
        .bind(&entry.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for version in &document.prompt_versions {
        let version_id = prompt_version_ids
            .get(&version.id)
            .ok_or_else(|| RepositoryError::integrity("提示词版本 ID 映射缺失"))?;
        let prompt_id = prompt_ids
            .get(&version.prompt_id)
            .ok_or_else(|| RepositoryError::integrity("提示词版本引用缺少提示词映射"))?;
        sqlx::query(
            "INSERT INTO prompt_versions (id, prompt_id, version, text, created_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(version_id)
        .bind(prompt_id)
        .bind(version.version)
        .bind(&version.text)
        .bind(&version.created_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }

    for reference in &document.workflow_refs {
        ensure_workflow_dependency(transaction, reference).await?;
    }
    for task in &document.tasks {
        let new_task_id = task_ids
            .get(&task.id)
            .ok_or_else(|| RepositoryError::integrity("任务 ID 映射缺失"))?;
        let terminal = task.is_terminal();
        let status = if terminal {
            task.status.clone()
        } else {
            "FAILED".to_owned()
        };
        let error_code = if terminal {
            task.error_code.clone()
        } else {
            Some("RESTORED_INCOMPLETE_TASK".to_owned())
        };
        let error_message = if terminal {
            task.error_message.clone()
        } else {
            Some("恢复时发现任务未完成，已安全标记为失败；不会自动重新提交。".to_owned())
        };
        let finished_at = if terminal {
            task.finished_at.clone()
        } else {
            Some(Utc::now().to_rfc3339())
        };
        sqlx::query(
            "INSERT INTO tasks (id, project_id, workflow_id, workflow_version_id, recipe_id,
             app_version, build_commit, workflow_version, workflow_sha256, recipe_version,
             recipe_sha256, package_name, package_source_path, dynamic_binding_targets_json, status,
             prompt_id, queue_number, progress_mode, progress_current, progress_total, current_node_id,
             error_code, error_message, raw_error_json, created_at, queued_at, started_at, finished_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(new_task_id)
        .bind(&project.id)
        .bind(&task.workflow_id)
        .bind(&task.workflow_version_id)
        .bind(&task.recipe_id)
        .bind(&task.app_version)
        .bind(&task.build_commit)
        .bind(&task.workflow_version)
        .bind(&task.workflow_sha256)
        .bind(&task.recipe_version)
        .bind(&task.recipe_sha256)
        .bind(&task.package_name)
        .bind(&task.package_source_path)
        .bind(task.dynamic_binding_targets.as_ref().map(|value| value.to_string()))
        .bind(status)
        .bind(&task.prompt_id)
        .bind(task.queue_number)
        .bind(&task.progress_mode)
        .bind(task.progress_current)
        .bind(task.progress_total)
        .bind(&task.current_node_id)
        .bind(error_code)
        .bind(error_message)
        .bind(task.raw_error.as_ref().map(|value| value.to_string()))
        .bind(&task.created_at)
        .bind(&task.queued_at)
        .bind(&task.started_at)
        .bind(finished_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
        sqlx::query(
            "UPDATE tasks SET
                generation_execution_id = ?, compiled_workflow_sha256 = ?,
                runtime_profile = ?, concurrency_class = ?,
                prepare_started_at = ?, prepared_at = ?, submitted_at = ?,
                execution_started_at = ?, execution_finished_at = ?,
                collection_finished_at = ?
             WHERE id = ?",
        )
        .bind(&task.generation_execution_id)
        .bind(&task.compiled_workflow_sha256)
        .bind(&task.runtime_profile)
        .bind(&task.concurrency_class)
        .bind(&task.prepare_started_at)
        .bind(&task.prepared_at)
        .bind(&task.submitted_at)
        .bind(&task.execution_started_at)
        .bind(&task.execution_finished_at)
        .bind(&task.collection_finished_at)
        .bind(new_task_id)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for event in &document.task_events {
        let Some(task_id) = task_ids.get(&event.task_id) else {
            continue;
        };
        sqlx::query(
            "INSERT INTO task_events (id, task_id, sequence, event_type, payload_json, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(format!("evt_{}", Uuid::new_v4()))
        .bind(task_id)
        .bind(event.sequence)
        .bind(&event.event_type)
        .bind(event.payload.as_ref().map(|value| value.to_string()))
        .bind(&event.created_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for snapshot in restored_snapshots {
        let (Some(snapshot_id), Some(task_id)) = (
            snapshot_ids.get(&snapshot.id),
            task_ids.get(&snapshot.task_id),
        ) else {
            continue;
        };
        sqlx::query(
            "INSERT INTO generation_snapshots (id, task_id, workflow_json, recipe_yaml, user_inputs_json, resolved_inputs_json, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(snapshot_id)
        .bind(task_id)
        .bind(snapshot.workflow.to_string())
        .bind(&snapshot.recipe_yaml)
        .bind(snapshot.user_inputs.to_string())
        .bind(snapshot.resolved_inputs.to_string())
        .bind(&snapshot.created_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for asset in &document.assets {
        let Some(restored) = restored_assets.iter().find(|item| item.old_id == asset.id) else {
            continue;
        };
        sqlx::query(
            "INSERT INTO assets (id, project_id, type, category, name, original_name, storage_path, thumbnail_path,
             sha256, mime_type, width, height, duration_ms, file_size, source_task_id, metadata_json, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&restored.new_id)
        .bind(&project.id)
        .bind(&asset.asset_type)
        .bind(&asset.category)
        .bind(&asset.name)
        .bind(&asset.original_name)
        .bind(&restored.storage_path)
        .bind(&restored.thumbnail_path)
        .bind(&asset.sha256)
        .bind(&asset.mime_type)
        .bind(asset.width)
        .bind(asset.height)
        .bind(asset.duration_ms)
        .bind(asset.file_size)
        .bind(asset.source_task_id.as_ref().and_then(|id| task_ids.get(id)))
        .bind(asset.metadata.to_string())
        .bind(&asset.created_at)
        .bind(&asset.updated_at)
        .execute(&mut **transaction)
        .await
            .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for profile in &document.style_profiles {
        let profile_id =
            consistency_required_id(&consistency_ids.profiles, &profile.id, "Style Profile")?;
        let active_revision_id = consistency_optional_id(
            &consistency_ids.profile_revisions,
            profile.active_revision_id.as_ref(),
        );
        sqlx::query(
            "INSERT INTO style_profiles
             (id, project_id, name, style_prompt, color_prompt, line_prompt,
              negative_prompt, output_notes, active_revision_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(profile_id)
        .bind(&project.id)
        .bind(&profile.name)
        .bind(&profile.style_prompt)
        .bind(&profile.color_prompt)
        .bind(&profile.line_prompt)
        .bind(&profile.negative_prompt)
        .bind(&profile.output_notes)
        .bind(active_revision_id)
        .bind(&profile.created_at)
        .bind(&profile.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for reference_set in &document.reference_sets {
        let reference_set_id = consistency_required_id(
            &consistency_ids.reference_sets,
            &reference_set.id,
            "Reference Set",
        )?;
        let owner_profile_id = consistency_optional_id(
            &consistency_ids.profiles,
            reference_set.owner_profile_id.as_ref(),
        );
        let active_revision_id = consistency_optional_id(
            &consistency_ids.profile_revisions,
            reference_set.active_revision_id.as_ref(),
        );
        sqlx::query(
            "INSERT INTO reference_sets
             (id, project_id, name, purpose, description, owner_profile_type,
              owner_profile_id, active_revision_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(reference_set_id)
        .bind(&project.id)
        .bind(&reference_set.name)
        .bind(&reference_set.purpose)
        .bind(&reference_set.description)
        .bind(&reference_set.owner_profile_type)
        .bind(owner_profile_id)
        .bind(active_revision_id)
        .bind(&reference_set.created_at)
        .bind(&reference_set.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for profile in &document.character_profiles {
        let profile_id =
            consistency_required_id(&consistency_ids.profiles, &profile.id, "Character Profile")?;
        let default_style_profile_id = consistency_optional_id(
            &consistency_ids.profiles,
            profile.default_style_profile_id.as_ref(),
        );
        let default_reference_set_id = consistency_optional_id(
            &consistency_ids.reference_sets,
            profile.default_reference_set_id.as_ref(),
        );
        let active_revision_id = consistency_optional_id(
            &consistency_ids.profile_revisions,
            profile.active_revision_id.as_ref(),
        );
        sqlx::query(
            "INSERT INTO character_profiles
             (id, project_id, name, description, canonical_prompt, negative_prompt,
              default_style_profile_id, default_reference_set_id, active_revision_id,
              metadata_json, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(profile_id)
        .bind(&project.id)
        .bind(&profile.name)
        .bind(&profile.description)
        .bind(&profile.canonical_prompt)
        .bind(&profile.negative_prompt)
        .bind(default_style_profile_id)
        .bind(default_reference_set_id)
        .bind(active_revision_id)
        .bind(&profile.metadata_json)
        .bind(&profile.created_at)
        .bind(&profile.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for profile in &document.scene_profiles {
        let profile_id =
            consistency_required_id(&consistency_ids.profiles, &profile.id, "Scene Profile")?;
        let default_style_profile_id = consistency_optional_id(
            &consistency_ids.profiles,
            profile.default_style_profile_id.as_ref(),
        );
        let default_reference_set_id = consistency_optional_id(
            &consistency_ids.reference_sets,
            profile.default_reference_set_id.as_ref(),
        );
        let active_revision_id = consistency_optional_id(
            &consistency_ids.profile_revisions,
            profile.active_revision_id.as_ref(),
        );
        sqlx::query(
            "INSERT INTO scene_profiles
             (id, project_id, name, description, environment_prompt, lighting_prompt,
              negative_prompt, default_style_profile_id, default_reference_set_id,
              active_revision_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(profile_id)
        .bind(&project.id)
        .bind(&profile.name)
        .bind(&profile.description)
        .bind(&profile.environment_prompt)
        .bind(&profile.lighting_prompt)
        .bind(&profile.negative_prompt)
        .bind(default_style_profile_id)
        .bind(default_reference_set_id)
        .bind(active_revision_id)
        .bind(&profile.created_at)
        .bind(&profile.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for profile in &document.prop_profiles {
        let profile_id =
            consistency_required_id(&consistency_ids.profiles, &profile.id, "Prop Profile")?;
        let default_reference_set_id = consistency_optional_id(
            &consistency_ids.reference_sets,
            profile.default_reference_set_id.as_ref(),
        );
        let active_revision_id = consistency_optional_id(
            &consistency_ids.profile_revisions,
            profile.active_revision_id.as_ref(),
        );
        sqlx::query(
            "INSERT INTO prop_profiles
             (id, project_id, name, description, canonical_prompt, material_prompt,
              scale_prompt, default_reference_set_id, active_revision_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(profile_id)
        .bind(&project.id)
        .bind(&profile.name)
        .bind(&profile.description)
        .bind(&profile.canonical_prompt)
        .bind(&profile.material_prompt)
        .bind(&profile.scale_prompt)
        .bind(default_reference_set_id)
        .bind(active_revision_id)
        .bind(&profile.created_at)
        .bind(&profile.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for variant in &document.costume_variants {
        let variant_id = consistency_required_id(
            &consistency_ids.costume_variants,
            &variant.id,
            "Costume Variant",
        )?;
        let character_profile_id = consistency_required_id(
            &consistency_ids.profiles,
            &variant.character_profile_id,
            "Costume Character Profile",
        )?;
        let reference_set_id = consistency_optional_id(
            &consistency_ids.reference_sets,
            variant.reference_set_id.as_ref(),
        );
        let active_revision_id = consistency_optional_id(
            &consistency_ids.profile_revisions,
            variant.active_revision_id.as_ref(),
        );
        sqlx::query(
            "INSERT INTO costume_variants
             (id, character_profile_id, name, prompt_fragment, reference_set_id,
              is_default, ordinal, active_revision_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(variant_id)
        .bind(character_profile_id)
        .bind(&variant.name)
        .bind(&variant.prompt_fragment)
        .bind(reference_set_id)
        .bind(variant.is_default)
        .bind(variant.ordinal)
        .bind(active_revision_id)
        .bind(&variant.created_at)
        .bind(&variant.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for item in &document.reference_set_items {
        let reference_set_id = consistency_required_id(
            &consistency_ids.reference_sets,
            &item.reference_set_id,
            "Reference Set Item",
        )?;
        let asset_id = asset_ids
            .get(&item.asset_id)
            .ok_or_else(|| RepositoryError::integrity("Reference Set Item 素材映射缺失"))?;
        sqlx::query(
            "INSERT INTO reference_set_items
             (reference_set_id, asset_id, ordinal, role, is_primary, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(reference_set_id)
        .bind(asset_id)
        .bind(item.ordinal)
        .bind(&item.role)
        .bind(item.is_primary)
        .bind(&item.created_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for revision in &document.profile_revisions {
        let revision_id = consistency_required_id(
            &consistency_ids.profile_revisions,
            &revision.id,
            "Profile Revision",
        )?;
        let profile_id = consistency_required_id(
            &consistency_ids.profiles,
            &revision.profile_id,
            "Profile Revision",
        )?;
        sqlx::query(
            "INSERT INTO profile_revisions
             (id, profile_type, profile_id, revision_number, content_json,
              content_sha256, status, created_at, created_by)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(revision_id)
        .bind(&revision.profile_type)
        .bind(profile_id)
        .bind(revision.revision_number)
        .bind(&revision.content_json)
        .bind(&revision.content_sha256)
        .bind(&revision.status)
        .bind(&revision.created_at)
        .bind(&revision.created_by)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for anchor in &document.reference_anchors {
        let anchor_id = reference_anchor_ids
            .get(&anchor.id)
            .ok_or_else(|| RepositoryError::integrity("参考锚点 ID 映射缺失"))?;
        sqlx::query(
            "INSERT INTO reference_anchors
             (id, project_id, kind, name, normalized_name, description, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(anchor_id)
        .bind(&project.id)
        .bind(&anchor.kind)
        .bind(&anchor.name)
        .bind(&anchor.normalized_name)
        .bind(&anchor.description)
        .bind(&anchor.created_at)
        .bind(&anchor.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
        for asset in remap_reference_anchor_assets(anchor, asset_ids)? {
            sqlx::query(
                "INSERT INTO reference_anchor_assets (anchor_id, asset_id, ordinal, created_at)
                 VALUES (?, ?, ?, ?)",
            )
            .bind(anchor_id)
            .bind(&asset.asset_id)
            .bind(asset.ordinal)
            .bind(&asset.created_at)
            .execute(&mut **transaction)
            .await
            .map_err(|error| RepositoryError::database(error.to_string()))?;
        }
    }
    for prompt in &document.asset_video_prompts {
        let Some(asset_id) = asset_ids.get(&prompt.asset_id) else {
            continue;
        };
        sqlx::query(
            "INSERT INTO asset_video_prompts (asset_id, project_id, prompt_text, updated_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(asset_id)
        .bind(&project.id)
        .bind(&prompt.prompt_text)
        .bind(&prompt.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for tag in &document.asset_tags {
        let tag_id = tag_ids
            .get(&tag.id)
            .ok_or_else(|| RepositoryError::integrity("标签 ID 映射缺失"))?;
        sqlx::query("INSERT INTO asset_tags (id, project_id, name, normalized_name, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(tag_id).bind(&project.id).bind(&tag.name).bind(&tag.normalized_name).bind(&tag.created_at).bind(&tag.updated_at)
            .execute(&mut **transaction).await.map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for link in &document.asset_tag_links {
        let asset_id = asset_ids
            .get(&link.asset_id)
            .ok_or_else(|| RepositoryError::integrity("标签链接缺少资产映射"))?;
        let tag_id = tag_ids
            .get(&link.tag_id)
            .ok_or_else(|| RepositoryError::integrity("标签链接缺少标签映射"))?;
        sqlx::query("INSERT INTO asset_tag_links (asset_id, tag_id, project_id, created_at) VALUES (?, ?, ?, ?)")
            .bind(asset_id).bind(tag_id).bind(&project.id).bind(&link.created_at)
            .execute(&mut **transaction).await.map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for favorite in &document.asset_favorites {
        let asset_id = asset_ids
            .get(&favorite.asset_id)
            .ok_or_else(|| RepositoryError::integrity("收藏缺少资产映射"))?;
        sqlx::query(
            "INSERT INTO asset_favorites (asset_id, project_id, created_at) VALUES (?, ?, ?)",
        )
        .bind(asset_id)
        .bind(&project.id)
        .bind(&favorite.created_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    validate_restored_snapshot_asset_ownership(
        transaction,
        &project.id,
        restored_snapshots,
        asset_ids,
    )
    .await?;
    for mapping in &document.mappings {
        let (Some(task_id), Some(asset_id)) = (
            task_ids.get(&mapping.task_id),
            asset_ids.get(&mapping.asset_id),
        ) else {
            continue;
        };
        if !restored_assets
            .iter()
            .any(|asset| asset.old_id == mapping.asset_id)
        {
            continue;
        }
        sqlx::query("INSERT INTO task_output_assets (task_id, output_id, ordinal, asset_id, created_at) VALUES (?, ?, ?, ?, ?)")
            .bind(task_id).bind(&mapping.output_id).bind(mapping.ordinal).bind(asset_id).bind(&mapping.created_at)
            .execute(&mut **transaction).await.map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for preset in &document.presets {
        let Some(preset_id) = preset_ids.get(&preset.id) else {
            continue;
        };
        ensure_version_recipe_dependency(
            transaction,
            &preset.workflow_version_id,
            &preset.recipe_id,
        )
        .await?;
        sqlx::query("INSERT INTO presets (id, project_id, workflow_version_id, recipe_id, name, values_json, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(preset_id).bind(&project.id).bind(&preset.workflow_version_id).bind(&preset.recipe_id).bind(&preset.name).bind(preset.values.to_string()).bind(&preset.created_at).bind(&preset.updated_at)
            .execute(&mut **transaction).await.map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for batch in &document.batches {
        let Some(batch_id) = batch_ids.get(&batch.id) else {
            continue;
        };
        let status = if batch.status == "RUNNING" {
            "PAUSED"
        } else {
            &batch.status
        };
        sqlx::query("INSERT INTO production_batches (id, project_id, name, status, continue_on_failure, created_at, updated_at, archived_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(batch_id).bind(&project.id).bind(&batch.name).bind(status).bind(batch.continue_on_failure).bind(&batch.created_at).bind(&batch.updated_at).bind(&batch.archived_at)
            .execute(&mut **transaction).await.map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for item in &document.items {
        let (Some(item_id), Some(batch_id)) =
            (item_ids.get(&item.id), batch_ids.get(&item.batch_id))
        else {
            continue;
        };
        ensure_version_recipe_dependency(transaction, &item.workflow_version_id, &item.recipe_id)
            .await?;
        let linked_task = item.task_id.as_ref().and_then(|id| task_ids.get(id));
        let terminal = matches!(
            item.status.as_str(),
            "SUCCEEDED" | "FAILED" | "CANCELLED" | "SKIPPED"
        );
        let status = if terminal {
            item.status.clone()
        } else {
            "FAILED".to_owned()
        };
        let error_code = if terminal {
            item.error_code.clone()
        } else {
            Some("RESTORED_INCOMPLETE_TASK".to_owned())
        };
        let error_message = if terminal {
            item.error_message.clone()
        } else {
            Some("恢复时未自动重新提交生产队列项目。".to_owned())
        };
        sqlx::query("INSERT INTO production_batch_items (id, batch_id, ordinal, workflow_version_id, recipe_id, values_json, status, task_id, error_code, error_message, created_at, updated_at, retry_of_item_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(item_id).bind(batch_id).bind(item.ordinal).bind(&item.workflow_version_id).bind(&item.recipe_id).bind(item.values.to_string()).bind(status).bind(linked_task).bind(error_code).bind(error_message).bind(&item.created_at).bind(&item.updated_at).bind(item.retry_of_item_id.as_ref().and_then(|id| item_ids.get(id)))
            .execute(&mut **transaction).await.map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for template in &document.production_run_templates {
        let Some(template_id) = production_run_template_ids.get(&template.id) else {
            continue;
        };
        if let (Some(workflow_version_id), Some(recipe_id)) = (
            template.krea2_workflow_version_id.as_deref(),
            template.krea2_recipe_id.as_deref(),
        ) {
            ensure_version_recipe_dependency(transaction, workflow_version_id, recipe_id).await?;
        }
        if let (Some(workflow_version_id), Some(recipe_id)) = (
            template.h3_workflow_version_id.as_deref(),
            template.h3_recipe_id.as_deref(),
        ) {
            ensure_version_recipe_dependency(transaction, workflow_version_id, recipe_id).await?;
        }
        sqlx::query(
            "INSERT INTO production_run_templates
             (id, project_id, name, krea2_workflow_version_id, krea2_recipe_id, krea2_preset_id,
              default_image_count, h3_workflow_version_id, h3_recipe_id, h3_profile,
              default_duration_seconds, default_width, default_height, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(template_id)
        .bind(&project.id)
        .bind(&template.name)
        .bind(&template.krea2_workflow_version_id)
        .bind(&template.krea2_recipe_id)
        .bind(
            template
                .krea2_preset_id
                .as_ref()
                .and_then(|id| preset_ids.get(id)),
        )
        .bind(template.default_image_count)
        .bind(&template.h3_workflow_version_id)
        .bind(&template.h3_recipe_id)
        .bind(&template.h3_profile)
        .bind(template.default_duration_seconds)
        .bind(template.default_width)
        .bind(template.default_height)
        .bind(&template.created_at)
        .bind(&template.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for run in &document.production_runs {
        let Some(run_id) = production_run_ids.get(&run.id) else {
            continue;
        };
        let status = if run.status == "RUNNING" {
            "FAILED"
        } else {
            &run.status
        };
        sqlx::query(
            "INSERT INTO production_runs
             (id, project_id, name, status, current_stage_ordinal, template_id,
              created_at, updated_at, started_at, finished_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(run_id)
        .bind(&project.id)
        .bind(&run.name)
        .bind(status)
        .bind(run.current_stage_ordinal)
        .bind(
            run.template_id
                .as_ref()
                .and_then(|id| production_run_template_ids.get(id)),
        )
        .bind(&run.created_at)
        .bind(&run.updated_at)
        .bind(&run.started_at)
        .bind(&run.finished_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for stage in &document.production_stages {
        let (Some(stage_id), Some(run_id)) = (
            production_stage_ids.get(&stage.id),
            production_run_ids.get(&stage.run_id),
        ) else {
            continue;
        };
        if let (Some(workflow_version_id), Some(recipe_id)) = (
            stage.workflow_version_id.as_deref(),
            stage.recipe_id.as_deref(),
        ) {
            ensure_version_recipe_dependency(transaction, workflow_version_id, recipe_id).await?;
        }
        let status = if stage.status == "RUNNING" {
            "FAILED"
        } else {
            &stage.status
        };
        let mut frozen_config = stage.frozen_config.clone();
        remap_snapshot_asset_references(&mut frozen_config, asset_ids);
        sqlx::query(
            "INSERT INTO production_stages
             (id, run_id, ordinal, stage_type, status, workflow_version_id, recipe_id,
              production_batch_id, frozen_config_json, prompt, created_at, updated_at,
              started_at, finished_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(stage_id)
        .bind(run_id)
        .bind(stage.ordinal)
        .bind(&stage.stage_type)
        .bind(status)
        .bind(&stage.workflow_version_id)
        .bind(&stage.recipe_id)
        .bind(
            stage
                .production_batch_id
                .as_ref()
                .and_then(|id| batch_ids.get(id)),
        )
        .bind(frozen_config.to_string())
        .bind(&stage.prompt)
        .bind(&stage.created_at)
        .bind(&stage.updated_at)
        .bind(&stage.started_at)
        .bind(&stage.finished_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for item in &document.production_stage_items {
        let (Some(item_id), Some(stage_id)) = (
            production_stage_item_ids.get(&item.id),
            production_stage_ids.get(&item.stage_id),
        ) else {
            continue;
        };
        let status = if matches!(item.status.as_str(), "PENDING" | "READY" | "RUNNING") {
            "FAILED"
        } else {
            &item.status
        };
        let mut frozen_values = item.frozen_values.clone();
        remap_snapshot_asset_references(&mut frozen_values, asset_ids);
        sqlx::query(
            "INSERT INTO production_stage_items
             (id, stage_id, ordinal, status, production_batch_item_id, task_id, asset_id,
              source_asset_id, reference_index, attempt, submission_idempotency_key,
              parent_stage_item_id, frozen_values_json, error_code, error_message,
              created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(item_id)
        .bind(stage_id)
        .bind(item.ordinal)
        .bind(status)
        .bind(
            item.production_batch_item_id
                .as_ref()
                .and_then(|id| item_ids.get(id)),
        )
        .bind(item.task_id.as_ref().and_then(|id| task_ids.get(id)))
        .bind(item.asset_id.as_ref().and_then(|id| asset_ids.get(id)))
        .bind(
            item.source_asset_id
                .as_ref()
                .and_then(|id| asset_ids.get(id)),
        )
        .bind(item.reference_index)
        .bind(item.attempt)
        .bind(&item.submission_idempotency_key)
        .bind(
            item.parent_stage_item_id
                .as_ref()
                .and_then(|id| production_stage_item_ids.get(id)),
        )
        .bind(frozen_values.to_string())
        .bind(&item.error_code)
        .bind(&item.error_message)
        .bind(&item.created_at)
        .bind(&item.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for review in &document.production_item_reviews {
        let (Some(item_id), Some(batch_id)) = (
            item_ids.get(&review.production_batch_item_id),
            batch_ids.get(&review.production_batch_id),
        ) else {
            continue;
        };
        let lineage_key = item_ids
            .get(&review.lineage_key)
            .cloned()
            .unwrap_or_else(|| review.lineage_key.clone());
        sqlx::query(
            "INSERT INTO production_item_reviews
             (id, project_id, production_batch_id, production_batch_item_id, task_id,
              result_asset_id, review_status, review_note, version, lineage_key,
              parent_batch_id, parent_item_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(format!("pri_{}", Uuid::new_v4().simple()))
        .bind(&project.id)
        .bind(batch_id)
        .bind(item_id)
        .bind(review.task_id.as_ref().and_then(|id| task_ids.get(id)))
        .bind(
            review
                .result_asset_id
                .as_ref()
                .and_then(|id| asset_ids.get(id)),
        )
        .bind(&review.review_status)
        .bind(&review.review_note)
        .bind(review.version)
        .bind(lineage_key)
        .bind(
            review
                .parent_batch_id
                .as_ref()
                .and_then(|id| batch_ids.get(id)),
        )
        .bind(
            review
                .parent_item_id
                .as_ref()
                .and_then(|id| item_ids.get(id)),
        )
        .bind(&review.created_at)
        .bind(&review.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for experiment in &document.benchmark_experiments {
        let experiment_id = benchmark_experiment_ids
            .get(&experiment.id)
            .ok_or_else(|| RepositoryError::integrity("Benchmark 实验 ID 映射缺失"))?;
        let status = match experiment.status.as_str() {
            "QUEUED" | "RUNNING" => "FAILED_TO_QUEUE",
            other => other,
        };
        let winner_candidate_id = experiment
            .winner_candidate_id
            .as_ref()
            .and_then(|id| benchmark_candidate_ids.get(id));
        let production_batch_id = experiment
            .production_batch_id
            .as_ref()
            .and_then(|id| batch_ids.get(id));
        let mut base_values = experiment.base_values.clone();
        remap_snapshot_asset_references(&mut base_values, asset_ids);
        let asset_ids = experiment
            .asset_ids
            .iter()
            .filter_map(|id| asset_ids.get(id))
            .cloned()
            .collect::<Vec<_>>();
        sqlx::query(
            "INSERT INTO benchmark_experiments
             (id, project_id, name, media_type, status, base_values_json, asset_ids_json,
              winner_candidate_id, production_batch_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(experiment_id)
        .bind(&project.id)
        .bind(&experiment.name)
        .bind(&experiment.media_type)
        .bind(status)
        .bind(base_values.to_string())
        .bind(serde_json::to_string(&asset_ids).map_err(|error| {
            RepositoryError::integrity(format!("Benchmark 素材序列化失败：{error}"))
        })?)
        .bind(winner_candidate_id)
        .bind(production_batch_id)
        .bind(&experiment.created_at)
        .bind(&experiment.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for candidate in &document.benchmark_candidates {
        let candidate_id = benchmark_candidate_ids
            .get(&candidate.id)
            .ok_or_else(|| RepositoryError::integrity("Benchmark 候选 ID 映射缺失"))?;
        let experiment_id = benchmark_experiment_ids
            .get(&candidate.experiment_id)
            .ok_or_else(|| RepositoryError::integrity("Benchmark 候选缺少实验映射"))?;
        ensure_version_recipe_dependency(
            transaction,
            &candidate.workflow_version_id,
            &candidate.recipe_id,
        )
        .await?;
        let mut values = candidate.values.clone();
        remap_snapshot_asset_references(&mut values, asset_ids);
        let restored_asset_ids = candidate
            .asset_ids
            .iter()
            .filter_map(|id| asset_ids.get(id))
            .cloned()
            .collect::<Vec<_>>();
        sqlx::query(
            "INSERT INTO benchmark_candidates
             (id, experiment_id, position, workflow_version_id, recipe_id, preset_id,
              preset_name, label, values_json, asset_ids_json, production_batch_item_id,
              task_id, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(candidate_id)
        .bind(experiment_id)
        .bind(candidate.position)
        .bind(&candidate.workflow_version_id)
        .bind(&candidate.recipe_id)
        .bind(
            candidate
                .preset_id
                .as_ref()
                .and_then(|id| preset_ids.get(id)),
        )
        .bind(&candidate.preset_name)
        .bind(&candidate.label)
        .bind(values.to_string())
        .bind(serde_json::to_string(&restored_asset_ids).map_err(|error| {
            RepositoryError::integrity(format!("Benchmark 候选素材序列化失败：{error}"))
        })?)
        .bind(
            candidate
                .production_batch_item_id
                .as_ref()
                .and_then(|id| item_ids.get(id)),
        )
        .bind(candidate.task_id.as_ref().and_then(|id| task_ids.get(id)))
        .bind(&candidate.created_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for run in &document.benchmark_runs {
        let (Some(run_id), Some(experiment_id), Some(candidate_id)) = (
            benchmark_run_ids.get(&run.id),
            benchmark_experiment_ids.get(&run.experiment_id),
            benchmark_candidate_ids.get(&run.candidate_id),
        ) else {
            continue;
        };
        sqlx::query(
            "INSERT INTO benchmark_runs
             (id, experiment_id, candidate_id, run_number, production_batch_item_id, task_id,
              snapshot_id, output_asset_id, generation_execution_id, compiled_workflow_sha256,
              runtime_profile, concurrency_class, queue_wait_ms, prepare_ms, submit_ms,
              comfy_execution_ms, collect_ms, total_ms, status, error_code, output_file_size,
              created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(run_id)
        .bind(experiment_id)
        .bind(candidate_id)
        .bind(run.run_number)
        .bind(
            run.production_batch_item_id
                .as_ref()
                .and_then(|id| item_ids.get(id)),
        )
        .bind(run.task_id.as_ref().and_then(|id| task_ids.get(id)))
        .bind(run.snapshot_id.as_ref().and_then(|id| snapshot_ids.get(id)))
        .bind(
            run.output_asset_id
                .as_ref()
                .and_then(|id| asset_ids.get(id)),
        )
        .bind(&run.generation_execution_id)
        .bind(&run.compiled_workflow_sha256)
        .bind(&run.runtime_profile)
        .bind(&run.concurrency_class)
        .bind(run.queue_wait_ms)
        .bind(run.prepare_ms)
        .bind(run.submit_ms)
        .bind(run.comfy_execution_ms)
        .bind(run.collect_ms)
        .bind(run.total_ms)
        .bind(&run.status)
        .bind(&run.error_code)
        .bind(run.output_file_size)
        .bind(&run.created_at)
        .bind(&run.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for score in &document.benchmark_quality_scores {
        let (Some(score_id), Some(candidate_id)) = (
            benchmark_quality_score_ids.get(&score.id),
            benchmark_candidate_ids.get(&score.candidate_id),
        ) else {
            continue;
        };
        sqlx::query(
            "INSERT INTO benchmark_quality_scores
             (id, candidate_id, prompt_adherence, visual_quality, motion_quality,
              reference_consistency, overall, note, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(score_id)
        .bind(candidate_id)
        .bind(score.prompt_adherence)
        .bind(score.visual_quality)
        .bind(score.motion_quality)
        .bind(score.reference_consistency)
        .bind(score.overall)
        .bind(&score.note)
        .bind(&score.created_at)
        .bind(&score.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for series in &document.production_series {
        let series_id = production_structure_ids
            .series
            .get(&series.id)
            .ok_or_else(|| RepositoryError::integrity("Series ID 映射缺失"))?;
        sqlx::query(
            "INSERT INTO production_series
             (id, project_id, ordinal, name, description, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(series_id)
        .bind(&project.id)
        .bind(series.ordinal)
        .bind(&series.name)
        .bind(&series.description)
        .bind(&series.created_at)
        .bind(&series.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for episode in &document.production_episodes {
        let episode_id = production_structure_ids
            .episodes
            .get(&episode.id)
            .ok_or_else(|| RepositoryError::integrity("Episode ID 映射缺失"))?;
        let series_id = production_structure_ids
            .series
            .get(&episode.series_id)
            .ok_or_else(|| RepositoryError::integrity("Episode 缺少 Series 映射"))?;
        sqlx::query(
            "INSERT INTO production_episodes
             (id, series_id, ordinal, name, description, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(episode_id)
        .bind(series_id)
        .bind(episode.ordinal)
        .bind(&episode.name)
        .bind(&episode.description)
        .bind(&episode.created_at)
        .bind(&episode.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for scene in &document.production_scenes {
        let scene_id = production_structure_ids
            .scenes
            .get(&scene.id)
            .ok_or_else(|| RepositoryError::integrity("Scene ID 映射缺失"))?;
        let episode_id = production_structure_ids
            .episodes
            .get(&scene.episode_id)
            .ok_or_else(|| RepositoryError::integrity("Scene 缺少 Episode 映射"))?;
        sqlx::query(
            "INSERT INTO production_scenes
             (id, episode_id, ordinal, name, description, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(scene_id)
        .bind(episode_id)
        .bind(scene.ordinal)
        .bind(&scene.name)
        .bind(&scene.description)
        .bind(&scene.created_at)
        .bind(&scene.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for shot in &document.shots {
        let shot_id = shot_ids
            .get(&shot.id)
            .ok_or_else(|| RepositoryError::integrity("镜头 ID 映射缺失"))?;
        let prompt_entry_id = shot
            .prompt_entry_id
            .as_ref()
            .and_then(|id| prompt_ids.get(id))
            .map(String::as_str);
        let prompt_version_id = shot
            .prompt_version_id
            .as_ref()
            .and_then(|id| prompt_version_ids.get(id))
            .map(String::as_str);
        let selected_image_asset_id = shot
            .selected_image_asset_id
            .as_ref()
            .and_then(|id| asset_ids.get(id))
            .map(String::as_str);
        let selected_video_asset_id = shot
            .selected_video_asset_id
            .as_ref()
            .and_then(|id| asset_ids.get(id))
            .map(String::as_str);
        sqlx::query(
            "INSERT INTO shots (id, project_id, ordinal, name, prompt_text, prompt_entry_id,
             prompt_version_id, selected_image_asset_id, selected_video_asset_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(shot_id)
        .bind(&project.id)
        .bind(shot.ordinal)
        .bind(&shot.name)
        .bind(&shot.prompt_text)
        .bind(prompt_entry_id)
        .bind(prompt_version_id)
        .bind(selected_image_asset_id)
        .bind(selected_video_asset_id)
        .bind(&shot.created_at)
        .bind(&shot.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    // `snapshot_json` is immutable historical evidence. Only the outer
    // relational IDs are remapped so live associations point at the restored
    // project; IDs inside the evidence are intentionally left untouched.
    for snapshot in &document.preparation_snapshots {
        let snapshot_id = preparation_snapshot_ids
            .get(&snapshot.id)
            .ok_or_else(|| RepositoryError::integrity("Preparation Snapshot ID 映射缺失"))?;
        let shot_id = shot_ids
            .get(&snapshot.shot_id)
            .ok_or_else(|| RepositoryError::integrity("Preparation Snapshot 镜头映射缺失"))?;
        let batch_id = batch_ids
            .get(&snapshot.production_batch_id)
            .ok_or_else(|| RepositoryError::integrity("Preparation Snapshot 批次映射缺失"))?;
        let item_id = item_ids
            .get(&snapshot.production_batch_item_id)
            .ok_or_else(|| RepositoryError::integrity("Preparation Snapshot 项目映射缺失"))?;
        sqlx::query(
            "INSERT INTO production_preparation_snapshots
             (id, project_id, shot_id, stage, context_hash, production_batch_id,
              production_batch_item_id, snapshot_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(snapshot_id)
        .bind(&project.id)
        .bind(shot_id)
        .bind(&snapshot.stage)
        .bind(&snapshot.context_hash)
        .bind(batch_id)
        .bind(item_id)
        .bind(&snapshot.snapshot_json)
        .bind(&snapshot.created_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for binding in &document.shot_profile_bindings {
        let binding_id = consistency_required_id(
            &consistency_ids.shot_profile_bindings,
            &binding.id,
            "Shot Profile Binding",
        )?;
        let shot_id = shot_ids
            .get(&binding.shot_id)
            .ok_or_else(|| RepositoryError::integrity("Shot Profile Binding 镜头映射缺失"))?;
        let profile_id = consistency_required_id(
            &consistency_ids.profiles,
            &binding.profile_id,
            "Shot Profile Binding",
        )?;
        let costume_variant_id = consistency_optional_id(
            &consistency_ids.costume_variants,
            binding.costume_variant_id.as_ref(),
        );
        sqlx::query(
            "INSERT INTO shot_profile_bindings
             (id, shot_id, role, profile_type, profile_id, costume_variant_id,
              ordinal, inheritance_mode, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(binding_id)
        .bind(shot_id)
        .bind(&binding.role)
        .bind(&binding.profile_type)
        .bind(profile_id)
        .bind(costume_variant_id)
        .bind(binding.ordinal)
        .bind(&binding.inheritance_mode)
        .bind(&binding.created_at)
        .bind(&binding.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for binding in &document.shot_reference_set_bindings {
        let binding_id = consistency_required_id(
            &consistency_ids.shot_reference_set_bindings,
            &binding.id,
            "Shot Reference Set Binding",
        )?;
        let shot_id = shot_ids
            .get(&binding.shot_id)
            .ok_or_else(|| RepositoryError::integrity("Shot Reference Set Binding 镜头映射缺失"))?;
        let reference_set_id = consistency_required_id(
            &consistency_ids.reference_sets,
            &binding.reference_set_id,
            "Shot Reference Set Binding",
        )?;
        sqlx::query(
            "INSERT INTO shot_reference_set_bindings
             (id, shot_id, role, reference_set_id, ordinal, required,
              inheritance_mode, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(binding_id)
        .bind(shot_id)
        .bind(&binding.role)
        .bind(reference_set_id)
        .bind(binding.ordinal)
        .bind(binding.required)
        .bind(&binding.inheritance_mode)
        .bind(&binding.created_at)
        .bind(&binding.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for binding in &document.scope_profile_bindings {
        let binding_id = consistency_required_id(
            &consistency_ids.scope_profile_bindings,
            &binding.id,
            "Scope Profile Binding",
        )?;
        if binding.project_id != document.project.id {
            return Err(RepositoryError::integrity(
                "Scope Profile Binding 项目归属不一致",
            ));
        }
        let scope_id = remap_consistency_scope_id(
            &binding.scope_type,
            &binding.scope_id,
            &document.project.id,
            &project.id,
            production_structure_ids,
        )?;
        let profile_id = consistency_required_id(
            &consistency_ids.profiles,
            &binding.profile_id,
            "Scope Profile Binding",
        )?;
        let costume_variant_id = consistency_optional_id(
            &consistency_ids.costume_variants,
            binding.costume_variant_id.as_ref(),
        );
        sqlx::query(
            "INSERT INTO consistency_scope_profile_bindings
             (id, project_id, scope_type, scope_id, role, profile_type,
              profile_id, costume_variant_id, ordinal, inheritance_mode,
              created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(binding_id)
        .bind(&project.id)
        .bind(&binding.scope_type)
        .bind(scope_id)
        .bind(&binding.role)
        .bind(&binding.profile_type)
        .bind(profile_id)
        .bind(costume_variant_id)
        .bind(binding.ordinal)
        .bind(&binding.inheritance_mode)
        .bind(&binding.created_at)
        .bind(&binding.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for binding in &document.scope_reference_set_bindings {
        let binding_id = consistency_required_id(
            &consistency_ids.scope_reference_set_bindings,
            &binding.id,
            "Scope Reference Set Binding",
        )?;
        if binding.project_id != document.project.id {
            return Err(RepositoryError::integrity(
                "Scope Reference Set Binding 项目归属不一致",
            ));
        }
        let scope_id = remap_consistency_scope_id(
            &binding.scope_type,
            &binding.scope_id,
            &document.project.id,
            &project.id,
            production_structure_ids,
        )?;
        let reference_set_id = consistency_required_id(
            &consistency_ids.reference_sets,
            &binding.reference_set_id,
            "Scope Reference Set Binding",
        )?;
        sqlx::query(
            "INSERT INTO consistency_scope_reference_set_bindings
             (id, project_id, scope_type, scope_id, role, reference_set_id,
              ordinal, required, inheritance_mode, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(binding_id)
        .bind(&project.id)
        .bind(&binding.scope_type)
        .bind(scope_id)
        .bind(&binding.role)
        .bind(reference_set_id)
        .bind(binding.ordinal)
        .bind(binding.required)
        .bind(&binding.inheritance_mode)
        .bind(&binding.created_at)
        .bind(&binding.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for assignment in &document.shot_scene_assignments {
        let shot_id = shot_ids
            .get(&assignment.shot_id)
            .ok_or_else(|| RepositoryError::integrity("Scene Assignment 缺少镜头映射"))?;
        let scene_id = production_structure_ids
            .scenes
            .get(&assignment.scene_id)
            .ok_or_else(|| RepositoryError::integrity("Scene Assignment 缺少 Scene 映射"))?;
        sqlx::query(
            "INSERT INTO shot_scene_assignments
             (shot_id, scene_id, ordinal, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(shot_id)
        .bind(scene_id)
        .bind(assignment.ordinal)
        .bind(&assignment.created_at)
        .bind(&assignment.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for source in &document.script_sources {
        let source_id = script_source_ids
            .get(&source.id)
            .ok_or_else(|| RepositoryError::integrity("Script Source ID 映射缺失"))?;
        sqlx::query(
            "INSERT INTO script_sources
             (id, project_id, format, original_filename, source_checksum, source_bytes, source_text,
              schema_version, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(source_id)
        .bind(&project.id)
        .bind(&source.format)
        .bind(&source.original_filename)
        .bind(&source.source_checksum)
        .bind(source.source_bytes)
        .bind(&source.source_text)
        .bind(source.schema_version)
        .bind(&source.created_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    let mut script_draft_revisions = document.script_draft_revisions.clone();
    script_draft_revisions.sort_by(|left, right| {
        left.draft_id
            .cmp(&right.draft_id)
            .then(left.revision.cmp(&right.revision))
            .then(left.id.cmp(&right.id))
    });
    for revision in &script_draft_revisions {
        let revision_id = script_revision_ids
            .get(&revision.id)
            .ok_or_else(|| RepositoryError::integrity("Script Draft Revision ID 映射缺失"))?;
        let draft_id = script_draft_ids
            .get(&revision.draft_id)
            .ok_or_else(|| RepositoryError::integrity("Script Draft ID 映射缺失"))?;
        let source_id = script_source_ids
            .get(&revision.source_id)
            .ok_or_else(|| RepositoryError::integrity("Script Draft Source 映射缺失"))?;
        let previous_revision_id = revision
            .previous_revision_id
            .as_ref()
            .map(|id| {
                script_revision_ids.get(id).ok_or_else(|| {
                    RepositoryError::integrity("Script Draft previous revision 映射缺失")
                })
            })
            .transpose()?;
        let mut payload = serde_json::from_str::<Value>(&revision.payload_json)
            .map_err(|_| RepositoryError::integrity("Script Draft payload JSON 无效"))?;
        let mut script_id_map = HashMap::new();
        script_id_map.extend(
            script_source_ids
                .iter()
                .map(|(old, new)| (old.clone(), new.clone())),
        );
        script_id_map.extend(
            script_draft_ids
                .iter()
                .map(|(old, new)| (old.clone(), new.clone())),
        );
        script_id_map.extend(
            script_revision_ids
                .iter()
                .map(|(old, new)| (old.clone(), new.clone())),
        );
        remap_exact_string_ids(&mut payload, &script_id_map);
        let payload_json = serde_json::to_string(&payload)
            .map_err(|_| RepositoryError::integrity("Script Draft payload 序列化失败"))?;
        let payload_checksum = hash_bytes(payload_json.as_bytes());
        sqlx::query(
            "INSERT INTO script_import_drafts
             (id, draft_id, project_id, source_id, revision, previous_revision_id,
              schema_version, revision_kind, parser_version, contract_version,
              provider_kind, provider_model, provider_metadata_json,
              payload_checksum, summary_json, payload_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(revision_id)
        .bind(draft_id)
        .bind(&project.id)
        .bind(source_id)
        .bind(revision.revision)
        .bind(previous_revision_id)
        .bind(revision.schema_version)
        .bind(&revision.revision_kind)
        .bind(&revision.parser_version)
        .bind(revision.contract_version)
        .bind(&revision.provider_kind)
        .bind(&revision.provider_model)
        .bind(&revision.provider_metadata_json)
        .bind(payload_checksum)
        .bind(&revision.summary_json)
        .bind(payload_json)
        .bind(&revision.created_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for config in &document.shot_stage_configs {
        let shot_id = shot_ids
            .get(&config.shot_id)
            .ok_or_else(|| RepositoryError::integrity("镜头阶段配置缺少镜头映射"))?;
        ensure_workflow_dependency(
            transaction,
            &WorkflowReference {
                workflow_id: config.workflow_id.clone(),
                workflow_version_id: config.workflow_version_id.clone(),
                recipe_id: config.recipe_id.clone(),
            },
        )
        .await?;
        sqlx::query(
            "INSERT INTO shot_stage_configs
             (shot_id, stage, workflow_version_id, recipe_id, scalar_values_json, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(shot_id)
        .bind(&config.stage)
        .bind(&config.workflow_version_id)
        .bind(&config.recipe_id)
        .bind(config.scalar_values.to_string())
        .bind(&config.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for prompt in &document.shot_stage_prompts {
        let shot_id = shot_ids
            .get(&prompt.shot_id)
            .ok_or_else(|| RepositoryError::integrity("镜头阶段 Prompt 缺少镜头映射"))?;
        let prompt_entry_id = prompt
            .prompt_entry_id
            .as_ref()
            .and_then(|id| prompt_ids.get(id))
            .map(String::as_str);
        let prompt_version_id = prompt
            .prompt_version_id
            .as_ref()
            .and_then(|id| prompt_version_ids.get(id))
            .map(String::as_str);
        sqlx::query(
            "INSERT INTO shot_stage_prompts
             (shot_id, stage, prompt_text, prompt_entry_id, prompt_version_id, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(shot_id)
        .bind(&prompt.stage)
        .bind(&prompt.prompt_text)
        .bind(prompt_entry_id)
        .bind(prompt_version_id)
        .bind(&prompt.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for reference in &document.shot_reference_assets {
        let shot_id = shot_ids
            .get(&reference.shot_id)
            .ok_or_else(|| RepositoryError::integrity("镜头 Reference 缺少镜头映射"))?;
        let asset_id = asset_ids
            .get(&reference.asset_id)
            .ok_or_else(|| RepositoryError::integrity("镜头 Reference 缺少素材映射"))?;
        sqlx::query(
            "INSERT INTO shot_reference_assets (shot_id, stage, asset_id, ordinal)
             VALUES (?, ?, ?, ?)",
        )
        .bind(shot_id)
        .bind(&reference.stage)
        .bind(asset_id)
        .bind(reference.ordinal)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    for link in &document.shot_generation_links {
        let link_id = shot_generation_link_ids
            .get(&link.id)
            .ok_or_else(|| RepositoryError::integrity("镜头生成关联 ID 映射缺失"))?;
        let shot_id = shot_ids
            .get(&link.shot_id)
            .ok_or_else(|| RepositoryError::integrity("镜头生成关联缺少镜头映射"))?;
        let task_id = link
            .task_id
            .as_ref()
            .and_then(|id| task_ids.get(id))
            .map(String::as_str);
        let item_id = link
            .production_batch_item_id
            .as_ref()
            .and_then(|id| item_ids.get(id))
            .map(String::as_str);
        sqlx::query(
            "INSERT INTO shot_generation_links
             (id, shot_id, stage, task_id, production_batch_item_id, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(link_id)
        .bind(shot_id)
        .bind(&link.stage)
        .bind(task_id)
        .bind(item_id)
        .bind(&link.created_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    Ok(())
}

async fn validate_restored_snapshot_asset_ownership(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
    snapshots: &[BackupSnapshot],
    asset_ids: &HashMap<String, String>,
) -> Result<(), RepositoryError> {
    let restored_asset_ids = asset_ids.values().cloned().collect::<HashSet<_>>();
    let references = snapshots
        .iter()
        .flat_map(|snapshot| {
            collect_exact_asset_id_references(&snapshot.user_inputs, &restored_asset_ids)
                .into_iter()
                .chain(collect_exact_asset_id_references(
                    &snapshot.resolved_inputs,
                    &restored_asset_ids,
                ))
        })
        .collect::<HashSet<_>>();

    for asset_id in references {
        let owned = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM assets WHERE id = ? AND project_id = ?",
        )
        .bind(&asset_id)
        .bind(project_id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
        if owned == 0 {
            return Err(RepositoryError::integrity(
                "恢复后的任务快照引用了不属于当前项目的素材，恢复已取消。",
            ));
        }
    }

    Ok(())
}

async fn ensure_workflow_dependency(
    transaction: &mut Transaction<'_, Sqlite>,
    reference: &WorkflowReference,
) -> Result<(), RepositoryError> {
    let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflows WHERE id = ?")
        .bind(&reference.workflow_id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    if exists == 0 {
        sqlx::query("INSERT INTO workflows (id, name, category, mode, current_version_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&reference.workflow_id).bind("已恢复历史工作流").bind("restored").bind("api").bind(&reference.workflow_version_id).bind(Utc::now().to_rfc3339()).bind(Utc::now().to_rfc3339())
            .execute(&mut **transaction).await.map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    ensure_version_recipe_dependency(
        transaction,
        &reference.workflow_version_id,
        &reference.recipe_id,
    )
    .await
}

async fn restore_workflow_registry(
    transaction: &mut Transaction<'_, Sqlite>,
    snapshot: &BackupWorkflowRegistry,
) -> Result<(), RepositoryError> {
    for workflow in &snapshot.workflows {
        let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflows WHERE id = ?")
            .bind(&workflow.id)
            .fetch_one(&mut **transaction)
            .await
            .map_err(|error| RepositoryError::database(error.to_string()))?;
        if exists == 0 {
            sqlx::query(
                "INSERT INTO workflows
                 (id, name, category, mode, source_kind, library_state, current_version_id,
                  removed_at, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&workflow.id)
            .bind(&workflow.name)
            .bind(&workflow.category)
            .bind(&workflow.mode)
            .bind(&workflow.source_kind)
            .bind(&workflow.library_state)
            .bind(&workflow.current_version_id)
            .bind(&workflow.removed_at)
            .bind(&workflow.created_at)
            .bind(&workflow.updated_at)
            .execute(&mut **transaction)
            .await
            .map_err(|error| RepositoryError::database(error.to_string()))?;
        }
    }

    for version in &snapshot.versions {
        let existing = sqlx::query_as::<_, (String, String)>(
            "SELECT workflow_id, workflow_sha256 FROM workflow_versions WHERE id = ?",
        )
        .bind(&version.id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
        if let Some((workflow_id, workflow_sha256)) = existing {
            if workflow_id != version.workflow_id || workflow_sha256 != version.workflow_sha256 {
                return Err(RepositoryError::integrity(format!(
                    "工作流版本 {} 的不可变身份与当前数据库冲突",
                    version.id
                )));
            }
            continue;
        }
        sqlx::query(
            "INSERT INTO workflow_versions
             (id, workflow_id, version, api_workflow_json, workflow_sha256, package_name,
              package_source_path, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&version.id)
        .bind(&version.workflow_id)
        .bind(&version.version)
        .bind(&version.api_workflow_json)
        .bind(&version.workflow_sha256)
        .bind(&version.package_name)
        .bind(&version.package_source_path)
        .bind(&version.created_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }

    for recipe in &snapshot.recipes {
        let existing = sqlx::query_as::<_, (String, String)>(
            "SELECT workflow_version_id, recipe_sha256 FROM recipes WHERE id = ?",
        )
        .bind(&recipe.id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
        if let Some((workflow_version_id, recipe_sha256)) = existing {
            if workflow_version_id != recipe.workflow_version_id
                || recipe_sha256 != recipe.recipe_sha256
            {
                return Err(RepositoryError::integrity(format!(
                    "Recipe {} 的不可变身份与当前数据库冲突",
                    recipe.id
                )));
            }
            continue;
        }
        sqlx::query(
            "INSERT INTO recipes
             (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256,
              created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&recipe.id)
        .bind(&recipe.workflow_version_id)
        .bind(&recipe.version)
        .bind(recipe.schema_version)
        .bind(&recipe.recipe_yaml)
        .bind(&recipe.recipe_sha256)
        .bind(&recipe.created_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }

    for artifact in &snapshot.runtime_artifacts {
        let existing = sqlx::query_as::<_, BackupWorkflowRuntimeArtifact>(
            "SELECT id, workflow_version_id, recipe_id, package_name, source_kind,
                    package_source_path, workflow_sha256, recipe_sha256, created_at
             FROM workflow_runtime_artifacts
             WHERE package_name = ?",
        )
        .bind(&artifact.package_name)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
        if let Some(existing) = existing {
            if existing.id != artifact.id
                || existing.workflow_version_id != artifact.workflow_version_id
                || existing.recipe_id != artifact.recipe_id
                || existing.workflow_sha256 != artifact.workflow_sha256
                || existing.recipe_sha256 != artifact.recipe_sha256
            {
                return Err(RepositoryError::integrity(format!(
                    "Runtime Artifact {} 的精确映射与当前数据库冲突",
                    artifact.package_name
                )));
            }
            continue;
        }
        sqlx::query(
            "INSERT INTO workflow_runtime_artifacts
             (id, workflow_version_id, recipe_id, package_name, source_kind,
              package_source_path, workflow_sha256, recipe_sha256, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&artifact.id)
        .bind(&artifact.workflow_version_id)
        .bind(&artifact.recipe_id)
        .bind(&artifact.package_name)
        .bind(&artifact.source_kind)
        .bind(&artifact.package_source_path)
        .bind(&artifact.workflow_sha256)
        .bind(&artifact.recipe_sha256)
        .bind(&artifact.created_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    Ok(())
}

async fn ensure_version_recipe_dependency(
    transaction: &mut Transaction<'_, Sqlite>,
    workflow_version_id: &str,
    recipe_id: &str,
) -> Result<(), RepositoryError> {
    let version = sqlx::query_as::<_, (String, String)>(
        "SELECT workflow_id, version FROM workflow_versions WHERE id = ?",
    )
    .bind(workflow_version_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::database(error.to_string()))?;
    if version.is_none() {
        let workflow_id = format!("wf_restored_{}", Uuid::new_v4());
        sqlx::query("INSERT OR IGNORE INTO workflows (id, name, category, mode, current_version_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&workflow_id).bind("已恢复历史工作流").bind("restored").bind("api").bind(workflow_version_id).bind(Utc::now().to_rfc3339()).bind(Utc::now().to_rfc3339())
            .execute(&mut **transaction).await.map_err(|error| RepositoryError::database(error.to_string()))?;
        sqlx::query("INSERT OR IGNORE INTO workflow_versions (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(workflow_version_id).bind(&workflow_id).bind("restored").bind("{}").bind(hash_bytes(b"{}")).bind(Utc::now().to_rfc3339())
            .execute(&mut **transaction).await.map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    let recipe_exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM recipes WHERE id = ?")
        .bind(recipe_id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(|error| RepositoryError::database(error.to_string()))?;
    if recipe_exists == 0 {
        sqlx::query("INSERT INTO recipes (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(recipe_id).bind(workflow_version_id).bind("restored").bind(1_i64).bind("schema_version: 1\ninputs: {}\n").bind(hash_bytes(b"schema_version: 1\ninputs: {}\n")).bind(Utc::now().to_rfc3339())
            .execute(&mut **transaction).await.map_err(|error| RepositoryError::database(error.to_string()))?;
    }
    Ok(())
}

impl<'r> FromRow<'r, sqlx::sqlite::SqliteRow> for BackupWorkflow {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            category: row.try_get("category")?,
            mode: row.try_get("mode")?,
            source_kind: row.try_get("source_kind")?,
            library_state: row.try_get("library_state")?,
            current_version_id: row.try_get("current_version_id")?,
            removed_at: row.try_get("removed_at")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl<'r> FromRow<'r, sqlx::sqlite::SqliteRow> for BackupWorkflowVersion {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            workflow_id: row.try_get("workflow_id")?,
            version: row.try_get("version")?,
            api_workflow_json: row.try_get("api_workflow_json")?,
            workflow_sha256: row.try_get("workflow_sha256")?,
            package_name: row.try_get("package_name")?,
            package_source_path: row.try_get("package_source_path")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

impl<'r> FromRow<'r, sqlx::sqlite::SqliteRow> for BackupWorkflowRecipe {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            workflow_version_id: row.try_get("workflow_version_id")?,
            version: row.try_get("version")?,
            schema_version: row.try_get("schema_version")?,
            recipe_yaml: row.try_get("recipe_yaml")?,
            recipe_sha256: row.try_get("recipe_sha256")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

impl<'r> FromRow<'r, sqlx::sqlite::SqliteRow> for BackupWorkflowRuntimeArtifact {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            workflow_version_id: row.try_get("workflow_version_id")?,
            recipe_id: row.try_get("recipe_id")?,
            package_name: row.try_get("package_name")?,
            source_kind: row.try_get("source_kind")?,
            package_source_path: row.try_get("package_source_path")?,
            workflow_sha256: row.try_get("workflow_sha256")?,
            recipe_sha256: row.try_get("recipe_sha256")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

impl<'r> FromRow<'r, sqlx::sqlite::SqliteRow> for BackupCharacterProfile {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            project_id: row.try_get("project_id")?,
            name: row.try_get("name")?,
            description: row.try_get("description")?,
            canonical_prompt: row.try_get("canonical_prompt")?,
            negative_prompt: row.try_get("negative_prompt")?,
            default_style_profile_id: row.try_get("default_style_profile_id")?,
            default_reference_set_id: row.try_get("default_reference_set_id")?,
            active_revision_id: row.try_get("active_revision_id")?,
            metadata_json: row.try_get("metadata_json")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl<'r> FromRow<'r, sqlx::sqlite::SqliteRow> for BackupSceneProfile {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            project_id: row.try_get("project_id")?,
            name: row.try_get("name")?,
            description: row.try_get("description")?,
            environment_prompt: row.try_get("environment_prompt")?,
            lighting_prompt: row.try_get("lighting_prompt")?,
            negative_prompt: row.try_get("negative_prompt")?,
            default_style_profile_id: row.try_get("default_style_profile_id")?,
            default_reference_set_id: row.try_get("default_reference_set_id")?,
            active_revision_id: row.try_get("active_revision_id")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl<'r> FromRow<'r, sqlx::sqlite::SqliteRow> for BackupPropProfile {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            project_id: row.try_get("project_id")?,
            name: row.try_get("name")?,
            description: row.try_get("description")?,
            canonical_prompt: row.try_get("canonical_prompt")?,
            material_prompt: row.try_get("material_prompt")?,
            scale_prompt: row.try_get("scale_prompt")?,
            default_reference_set_id: row.try_get("default_reference_set_id")?,
            active_revision_id: row.try_get("active_revision_id")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl<'r> FromRow<'r, sqlx::sqlite::SqliteRow> for BackupStyleProfile {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            project_id: row.try_get("project_id")?,
            name: row.try_get("name")?,
            style_prompt: row.try_get("style_prompt")?,
            color_prompt: row.try_get("color_prompt")?,
            line_prompt: row.try_get("line_prompt")?,
            negative_prompt: row.try_get("negative_prompt")?,
            output_notes: row.try_get("output_notes")?,
            active_revision_id: row.try_get("active_revision_id")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl<'r> FromRow<'r, sqlx::sqlite::SqliteRow> for BackupCostumeVariant {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            character_profile_id: row.try_get("character_profile_id")?,
            name: row.try_get("name")?,
            prompt_fragment: row.try_get("prompt_fragment")?,
            reference_set_id: row.try_get("reference_set_id")?,
            is_default: row.try_get("is_default")?,
            ordinal: row.try_get("ordinal")?,
            active_revision_id: row.try_get("active_revision_id")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl<'r> FromRow<'r, sqlx::sqlite::SqliteRow> for BackupProfileRevision {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            profile_type: row.try_get("profile_type")?,
            profile_id: row.try_get("profile_id")?,
            revision_number: row.try_get("revision_number")?,
            content_json: row.try_get("content_json")?,
            content_sha256: row.try_get("content_sha256")?,
            status: row.try_get("status")?,
            created_at: row.try_get("created_at")?,
            created_by: row.try_get("created_by")?,
        })
    }
}

impl<'r> FromRow<'r, sqlx::sqlite::SqliteRow> for BackupReferenceSet {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            project_id: row.try_get("project_id")?,
            name: row.try_get("name")?,
            purpose: row.try_get("purpose")?,
            description: row.try_get("description")?,
            owner_profile_type: row.try_get("owner_profile_type")?,
            owner_profile_id: row.try_get("owner_profile_id")?,
            active_revision_id: row.try_get("active_revision_id")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl<'r> FromRow<'r, sqlx::sqlite::SqliteRow> for BackupReferenceSetItem {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            reference_set_id: row.try_get("reference_set_id")?,
            asset_id: row.try_get("asset_id")?,
            ordinal: row.try_get("ordinal")?,
            role: row.try_get("role")?,
            is_primary: row.try_get("is_primary")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

impl<'r> FromRow<'r, sqlx::sqlite::SqliteRow> for BackupShotProfileBinding {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            shot_id: row.try_get("shot_id")?,
            role: row.try_get("role")?,
            profile_type: row.try_get("profile_type")?,
            profile_id: row.try_get("profile_id")?,
            costume_variant_id: row.try_get("costume_variant_id")?,
            ordinal: row.try_get("ordinal")?,
            inheritance_mode: row.try_get("inheritance_mode")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl<'r> FromRow<'r, sqlx::sqlite::SqliteRow> for BackupShotReferenceSetBinding {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            shot_id: row.try_get("shot_id")?,
            role: row.try_get("role")?,
            reference_set_id: row.try_get("reference_set_id")?,
            ordinal: row.try_get("ordinal")?,
            required: row.try_get("required")?,
            inheritance_mode: row.try_get("inheritance_mode")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl<'r> FromRow<'r, sqlx::sqlite::SqliteRow> for BackupScopeProfileBinding {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            project_id: row.try_get("project_id")?,
            scope_type: row.try_get("scope_type")?,
            scope_id: row.try_get("scope_id")?,
            role: row.try_get("role")?,
            profile_type: row.try_get("profile_type")?,
            profile_id: row.try_get("profile_id")?,
            costume_variant_id: row.try_get("costume_variant_id")?,
            ordinal: row.try_get("ordinal")?,
            inheritance_mode: row.try_get("inheritance_mode")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl<'r> FromRow<'r, sqlx::sqlite::SqliteRow> for BackupScopeReferenceSetBinding {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            project_id: row.try_get("project_id")?,
            scope_type: row.try_get("scope_type")?,
            scope_id: row.try_get("scope_id")?,
            role: row.try_get("role")?,
            reference_set_id: row.try_get("reference_set_id")?,
            ordinal: row.try_get("ordinal")?,
            required: row.try_get("required")?,
            inheritance_mode: row.try_get("inheritance_mode")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl<'r> FromRow<'r, sqlx::sqlite::SqliteRow> for BackupAssetTag {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            project_id: row.try_get("project_id")?,
            name: row.try_get("name")?,
            normalized_name: row.try_get("normalized_name")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl<'r> FromRow<'r, sqlx::sqlite::SqliteRow> for BackupAssetTagLink {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            asset_id: row.try_get("asset_id")?,
            tag_id: row.try_get("tag_id")?,
            project_id: row.try_get("project_id")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

impl<'r> FromRow<'r, sqlx::sqlite::SqliteRow> for BackupAssetFavorite {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            asset_id: row.try_get("asset_id")?,
            project_id: row.try_get("project_id")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

impl<'r> FromRow<'r, sqlx::sqlite::SqliteRow> for BackupAssetVideoPrompt {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            asset_id: row.try_get("asset_id")?,
            project_id: row.try_get("project_id")?,
            prompt_text: row.try_get("prompt_text")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl<'r> FromRow<'r, sqlx::sqlite::SqliteRow> for BackupScriptSource {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            project_id: row.try_get("project_id")?,
            format: row.try_get("format")?,
            original_filename: row.try_get("original_filename")?,
            source_checksum: row.try_get("source_checksum")?,
            source_bytes: row.try_get("source_bytes")?,
            source_text: row.try_get("source_text")?,
            schema_version: row.try_get("schema_version")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

impl<'r> FromRow<'r, sqlx::sqlite::SqliteRow> for BackupScriptDraftRevision {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            draft_id: row.try_get("draft_id")?,
            project_id: row.try_get("project_id")?,
            source_id: row.try_get("source_id")?,
            revision: row.try_get("revision")?,
            previous_revision_id: row.try_get("previous_revision_id")?,
            schema_version: row.try_get("schema_version")?,
            revision_kind: row.try_get("revision_kind")?,
            parser_version: row.try_get("parser_version")?,
            contract_version: row.try_get("contract_version")?,
            provider_kind: row.try_get("provider_kind")?,
            provider_model: row.try_get("provider_model")?,
            provider_metadata_json: row.try_get("provider_metadata_json")?,
            payload_checksum: row.try_get("payload_checksum")?,
            summary_json: row.try_get("summary_json")?,
            payload_json: row.try_get("payload_json")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

impl<'r> FromRow<'r, sqlx::sqlite::SqliteRow> for BackupProjectWorkflowBinding {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            stage: row.try_get("stage")?,
            mode: row.try_get("mode")?,
            workflow_version_id: row.try_get("workflow_version_id")?,
            recipe_id: row.try_get("recipe_id")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            workflow_id: row.try_get("workflow_id")?,
        })
    }
}
