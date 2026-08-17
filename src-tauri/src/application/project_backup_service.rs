use crate::application::ports::ProjectRecord;
use crate::error::AppError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, Sqlite, SqlitePool, Transaction};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{self, BufWriter, Read, Write},
    path::{Component, Path, PathBuf},
    sync::Mutex,
    time::Duration,
};
use uuid::Uuid;
use zip::{write::FileOptions, CompressionMethod, ZipArchive, ZipWriter};

const BACKUP_FORMAT: &str = "ai-studio-project-backup";
const BACKUP_VERSION: u32 = 10;
const MAX_ZIP_BYTES: u64 = 20 * 1024 * 1024 * 1024;
const MAX_ENTRY_BYTES: u64 = 10 * 1024 * 1024 * 1024;
const MAX_ENTRIES: usize = 100_000;
const INSPECTION_TTL: Duration = Duration::from_secs(20 * 60);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBackupManifest {
    pub format: String,
    pub version: u32,
    pub created_by: String,
    pub project: BackupProject,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupProject {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBackupExportView {
    pub file_name: String,
    pub bytes: u64,
    pub entries: usize,
    pub active_tasks_excluded: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBackupPreviewView {
    pub inspection_id: String,
    pub project_name: String,
    pub image_count: usize,
    pub video_count: usize,
    pub audio_count: usize,
    pub history_tasks: usize,
    pub presets: usize,
    pub production_queues: usize,
    pub benchmarks: usize,
    pub production_runs: usize,
    pub prompt_entries: usize,
    pub shots: usize,
    pub missing_workflows: Vec<String>,
    pub active_tasks_excluded: usize,
    pub warning: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoredProjectView {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone)]
struct Inspection {
    archive_path: PathBuf,
    expires_at: std::time::Instant,
}

pub struct ProjectBackupService {
    pool: SqlitePool,
    projects_dir: PathBuf,
    inspection_dir: PathBuf,
    inspections: Mutex<HashMap<String, Inspection>>,
}

impl ProjectBackupService {
    pub fn new(pool: SqlitePool, projects_dir: PathBuf, cache_dir: PathBuf) -> Self {
        Self {
            pool,
            projects_dir,
            inspection_dir: cache_dir.join("backup-inspections"),
            inspections: Mutex::new(HashMap::new()),
        }
    }

    pub async fn export(
        &self,
        project_id: &str,
        destination: PathBuf,
    ) -> Result<ProjectBackupExportView, AppError> {
        let built = self.build_backup(project_id).await?;
        let parent = destination
            .parent()
            .ok_or_else(|| AppError::filesystem("备份保存目录不可用"))?;
        fs::create_dir_all(parent).map_err(|error| AppError::filesystem(error.to_string()))?;
        let temporary = parent.join(format!(
            ".{}.backup-{}.tmp",
            destination
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("project"),
            Uuid::new_v4()
        ));
        let document = built.document.clone();
        let files = built.files.clone();
        let temporary_for_writer = temporary.clone();
        let write_result = match tokio::task::spawn_blocking(move || {
            write_zip_to_path(&document, &files, &temporary_for_writer)
        })
        .await
        {
            Ok(result) => result,
            Err(error) => {
                let _ = fs::remove_file(&temporary);
                return Err(AppError::internal(format!("备份写入任务失败：{error}")));
            }
        };
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        if let Err(error) = publish_backup_file(&temporary, &destination) {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        let bytes = fs::metadata(&destination)
            .map_err(|error| AppError::filesystem(error.to_string()))?
            .len();
        Ok(ProjectBackupExportView {
            file_name: destination
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("AI-Studio-Project-Backup.zip")
                .to_owned(),
            bytes,
            entries: 5 + built.files.len(),
            active_tasks_excluded: built.document.active_tasks_excluded,
        })
    }

    pub async fn inspect(&self, source: PathBuf) -> Result<ProjectBackupPreviewView, AppError> {
        let (manifest, document, entry_names) = inspect_archive(&source)?;
        validate_document_entries(&document, &entry_names)?;
        let missing_workflows = self.find_missing_workflows(&document).await?;
        fs::create_dir_all(&self.inspection_dir)
            .map_err(|error| AppError::filesystem(error.to_string()))?;
        let inspection_id = format!("bki_{}", Uuid::new_v4());
        let archive_path = self.inspection_dir.join(format!("{inspection_id}.zip"));
        fs::copy(&source, &archive_path)
            .map_err(|error| AppError::filesystem(error.to_string()))?;
        let preview = ProjectBackupPreviewView {
            inspection_id: inspection_id.clone(),
            project_name: manifest.project.name,
            image_count: document
                .assets
                .iter()
                .filter(|asset| asset.asset_type == "image")
                .count(),
            video_count: document
                .assets
                .iter()
                .filter(|asset| asset.asset_type == "video")
                .count(),
            audio_count: document
                .assets
                .iter()
                .filter(|asset| asset.asset_type == "audio")
                .count(),
            history_tasks: document.tasks.len(),
            presets: document.presets.len(),
            production_queues: document.batches.len(),
            benchmarks: document.benchmark_experiments.len(),
            production_runs: document.production_runs.len(),
            prompt_entries: document.prompt_entries.len(),
            shots: document.shots.len(),
            missing_workflows,
            active_tasks_excluded: document.active_tasks_excluded,
            warning: "项目备份包含项目历史、提示词和素材，请妥善保存。".to_owned(),
        };
        self.inspections
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(
                inspection_id,
                Inspection {
                    archive_path,
                    expires_at: std::time::Instant::now() + INSPECTION_TTL,
                },
            );
        Ok(preview)
    }

    pub async fn restore(&self, inspection_id: &str) -> Result<RestoredProjectView, AppError> {
        let inspection = self.take_inspection(inspection_id)?;
        let (manifest, document, entry_names) = inspect_archive(&inspection.archive_path)?;
        validate_document_entries(&document, &entry_names)?;

        let new_project_id = format!("prj_{}", Uuid::new_v4());
        let final_root = self.projects_dir.join(&new_project_id);
        let staging_root =
            self.projects_dir
                .join(format!(".restore-{}-{}", new_project_id, Uuid::new_v4()));
        let mut archive = ZipArchive::new(
            File::open(&inspection.archive_path)
                .map_err(|error| AppError::filesystem(error.to_string()))?,
        )
        .map_err(|error| AppError::backup_invalid(format!("备份压缩包无法读取：{error}")))?;

        let now = Utc::now();
        let new_name = restored_name(&manifest.project.name);
        let new_description = Some(format!("从项目“{}”恢复", manifest.project.name));
        let project = ProjectRecord {
            id: new_project_id.clone(),
            name: new_name.clone(),
            description: new_description.clone(),
            root_path: final_root.clone(),
            created_at: now,
            updated_at: now,
        };
        let mut task_ids = HashMap::new();
        for task in &document.tasks {
            task_ids.insert(task.id.clone(), format!("tsk_{}", Uuid::new_v4()));
        }
        let mut asset_ids = HashMap::new();
        for asset in &document.assets {
            asset_ids.insert(asset.id.clone(), format!("ast_{}", Uuid::new_v4()));
        }
        let mut snapshot_ids = HashMap::new();
        for snapshot in &document.snapshots {
            snapshot_ids.insert(snapshot.id.clone(), format!("snp_{}", Uuid::new_v4()));
        }
        let mut preset_ids = HashMap::new();
        for preset in &document.presets {
            preset_ids.insert(preset.id.clone(), format!("pst_{}", Uuid::new_v4()));
        }
        let mut prompt_ids = HashMap::new();
        for entry in &document.prompt_entries {
            prompt_ids.insert(entry.id.clone(), format!("prm_{}", Uuid::new_v4()));
        }
        let mut prompt_version_ids = HashMap::new();
        for version in &document.prompt_versions {
            prompt_version_ids.insert(version.id.clone(), format!("prv_{}", Uuid::new_v4()));
        }
        let mut batch_ids = HashMap::new();
        for batch in &document.batches {
            batch_ids.insert(batch.id.clone(), format!("pbt_{}", Uuid::new_v4().simple()));
        }
        let mut item_ids = HashMap::new();
        for item in &document.items {
            item_ids.insert(item.id.clone(), format!("pbi_{}", Uuid::new_v4().simple()));
        }
        let mut benchmark_experiment_ids = HashMap::new();
        for experiment in &document.benchmark_experiments {
            benchmark_experiment_ids.insert(
                experiment.id.clone(),
                format!("bmk_{}", Uuid::new_v4().simple()),
            );
        }
        let mut benchmark_candidate_ids = HashMap::new();
        for candidate in &document.benchmark_candidates {
            benchmark_candidate_ids.insert(
                candidate.id.clone(),
                format!("bmc_{}", Uuid::new_v4().simple()),
            );
        }
        let mut production_run_ids = HashMap::new();
        for run in &document.production_runs {
            production_run_ids.insert(run.id.clone(), format!("prun_{}", Uuid::new_v4().simple()));
        }
        let mut production_stage_ids = HashMap::new();
        for stage in &document.production_stages {
            production_stage_ids.insert(
                stage.id.clone(),
                format!("prst_{}", Uuid::new_v4().simple()),
            );
        }
        let mut production_stage_item_ids = HashMap::new();
        for item in &document.production_stage_items {
            production_stage_item_ids
                .insert(item.id.clone(), format!("prsi_{}", Uuid::new_v4().simple()));
        }
        let mut production_run_template_ids = HashMap::new();
        for template in &document.production_run_templates {
            production_run_template_ids.insert(
                template.id.clone(),
                format!("prt_{}", Uuid::new_v4().simple()),
            );
        }
        let mut benchmark_run_ids = HashMap::new();
        for run in &document.benchmark_runs {
            benchmark_run_ids.insert(run.id.clone(), format!("bmr_{}", Uuid::new_v4().simple()));
        }
        let mut benchmark_quality_score_ids = HashMap::new();
        for score in &document.benchmark_quality_scores {
            benchmark_quality_score_ids
                .insert(score.id.clone(), format!("bmq_{}", Uuid::new_v4().simple()));
        }
        let mut shot_ids = HashMap::new();
        for shot in &document.shots {
            shot_ids.insert(shot.id.clone(), format!("sht_{}", Uuid::new_v4()));
        }
        let mut shot_generation_link_ids = HashMap::new();
        for link in &document.shot_generation_links {
            shot_generation_link_ids.insert(link.id.clone(), format!("sgl_{}", Uuid::new_v4()));
        }
        let mut tag_ids = HashMap::new();
        for tag in &document.asset_tags {
            tag_ids.insert(tag.id.clone(), format!("tag_{}", Uuid::new_v4()));
        }

        let copy_result = copy_assets(
            &mut archive,
            &staging_root,
            &final_root,
            &document.assets,
            &asset_ids,
        );
        let restored_assets = match copy_result {
            Ok(assets) => assets,
            Err(error) => {
                let _ = fs::remove_dir_all(&staging_root);
                return Err(error);
            }
        };
        if let Err(error) = fs::rename(&staging_root, &final_root) {
            let _ = fs::remove_dir_all(&staging_root);
            return Err(AppError::filesystem(format!(
                "恢复项目目录发布失败：{error}"
            )));
        }

        let restore_result = self
            .restore_rows(
                &project,
                &document,
                &task_ids,
                &asset_ids,
                &snapshot_ids,
                &preset_ids,
                &prompt_ids,
                &prompt_version_ids,
                &batch_ids,
                &item_ids,
                &benchmark_experiment_ids,
                &benchmark_candidate_ids,
                &production_run_ids,
                &production_stage_ids,
                &production_stage_item_ids,
                &production_run_template_ids,
                &benchmark_run_ids,
                &benchmark_quality_score_ids,
                &tag_ids,
                &shot_ids,
                &shot_generation_link_ids,
                &restored_assets,
            )
            .await;
        if let Err(error) = restore_result {
            let _ = fs::remove_dir_all(&final_root);
            return Err(error);
        }
        let _ = fs::remove_file(&inspection.archive_path);
        Ok(RestoredProjectView {
            id: project.id,
            name: project.name,
            description: project.description,
            created_at: project.created_at,
            updated_at: project.updated_at,
        })
    }

    fn take_inspection(&self, inspection_id: &str) -> Result<Inspection, AppError> {
        let mut inspections = self
            .inspections
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some(inspection) = inspections.remove(inspection_id) else {
            return Err(AppError::backup_inspection_expired(
                "备份预览已过期，请重新选择备份文件。",
            ));
        };
        if std::time::Instant::now() > inspection.expires_at {
            let _ = fs::remove_file(&inspection.archive_path);
            return Err(AppError::backup_inspection_expired(
                "备份预览已过期，请重新选择备份文件。",
            ));
        }
        Ok(inspection)
    }

    async fn find_missing_workflows(
        &self,
        document: &BackupDocument,
    ) -> Result<Vec<String>, AppError> {
        let mut missing = Vec::new();
        for reference in &document.workflow_refs {
            let exists =
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflows WHERE id = ?")
                    .bind(&reference.workflow_id)
                    .fetch_one(&self.pool)
                    .await
                    .map_err(|error| AppError::database(error.to_string()))?
                    > 0;
            if !exists && !missing.contains(&reference.workflow_id) {
                missing.push(reference.workflow_id.clone());
            }
        }
        Ok(missing)
    }

    async fn build_backup(&self, project_id: &str) -> Result<BuiltBackup, AppError> {
        // Keep the metadata snapshot short: no filesystem reads or ZIP writes
        // happen while this SQLite read transaction is open.
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| AppError::database(error.to_string()))?;
        let project = query_project(&mut transaction, project_id)
            .await?
            .ok_or_else(|| AppError::project_not_found(project_id.to_owned()))?;
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
        .map_err(|error| AppError::database(error.to_string()))?;
        let db_assets = sqlx::query_as::<_, DbAsset>(
            "SELECT id, project_id, type, category, name, original_name, storage_path,
             thumbnail_path, sha256, mime_type, width, height, duration_ms, file_size,
             source_task_id, metadata_json, created_at, updated_at FROM assets WHERE project_id = ? ORDER BY created_at, id",
        )
        .bind(project_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|error| AppError::database(error.to_string()))?;
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
        ).bind(project_id).fetch_all(&mut *transaction).await.map_err(|error| AppError::database(error.to_string()))?;
        let mut asset_tag_links = sqlx::query_as::<_, BackupAssetTagLink>(
            "SELECT asset_id, tag_id, project_id, created_at FROM asset_tag_links WHERE project_id = ? ORDER BY created_at, asset_id, tag_id",
        ).bind(project_id).fetch_all(&mut *transaction).await.map_err(|error| AppError::database(error.to_string()))?;
        let mut asset_favorites = sqlx::query_as::<_, BackupAssetFavorite>(
            "SELECT asset_id, project_id, created_at FROM asset_favorites WHERE project_id = ? ORDER BY created_at, asset_id",
        ).bind(project_id).fetch_all(&mut *transaction).await.map_err(|error| AppError::database(error.to_string()))?;
        let mut asset_video_prompts = sqlx::query_as::<_, DbAssetVideoPrompt>(
            "SELECT asset_id, project_id, prompt_text, updated_at
             FROM asset_video_prompts WHERE project_id = ? ORDER BY asset_id",
        )
        .bind(project_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|error| AppError::database(error.to_string()))?
        .into_iter()
        .map(|row| BackupAssetVideoPrompt {
            asset_id: row.asset_id,
            project_id: row.project_id,
            prompt_text: row.prompt_text,
            updated_at: row.updated_at,
        })
        .collect::<Vec<_>>();
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
        transaction
            .commit()
            .await
            .map_err(|error| AppError::database(error.to_string()))?;

        // Only after the short metadata transaction commits do we inspect and
        // stream potentially large asset files.
        let mut files = Vec::new();
        let mut assets = Vec::new();
        for asset in db_assets {
            if asset
                .source_task_id
                .as_ref()
                .is_some_and(|task_id| excluded_tasks.contains(task_id))
            {
                continue;
            }
            if !safe_component(&asset.id) {
                return Err(AppError::backup_invalid("资产 ID 不能用于备份路径"));
            }
            let extension = extension_for_path(&asset.storage_path);
            let content_path = format!("assets/{}/content.{}", asset.id, extension);
            let content_metadata = fs::metadata(&asset.storage_path).map_err(|error| {
                AppError::backup_asset_hash_mismatch(format!(
                    "备份资产不存在或不可读取：{}：{error}",
                    asset.id
                ))
            })?;
            if !content_metadata.is_file() {
                return Err(AppError::backup_asset_hash_mismatch(format!(
                    "备份资产不是普通文件：{}",
                    asset.id
                )));
            }
            let expected_size = asset
                .file_size
                .map(|size| {
                    u64::try_from(size).map_err(|_| {
                        AppError::backup_asset_hash_mismatch(format!(
                            "资产 {} 文件大小无效",
                            asset.id
                        ))
                    })
                })
                .transpose()?
                .unwrap_or(content_metadata.len());
            files.push(BackupFileSource {
                zip_path: content_path.clone(),
                source_path: PathBuf::from(&asset.storage_path),
                expected_size,
                expected_sha256: Some(asset.sha256.clone()),
            });
            let thumbnail_path = asset.thumbnail_path.as_ref().and_then(|path| {
                let metadata = fs::metadata(path).ok()?;
                if !metadata.is_file() {
                    return None;
                }
                let extension = extension_for_path(path);
                let zip_path = format!("assets/{}/thumbnail.{}", asset.id, extension);
                files.push(BackupFileSource {
                    zip_path: zip_path.clone(),
                    source_path: PathBuf::from(path),
                    expected_size: metadata.len(),
                    expected_sha256: None,
                });
                Some(zip_path)
            });
            assets.push(BackupAsset {
                id: asset.id,
                asset_type: asset.r#type,
                category: asset.category.unwrap_or_default(),
                name: asset.name,
                original_name: asset.original_name.unwrap_or_default(),
                sha256: asset.sha256,
                mime_type: asset.mime_type.unwrap_or_default(),
                width: asset.width.unwrap_or_default(),
                height: asset.height.unwrap_or_default(),
                duration_ms: asset.duration_ms,
                file_size: asset.file_size.map(Ok).unwrap_or_else(|| {
                    i64::try_from(expected_size).map_err(|_| {
                        AppError::backup_asset_hash_mismatch("资产文件大小超出支持范围")
                    })
                })?,
                source_task_id: asset.source_task_id,
                metadata: parse_value(asset.metadata_json.as_deref(), "asset metadata")?,
                created_at: asset.created_at,
                updated_at: asset.updated_at,
                content_path,
                thumbnail_path,
            });
        }
        let included_asset_ids = assets
            .iter()
            .map(|asset| asset.id.as_str())
            .collect::<HashSet<_>>();
        asset_tag_links.retain(|link| included_asset_ids.contains(link.asset_id.as_str()));
        asset_favorites.retain(|favorite| included_asset_ids.contains(favorite.asset_id.as_str()));
        asset_video_prompts.retain(|prompt| included_asset_ids.contains(prompt.asset_id.as_str()));
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
            assets,
            mappings,
            snapshots,
            presets,
            prompt_entries,
            prompt_versions,
            batches,
            items,
            workflow_refs,
            asset_tags,
            asset_tag_links,
            asset_favorites,
            asset_video_prompts,
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
        };
        Ok(BuiltBackup { document, files })
    }

    async fn restore_rows(
        &self,
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
        benchmark_experiment_ids: &HashMap<String, String>,
        benchmark_candidate_ids: &HashMap<String, String>,
        production_run_ids: &HashMap<String, String>,
        production_stage_ids: &HashMap<String, String>,
        production_stage_item_ids: &HashMap<String, String>,
        production_run_template_ids: &HashMap<String, String>,
        benchmark_run_ids: &HashMap<String, String>,
        benchmark_quality_score_ids: &HashMap<String, String>,
        tag_ids: &HashMap<String, String>,
        shot_ids: &HashMap<String, String>,
        shot_generation_link_ids: &HashMap<String, String>,
        restored_assets: &[RestoredAsset],
    ) -> Result<(), AppError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| AppError::database(error.to_string()))?;
        let restored_snapshots = prepare_restored_snapshots(document, asset_ids)?;
        let result = restore_rows_in_transaction(
            &mut transaction,
            project,
            document,
            task_ids,
            asset_ids,
            snapshot_ids,
            preset_ids,
            prompt_ids,
            prompt_version_ids,
            batch_ids,
            item_ids,
            benchmark_experiment_ids,
            benchmark_candidate_ids,
            production_run_ids,
            production_stage_ids,
            production_stage_item_ids,
            production_run_template_ids,
            benchmark_run_ids,
            benchmark_quality_score_ids,
            tag_ids,
            shot_ids,
            shot_generation_link_ids,
            restored_assets,
            &restored_snapshots,
        )
        .await;
        match result {
            Ok(()) => transaction
                .commit()
                .await
                .map_err(|error| AppError::database(error.to_string())),
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error)
            }
        }
    }
}

#[derive(Clone)]
struct BuiltBackup {
    document: BackupDocument,
    files: Vec<BackupFileSource>,
}

#[derive(Clone)]
struct BackupFileSource {
    zip_path: String,
    source_path: PathBuf,
    expected_size: u64,
    expected_sha256: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupDocument {
    project: BackupProject,
    description: Option<String>,
    created_at: String,
    updated_at: String,
    active_tasks_excluded: usize,
    incomplete_tasks_excluded: usize,
    tasks: Vec<BackupTask>,
    task_events: Vec<BackupTaskEvent>,
    assets: Vec<BackupAsset>,
    mappings: Vec<BackupMapping>,
    snapshots: Vec<BackupSnapshot>,
    presets: Vec<BackupPreset>,
    #[serde(default)]
    prompt_entries: Vec<BackupPromptEntry>,
    #[serde(default)]
    prompt_versions: Vec<BackupPromptVersion>,
    batches: Vec<BackupBatch>,
    items: Vec<BackupBatchItem>,
    workflow_refs: Vec<WorkflowReference>,
    #[serde(default)]
    asset_tags: Vec<BackupAssetTag>,
    #[serde(default)]
    asset_tag_links: Vec<BackupAssetTagLink>,
    #[serde(default)]
    asset_favorites: Vec<BackupAssetFavorite>,
    #[serde(default)]
    asset_video_prompts: Vec<BackupAssetVideoPrompt>,
    #[serde(default)]
    production_item_reviews: Vec<BackupProductionItemReview>,
    #[serde(default)]
    benchmark_experiments: Vec<BackupBenchmarkExperiment>,
    #[serde(default)]
    benchmark_candidates: Vec<BackupBenchmarkCandidate>,
    #[serde(default)]
    production_runs: Vec<BackupProductionRun>,
    #[serde(default)]
    production_stages: Vec<BackupProductionStage>,
    #[serde(default)]
    production_stage_items: Vec<BackupProductionStageItem>,
    #[serde(default)]
    production_run_templates: Vec<BackupProductionRunTemplate>,
    #[serde(default)]
    benchmark_runs: Vec<BackupBenchmarkRun>,
    #[serde(default)]
    benchmark_quality_scores: Vec<BackupBenchmarkQualityScore>,
    #[serde(default)]
    shots: Vec<BackupShot>,
    #[serde(default)]
    shot_stage_configs: Vec<BackupShotStageConfig>,
    #[serde(default)]
    shot_stage_prompts: Vec<BackupShotStagePrompt>,
    #[serde(default)]
    shot_reference_assets: Vec<BackupShotReferenceAsset>,
    #[serde(default)]
    shot_generation_links: Vec<BackupShotGenerationLink>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupPromptEntry {
    id: String,
    project_id: String,
    kind: String,
    name: String,
    normalized_name: String,
    tags: Vec<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupPromptVersion {
    id: String,
    project_id: String,
    prompt_id: String,
    version: i64,
    text: String,
    created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
struct BackupAssetTag {
    id: String,
    project_id: String,
    name: String,
    normalized_name: String,
    created_at: String,
    updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
struct BackupAssetTagLink {
    asset_id: String,
    tag_id: String,
    project_id: String,
    created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
struct BackupAssetFavorite {
    asset_id: String,
    project_id: String,
    created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
struct BackupAssetVideoPrompt {
    asset_id: String,
    project_id: String,
    prompt_text: String,
    updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupTask {
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
    dynamic_binding_targets: Option<Value>,
    #[serde(default)]
    generation_execution_id: Option<String>,
    #[serde(default)]
    compiled_workflow_sha256: Option<String>,
    #[serde(default)]
    runtime_profile: Option<String>,
    #[serde(default)]
    concurrency_class: Option<String>,
    #[serde(default)]
    prepare_started_at: Option<String>,
    #[serde(default)]
    prepared_at: Option<String>,
    #[serde(default)]
    submitted_at: Option<String>,
    #[serde(default)]
    execution_started_at: Option<String>,
    #[serde(default)]
    execution_finished_at: Option<String>,
    #[serde(default)]
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
    raw_error: Option<Value>,
    created_at: String,
    queued_at: Option<String>,
    started_at: Option<String>,
    finished_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupTaskEvent {
    id: String,
    task_id: String,
    sequence: i64,
    event_type: String,
    payload: Option<Value>,
    created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupAsset {
    id: String,
    asset_type: String,
    category: String,
    name: String,
    original_name: String,
    sha256: String,
    mime_type: String,
    width: i64,
    height: i64,
    duration_ms: Option<i64>,
    file_size: i64,
    source_task_id: Option<String>,
    metadata: Value,
    created_at: String,
    updated_at: String,
    content_path: String,
    thumbnail_path: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupMapping {
    task_id: String,
    output_id: String,
    ordinal: i64,
    asset_id: String,
    created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupSnapshot {
    id: String,
    task_id: String,
    workflow: Value,
    recipe_yaml: String,
    user_inputs: Value,
    resolved_inputs: Value,
    created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupPreset {
    id: String,
    workflow_version_id: String,
    recipe_id: String,
    name: String,
    values: Value,
    created_at: String,
    updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupBatch {
    id: String,
    name: String,
    status: String,
    continue_on_failure: i64,
    archived_at: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupBatchItem {
    id: String,
    batch_id: String,
    ordinal: i64,
    workflow_version_id: String,
    recipe_id: String,
    values: Value,
    status: String,
    task_id: Option<String>,
    retry_of_item_id: Option<String>,
    error_code: Option<String>,
    error_message: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupBenchmarkExperiment {
    id: String,
    name: String,
    media_type: String,
    status: String,
    base_values: Value,
    asset_ids: Vec<String>,
    winner_candidate_id: Option<String>,
    production_batch_id: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupBenchmarkCandidate {
    id: String,
    experiment_id: String,
    position: i64,
    workflow_version_id: String,
    recipe_id: String,
    preset_id: Option<String>,
    preset_name: Option<String>,
    label: String,
    values: Value,
    asset_ids: Vec<String>,
    production_batch_item_id: Option<String>,
    task_id: Option<String>,
    created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupProductionRun {
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupProductionStage {
    id: String,
    run_id: String,
    ordinal: i64,
    stage_type: String,
    status: String,
    workflow_version_id: Option<String>,
    recipe_id: Option<String>,
    production_batch_id: Option<String>,
    frozen_config: Value,
    prompt: Option<String>,
    created_at: String,
    updated_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupProductionStageItem {
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
    frozen_values: Value,
    error_code: Option<String>,
    error_message: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupProductionRunTemplate {
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupBenchmarkRun {
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupBenchmarkQualityScore {
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupProductionItemReview {
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupShot {
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupShotStageConfig {
    shot_id: String,
    stage: String,
    workflow_id: String,
    workflow_version_id: String,
    recipe_id: String,
    scalar_values: Value,
    updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupShotStagePrompt {
    shot_id: String,
    stage: String,
    prompt_text: String,
    prompt_entry_id: Option<String>,
    prompt_version_id: Option<String>,
    updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupShotReferenceAsset {
    shot_id: String,
    stage: String,
    asset_id: String,
    ordinal: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupShotGenerationLink {
    id: String,
    shot_id: String,
    stage: String,
    task_id: Option<String>,
    production_batch_item_id: Option<String>,
    created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
struct WorkflowReference {
    workflow_id: String,
    workflow_version_id: String,
    recipe_id: String,
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
    type Error = AppError;

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

#[derive(Clone)]
struct RestoredAsset {
    old_id: String,
    new_id: String,
    storage_path: String,
    thumbnail_path: Option<String>,
}

impl BackupTask {
    fn is_terminal(&self) -> bool {
        is_terminal_task(&self.status)
    }
}

fn is_terminal_task(status: &str) -> bool {
    matches!(status, "SUCCEEDED" | "FAILED" | "CANCELLED")
}

fn parse_value(value: Option<&str>, label: &str) -> Result<Value, AppError> {
    value
        .ok_or_else(|| AppError::backup_invalid(format!("{label} 缺失")))
        .and_then(|value| {
            serde_json::from_str(value)
                .map_err(|error| AppError::backup_invalid(format!("{label} JSON 无效：{error}")))
        })
}

fn parse_optional_value(value: Option<&str>, label: &str) -> Result<Option<Value>, AppError> {
    value
        .map(|value| parse_value(Some(value), label))
        .transpose()
}

fn parse_string_array(value: Option<&str>, label: &str) -> Result<Vec<String>, AppError> {
    let value = parse_value(value, label)?;
    let Some(values) = value.as_array() else {
        return Err(AppError::backup_invalid(format!(
            "{label} 必须是字符串数组"
        )));
    };
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| AppError::backup_invalid(format!("{label} 必须只包含字符串")))
        })
        .collect()
}

fn extension_for_path(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| extension.chars().all(|value| value.is_ascii_alphanumeric()))
        .unwrap_or("bin")
        .to_ascii_lowercase()
}

fn hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

async fn query_project(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Option<ProjectRecord>, AppError> {
    let row = sqlx::query_as::<_, DbProject>(
        "SELECT id, name, description, root_path, created_at, updated_at FROM projects WHERE id = ?",
    )
    .bind(project_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
    row.map(|row| {
        if row.root_path.trim().is_empty() {
            return Err(AppError::database("项目 root_path 不能为空"));
        }
        let created_at = DateTime::parse_from_rfc3339(&row.created_at)
            .map(|value| value.with_timezone(&Utc))
            .map_err(|error| AppError::database(format!("项目 created_at 无效：{error}")))?;
        let updated_at = DateTime::parse_from_rfc3339(&row.updated_at)
            .map(|value| value.with_timezone(&Utc))
            .map_err(|error| AppError::database(format!("项目 updated_at 无效：{error}")))?;
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
) -> Result<Vec<BackupTaskEvent>, AppError> {
    let mut result = Vec::new();
    for task_id in task_ids {
        let rows = sqlx::query_as::<_, DbTaskEvent>(
            "SELECT id, task_id, sequence, event_type, payload_json, created_at FROM task_events WHERE task_id = ? ORDER BY sequence",
        )
        .bind(task_id)
        .fetch_all(&mut **transaction)
        .await
        .map_err(|error| AppError::database(error.to_string()))?;
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
) -> Result<Vec<BackupSnapshot>, AppError> {
    let mut result = Vec::new();
    for task_id in task_ids {
        let row = sqlx::query_as::<_, DbSnapshot>(
            "SELECT id, task_id, workflow_json, recipe_yaml, user_inputs_json, resolved_inputs_json, created_at FROM generation_snapshots WHERE task_id = ?",
        )
        .bind(task_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(|error| AppError::database(error.to_string()))?;
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
) -> Result<Vec<BackupMapping>, AppError> {
    let mut result = Vec::new();
    for task_id in task_ids {
        let rows = sqlx::query_as::<_, DbMapping>(
            "SELECT task_id, output_id, ordinal, asset_id, created_at FROM task_output_assets WHERE task_id = ? ORDER BY output_id, ordinal",
        )
        .bind(task_id)
        .fetch_all(&mut **transaction)
        .await
        .map_err(|error| AppError::database(error.to_string()))?;
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
) -> Result<Vec<BackupPreset>, AppError> {
    let rows = sqlx::query_as::<_, DbPreset>(
        "SELECT id, project_id, workflow_version_id, recipe_id, name, values_json, created_at, updated_at FROM presets WHERE project_id = ? ORDER BY updated_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
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
) -> Result<Vec<BackupPromptEntry>, AppError> {
    let rows = sqlx::query_as::<_, DbPromptEntry>(
        "SELECT id, project_id, kind, name, normalized_name, tags_json, created_at, updated_at
         FROM prompt_entries WHERE project_id = ? ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
    rows.into_iter()
        .map(|row| {
            let tags = serde_json::from_str::<Vec<String>>(&row.tags_json)
                .map_err(|error| AppError::database(format!("提示词标签 JSON 无效：{error}")))?;
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
) -> Result<Vec<BackupPromptVersion>, AppError> {
    let rows = sqlx::query_as::<_, DbPromptVersion>(
        "SELECT v.id, v.prompt_id, v.version, v.text, v.created_at
         FROM prompt_versions v
         JOIN prompt_entries e ON e.id = v.prompt_id
         WHERE e.project_id = ? ORDER BY v.prompt_id, v.version",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
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
) -> Result<Vec<BackupBatch>, AppError> {
    let rows = sqlx::query_as::<_, DbBatch>(
        "SELECT id, project_id, name, status, continue_on_failure, archived_at, created_at, updated_at FROM production_batches WHERE project_id = ? ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
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
) -> Result<Vec<BackupBatchItem>, AppError> {
    let mut result = Vec::new();
    for batch in batches {
        let rows = sqlx::query_as::<_, DbBatchItem>(
            "SELECT id, batch_id, ordinal, workflow_version_id, recipe_id, values_json, status, task_id, retry_of_item_id, error_code, error_message, created_at, updated_at FROM production_batch_items WHERE batch_id = ? ORDER BY ordinal",
        )
        .bind(&batch.id)
        .fetch_all(&mut **transaction)
        .await
        .map_err(|error| AppError::database(error.to_string()))?;
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

async fn query_benchmark_experiments(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<Vec<BackupBenchmarkExperiment>, AppError> {
    let rows = sqlx::query_as::<_, DbBenchmarkExperiment>(
        "SELECT id, name, media_type, status, base_values_json, asset_ids_json,
                winner_candidate_id, production_batch_id, created_at, updated_at
         FROM benchmark_experiments WHERE project_id = ? ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
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
) -> Result<Vec<BackupBenchmarkCandidate>, AppError> {
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
    .map_err(|error| AppError::database(error.to_string()))?;
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
) -> Result<Vec<BackupProductionRun>, AppError> {
    let rows = sqlx::query_as::<_, DbProductionRun>(
        "SELECT id, project_id, name, status, current_stage_ordinal, template_id,
                created_at, updated_at, started_at, finished_at
         FROM production_runs WHERE project_id = ? ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
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
) -> Result<Vec<BackupProductionStage>, AppError> {
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
        .map_err(|error| AppError::database(error.to_string()))?;
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
) -> Result<Vec<BackupProductionStageItem>, AppError> {
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
        .map_err(|error| AppError::database(error.to_string()))?;
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
) -> Result<Vec<BackupProductionRunTemplate>, AppError> {
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
    .map_err(|error| AppError::database(error.to_string()))?;
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
) -> Result<Vec<BackupBenchmarkRun>, AppError> {
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
    .map_err(|error| AppError::database(error.to_string()))?;
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
) -> Result<Vec<BackupBenchmarkQualityScore>, AppError> {
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
    .map_err(|error| AppError::database(error.to_string()))?;
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
) -> Result<Vec<WorkflowReference>, AppError> {
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
    .map_err(|error| AppError::database(error.to_string()))?;
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
) -> Result<Vec<BackupProductionItemReview>, AppError> {
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
    .map_err(|error| AppError::database(error.to_string()))?;
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
) -> Result<Vec<BackupShot>, AppError> {
    let rows = sqlx::query_as::<_, DbShot>(
        "SELECT id, project_id, ordinal, name, prompt_text, prompt_entry_id, prompt_version_id,
                selected_image_asset_id, selected_video_asset_id, created_at, updated_at
         FROM shots WHERE project_id = ? ORDER BY ordinal, id",
    )
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
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
) -> Result<Vec<BackupShotStageConfig>, AppError> {
    let rows = sqlx::query_as::<_, DbShotStageConfig>(
        "SELECT c.shot_id, c.stage, v.workflow_id, c.workflow_version_id, c.recipe_id,
                c.scalar_values_json, c.updated_at
         FROM shot_stage_configs c
         JOIN workflow_versions v ON v.id = c.workflow_version_id
         ORDER BY c.shot_id, c.stage",
    )
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
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
) -> Result<Vec<BackupShotStagePrompt>, AppError> {
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
    .map_err(|error| AppError::database(error.to_string()))?;
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
) -> Result<Vec<BackupShotReferenceAsset>, AppError> {
    let rows = sqlx::query_as::<_, DbShotReferenceAsset>(
        "SELECT shot_id, stage, asset_id, ordinal
         FROM shot_reference_assets ORDER BY shot_id, stage, ordinal, asset_id",
    )
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
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
) -> Result<Vec<BackupShotGenerationLink>, AppError> {
    let rows = sqlx::query_as::<_, DbShotGenerationLink>(
        "SELECT id, shot_id, stage, task_id, production_batch_item_id, created_at
         FROM shot_generation_links ORDER BY shot_id, created_at, id",
    )
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
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

const STREAM_CHUNK_BYTES: usize = 1024 * 1024;

fn write_zip_to_path(
    document: &BackupDocument,
    files: &[BackupFileSource],
    destination: &Path,
) -> Result<(), AppError> {
    if files.len().saturating_add(5) > MAX_ENTRIES {
        return Err(AppError::backup_invalid("备份文件数量超过限制"));
    }
    let manifest = ProjectBackupManifest {
        format: BACKUP_FORMAT.to_owned(),
        version: BACKUP_VERSION,
        created_by: env!("CARGO_PKG_VERSION").to_owned(),
        project: document.project.clone(),
    };
    let file = File::create(destination)
        .map_err(|error| AppError::filesystem(format!("备份临时文件创建失败：{error}")))?;
    let mut writer = ZipWriter::new(BufWriter::with_capacity(STREAM_CHUNK_BYTES, file));
    let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
    write_zip_json(&mut writer, "manifest.json", &manifest, options)?;
    write_zip_json(&mut writer, "project.json", document, options)?;
    write_zip_json(
        &mut writer,
        "history/task_snapshots.json",
        &document.snapshots,
        options,
    )?;
    write_zip_json(&mut writer, "presets.json", &document.presets, options)?;
    write_zip_json(
        &mut writer,
        "production_queue.json",
        &document.batches,
        options,
    )?;
    for file in files {
        if file.expected_size > MAX_ENTRY_BYTES {
            return Err(AppError::backup_invalid("备份资产超过单文件大小限制"));
        }
        writer
            .start_file(&file.zip_path, options)
            .map_err(|error| AppError::backup_invalid(format!("备份文件写入失败：{error}")))?;
        write_source_to_zip(&mut writer, file)?;
    }
    let buffered = writer
        .finish()
        .map_err(|error| AppError::backup_invalid(format!("备份压缩包生成失败：{error}")))?;
    let file = buffered
        .into_inner()
        .map_err(|error| AppError::filesystem(error.into_error().to_string()))?;
    file.sync_all()
        .map_err(|error| AppError::filesystem(format!("备份临时文件同步失败：{error}")))?;
    let bytes = fs::metadata(destination)
        .map_err(|error| AppError::filesystem(error.to_string()))?
        .len();
    if bytes > MAX_ZIP_BYTES {
        return Err(AppError::backup_invalid("备份压缩包超过 20 GiB 限制"));
    }
    Ok(())
}

fn write_source_to_zip<W: Write + io::Seek>(
    writer: &mut ZipWriter<W>,
    source: &BackupFileSource,
) -> Result<(), AppError> {
    let mut input = File::open(&source.source_path).map_err(|error| {
        if source.expected_sha256.is_some() {
            AppError::backup_asset_hash_mismatch(format!("备份资产读取失败：{}", source.zip_path))
        } else {
            AppError::filesystem(error.to_string())
        }
    })?;
    let mut buffer = vec![0_u8; STREAM_CHUNK_BYTES];
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    loop {
        let read = input.read(&mut buffer).map_err(|error| {
            if source.expected_sha256.is_some() {
                AppError::backup_asset_hash_mismatch(format!(
                    "备份资产读取失败：{}",
                    source.zip_path
                ))
            } else {
                AppError::filesystem(error.to_string())
            }
        })?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or_else(|| AppError::backup_invalid("备份资产大小溢出"))?;
        if total > MAX_ENTRY_BYTES {
            return Err(AppError::backup_invalid("备份资产超过单文件大小限制"));
        }
        hasher.update(&buffer[..read]);
        writer
            .write_all(&buffer[..read])
            .map_err(|error| AppError::filesystem(error.to_string()))?;
    }
    if total != source.expected_size {
        return Err(AppError::backup_asset_hash_mismatch(format!(
            "备份资产大小不匹配：{}",
            source.zip_path
        )));
    }
    if let Some(expected_sha256) = &source.expected_sha256 {
        let actual_sha256 = hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if !actual_sha256.eq_ignore_ascii_case(expected_sha256) {
            return Err(AppError::backup_asset_hash_mismatch(format!(
                "备份资产校验值不匹配：{}",
                source.zip_path
            )));
        }
    }
    Ok(())
}

fn write_zip_json<W: Write + io::Seek, T: Serialize>(
    writer: &mut ZipWriter<W>,
    path: &str,
    value: &T,
    options: FileOptions,
) -> Result<(), AppError> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| AppError::backup_invalid(format!("备份 JSON 生成失败：{error}")))?;
    writer
        .start_file(path, options)
        .map_err(|error| AppError::backup_invalid(format!("备份目录写入失败：{error}")))?;
    writer
        .write_all(&bytes)
        .map_err(|error| AppError::filesystem(error.to_string()))?;
    Ok(())
}

fn publish_backup_file(source: &Path, destination: &Path) -> Result<(), AppError> {
    replace_backup_file(source, destination)
        .map_err(|error| AppError::filesystem(format!("备份文件发布失败：{error}")))
}

#[cfg(windows)]
fn replace_backup_file(source: &Path, destination: &Path) -> io::Result<()> {
    use std::{ffi::OsStr, iter::once, os::windows::ffi::OsStrExt};
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source = OsStr::new(source)
        .encode_wide()
        .chain(once(0))
        .collect::<Vec<_>>();
    let destination = OsStr::new(destination)
        .encode_wide()
        .chain(once(0))
        .collect::<Vec<_>>();
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_backup_file(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)
}

fn inspect_archive(
    source: &Path,
) -> Result<(ProjectBackupManifest, BackupDocument, HashSet<String>), AppError> {
    let metadata = fs::metadata(source).map_err(|error| AppError::filesystem(error.to_string()))?;
    if metadata.len() > MAX_ZIP_BYTES {
        return Err(AppError::backup_invalid("备份压缩包超过 20 GiB 限制"));
    }
    let mut archive = ZipArchive::new(
        File::open(source).map_err(|error| AppError::filesystem(error.to_string()))?,
    )
    .map_err(|error| AppError::backup_invalid(format!("备份压缩包无效：{error}")))?;
    if archive.len() > MAX_ENTRIES {
        return Err(AppError::backup_invalid("备份文件数量超过限制"));
    }
    let mut names = HashSet::new();
    let mut total_uncompressed = 0_u64;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|error| AppError::backup_invalid(format!("备份条目无效：{error}")))?;
        let name = entry.name().to_owned();
        if !safe_zip_path(&name) || entry.unix_mode().is_some_and(is_symlink_mode) {
            return Err(AppError::backup_invalid("备份包含不安全的文件路径"));
        }
        if entry.size() > MAX_ENTRY_BYTES {
            return Err(AppError::backup_invalid("备份条目超过单文件大小限制"));
        }
        total_uncompressed = total_uncompressed.saturating_add(entry.size());
        names.insert(name);
    }
    if total_uncompressed > MAX_ZIP_BYTES {
        return Err(AppError::backup_invalid("备份解压后超过大小限制"));
    }
    let first_entry_is_manifest = archive
        .by_index(0)
        .map(|entry| entry.name() == "manifest.json")
        .unwrap_or(false);
    if !first_entry_is_manifest {
        return Err(AppError::backup_invalid("备份必须先包含 manifest.json"));
    }
    let manifest: ProjectBackupManifest = read_zip_json(&mut archive, "manifest.json")?;
    if manifest.format != BACKUP_FORMAT
        || !matches!(manifest.version, 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10)
    {
        return Err(AppError::backup_invalid("备份格式或版本不受支持"));
    }
    let document: BackupDocument = read_zip_json(&mut archive, "project.json")?;
    if document.project.id != manifest.project.id || document.project.name != manifest.project.name
    {
        return Err(AppError::backup_invalid("备份项目元数据与 manifest 不一致"));
    }
    Ok((manifest, document, names))
}

fn read_zip_json<T: for<'de> Deserialize<'de>>(
    archive: &mut ZipArchive<File>,
    path: &str,
) -> Result<T, AppError> {
    let entry = archive
        .by_name(path)
        .map_err(|_| AppError::backup_invalid(format!("备份缺少 {path}")))?;
    let mut bytes = Vec::new();
    entry
        .take(MAX_ENTRY_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|error| AppError::backup_invalid(format!("备份 JSON 读取失败：{error}")))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| AppError::backup_invalid(format!("备份 JSON 无效：{error}")))
}

fn validate_document_entries(
    document: &BackupDocument,
    entry_names: &HashSet<String>,
) -> Result<(), AppError> {
    for asset in &document.assets {
        if !safe_zip_path(&asset.content_path) || !entry_names.contains(&asset.content_path) {
            return Err(AppError::backup_invalid("备份缺少资产文件"));
        }
        if let Some(path) = &asset.thumbnail_path {
            if !safe_zip_path(path) || !entry_names.contains(path) {
                return Err(AppError::backup_invalid("备份缺少资产缩略图"));
            }
        }
    }
    validate_asset_video_prompt_document(document)?;
    validate_production_item_review_document(document)?;
    validate_organization_document(document)?;
    validate_prompt_document(document)?;
    validate_benchmark_document(document)?;
    validate_production_orchestrator_document(document)?;
    validate_shot_document(document)?;
    Ok(())
}

fn validate_production_orchestrator_document(document: &BackupDocument) -> Result<(), AppError> {
    use crate::domain::{ProductionRunStatus, ProductionStageStatus, ProductionStageType};

    let run_ids = document
        .production_runs
        .iter()
        .map(|run| run.id.as_str())
        .collect::<HashSet<_>>();
    if run_ids.len() != document.production_runs.len() {
        return Err(AppError::backup_invalid("Production Run ID 重复"));
    }
    let template_ids = document
        .production_run_templates
        .iter()
        .map(|template| template.id.as_str())
        .collect::<HashSet<_>>();
    if template_ids.len() != document.production_run_templates.len() {
        return Err(AppError::backup_invalid("Production Run 模板 ID 重复"));
    }
    for template in &document.production_run_templates {
        if template.project_id != document.project.id
            || template.name.trim().is_empty()
            || template.name.chars().count() > 120
            || !(1..=100).contains(&template.default_image_count)
            || template
                .default_duration_seconds
                .is_some_and(|value| !(1..=15).contains(&value))
        {
            return Err(AppError::backup_invalid("Production Run 模板无效"));
        }
    }
    for run in &document.production_runs {
        if run.project_id != document.project.id
            || run.id.trim().is_empty()
            || run.name.trim().is_empty()
            || run.name.chars().count() > 120
            || run.current_stage_ordinal < 0
            || ProductionRunStatus::parse(&run.status).is_none()
            || run
                .template_id
                .as_ref()
                .is_some_and(|id| !template_ids.contains(id.as_str()))
        {
            return Err(AppError::backup_invalid("Production Run 元数据无效"));
        }
    }
    let mut stage_ids = HashSet::new();
    let mut stage_keys = HashSet::new();
    for stage in &document.production_stages {
        if stage.id.trim().is_empty()
            || !stage_ids.insert(stage.id.as_str())
            || !run_ids.contains(stage.run_id.as_str())
            || stage.ordinal < 0
            || !stage_keys.insert((stage.run_id.as_str(), stage.ordinal))
            || ProductionStageType::parse(&stage.stage_type).is_none()
            || ProductionStageStatus::parse(&stage.status).is_none()
            || stage
                .production_batch_id
                .as_ref()
                .is_some_and(|id| !document.batches.iter().any(|batch| batch.id == *id))
            || !stage.frozen_config.is_object()
            || contains_external_absolute_path(&stage.frozen_config)
        {
            return Err(AppError::backup_invalid("Production Stage 元数据无效"));
        }
        if stage.workflow_version_id.is_some() != stage.recipe_id.is_some() {
            return Err(AppError::backup_invalid(
                "Production Stage Workflow / Recipe 引用不完整",
            ));
        }
    }
    let mut stage_item_ids = HashSet::new();
    let stage_id_set = stage_ids.clone();
    let item_ids = document
        .items
        .iter()
        .map(|item| item.id.as_str())
        .collect::<HashSet<_>>();
    let task_ids = document
        .tasks
        .iter()
        .map(|task| task.id.as_str())
        .collect::<HashSet<_>>();
    let asset_ids = document
        .assets
        .iter()
        .map(|asset| asset.id.as_str())
        .collect::<HashSet<_>>();
    let mut parent_ids = HashSet::new();
    for item in &document.production_stage_items {
        if item.id.trim().is_empty()
            || !stage_item_ids.insert(item.id.as_str())
            || !stage_id_set.contains(item.stage_id.as_str())
            || item.ordinal < 0
            || item.attempt < 1
            || !item.frozen_values.is_object()
            || contains_external_absolute_path(&item.frozen_values)
            || !ProductionStageStatus::parse(&item.status).is_some()
            || item
                .production_batch_item_id
                .as_ref()
                .is_some_and(|id| !item_ids.contains(id.as_str()))
            || item
                .task_id
                .as_ref()
                .is_some_and(|id| !task_ids.contains(id.as_str()))
            || item
                .asset_id
                .as_ref()
                .is_some_and(|id| !asset_ids.contains(id.as_str()))
            || item
                .source_asset_id
                .as_ref()
                .is_some_and(|id| !asset_ids.contains(id.as_str()))
        {
            return Err(AppError::backup_invalid("Production Stage Item 元数据无效"));
        }
        if let Some(parent_id) = &item.parent_stage_item_id {
            if !stage_item_ids.contains(parent_id.as_str()) {
                parent_ids.insert(parent_id.clone());
            }
        }
    }
    if parent_ids
        .iter()
        .any(|id| !stage_item_ids.contains(id.as_str()))
    {
        return Err(AppError::backup_invalid(
            "Production Stage Item 父级引用无效",
        ));
    }

    let experiment_ids = document
        .benchmark_experiments
        .iter()
        .map(|experiment| experiment.id.as_str())
        .collect::<HashSet<_>>();
    let candidate_ids = document
        .benchmark_candidates
        .iter()
        .map(|candidate| (candidate.id.as_str(), candidate.experiment_id.as_str()))
        .collect::<HashMap<_, _>>();
    let snapshot_ids = document
        .snapshots
        .iter()
        .map(|snapshot| snapshot.id.as_str())
        .collect::<HashSet<_>>();
    let mut benchmark_run_ids = HashSet::new();
    for run in &document.benchmark_runs {
        if run.id.trim().is_empty()
            || !benchmark_run_ids.insert(run.id.as_str())
            || !experiment_ids.contains(run.experiment_id.as_str())
            || candidate_ids.get(run.candidate_id.as_str()).copied()
                != Some(run.experiment_id.as_str())
            || run.run_number < 1
            || run
                .production_batch_item_id
                .as_ref()
                .is_some_and(|id| !item_ids.contains(id.as_str()))
            || run
                .task_id
                .as_ref()
                .is_some_and(|id| !task_ids.contains(id.as_str()))
            || run
                .snapshot_id
                .as_ref()
                .is_some_and(|id| !snapshot_ids.contains(id.as_str()))
            || run
                .output_asset_id
                .as_ref()
                .is_some_and(|id| !asset_ids.contains(id.as_str()))
            || run.output_file_size.is_some_and(|value| value < 0)
        {
            return Err(AppError::backup_invalid("Benchmark Run 元数据无效"));
        }
    }
    let mut quality_ids = HashSet::new();
    let mut scored_candidates = HashSet::new();
    for score in &document.benchmark_quality_scores {
        if score.id.trim().is_empty()
            || !quality_ids.insert(score.id.as_str())
            || !candidate_ids.contains_key(score.candidate_id.as_str())
            || !scored_candidates.insert(score.candidate_id.as_str())
            || !valid_score(score.prompt_adherence)
            || !valid_score(score.visual_quality)
            || !valid_score(score.motion_quality)
            || !valid_score(score.reference_consistency)
            || !valid_score(score.overall)
        {
            return Err(AppError::backup_invalid(
                "Benchmark Quality Score 元数据无效",
            ));
        }
    }
    Ok(())
}

fn valid_score(value: Option<i64>) -> bool {
    value.is_none_or(|value| (1..=5).contains(&value))
}

fn contains_external_absolute_path(value: &Value) -> bool {
    match value {
        Value::Array(values) => values.iter().any(contains_external_absolute_path),
        Value::Object(values) => values.values().any(contains_external_absolute_path),
        Value::String(value) => {
            value.starts_with('/')
                || value.starts_with('\\')
                || value.as_bytes().get(1) == Some(&b':')
        }
        Value::Bool(_) | Value::Number(_) | Value::Null => false,
    }
}

fn validate_benchmark_document(document: &BackupDocument) -> Result<(), AppError> {
    let batch_ids = document
        .batches
        .iter()
        .map(|batch| batch.id.as_str())
        .collect::<HashSet<_>>();
    let item_batches = document
        .items
        .iter()
        .map(|item| (item.id.as_str(), item.batch_id.as_str()))
        .collect::<HashMap<_, _>>();
    let task_ids = document
        .tasks
        .iter()
        .map(|task| task.id.as_str())
        .collect::<HashSet<_>>();
    let asset_ids = document
        .assets
        .iter()
        .map(|asset| asset.id.as_str())
        .collect::<HashSet<_>>();
    let experiment_ids = document
        .benchmark_experiments
        .iter()
        .map(|experiment| experiment.id.as_str())
        .collect::<HashSet<_>>();
    if experiment_ids.len() != document.benchmark_experiments.len() {
        return Err(AppError::backup_invalid("Benchmark 实验 ID 重复"));
    }
    let mut candidate_ids = HashSet::new();
    let mut candidate_positions = HashSet::new();
    for experiment in &document.benchmark_experiments {
        if experiment.name.trim().is_empty()
            || experiment.name.chars().count() > 120
            || !matches!(experiment.media_type.as_str(), "IMAGE" | "VIDEO")
            || !matches!(
                experiment.status.as_str(),
                "DRAFT"
                    | "QUEUED"
                    | "RUNNING"
                    | "COMPLETED"
                    | "PARTIAL"
                    | "CANCELLED"
                    | "FAILED_TO_QUEUE"
            )
            || !experiment.base_values.is_object()
            || experiment
                .asset_ids
                .iter()
                .any(|id| !asset_ids.contains(id.as_str()))
            || experiment
                .production_batch_id
                .as_ref()
                .is_some_and(|id| !batch_ids.contains(id.as_str()))
            || experiment.winner_candidate_id.as_ref().is_some_and(|id| {
                !document.benchmark_candidates.iter().any(|candidate| {
                    candidate.id == *id && candidate.experiment_id == experiment.id
                })
            })
        {
            return Err(AppError::backup_invalid("Benchmark 实验元数据无效"));
        }
    }
    for candidate in &document.benchmark_candidates {
        if !experiment_ids.contains(candidate.experiment_id.as_str())
            || !candidate_ids.insert(candidate.id.as_str())
            || candidate.position < 0
            || !candidate_positions.insert((candidate.experiment_id.as_str(), candidate.position))
            || candidate.workflow_version_id.trim().is_empty()
            || candidate.recipe_id.trim().is_empty()
            || !candidate.values.is_object()
            || candidate
                .asset_ids
                .iter()
                .any(|id| !asset_ids.contains(id.as_str()))
            || candidate
                .production_batch_item_id
                .as_ref()
                .is_some_and(|id| !item_batches.contains_key(id.as_str()))
            || candidate
                .task_id
                .as_ref()
                .is_some_and(|id| !task_ids.contains(id.as_str()))
        {
            return Err(AppError::backup_invalid("Benchmark 候选元数据无效"));
        }
        if let Some(item_id) = &candidate.production_batch_item_id {
            let experiment_batch = document
                .benchmark_experiments
                .iter()
                .find(|experiment| experiment.id == candidate.experiment_id)
                .and_then(|experiment| experiment.production_batch_id.as_deref());
            if experiment_batch != item_batches.get(item_id.as_str()).copied() {
                return Err(AppError::backup_invalid("Benchmark 候选队列引用不一致"));
            }
        }
    }
    for experiment in &document.benchmark_experiments {
        let count = document
            .benchmark_candidates
            .iter()
            .filter(|candidate| candidate.experiment_id == experiment.id)
            .count();
        if !(2..=crate::application::workflow_benchmark_service::MAX_BENCHMARK_CANDIDATES)
            .contains(&count)
        {
            return Err(AppError::backup_invalid("Benchmark 候选数量必须为 2–8"));
        }
    }
    Ok(())
}

fn validate_production_item_review_document(document: &BackupDocument) -> Result<(), AppError> {
    use crate::domain::ProductionReviewStatus;

    let batch_ids = document
        .batches
        .iter()
        .map(|batch| batch.id.as_str())
        .collect::<HashSet<_>>();
    let item_ids = document
        .items
        .iter()
        .map(|item| (item.id.as_str(), item.batch_id.as_str()))
        .collect::<HashMap<_, _>>();
    let task_ids = document
        .tasks
        .iter()
        .map(|task| task.id.as_str())
        .collect::<HashSet<_>>();
    let asset_ids = document
        .assets
        .iter()
        .map(|asset| asset.id.as_str())
        .collect::<HashSet<_>>();
    let mut review_ids = HashSet::new();
    let mut reviewed_items = HashSet::new();
    let mut lineage_versions = HashSet::new();
    for review in &document.production_item_reviews {
        if review.project_id != document.project.id
            || review.id.trim().is_empty()
            || !review_ids.insert(review.id.as_str())
            || !reviewed_items.insert(review.production_batch_item_id.as_str())
            || item_ids
                .get(review.production_batch_item_id.as_str())
                .copied()
                != Some(review.production_batch_id.as_str())
            || !batch_ids.contains(review.production_batch_id.as_str())
            || review.version < 1
            || review.lineage_key.trim().is_empty()
            || !lineage_versions.insert((review.lineage_key.as_str(), review.version))
            || review.review_note.as_bytes().len() > 4 * 1024
            || review
                .task_id
                .as_ref()
                .is_some_and(|task_id| !task_ids.contains(task_id.as_str()))
            || review
                .result_asset_id
                .as_ref()
                .is_some_and(|asset_id| !asset_ids.contains(asset_id.as_str()))
            || review
                .parent_batch_id
                .as_ref()
                .is_some_and(|batch_id| !batch_ids.contains(batch_id.as_str()))
            || review
                .parent_item_id
                .as_ref()
                .is_some_and(|item_id| !item_ids.contains_key(item_id.as_str()))
            || ProductionReviewStatus::parse(&review.review_status).is_err()
        {
            return Err(AppError::backup_invalid(
                "备份审片版本无效或引用了未知项目数据",
            ));
        }
    }
    Ok(())
}

fn validate_asset_video_prompt_document(document: &BackupDocument) -> Result<(), AppError> {
    use crate::application::asset_video_prompt_service::MAX_ASSET_VIDEO_PROMPT_BYTES;

    let asset_types = document
        .assets
        .iter()
        .map(|asset| (asset.id.as_str(), asset.asset_type.as_str()))
        .collect::<HashMap<_, _>>();
    let mut asset_ids = HashSet::new();
    for prompt in &document.asset_video_prompts {
        if prompt.project_id != document.project.id
            || !asset_ids.insert(prompt.asset_id.as_str())
            || asset_types.get(prompt.asset_id.as_str()).copied() != Some("image")
            || prompt.prompt_text.trim().is_empty()
            || prompt.prompt_text.trim() != prompt.prompt_text
            || prompt.prompt_text.len() > MAX_ASSET_VIDEO_PROMPT_BYTES
        {
            return Err(AppError::backup_invalid(
                "备份资产视频提示词无效或项目归属不一致",
            ));
        }
    }
    Ok(())
}

fn validate_shot_document(document: &BackupDocument) -> Result<(), AppError> {
    use crate::domain::{canonical_shot_name, validate_scalar_values, ShotStage};

    let mut shot_ids = HashSet::new();
    let mut ordinals = Vec::with_capacity(document.shots.len());
    let asset_types = document
        .assets
        .iter()
        .map(|asset| (asset.id.as_str(), asset.asset_type.as_str()))
        .collect::<HashMap<_, _>>();
    let prompt_entry_ids = document
        .prompt_entries
        .iter()
        .map(|entry| entry.id.as_str())
        .collect::<HashSet<_>>();
    let prompt_versions = document
        .prompt_versions
        .iter()
        .map(|version| (version.id.as_str(), version.prompt_id.as_str()))
        .collect::<HashMap<_, _>>();
    for shot in &document.shots {
        if shot.project_id != document.project.id
            || shot.id.trim().is_empty()
            || !shot_ids.insert(shot.id.as_str())
        {
            return Err(AppError::backup_invalid("备份镜头 ID 或项目归属无效"));
        }
        if shot.ordinal < 0 {
            return Err(AppError::backup_invalid("备份镜头序号无效"));
        }
        canonical_shot_name(&shot.name)
            .map_err(|error| AppError::backup_invalid(format!("镜头名称无效：{error}")))?;
        if shot.prompt_entry_id.is_some() != shot.prompt_version_id.is_some() {
            return Err(AppError::backup_invalid(
                "备份镜头 Prompt provenance 不完整",
            ));
        }
        if let (Some(entry_id), Some(version_id)) = (&shot.prompt_entry_id, &shot.prompt_version_id)
        {
            if !prompt_entry_ids.contains(entry_id.as_str())
                || prompt_versions.get(version_id.as_str()).copied() != Some(entry_id.as_str())
            {
                return Err(AppError::backup_invalid(
                    "备份镜头 Prompt provenance 引用无效",
                ));
            }
        }
        if shot
            .selected_image_asset_id
            .as_ref()
            .is_some_and(|asset_id| asset_types.get(asset_id.as_str()).copied() != Some("image"))
            || shot
                .selected_video_asset_id
                .as_ref()
                .is_some_and(|asset_id| {
                    asset_types.get(asset_id.as_str()).copied() != Some("video")
                })
        {
            return Err(AppError::backup_invalid("备份镜头选定素材类型无效"));
        }
        ordinals.push(shot.ordinal);
    }
    ordinals.sort_unstable();
    if ordinals
        .iter()
        .enumerate()
        .any(|(index, ordinal)| *ordinal != index as i64)
    {
        return Err(AppError::backup_invalid("备份镜头序号必须从 0 连续排列"));
    }

    let mut stage_configs = HashSet::new();
    for config in &document.shot_stage_configs {
        if !shot_ids.contains(config.shot_id.as_str())
            || ShotStage::try_from_str(&config.stage).is_err()
            || config.workflow_id.trim().is_empty()
            || config.workflow_version_id.trim().is_empty()
            || config.recipe_id.trim().is_empty()
            || !stage_configs.insert((config.shot_id.as_str(), config.stage.as_str()))
        {
            return Err(AppError::backup_invalid("备份镜头阶段配置无效或重复"));
        }
        validate_scalar_values(&config.scalar_values)
            .map_err(|error| AppError::backup_invalid(format!("镜头阶段参数无效：{error}")))?;
    }

    let mut references = HashSet::new();
    let mut reference_ordinals = HashMap::<(&str, &str), Vec<i64>>::new();
    for reference in &document.shot_reference_assets {
        if !shot_ids.contains(reference.shot_id.as_str())
            || ShotStage::try_from_str(&reference.stage).is_err()
            || asset_types.get(reference.asset_id.as_str()).copied() != Some("image")
            || reference.ordinal < 0
            || !references.insert((
                reference.shot_id.as_str(),
                reference.stage.as_str(),
                reference.asset_id.as_str(),
            ))
        {
            return Err(AppError::backup_invalid(
                "备份镜头 Reference 素材关系无效或重复",
            ));
        }
        reference_ordinals
            .entry((reference.shot_id.as_str(), reference.stage.as_str()))
            .or_default()
            .push(reference.ordinal);
    }
    for mut ordinals in reference_ordinals.into_values() {
        ordinals.sort_unstable();
        if ordinals
            .iter()
            .enumerate()
            .any(|(index, ordinal)| *ordinal != index as i64)
        {
            return Err(AppError::backup_invalid("备份镜头 Reference 序号必须连续"));
        }
    }

    let task_ids = document
        .tasks
        .iter()
        .map(|task| task.id.as_str())
        .collect::<HashSet<_>>();
    let item_ids = document
        .items
        .iter()
        .map(|item| item.id.as_str())
        .collect::<HashSet<_>>();
    let mut link_ids = HashSet::new();
    let mut linked_tasks = HashSet::new();
    let mut linked_batch_items = HashSet::new();
    for link in &document.shot_generation_links {
        if !shot_ids.contains(link.shot_id.as_str())
            || ShotStage::try_from_str(&link.stage).is_err()
            || !link_ids.insert(link.id.as_str())
            || link
                .task_id
                .as_ref()
                .is_some_and(|task_id| !task_ids.contains(task_id.as_str()))
            || link
                .production_batch_item_id
                .as_ref()
                .is_some_and(|item_id| !item_ids.contains(item_id.as_str()))
            || link
                .production_batch_item_id
                .as_ref()
                .is_some_and(|item_id| !linked_batch_items.insert(item_id.as_str()))
        {
            return Err(AppError::backup_invalid(
                "备份镜头生成关联无效或引用未知任务",
            ));
        }
        if let Some(task_id) = &link.task_id {
            if !linked_tasks.insert(task_id.as_str()) {
                return Err(AppError::backup_invalid("备份镜头生成关联重复引用任务"));
            }
        }
    }
    Ok(())
}

fn validate_prompt_document(document: &BackupDocument) -> Result<(), AppError> {
    use crate::application::prompt_library_service::{
        canonical_prompt_name, canonical_prompt_tags, canonical_prompt_text,
    };

    let mut entry_ids = HashSet::new();
    let mut names = HashSet::new();
    for entry in &document.prompt_entries {
        if entry.id.trim().is_empty() || !entry_ids.insert(entry.id.as_str()) {
            return Err(AppError::backup_invalid("备份包含重复或空提示词 ID"));
        }
        if entry.project_id != document.project.id {
            return Err(AppError::backup_invalid("备份提示词项目归属不一致"));
        }
        if !matches!(entry.kind.as_str(), "prompt" | "snippet") {
            return Err(AppError::backup_invalid("备份提示词类型无效"));
        }
        let (name, normalized_name) = canonical_prompt_name(&entry.name)
            .map_err(|error| AppError::backup_invalid(format!("提示词名称无效：{error}")))?;
        if name != entry.name || normalized_name != entry.normalized_name {
            return Err(AppError::backup_invalid("备份提示词名称规范化字段不匹配"));
        }
        let tags = canonical_prompt_tags(&entry.tags)
            .map_err(|error| AppError::backup_invalid(format!("提示词标签无效：{error}")))?;
        if tags != entry.tags {
            return Err(AppError::backup_invalid("备份提示词标签不是规范数组"));
        }
        if !names.insert((entry.kind.as_str(), entry.normalized_name.as_str())) {
            return Err(AppError::backup_invalid("备份包含重复提示词名称"));
        }
    }

    let mut version_ids = HashSet::new();
    let mut version_numbers = HashSet::new();
    for version in &document.prompt_versions {
        if version.id.trim().is_empty() || !version_ids.insert(version.id.as_str()) {
            return Err(AppError::backup_invalid("备份包含重复或空提示词版本 ID"));
        }
        if version.project_id != document.project.id {
            return Err(AppError::backup_invalid("备份提示词版本项目归属不一致"));
        }
        if !entry_ids.contains(version.prompt_id.as_str()) || version.version <= 0 {
            return Err(AppError::backup_invalid("备份提示词版本引用或编号无效"));
        }
        if !version_numbers.insert((version.prompt_id.as_str(), version.version)) {
            return Err(AppError::backup_invalid("备份包含重复提示词版本编号"));
        }
        let text = canonical_prompt_text(&version.text)
            .map_err(|error| AppError::backup_invalid(format!("提示词版本正文无效：{error}")))?;
        if text != version.text {
            return Err(AppError::backup_invalid("备份提示词版本正文不是规范文本"));
        }
    }
    Ok(())
}

fn validate_organization_document(document: &BackupDocument) -> Result<(), AppError> {
    const MAX_PROJECT_TAGS: usize = 100;
    const MAX_ASSET_TAGS: usize = 20;
    let asset_ids = document
        .assets
        .iter()
        .map(|asset| asset.id.as_str())
        .collect::<HashSet<_>>();
    if document.asset_tags.len() > MAX_PROJECT_TAGS {
        return Err(AppError::backup_invalid("备份项目标签数量超过 100 个上限"));
    }
    let tag_ids = document
        .asset_tags
        .iter()
        .map(|tag| tag.id.as_str())
        .collect::<HashSet<_>>();
    if tag_ids.len() != document.asset_tags.len() {
        return Err(AppError::backup_invalid("备份包含重复标签 ID"));
    }
    let mut normalized = HashSet::new();
    for tag in &document.asset_tags {
        let (canonical_name, canonical_normalized_name) =
            crate::application::organization_service::normalize_name(&tag.name, 32, "ASSET_TAG")
                .map_err(|error| AppError::backup_invalid(format!("标签名称无效：{error}")))?;
        if tag.name != canonical_name {
            return Err(AppError::backup_invalid("备份包含非规范标签名称"));
        }
        if tag.normalized_name != canonical_normalized_name {
            return Err(AppError::backup_invalid("备份包含不匹配的标签规范名称"));
        }
        if !normalized.insert(canonical_normalized_name) {
            return Err(AppError::backup_invalid("备份包含重复标签名称"));
        }
    }
    if document
        .asset_tags
        .iter()
        .any(|tag| tag.project_id != document.project.id)
        || document
            .asset_tag_links
            .iter()
            .any(|link| link.project_id != document.project.id)
        || document
            .asset_favorites
            .iter()
            .any(|favorite| favorite.project_id != document.project.id)
    {
        return Err(AppError::backup_invalid("备份组织数据项目归属不一致"));
    }
    let mut links = HashSet::new();
    let mut tags_per_asset = HashMap::<&str, usize>::new();
    for link in &document.asset_tag_links {
        if !asset_ids.contains(link.asset_id.as_str()) || !tag_ids.contains(link.tag_id.as_str()) {
            return Err(AppError::backup_invalid("备份标签链接引用了未知素材或标签"));
        }
        if !links.insert((link.asset_id.as_str(), link.tag_id.as_str())) {
            return Err(AppError::backup_invalid("备份包含重复标签链接"));
        }
        let count = tags_per_asset.entry(link.asset_id.as_str()).or_default();
        *count += 1;
        if *count > MAX_ASSET_TAGS {
            return Err(AppError::backup_invalid("备份素材标签数量超过 20 个上限"));
        }
    }
    let mut favorites = HashSet::new();
    for favorite in &document.asset_favorites {
        if !asset_ids.contains(favorite.asset_id.as_str()) {
            return Err(AppError::backup_invalid("备份收藏引用了未知素材"));
        }
        if !favorites.insert(favorite.asset_id.as_str()) {
            return Err(AppError::backup_invalid("备份包含重复收藏"));
        }
    }
    Ok(())
}

fn safe_zip_path(name: &str) -> bool {
    if name.is_empty()
        || name.starts_with('/')
        || name.starts_with('\\')
        || name.as_bytes().get(1) == Some(&b':')
    {
        return false;
    }
    let path = Path::new(name);
    if path.is_absolute() {
        return false;
    }
    !path.components().any(|component| {
        matches!(component, Component::ParentDir)
            || matches!(component, Component::Normal(value) if value.to_string_lossy().contains(':'))
    }) && !name.split(['/', '\\']).any(|part| part == "..")
}

fn safe_component(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
}

fn is_symlink_mode(mode: u32) -> bool {
    mode & 0o170000 == 0o120000
}

fn restored_name(name: &str) -> String {
    let suffix = "（恢复）";
    let mut result = format!("{name}{suffix}");
    if result.chars().count() > 80 {
        result = result
            .chars()
            .take(80 - suffix.chars().count())
            .collect::<String>()
            + suffix;
    }
    result
}

fn copy_assets(
    archive: &mut ZipArchive<File>,
    staging_root: &Path,
    final_root: &Path,
    assets: &[BackupAsset],
    asset_ids: &HashMap<String, String>,
) -> Result<Vec<RestoredAsset>, AppError> {
    fs::create_dir_all(staging_root).map_err(|error| AppError::filesystem(error.to_string()))?;
    let mut restored = Vec::new();
    for asset in assets {
        if !safe_component(&asset.category) || !safe_component(&asset.asset_type) {
            return Err(AppError::backup_invalid("备份资产分类不安全"));
        }
        let new_id = asset_ids
            .get(&asset.id)
            .ok_or_else(|| AppError::backup_invalid("资产 ID 映射缺失"))?;
        let extension = extension_for_path(&asset.content_path);
        let relative = PathBuf::from("assets")
            .join(&asset.category)
            .join(&asset.asset_type)
            .join(format!("{new_id}.{extension}"));
        let storage_path = final_root.join(&relative);
        write_zip_entry(
            archive,
            &asset.content_path,
            staging_root.join(&relative),
            &asset.sha256,
            asset.file_size,
        )?;
        let thumbnail_path = if let Some(zip_path) = &asset.thumbnail_path {
            let thumb_ext = extension_for_path(zip_path);
            let relative = PathBuf::from("assets")
                .join("thumbnails")
                .join(&asset.asset_type)
                .join(format!("{new_id}.{thumb_ext}"));
            write_zip_entry(archive, zip_path, staging_root.join(&relative), "", -1)?;
            Some(final_root.join(relative).to_string_lossy().to_string())
        } else {
            None
        };
        restored.push(RestoredAsset {
            old_id: asset.id.clone(),
            new_id: new_id.clone(),
            storage_path: storage_path.to_string_lossy().to_string(),
            thumbnail_path,
        });
    }
    Ok(restored)
}

fn write_zip_entry(
    archive: &mut ZipArchive<File>,
    zip_path: &str,
    destination: PathBuf,
    expected_hash: &str,
    expected_size: i64,
) -> Result<(), AppError> {
    let mut entry = archive
        .by_name(zip_path)
        .map_err(|_| AppError::backup_invalid("备份资产条目不存在"))?;
    let mut bytes = Vec::new();
    entry
        .read_to_end(&mut bytes)
        .map_err(|error| AppError::filesystem(error.to_string()))?;
    if expected_size >= 0 && bytes.len() as i64 != expected_size {
        return Err(AppError::backup_asset_hash_mismatch("备份资产大小不匹配"));
    }
    if !expected_hash.is_empty() && !hash_bytes(&bytes).eq_ignore_ascii_case(expected_hash) {
        return Err(AppError::backup_asset_hash_mismatch("备份资产校验值不匹配"));
    }
    let parent = destination
        .parent()
        .ok_or_else(|| AppError::filesystem("恢复资产目录不可用"))?;
    fs::create_dir_all(parent).map_err(|error| AppError::filesystem(error.to_string()))?;
    let mut file =
        File::create(&destination).map_err(|error| AppError::filesystem(error.to_string()))?;
    file.write_all(&bytes)
        .map_err(|error| AppError::filesystem(error.to_string()))?;
    file.sync_all()
        .map_err(|error| AppError::filesystem(error.to_string()))?;
    Ok(())
}

fn remap_snapshot_asset_references(value: &mut Value, asset_ids: &HashMap<String, String>) {
    match value {
        Value::Array(values) => {
            for value in values {
                remap_snapshot_asset_references(value, asset_ids);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                remap_snapshot_asset_references(value, asset_ids);
            }
        }
        Value::String(value) => {
            if let Some(remapped) = asset_ids.get(value) {
                *value = remapped.clone();
            }
        }
        Value::Bool(_) | Value::Number(_) | Value::Null => {}
    }
}

fn collect_exact_asset_id_references(
    value: &Value,
    known_asset_ids: &HashSet<String>,
) -> Vec<String> {
    match value {
        Value::Array(values) => values
            .iter()
            .flat_map(|value| collect_exact_asset_id_references(value, known_asset_ids))
            .collect(),
        Value::Object(values) => values
            .values()
            .flat_map(|value| collect_exact_asset_id_references(value, known_asset_ids))
            .collect(),
        Value::String(value) if known_asset_ids.contains(value) => vec![value.clone()],
        Value::Bool(_) | Value::Number(_) | Value::Null | Value::String(_) => Vec::new(),
    }
}

fn prepare_restored_snapshots(
    document: &BackupDocument,
    asset_ids: &HashMap<String, String>,
) -> Result<Vec<BackupSnapshot>, AppError> {
    let backup_asset_ids = document
        .assets
        .iter()
        .map(|asset| asset.id.clone())
        .collect::<HashSet<_>>();
    if backup_asset_ids.len() != document.assets.len()
        || backup_asset_ids.len() != asset_ids.len()
        || backup_asset_ids
            .iter()
            .any(|asset_id| !asset_ids.contains_key(asset_id))
    {
        return Err(AppError::backup_snapshot_asset_remap_failed(
            "备份快照资产映射不完整，恢复已取消。",
        ));
    }

    let restored_asset_ids = asset_ids.values().cloned().collect::<HashSet<_>>();
    if restored_asset_ids.len() != asset_ids.len() {
        return Err(AppError::backup_snapshot_asset_remap_failed(
            "备份快照资产映射存在重复目标，恢复已取消。",
        ));
    }

    document
        .snapshots
        .iter()
        .map(|snapshot| {
            let mut user_inputs = snapshot.user_inputs.clone();
            let mut resolved_inputs = snapshot.resolved_inputs.clone();
            remap_snapshot_asset_references(&mut user_inputs, asset_ids);
            remap_snapshot_asset_references(&mut resolved_inputs, asset_ids);

            let stale_references =
                collect_exact_asset_id_references(&user_inputs, &backup_asset_ids)
                    .into_iter()
                    .chain(collect_exact_asset_id_references(
                        &resolved_inputs,
                        &backup_asset_ids,
                    ))
                    .collect::<Vec<_>>();
            if !stale_references.is_empty() {
                return Err(AppError::backup_snapshot_asset_remap_failed(
                    "恢复后的任务快照仍包含原项目素材引用，恢复已取消。",
                ));
            }

            Ok(BackupSnapshot {
                user_inputs,
                resolved_inputs,
                ..snapshot.clone()
            })
        })
        .collect()
}

async fn restore_rows_in_transaction(
    transaction: &mut Transaction<'_, sqlx::Sqlite>,
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
    benchmark_experiment_ids: &HashMap<String, String>,
    benchmark_candidate_ids: &HashMap<String, String>,
    production_run_ids: &HashMap<String, String>,
    production_stage_ids: &HashMap<String, String>,
    production_stage_item_ids: &HashMap<String, String>,
    production_run_template_ids: &HashMap<String, String>,
    benchmark_run_ids: &HashMap<String, String>,
    benchmark_quality_score_ids: &HashMap<String, String>,
    tag_ids: &HashMap<String, String>,
    shot_ids: &HashMap<String, String>,
    shot_generation_link_ids: &HashMap<String, String>,
    restored_assets: &[RestoredAsset],
    restored_snapshots: &[BackupSnapshot],
) -> Result<(), AppError> {
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
    .map_err(|error| AppError::database(error.to_string()))?;

    for entry in &document.prompt_entries {
        let prompt_id = prompt_ids
            .get(&entry.id)
            .ok_or_else(|| AppError::backup_invalid("提示词 ID 映射缺失"))?;
        let tags_json = serde_json::to_string(&entry.tags)
            .map_err(|error| AppError::backup_invalid(format!("提示词标签序列化失败：{error}")))?;
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
        .map_err(|error| AppError::database(error.to_string()))?;
    }
    for version in &document.prompt_versions {
        let version_id = prompt_version_ids
            .get(&version.id)
            .ok_or_else(|| AppError::backup_invalid("提示词版本 ID 映射缺失"))?;
        let prompt_id = prompt_ids
            .get(&version.prompt_id)
            .ok_or_else(|| AppError::backup_invalid("提示词版本引用缺少提示词映射"))?;
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
        .map_err(|error| AppError::database(error.to_string()))?;
    }

    for reference in &document.workflow_refs {
        ensure_workflow_dependency(transaction, reference).await?;
    }
    for task in &document.tasks {
        let new_task_id = task_ids
            .get(&task.id)
            .ok_or_else(|| AppError::backup_invalid("任务 ID 映射缺失"))?;
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
        .map_err(|error| AppError::database(error.to_string()))?;
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
        .map_err(|error| AppError::database(error.to_string()))?;
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
        .map_err(|error| AppError::database(error.to_string()))?;
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
        .map_err(|error| AppError::database(error.to_string()))?;
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
        .map_err(|error| AppError::database(error.to_string()))?;
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
        .map_err(|error| AppError::database(error.to_string()))?;
    }
    for tag in &document.asset_tags {
        let tag_id = tag_ids
            .get(&tag.id)
            .ok_or_else(|| AppError::backup_invalid("标签 ID 映射缺失"))?;
        sqlx::query("INSERT INTO asset_tags (id, project_id, name, normalized_name, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(tag_id).bind(&project.id).bind(&tag.name).bind(&tag.normalized_name).bind(&tag.created_at).bind(&tag.updated_at)
            .execute(&mut **transaction).await.map_err(|error| AppError::database(error.to_string()))?;
    }
    for link in &document.asset_tag_links {
        let asset_id = asset_ids
            .get(&link.asset_id)
            .ok_or_else(|| AppError::backup_invalid("标签链接缺少资产映射"))?;
        let tag_id = tag_ids
            .get(&link.tag_id)
            .ok_or_else(|| AppError::backup_invalid("标签链接缺少标签映射"))?;
        sqlx::query("INSERT INTO asset_tag_links (asset_id, tag_id, project_id, created_at) VALUES (?, ?, ?, ?)")
            .bind(asset_id).bind(tag_id).bind(&project.id).bind(&link.created_at)
            .execute(&mut **transaction).await.map_err(|error| AppError::database(error.to_string()))?;
    }
    for favorite in &document.asset_favorites {
        let asset_id = asset_ids
            .get(&favorite.asset_id)
            .ok_or_else(|| AppError::backup_invalid("收藏缺少资产映射"))?;
        sqlx::query(
            "INSERT INTO asset_favorites (asset_id, project_id, created_at) VALUES (?, ?, ?)",
        )
        .bind(asset_id)
        .bind(&project.id)
        .bind(&favorite.created_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| AppError::database(error.to_string()))?;
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
            .execute(&mut **transaction).await.map_err(|error| AppError::database(error.to_string()))?;
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
            .execute(&mut **transaction).await.map_err(|error| AppError::database(error.to_string()))?;
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
            .execute(&mut **transaction).await.map_err(|error| AppError::database(error.to_string()))?;
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
            .execute(&mut **transaction).await.map_err(|error| AppError::database(error.to_string()))?;
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
        .map_err(|error| AppError::database(error.to_string()))?;
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
        .map_err(|error| AppError::database(error.to_string()))?;
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
        .map_err(|error| AppError::database(error.to_string()))?;
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
        .map_err(|error| AppError::database(error.to_string()))?;
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
        .map_err(|error| AppError::database(error.to_string()))?;
    }
    for experiment in &document.benchmark_experiments {
        let experiment_id = benchmark_experiment_ids
            .get(&experiment.id)
            .ok_or_else(|| AppError::backup_invalid("Benchmark 实验 ID 映射缺失"))?;
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
            AppError::backup_invalid(format!("Benchmark 素材序列化失败：{error}"))
        })?)
        .bind(winner_candidate_id)
        .bind(production_batch_id)
        .bind(&experiment.created_at)
        .bind(&experiment.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| AppError::database(error.to_string()))?;
    }
    for candidate in &document.benchmark_candidates {
        let candidate_id = benchmark_candidate_ids
            .get(&candidate.id)
            .ok_or_else(|| AppError::backup_invalid("Benchmark 候选 ID 映射缺失"))?;
        let experiment_id = benchmark_experiment_ids
            .get(&candidate.experiment_id)
            .ok_or_else(|| AppError::backup_invalid("Benchmark 候选缺少实验映射"))?;
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
            AppError::backup_invalid(format!("Benchmark 候选素材序列化失败：{error}"))
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
        .map_err(|error| AppError::database(error.to_string()))?;
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
        .map_err(|error| AppError::database(error.to_string()))?;
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
        .map_err(|error| AppError::database(error.to_string()))?;
    }
    for shot in &document.shots {
        let shot_id = shot_ids
            .get(&shot.id)
            .ok_or_else(|| AppError::backup_invalid("镜头 ID 映射缺失"))?;
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
        .map_err(|error| AppError::database(error.to_string()))?;
    }
    for config in &document.shot_stage_configs {
        let shot_id = shot_ids
            .get(&config.shot_id)
            .ok_or_else(|| AppError::backup_invalid("镜头阶段配置缺少镜头映射"))?;
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
        .map_err(|error| AppError::database(error.to_string()))?;
    }
    for prompt in &document.shot_stage_prompts {
        let shot_id = shot_ids
            .get(&prompt.shot_id)
            .ok_or_else(|| AppError::backup_invalid("镜头阶段 Prompt 缺少镜头映射"))?;
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
        .map_err(|error| AppError::database(error.to_string()))?;
    }
    for reference in &document.shot_reference_assets {
        let shot_id = shot_ids
            .get(&reference.shot_id)
            .ok_or_else(|| AppError::backup_invalid("镜头 Reference 缺少镜头映射"))?;
        let asset_id = asset_ids
            .get(&reference.asset_id)
            .ok_or_else(|| AppError::backup_invalid("镜头 Reference 缺少素材映射"))?;
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
        .map_err(|error| AppError::database(error.to_string()))?;
    }
    for link in &document.shot_generation_links {
        let link_id = shot_generation_link_ids
            .get(&link.id)
            .ok_or_else(|| AppError::backup_invalid("镜头生成关联 ID 映射缺失"))?;
        let shot_id = shot_ids
            .get(&link.shot_id)
            .ok_or_else(|| AppError::backup_invalid("镜头生成关联缺少镜头映射"))?;
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
        .map_err(|error| AppError::database(error.to_string()))?;
    }
    Ok(())
}

async fn validate_restored_snapshot_asset_ownership(
    transaction: &mut Transaction<'_, Sqlite>,
    project_id: &str,
    snapshots: &[BackupSnapshot],
    asset_ids: &HashMap<String, String>,
) -> Result<(), AppError> {
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
        .map_err(|error| AppError::database(error.to_string()))?;
        if owned == 0 {
            return Err(AppError::backup_snapshot_asset_remap_failed(
                "恢复后的任务快照引用了不属于当前项目的素材，恢复已取消。",
            ));
        }
    }

    Ok(())
}

async fn ensure_workflow_dependency(
    transaction: &mut Transaction<'_, sqlx::Sqlite>,
    reference: &WorkflowReference,
) -> Result<(), AppError> {
    let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflows WHERE id = ?")
        .bind(&reference.workflow_id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(|error| AppError::database(error.to_string()))?;
    if exists == 0 {
        sqlx::query("INSERT INTO workflows (id, name, category, mode, current_version_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&reference.workflow_id).bind("已恢复历史工作流").bind("restored").bind("api").bind(&reference.workflow_version_id).bind(Utc::now().to_rfc3339()).bind(Utc::now().to_rfc3339())
            .execute(&mut **transaction).await.map_err(|error| AppError::database(error.to_string()))?;
    }
    ensure_version_recipe_dependency(
        transaction,
        &reference.workflow_version_id,
        &reference.recipe_id,
    )
    .await
}

async fn ensure_version_recipe_dependency(
    transaction: &mut Transaction<'_, sqlx::Sqlite>,
    workflow_version_id: &str,
    recipe_id: &str,
) -> Result<(), AppError> {
    let version = sqlx::query_as::<_, (String, String)>(
        "SELECT workflow_id, version FROM workflow_versions WHERE id = ?",
    )
    .bind(workflow_version_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|error| AppError::database(error.to_string()))?;
    if version.is_none() {
        let workflow_id = format!("wf_restored_{}", Uuid::new_v4());
        sqlx::query("INSERT OR IGNORE INTO workflows (id, name, category, mode, current_version_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&workflow_id).bind("已恢复历史工作流").bind("restored").bind("api").bind(workflow_version_id).bind(Utc::now().to_rfc3339()).bind(Utc::now().to_rfc3339())
            .execute(&mut **transaction).await.map_err(|error| AppError::database(error.to_string()))?;
        sqlx::query("INSERT OR IGNORE INTO workflow_versions (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(workflow_version_id).bind(&workflow_id).bind("restored").bind("{}").bind(hash_bytes(b"{}")).bind(Utc::now().to_rfc3339())
            .execute(&mut **transaction).await.map_err(|error| AppError::database(error.to_string()))?;
    }
    let recipe_exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM recipes WHERE id = ?")
        .bind(recipe_id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(|error| AppError::database(error.to_string()))?;
    if recipe_exists == 0 {
        sqlx::query("INSERT INTO recipes (id, workflow_version_id, version, schema_version, recipe_yaml, recipe_sha256, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(recipe_id).bind(workflow_version_id).bind("restored").bind(1_i64).bind("schema_version: 1\ninputs: {}\n").bind(hash_bytes(b"schema_version: 1\ninputs: {}\n")).bind(Utc::now().to_rfc3339())
            .execute(&mut **transaction).await.map_err(|error| AppError::database(error.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        collect_exact_asset_id_references, inspect_archive, remap_snapshot_asset_references,
        restored_name, safe_zip_path, validate_asset_video_prompt_document,
        validate_organization_document, validate_prompt_document, BackupAsset, BackupAssetTag,
        BackupAssetTagLink, BackupAssetVideoPrompt, BackupDocument, BackupProject,
        BackupPromptEntry, BackupPromptVersion, BackupSnapshot, BackupTask, ProjectBackupService,
    };
    use crate::application::ports::ProjectRecord;
    use crate::infrastructure::{database::initialize, filesystem::AppDataDirs};
    use chrono::Utc;
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::collections::{HashMap, HashSet};
    use std::{fs::File, io::Write, path::PathBuf};
    use tempfile::tempdir;
    use zip::{write::FileOptions, CompressionMethod, ZipWriter};

    #[test]
    fn backup_path_validation_rejects_traversal_and_absolute_paths() {
        assert!(safe_zip_path("assets/ast_1/content.png"));
        assert!(!safe_zip_path("../app.db"));
        assert!(!safe_zip_path("C:/app.db"));
        assert!(!safe_zip_path("/app.db"));
        assert!(!safe_zip_path("assets\\..\\app.db"));
    }

    #[test]
    fn restored_project_name_is_capped() {
        assert!(restored_name(&"项目".repeat(80)).chars().count() <= 80);
        assert!(restored_name("原项目").ends_with("（恢复）"));
    }

    #[test]
    fn remaps_single_exact_snapshot_asset_value() {
        let mut value = json!({"reference_image": "ast_original_1"});
        let mapping = HashMap::from([("ast_original_1".to_owned(), "ast_restored_1".to_owned())]);

        remap_snapshot_asset_references(&mut value, &mapping);

        assert_eq!(value, json!({"reference_image": "ast_restored_1"}));
    }

    #[test]
    fn remaps_multiple_assets_without_changing_order_or_duplicates() {
        let mut value = json!({
            "images": ["ast_original_1", "ast_original_2", "ast_original_1"]
        });
        let mapping = HashMap::from([
            ("ast_original_1".to_owned(), "ast_restored_1".to_owned()),
            ("ast_original_2".to_owned(), "ast_restored_2".to_owned()),
        ]);

        remap_snapshot_asset_references(&mut value, &mapping);

        assert_eq!(
            value,
            json!({"images": ["ast_restored_1", "ast_restored_2", "ast_restored_1"]})
        );
    }

    #[test]
    fn remaps_nested_snapshot_asset_values_but_not_object_keys() {
        let mut value = json!({
            "ast_original_1": "keep this key",
            "nested": {"refs": ["ast_original_1"]}
        });
        let mapping = HashMap::from([("ast_original_1".to_owned(), "ast_restored_1".to_owned())]);

        remap_snapshot_asset_references(&mut value, &mapping);

        assert_eq!(value["ast_original_1"], "keep this key");
        assert_eq!(value["nested"]["refs"][0], "ast_restored_1");
    }

    #[test]
    fn does_not_change_text_containing_an_asset_id() {
        let mut value = json!({
            "prompt": "use ast_original_1 in this sentence"
        });
        let mapping = HashMap::from([("ast_original_1".to_owned(), "ast_restored_1".to_owned())]);

        remap_snapshot_asset_references(&mut value, &mapping);

        assert_eq!(value["prompt"], "use ast_original_1 in this sentence");
    }

    #[test]
    fn does_not_change_unknown_asset_like_values() {
        let mut value = json!({"prompt": "ast_unknown"});
        let mapping = HashMap::from([("ast_original_1".to_owned(), "ast_restored_1".to_owned())]);

        remap_snapshot_asset_references(&mut value, &mapping);

        assert_eq!(value, json!({"prompt": "ast_unknown"}));
    }

    #[test]
    fn preserves_other_scalar_snapshot_inputs() {
        let original = json!({
            "prompt": "hello",
            "seed": "123",
            "width": 768,
            "height": 1280,
            "enabled": true,
            "empty": null
        });
        let mut value = original.clone();
        let mapping = HashMap::from([("ast_original_1".to_owned(), "ast_restored_1".to_owned())]);

        remap_snapshot_asset_references(&mut value, &mapping);

        assert_eq!(value, original);
    }

    #[test]
    fn h3_like_snapshot_remaps_reference_image_and_preserves_other_inputs() {
        let mut value = json!({
            "reference_image": {
                "type": "image_asset",
                "assetId": "ast_original_1"
            },
            "duration": 5,
            "fps": "24",
            "frames": 81
        });
        let mapping = HashMap::from([("ast_original_1".to_owned(), "ast_restored_1".to_owned())]);

        remap_snapshot_asset_references(&mut value, &mapping);

        assert_eq!(value["reference_image"]["assetId"], "ast_restored_1");
        assert_eq!(value["duration"], 5);
        assert_eq!(value["fps"], "24");
        assert_eq!(value["frames"], 81);
    }

    #[test]
    fn collects_only_exact_known_asset_id_values() {
        let value = json!({
            "exact": "ast_original_1",
            "text": "use ast_original_1 in a prompt",
            "unknown": "ast_unknown",
            "nested": ["ast_original_1"]
        });
        let known = HashSet::from(["ast_original_1".to_owned()]);

        assert_eq!(
            collect_exact_asset_id_references(&value, &known),
            vec!["ast_original_1", "ast_original_1"]
        );
    }

    fn organization_document(
        tags: Vec<BackupAssetTag>,
        links: Vec<BackupAssetTagLink>,
    ) -> BackupDocument {
        BackupDocument {
            project: BackupProject {
                id: "project-organization".to_owned(),
                name: "组织校验项目".to_owned(),
            },
            description: None,
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
            active_tasks_excluded: 0,
            incomplete_tasks_excluded: 0,
            tasks: Vec::new(),
            task_events: Vec::new(),
            assets: vec![BackupAsset {
                id: "ast_organization".to_owned(),
                asset_type: "image".to_owned(),
                category: "source_image".to_owned(),
                name: "测试素材".to_owned(),
                original_name: "test.png".to_owned(),
                sha256: String::new(),
                mime_type: "image/png".to_owned(),
                width: 1,
                height: 1,
                duration_ms: None,
                file_size: 0,
                source_task_id: None,
                metadata: json!({}),
                created_at: "2026-01-01T00:00:00Z".to_owned(),
                updated_at: "2026-01-01T00:00:00Z".to_owned(),
                content_path: "assets/source_image/image/test.png".to_owned(),
                thumbnail_path: None,
            }],
            mappings: Vec::new(),
            snapshots: Vec::new(),
            presets: Vec::new(),
            prompt_entries: Vec::new(),
            prompt_versions: Vec::new(),
            batches: Vec::new(),
            items: Vec::new(),
            workflow_refs: Vec::new(),
            asset_tags: tags,
            asset_tag_links: links,
            asset_favorites: Vec::new(),
            asset_video_prompts: Vec::new(),
            production_item_reviews: Vec::new(),
            benchmark_experiments: Vec::new(),
            benchmark_candidates: Vec::new(),
            production_runs: Vec::new(),
            production_stages: Vec::new(),
            production_stage_items: Vec::new(),
            production_run_templates: Vec::new(),
            benchmark_runs: Vec::new(),
            benchmark_quality_scores: Vec::new(),
            shots: Vec::new(),
            shot_stage_configs: Vec::new(),
            shot_stage_prompts: Vec::new(),
            shot_reference_assets: Vec::new(),
            shot_generation_links: Vec::new(),
        }
    }

    fn tag(id: &str, name: &str, normalized_name: &str) -> BackupAssetTag {
        BackupAssetTag {
            id: id.to_owned(),
            project_id: "project-organization".to_owned(),
            name: name.to_owned(),
            normalized_name: normalized_name.to_owned(),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
        }
    }

    fn link(asset_id: &str, tag_id: &str) -> BackupAssetTagLink {
        BackupAssetTagLink {
            asset_id: asset_id.to_owned(),
            tag_id: tag_id.to_owned(),
            project_id: "project-organization".to_owned(),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
        }
    }

    #[test]
    fn backup_blocks_invalid_tag_names_and_normalized_mismatches() {
        for (name, normalized_name) in vec![
            (String::new(), String::new()),
            ("bad\nname".to_owned(), "bad\nname".to_owned()),
            ("a".repeat(33), "a".repeat(33)),
            ("  padded".to_owned(), "  padded".to_owned()),
            ("Name".to_owned(), "wrong".to_owned()),
        ] {
            let error = validate_organization_document(&organization_document(
                vec![tag("tag_1", &name, &normalized_name)],
                Vec::new(),
            ))
            .expect_err("malformed tag must be blocked");
            assert_eq!(error.code(), "BACKUP_INVALID");
        }
    }

    #[test]
    fn backup_blocks_duplicate_normalized_names() {
        let error = validate_organization_document(&organization_document(
            vec![tag("tag_1", "Name", "name"), tag("tag_2", "NAME", "name")],
            Vec::new(),
        ))
        .expect_err("duplicate canonical names must be blocked");
        assert_eq!(error.code(), "BACKUP_INVALID");
    }

    #[test]
    fn backup_blocks_more_than_one_hundred_project_tags() {
        let tags = (0..101)
            .map(|index| {
                let name = format!("tag{index}");
                tag(&format!("tag_{index}"), &name, &name)
            })
            .collect();
        assert!(validate_organization_document(&organization_document(tags, Vec::new())).is_err());
    }

    #[test]
    fn backup_blocks_more_than_twenty_tags_on_one_asset() {
        let tags = (0..21)
            .map(|index| {
                let name = format!("tag{index}");
                tag(&format!("tag_{index}"), &name, &name)
            })
            .collect::<Vec<_>>();
        let links = (0..21)
            .map(|index| link("ast_organization", &format!("tag_{index}")))
            .collect();
        assert!(validate_organization_document(&organization_document(tags, links)).is_err());
    }

    #[test]
    fn backup_accepts_one_hundred_project_tags_and_twenty_tags_on_one_asset() {
        let tags = (0..100)
            .map(|index| {
                let name = format!("tag{index}");
                tag(&format!("tag_{index}"), &name, &name)
            })
            .collect::<Vec<_>>();
        let links = (0..20)
            .map(|index| link("ast_organization", &format!("tag_{index}")))
            .collect();
        validate_organization_document(&organization_document(tags, links))
            .expect("organization limits are inclusive");
    }

    fn prompt_document() -> BackupDocument {
        let mut document = organization_document(Vec::new(), Vec::new());
        document.prompt_entries = vec![BackupPromptEntry {
            id: "prm_1".to_owned(),
            project_id: document.project.id.clone(),
            kind: "prompt".to_owned(),
            name: "中文起点".to_owned(),
            normalized_name: "中文起点".to_owned(),
            tags: vec!["人物".to_owned(), "Kera2".to_owned()],
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
        }];
        document.prompt_versions = vec![BackupPromptVersion {
            id: "prv_1".to_owned(),
            project_id: document.project.id.clone(),
            prompt_id: "prm_1".to_owned(),
            version: 1,
            text: "人物，柔光".to_owned(),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
        }];
        document
    }

    #[test]
    fn backup_prompt_validation_rejects_duplicates_ownership_and_invalid_text() {
        let valid = prompt_document();
        validate_prompt_document(&valid).expect("valid prompt backup should pass");

        let mut duplicate_entry = valid.clone();
        duplicate_entry
            .prompt_entries
            .push(duplicate_entry.prompt_entries[0].clone());
        assert!(validate_prompt_document(&duplicate_entry).is_err());

        let mut wrong_project = valid.clone();
        wrong_project.prompt_entries[0].project_id = "other-project".to_owned();
        assert!(validate_prompt_document(&wrong_project).is_err());

        let mut duplicate_version = valid.clone();
        duplicate_version
            .prompt_versions
            .push(duplicate_version.prompt_versions[0].clone());
        assert!(validate_prompt_document(&duplicate_version).is_err());

        let mut invalid_text = valid;
        invalid_text.prompt_versions[0].text = format!(" x{}", "x".repeat(64 * 1024));
        assert!(validate_prompt_document(&invalid_text).is_err());
    }

    #[test]
    fn backup_asset_video_prompt_validation_rejects_malicious_entries() {
        let valid_prompt = BackupAssetVideoPrompt {
            asset_id: "ast_organization".to_owned(),
            project_id: "project-organization".to_owned(),
            prompt_text: "camera moves slowly".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
        };
        let mut valid = organization_document(Vec::new(), Vec::new());
        valid.asset_video_prompts = vec![valid_prompt.clone()];
        validate_asset_video_prompt_document(&valid)
            .expect("an image-owned prompt should pass validation");

        let mut video_asset = valid.clone();
        video_asset.assets[0].asset_type = "video".to_owned();
        assert!(validate_asset_video_prompt_document(&video_asset).is_err());

        let mut unknown_asset = valid.clone();
        unknown_asset.asset_video_prompts[0].asset_id = "asset-unknown".to_owned();
        assert!(validate_asset_video_prompt_document(&unknown_asset).is_err());

        let mut wrong_project = valid.clone();
        wrong_project.asset_video_prompts[0].project_id = "project-other".to_owned();
        assert!(validate_asset_video_prompt_document(&wrong_project).is_err());

        let mut duplicate = valid.clone();
        duplicate
            .asset_video_prompts
            .push(duplicate.asset_video_prompts[0].clone());
        assert!(validate_asset_video_prompt_document(&duplicate).is_err());

        for invalid_text in [String::new(), " \n\t ".to_owned()] {
            let mut invalid = valid.clone();
            invalid.asset_video_prompts[0].prompt_text = invalid_text;
            assert!(validate_asset_video_prompt_document(&invalid).is_err());
        }

        let mut too_large = valid;
        too_large.asset_video_prompts[0].prompt_text = "x".repeat(64 * 1024 + 1);
        assert!(validate_asset_video_prompt_document(&too_large).is_err());
    }

    #[tokio::test]
    async fn backup_round_trip_creates_new_project_and_keeps_asset_bytes() {
        let directory = tempdir().unwrap();
        let data_dirs = AppDataDirs::initialize(directory.path().join("AIStudioData")).unwrap();
        let pool = initialize(&data_dirs.database).await.unwrap();
        crate::infrastructure::database::repositories::test_support::seed_task_dependencies(&pool)
            .await;
        let project_root = data_dirs.projects.join("project-backup");
        std::fs::create_dir_all(&project_root).unwrap();
        sqlx::query("INSERT INTO projects (id, name, description, root_path, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind("project-backup")
            .bind("备份项目")
            .bind("测试")
            .bind(project_root.to_string_lossy().to_string())
            .bind("2026-01-01T00:00:00Z")
            .bind("2026-01-01T00:00:00Z")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO prompt_entries (id, project_id, kind, name, normalized_name, tags_json, created_at, updated_at) VALUES (?, ?, 'prompt', ?, ?, ?, ?, ?)")
            .bind("prm_backup")
            .bind("project-backup")
            .bind("中文起点")
            .bind("中文起点")
            .bind(r#"["人物","Kera2"]"#)
            .bind("2026-01-01T00:00:00Z")
            .bind("2026-01-01T00:00:00Z")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO prompt_versions (id, prompt_id, version, text, created_at) VALUES (?, ?, ?, ?, ?)")
            .bind("prv_backup_1")
            .bind("prm_backup")
            .bind(1_i64)
            .bind("人物，柔光")
            .bind("2026-01-01T00:00:00Z")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO prompt_versions (id, prompt_id, version, text, created_at) VALUES (?, ?, ?, ?, ?)")
            .bind("prv_backup_2")
            .bind("prm_backup")
            .bind(2_i64)
            .bind("人物，硬光")
            .bind("2026-01-01T00:00:30Z")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO tasks (id, project_id, workflow_id, workflow_version_id, recipe_id, status, progress_mode, created_at, finished_at) VALUES (?, ?, ?, ?, ?, 'SUCCEEDED', 'indeterminate', ?, ?)")
            .bind("tsk_backup")
            .bind("project-backup")
            .bind("workflow-1")
            .bind("workflow-version-1")
            .bind("recipe-1")
            .bind("2026-01-01T00:00:00Z")
            .bind("2026-01-01T00:01:00Z")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE tasks SET
                generation_execution_id = 'gen_backup', compiled_workflow_sha256 = 'compiled-backup',
                runtime_profile = 'H3_QUALITY', concurrency_class = 'GPU_HEAVY_SERIAL',
                prepare_started_at = '2026-01-01T00:00:01Z', prepared_at = '2026-01-01T00:00:02Z',
                submitted_at = '2026-01-01T00:00:03Z', execution_started_at = '2026-01-01T00:00:04Z',
                execution_finished_at = '2026-01-01T00:00:05Z', collection_finished_at = '2026-01-01T00:00:06Z'
             WHERE id = 'tsk_backup'",
        )
        .execute(&pool)
        .await
        .unwrap();
        let bytes = b"backup-image-bytes";
        let digest = Sha256::digest(bytes);
        let sha = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let asset_path = project_root.join("asset.png");
        std::fs::write(&asset_path, bytes).unwrap();
        sqlx::query("INSERT INTO assets (id, project_id, type, category, name, original_name, storage_path, sha256, mime_type, width, height, file_size, metadata_json, created_at, updated_at, source_task_id) VALUES (?, ?, 'image', 'generated_image', ?, ?, ?, ?, 'image/png', 1, 1, ?, '{}', ?, ?, ?)")
            .bind("ast_backup")
            .bind("project-backup")
            .bind("图像")
            .bind("图像.png")
            .bind(asset_path.to_string_lossy().to_string())
            .bind(sha)
            .bind(bytes.len() as i64)
            .bind("2026-01-01T00:01:00Z")
            .bind("2026-01-01T00:01:00Z")
            .bind("tsk_backup")
            .execute(&pool)
            .await
            .unwrap();
        let source_bytes = b"backup-source-image-bytes";
        let source_sha = Sha256::digest(source_bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let source_path = project_root.join("source.png");
        std::fs::write(&source_path, source_bytes).unwrap();
        sqlx::query("INSERT INTO assets (id, project_id, type, category, name, original_name, storage_path, sha256, mime_type, width, height, file_size, metadata_json, created_at, updated_at) VALUES (?, ?, 'image', 'source_image', ?, ?, ?, ?, 'image/png', 1, 1, ?, '{}', ?, ?)")
            .bind("ast_source_backup")
            .bind("project-backup")
            .bind("源图")
            .bind("源图.png")
            .bind(source_path.to_string_lossy().to_string())
            .bind(source_sha)
            .bind(source_bytes.len() as i64)
            .bind("2026-01-01T00:01:30Z")
            .bind("2026-01-01T00:01:30Z")
            .execute(&pool)
            .await
            .unwrap();
        for (asset_id, original_name, bytes) in [
            ("ast_ref_b", "ref-b.png", b"backup-reference-b".as_slice()),
            ("ast_ref_c", "ref-c.png", b"backup-reference-c".as_slice()),
        ] {
            let digest = Sha256::digest(bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            let path = project_root.join(original_name);
            std::fs::write(&path, bytes).unwrap();
            sqlx::query("INSERT INTO assets (id, project_id, type, category, name, original_name, storage_path, sha256, mime_type, width, height, file_size, metadata_json, created_at, updated_at) VALUES (?, ?, 'image', 'source_image', ?, ?, ?, ?, 'image/png', 1, 1, ?, '{}', ?, ?)")
                .bind(asset_id)
                .bind("project-backup")
                .bind(asset_id)
                .bind(original_name)
                .bind(path.to_string_lossy().to_string())
                .bind(digest)
                .bind(bytes.len() as i64)
                .bind("2026-01-01T00:01:45Z")
                .bind("2026-01-01T00:01:45Z")
                .execute(&pool)
                .await
                .unwrap();
        }
        sqlx::query(
            "INSERT INTO asset_video_prompts (asset_id, project_id, prompt_text, updated_at)
             VALUES ('ast_backup', 'project-backup', 'generated image camera orbit', '2026-01-01T00:02:00Z'),
                    ('ast_source_backup', 'project-backup', 'source image camera pan', '2026-01-01T00:02:01Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO generation_snapshots (id, task_id, workflow_json, recipe_yaml, user_inputs_json, resolved_inputs_json, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("snp_backup")
        .bind("tsk_backup")
        .bind("{}")
        .bind("schema_version: 1\ninputs: {}\n")
        .bind(r#"{"reference_images":{"type":"image_assets","assetIds":["ast_ref_b","ast_backup","ast_ref_c"]}}"#)
        .bind(r#"{"reference_images":{"type":"image_assets","assetIds":["ast_ref_b","ast_backup","ast_ref_c"]}}"#)
        .bind("2026-01-01T00:01:00Z")
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO asset_tags (id, project_id, name, normalized_name, created_at, updated_at) VALUES ('tag_people', 'project-backup', '人物', '人物', '2026-01-01T00:01:00Z', '2026-01-01T00:01:00Z'), ('tag_reference', 'project-backup', '参考图', '参考图', '2026-01-01T00:01:00Z', '2026-01-01T00:01:00Z')").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO asset_tag_links (asset_id, tag_id, project_id, created_at) VALUES ('ast_backup', 'tag_people', 'project-backup', '2026-01-01T00:01:00Z'), ('ast_backup', 'tag_reference', 'project-backup', '2026-01-01T00:01:00Z')").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO asset_favorites (asset_id, project_id, created_at) VALUES ('ast_backup', 'project-backup', '2026-01-01T00:01:00Z')").execute(&pool).await.unwrap();
        let video_bytes = b"backup-video-bytes";
        let video_sha = Sha256::digest(video_bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let video_path = project_root.join("video.mp4");
        std::fs::write(&video_path, video_bytes).unwrap();
        sqlx::query("INSERT INTO assets (id, project_id, type, category, name, original_name, storage_path, sha256, mime_type, width, height, duration_ms, file_size, metadata_json, created_at, updated_at) VALUES ('ast_video', 'project-backup', 'video', 'source_video', '视频', '视频.mp4', ?, ?, 'video/mp4', 1, 1, 1000, ?, '{}', '2026-01-01T00:02:00Z', '2026-01-01T00:02:00Z')")
            .bind(video_path.to_string_lossy().to_string()).bind(video_sha).bind(video_bytes.len() as i64).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO asset_tags (id, project_id, name, normalized_name, created_at, updated_at) VALUES ('tag_finish', 'project-backup', '成片', '成片', '2026-01-01T00:02:00Z', '2026-01-01T00:02:00Z')").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO asset_tag_links (asset_id, tag_id, project_id, created_at) VALUES ('ast_video', 'tag_finish', 'project-backup', '2026-01-01T00:02:00Z')").execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO shots (id, project_id, ordinal, name, prompt_text, prompt_entry_id, prompt_version_id, selected_image_asset_id, selected_video_asset_id, created_at, updated_at)
             VALUES ('sht_backup', 'project-backup', 0, '开场镜头', '人物，柔光', 'prm_backup', 'prv_backup_2', 'ast_backup', 'ast_video', '2026-01-01T00:03:00Z', '2026-01-01T00:03:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shot_stage_configs (shot_id, stage, workflow_version_id, recipe_id, scalar_values_json, updated_at)
             VALUES ('sht_backup', 'image', 'workflow-version-1', 'recipe-1', '{\"steps\":{\"type\":\"integer\",\"value\":4}}', '2026-01-01T00:03:00Z'),
                    ('sht_backup', 'video', 'workflow-version-1', 'recipe-1', '{\"seed\":{\"type\":\"seed_random\"}}', '2026-01-01T00:03:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shot_stage_prompts
             (shot_id, stage, prompt_text, prompt_entry_id, prompt_version_id, updated_at)
             VALUES ('sht_backup', 'image', '图片阶段快照', 'prm_backup', 'prv_backup_1', '2026-01-01T00:03:00Z'),
                    ('sht_backup', 'video', '视频阶段快照', 'prm_backup', 'prv_backup_2', '2026-01-01T00:03:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO shot_reference_assets (shot_id, stage, asset_id, ordinal) VALUES ('sht_backup', 'image', 'ast_backup', 0), ('sht_backup', 'video', 'ast_backup', 0)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO production_batches (id, project_id, name, status, continue_on_failure, archived_at, created_at, updated_at) VALUES ('pbt_backup', 'project-backup', '关键帧批次', 'COMPLETED', 0, NULL, '2026-01-01T00:03:00Z', '2026-01-01T00:03:00Z')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO production_batch_items (id, batch_id, ordinal, workflow_version_id, recipe_id, values_json, status, task_id, retry_of_item_id, error_code, error_message, created_at, updated_at) VALUES ('pbi_backup', 'pbt_backup', 0, 'workflow-version-1', 'recipe-1', '{}', 'SUCCEEDED', 'tsk_backup', NULL, NULL, NULL, '2026-01-01T00:03:00Z', '2026-01-01T00:03:00Z')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO production_batches (id, project_id, name, status, continue_on_failure, archived_at, created_at, updated_at) VALUES ('pbt_h3_backup', 'project-backup', 'H3 成片批次', 'COMPLETED', 1, NULL, '2026-01-01T00:03:10Z', '2026-01-01T00:03:10Z')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO production_batch_items (id, batch_id, ordinal, workflow_version_id, recipe_id, values_json, status, task_id, retry_of_item_id, error_code, error_message, created_at, updated_at) VALUES ('pbi_h3_backup', 'pbt_h3_backup', 0, 'workflow-version-1', 'recipe-1', '{\"reference_images\":{\"type\":\"image_assets\",\"assetIds\":[\"ast_ref_b\",\"ast_backup\",\"ast_ref_c\"]}}', 'SUCCEEDED', 'tsk_backup', NULL, NULL, NULL, '2026-01-01T00:03:10Z', '2026-01-01T00:03:10Z')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO production_run_templates (id, project_id, name, krea2_workflow_version_id, krea2_recipe_id, krea2_preset_id, default_image_count, h3_workflow_version_id, h3_recipe_id, h3_profile, default_duration_seconds, default_width, default_height, created_at, updated_at) VALUES ('prt_backup', 'project-backup', '默认生产模板', 'workflow-version-1', 'recipe-1', NULL, 2, 'workflow-version-1', 'recipe-1', 'H3_FAST', 5, 864, 480, '2026-01-01T00:03:00Z', '2026-01-01T00:03:00Z')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO production_runs (id, project_id, name, status, current_stage_ordinal, template_id, created_at, updated_at, started_at, finished_at) VALUES ('prun_backup', 'project-backup', '批量成片 Run', 'SUCCEEDED', 2, 'prt_backup', '2026-01-01T00:03:00Z', '2026-01-01T00:04:00Z', '2026-01-01T00:03:00Z', '2026-01-01T00:04:00Z')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO production_stages (id, run_id, ordinal, stage_type, status, workflow_version_id, recipe_id, production_batch_id, frozen_config_json, prompt, created_at, updated_at, started_at, finished_at) VALUES ('prst_backup_image', 'prun_backup', 0, 'KREA2_IMAGE_GENERATION', 'SUCCEEDED', 'workflow-version-1', 'recipe-1', 'pbt_backup', '{\"imageCount\":1,\"values\":{\"prompt\":\"image\"}}', NULL, '2026-01-01T00:03:00Z', '2026-01-01T00:03:30Z', '2026-01-01T00:03:00Z', '2026-01-01T00:03:30Z'), ('prst_backup_selection', 'prun_backup', 1, 'ASSET_SELECTION', 'SUCCEEDED', NULL, NULL, NULL, '{\"selectionMode\":\"MANUAL\"}', NULL, '2026-01-01T00:03:30Z', '2026-01-01T00:03:40Z', '2026-01-01T00:03:30Z', '2026-01-01T00:03:40Z'), ('prst_backup_h3', 'prun_backup', 2, 'H3_VIDEO_GENERATION', 'SUCCEEDED', 'workflow-version-1', 'recipe-1', 'pbt_h3_backup', '{\"values\":{\"prompt\":\"video\"}}', 'video prompt', '2026-01-01T00:03:40Z', '2026-01-01T00:04:00Z', '2026-01-01T00:03:40Z', '2026-01-01T00:04:00Z')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO production_stage_items (id, stage_id, ordinal, status, production_batch_item_id, task_id, asset_id, source_asset_id, reference_index, attempt, submission_idempotency_key, parent_stage_item_id, frozen_values_json, error_code, error_message, created_at, updated_at) VALUES
            ('prsi_backup_image', 'prst_backup_image', 0, 'SUCCEEDED', 'pbi_backup', 'tsk_backup', 'ast_backup', NULL, NULL, 1, 'production-stage-item:prsi_backup_image:attempt:1', NULL, '{\"assetId\":\"ast_backup\"}', NULL, NULL, '2026-01-01T00:03:00Z', '2026-01-01T00:03:30Z'),
            ('prsi_backup_selection_b', 'prst_backup_selection', 0, 'SUCCEEDED', NULL, NULL, 'ast_ref_b', 'ast_ref_b', 0, 1, 'production-stage-item:prsi_backup_selection_b:attempt:1', NULL, '{\"assetId\":\"ast_ref_b\",\"referenceIndex\":0}', NULL, NULL, '2026-01-01T00:03:30Z', '2026-01-01T00:03:40Z'),
            ('prsi_backup_selection_a', 'prst_backup_selection', 1, 'SUCCEEDED', NULL, NULL, 'ast_backup', 'ast_backup', 1, 1, 'production-stage-item:prsi_backup_selection_a:attempt:1', NULL, '{\"assetId\":\"ast_backup\",\"referenceIndex\":1}', NULL, NULL, '2026-01-01T00:03:30Z', '2026-01-01T00:03:40Z'),
            ('prsi_backup_selection_c', 'prst_backup_selection', 2, 'SUCCEEDED', NULL, NULL, 'ast_ref_c', 'ast_ref_c', 2, 1, 'production-stage-item:prsi_backup_selection_c:attempt:1', NULL, '{\"assetId\":\"ast_ref_c\",\"referenceIndex\":2}', NULL, NULL, '2026-01-01T00:03:30Z', '2026-01-01T00:03:40Z'),
            ('prsi_backup_h3_b', 'prst_backup_h3', 0, 'SUCCEEDED', 'pbi_h3_backup', 'tsk_backup', 'ast_video', 'ast_ref_b', 0, 1, 'production-stage-item:prsi_backup_h3_b:attempt:1', NULL, '{\"reference_image\":{\"type\":\"image_asset\",\"assetId\":\"ast_ref_b\"}}', NULL, NULL, '2026-01-01T00:03:40Z', '2026-01-01T00:04:00Z'),
            ('prsi_backup_h3_a', 'prst_backup_h3', 1, 'SUCCEEDED', 'pbi_h3_backup', 'tsk_backup', 'ast_video', 'ast_backup', 1, 1, 'production-stage-item:prsi_backup_h3_a:attempt:1', NULL, '{\"reference_image\":{\"type\":\"image_asset\",\"assetId\":\"ast_backup\"}}', NULL, NULL, '2026-01-01T00:03:40Z', '2026-01-01T00:04:00Z'),
            ('prsi_backup_h3_c', 'prst_backup_h3', 2, 'SUCCEEDED', 'pbi_h3_backup', 'tsk_backup', 'ast_video', 'ast_ref_c', 2, 1, 'production-stage-item:prsi_backup_h3_c:attempt:1', NULL, '{\"reference_image\":{\"type\":\"image_asset\",\"assetId\":\"ast_ref_c\"}}', NULL, NULL, '2026-01-01T00:03:40Z', '2026-01-01T00:04:00Z')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO benchmark_experiments
             (id, project_id, name, media_type, status, base_values_json, asset_ids_json,
              winner_candidate_id, production_batch_id, created_at, updated_at)
             VALUES ('bmk_backup', 'project-backup', 'H3 参数对比', 'VIDEO', 'COMPLETED',
                     '{\"prompt\":\"base\"}', '[\"ast_backup\"]', 'bmc_backup_2',
                     'pbt_backup', '2026-01-01T00:03:00Z', '2026-01-01T00:03:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO benchmark_candidates
             (id, experiment_id, position, workflow_version_id, recipe_id, preset_id,
              preset_name, label, values_json, asset_ids_json, production_batch_item_id,
              task_id, created_at)
             VALUES
             ('bmc_backup_1', 'bmk_backup', 0, 'workflow-version-1', 'recipe-1', NULL,
              NULL, '基准 A', '{\"reference_image\":{\"type\":\"image_asset\",\"assetId\":\"ast_backup\"}}',
              '[\"ast_backup\"]', 'pbi_backup', 'tsk_backup', '2026-01-01T00:03:00Z'),
             ('bmc_backup_2', 'bmk_backup', 1, 'workflow-version-1', 'recipe-1', NULL,
              NULL, '基准 B', '{\"reference_image\":{\"type\":\"image_asset\",\"assetId\":\"ast_backup\"}}',
              '[\"ast_backup\"]', 'pbi_backup', 'tsk_backup', '2026-01-01T00:03:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO production_item_reviews
             (id, project_id, production_batch_id, production_batch_item_id, task_id,
              result_asset_id, review_status, review_note, version, lineage_key,
              parent_batch_id, parent_item_id, created_at, updated_at)
             VALUES ('pri_backup', 'project-backup', 'pbt_backup', 'pbi_backup', 'tsk_backup',
                     'ast_video', 'APPROVED', '镜头稳定，保留。', 1, 'pbi_backup',
                     NULL, NULL, '2026-01-01T00:03:00Z', '2026-01-01T00:03:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO benchmark_runs (id, experiment_id, candidate_id, run_number, production_batch_item_id, task_id, snapshot_id, output_asset_id, generation_execution_id, compiled_workflow_sha256, runtime_profile, concurrency_class, queue_wait_ms, prepare_ms, submit_ms, comfy_execution_ms, collect_ms, total_ms, status, error_code, output_file_size, created_at, updated_at) VALUES ('bmr_backup', 'bmk_backup', 'bmc_backup_2', 1, 'pbi_backup', 'tsk_backup', 'snp_backup', 'ast_video', 'gen_backup', 'compiled-backup', 'H3_FAST', 'GPU_HEAVY_SERIAL', 10, 20, 30, 40, 5, 105, 'SUCCEEDED', NULL, 17, '2026-01-01T00:04:00Z', '2026-01-01T00:04:00Z')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO benchmark_quality_scores (id, candidate_id, prompt_adherence, visual_quality, motion_quality, reference_consistency, overall, note, created_at, updated_at) VALUES ('bmq_backup', 'bmc_backup_2', 5, 4, 4, 5, 4, '稳定且符合预期', '2026-01-01T00:04:00Z', '2026-01-01T00:04:00Z')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO shot_generation_links (id, shot_id, stage, task_id, production_batch_item_id, created_at) VALUES ('sgl_backup', 'sht_backup', 'image', 'tsk_backup', 'pbi_backup', '2026-01-01T00:03:00Z')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO project_templates (id, name, normalized_name, description, workflow_version_id, recipe_id, values_json, created_at, updated_at) VALUES ('ptm_global', '全局模板', '全局模板', NULL, 'workflow-version-1', 'recipe-1', '{}', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')").execute(&pool).await.unwrap();

        let service = ProjectBackupService::new(
            pool.clone(),
            data_dirs.projects.clone(),
            data_dirs.cache.clone(),
        );
        let archive_path = directory.path().join("backup.zip");
        let exported = service
            .export("project-backup", archive_path.clone())
            .await
            .unwrap();
        assert!(exported.entries >= 5);
        let (manifest, _document, names) = inspect_archive(&archive_path).unwrap();
        assert_eq!(manifest.format, "ai-studio-project-backup");
        assert_eq!(manifest.version, 10);
        assert_eq!(exported.entries, names.len());
        assert!(!names.contains("app.db"));
        assert!(!names.contains("workflow_api.json"));
        assert!(!names.contains("recipe.yaml"));
        let preview = service.inspect(archive_path).await.unwrap();
        assert_eq!(preview.image_count, 4);
        assert_eq!(preview.shots, 1);
        let restored = service.restore(&preview.inspection_id).await.unwrap();
        assert_ne!(restored.id, "project-backup");
        let restored_count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM assets WHERE project_id = ?")
                .bind(&restored.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(restored_count, 5);
        let production_counts: (i64, i64, i64, i64, i64, i64) = sqlx::query_as(
            "SELECT
                (SELECT COUNT(*) FROM production_runs WHERE project_id = ?),
                (SELECT COUNT(*) FROM production_stages WHERE run_id IN (SELECT id FROM production_runs WHERE project_id = ?)),
                (SELECT COUNT(*) FROM production_stage_items WHERE stage_id IN (SELECT id FROM production_stages WHERE run_id IN (SELECT id FROM production_runs WHERE project_id = ?))),
                (SELECT COUNT(*) FROM production_run_templates WHERE project_id = ?),
                (SELECT COUNT(*) FROM benchmark_runs WHERE experiment_id IN (SELECT id FROM benchmark_experiments WHERE project_id = ?)),
                (SELECT COUNT(*) FROM benchmark_quality_scores WHERE candidate_id IN (SELECT id FROM benchmark_candidates WHERE experiment_id IN (SELECT id FROM benchmark_experiments WHERE project_id = ?)))",
        )
        .bind(&restored.id)
        .bind(&restored.id)
        .bind(&restored.id)
        .bind(&restored.id)
        .bind(&restored.id)
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(production_counts, (1, 3, 7, 1, 1, 1));
        let restored_path = sqlx::query_scalar::<_, String>(
            "SELECT storage_path FROM assets WHERE project_id = ? AND type = 'image' AND original_name = '图像.png'",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(std::fs::read(restored_path).unwrap(), bytes);
        let restored_task_id: String =
            sqlx::query_scalar("SELECT id FROM tasks WHERE project_id = ?")
                .bind(&restored.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let restored_telemetry: (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ) = sqlx::query_as(
            "SELECT generation_execution_id, compiled_workflow_sha256, runtime_profile,
                        collection_finished_at FROM tasks WHERE id = ?",
        )
        .bind(&restored_task_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            restored_telemetry,
            (
                Some("gen_backup".to_owned()),
                Some("compiled-backup".to_owned()),
                Some("H3_QUALITY".to_owned()),
                Some("2026-01-01T00:00:06Z".to_owned())
            )
        );
        let restored_asset_id: String =
            sqlx::query_scalar("SELECT id FROM assets WHERE project_id = ? AND type = 'image' AND original_name = '图像.png'")
                .bind(&restored.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let restored_ref_b_id: String = sqlx::query_scalar(
            "SELECT id FROM assets WHERE project_id = ? AND original_name = 'ref-b.png'",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let restored_ref_c_id: String = sqlx::query_scalar(
            "SELECT id FROM assets WHERE project_id = ? AND original_name = 'ref-c.png'",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let expected_reference_ids = vec![
            restored_ref_b_id.clone(),
            restored_asset_id.clone(),
            restored_ref_c_id.clone(),
        ];
        for stage_ordinal in [1_i64, 2_i64] {
            let restored_references: Vec<(Option<i64>, Option<String>, String)> =
                sqlx::query_as(
                    "SELECT reference_index, source_asset_id, frozen_values_json
                     FROM production_stage_items
                     WHERE stage_id = (SELECT id FROM production_stages WHERE run_id = (SELECT id FROM production_runs WHERE project_id = ?) AND ordinal = ?)
                     ORDER BY reference_index",
                )
                .bind(&restored.id)
                .bind(stage_ordinal)
                .fetch_all(&pool)
                .await
                .unwrap();
            assert_eq!(
                restored_references
                    .iter()
                    .map(|(reference_index, _, _)| *reference_index)
                    .collect::<Vec<_>>(),
                vec![Some(0), Some(1), Some(2)]
            );
            assert_eq!(
                restored_references
                    .iter()
                    .map(|(_, source_asset_id, _)| source_asset_id.clone().unwrap())
                    .collect::<Vec<_>>(),
                expected_reference_ids
            );
            for ((reference_index, source_asset_id, frozen_values), expected_asset_id) in
                restored_references.iter().zip(&expected_reference_ids)
            {
                let frozen_values: serde_json::Value = serde_json::from_str(frozen_values).unwrap();
                let frozen_asset_id = if stage_ordinal == 1 {
                    frozen_values["assetId"].as_str().unwrap()
                } else {
                    frozen_values["reference_image"]["assetId"]
                        .as_str()
                        .unwrap()
                };
                assert_eq!(frozen_asset_id, expected_asset_id);
                if stage_ordinal == 1 {
                    assert_eq!(
                        frozen_values["referenceIndex"],
                        json!(reference_index.unwrap())
                    );
                }
                assert_eq!(source_asset_id.as_deref(), Some(expected_asset_id.as_str()));
            }
        }
        let restored_project_id: String =
            sqlx::query_scalar("SELECT project_id FROM assets WHERE id = ?")
                .bind(&restored_asset_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(restored_project_id, restored.id);
        let snapshot: (String, String) = sqlx::query_as(
            "SELECT user_inputs_json, resolved_inputs_json FROM generation_snapshots WHERE task_id = ?",
        )
        .bind(restored_task_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let user_inputs: serde_json::Value = serde_json::from_str(&snapshot.0).unwrap();
        let resolved_inputs: serde_json::Value = serde_json::from_str(&snapshot.1).unwrap();
        assert_eq!(
            user_inputs["reference_images"]["assetIds"],
            json!(expected_reference_ids)
        );
        assert_eq!(
            resolved_inputs["reference_images"]["assetIds"],
            json!(expected_reference_ids)
        );
        assert_ne!(restored_asset_id, "ast_backup");
        let restored_prompts: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT asset_id, project_id, prompt_text
             FROM asset_video_prompts WHERE project_id = ? ORDER BY prompt_text",
        )
        .bind(&restored.id)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(restored_prompts.len(), 2);
        assert_eq!(
            restored_prompts
                .iter()
                .map(|(_, _, prompt)| prompt.as_str())
                .collect::<Vec<_>>(),
            vec!["generated image camera orbit", "source image camera pan"]
        );
        assert!(restored_prompts.iter().all(|(asset_id, project_id, _)| {
            project_id == &restored.id
                && asset_id != "ast_backup"
                && asset_id != "ast_source_backup"
        }));
        let restored_review: (String, String, String, String, String, String) = sqlx::query_as(
            "SELECT project_id, production_batch_id, production_batch_item_id, task_id,
                    result_asset_id, review_status
             FROM production_item_reviews WHERE project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(restored_review.0, restored.id);
        assert_ne!(restored_review.1, "pbt_backup");
        assert_ne!(restored_review.2, "pbi_backup");
        assert_ne!(restored_review.3, "tsk_backup");
        assert_ne!(restored_review.4, "ast_video");
        assert_eq!(restored_review.5, "APPROVED");
        let restored_review_note: String = sqlx::query_scalar(
            "SELECT review_note FROM production_item_reviews WHERE project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(restored_review_note, "镜头稳定，保留。");
        let restored_benchmark: (String, String, Option<String>, String) = sqlx::query_as(
            "SELECT id, production_batch_id, winner_candidate_id, asset_ids_json
             FROM benchmark_experiments WHERE project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_ne!(restored_benchmark.0, "bmk_backup");
        assert_ne!(restored_benchmark.1, "pbt_backup");
        assert_ne!(restored_benchmark.2.as_deref(), Some("bmc_backup_2"));
        assert_eq!(
            serde_json::from_str::<Vec<String>>(&restored_benchmark.3).unwrap(),
            vec![restored_asset_id.clone()]
        );
        let restored_candidates: Vec<(
            String,
            i64,
            String,
            String,
            Option<String>,
            Option<String>,
        )> = sqlx::query_as(
            "SELECT id, position, asset_ids_json, values_json,
                        production_batch_item_id, task_id
                 FROM benchmark_candidates WHERE experiment_id = ? ORDER BY position",
        )
        .bind(&restored_benchmark.0)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(restored_candidates.len(), 2);
        assert_eq!(
            restored_candidates
                .iter()
                .map(|(_, position, ..)| *position)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        for (id, _, asset_ids_json, values_json, item_id, task_id) in &restored_candidates {
            assert!(!["bmc_backup_1", "bmc_backup_2"].contains(&id.as_str()));
            assert_eq!(
                serde_json::from_str::<Vec<String>>(asset_ids_json).unwrap(),
                vec![restored_asset_id.clone()]
            );
            assert!(values_json.contains(&restored_asset_id));
            assert_ne!(item_id.as_deref(), Some("pbi_backup"));
            assert_ne!(task_id.as_deref(), Some("tsk_backup"));
        }
        let restored_tags: Vec<(String, String)> =
            sqlx::query_as("SELECT id, name FROM asset_tags WHERE project_id = ? ORDER BY name")
                .bind(&restored.id)
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(restored_tags.len(), 3);
        assert!(restored_tags
            .iter()
            .all(|(id, _)| !["tag_people", "tag_reference", "tag_finish"].contains(&id.as_str())));
        let restored_links: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM asset_tag_links WHERE project_id = ? AND asset_id = ?",
        )
        .bind(&restored.id)
        .bind(&restored_asset_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let restored_favorite: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM asset_favorites WHERE project_id = ? AND asset_id = ?",
        )
        .bind(&restored.id)
        .bind(&restored_asset_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(restored_links, 2);
        assert_eq!(restored_favorite, 1);
        let video_tag: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM asset_tag_links l JOIN assets a ON a.id = l.asset_id JOIN asset_tags t ON t.id = l.tag_id WHERE a.project_id = ? AND a.type = 'video' AND t.name = '成片'")
            .bind(&restored.id).fetch_one(&pool).await.unwrap();
        assert_eq!(video_tag, 1);
        let template_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM project_templates")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            template_count, 1,
            "restoring a project must not duplicate global project templates"
        );
        let restored_prompt_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM prompt_entries WHERE project_id = ?")
                .bind(&restored.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let restored_version_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM prompt_versions v JOIN prompt_entries e ON e.id = v.prompt_id WHERE e.project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!((restored_prompt_count, restored_version_count), (1, 2));
        let restored_shot: (String, String, String, String) = sqlx::query_as(
            "SELECT id, prompt_entry_id, prompt_version_id, selected_image_asset_id FROM shots WHERE project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_ne!(restored_shot.0, "sht_backup");
        assert_ne!(restored_shot.1, "prm_backup");
        assert_ne!(restored_shot.2, "prv_backup_2");
        assert_ne!(restored_shot.3, "ast_backup");
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM shot_stage_configs WHERE shot_id = ?"
            )
            .bind(&restored_shot.0)
            .fetch_one(&pool)
            .await
            .unwrap(),
            2
        );
        let restored_stage_prompts: Vec<(String, String, String, String)> = sqlx::query_as(
            "SELECT stage, prompt_text, prompt_entry_id, prompt_version_id
             FROM shot_stage_prompts
             WHERE shot_id = ?
             ORDER BY stage",
        )
        .bind(&restored_shot.0)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(restored_stage_prompts.len(), 2);
        assert_eq!(
            restored_stage_prompts
                .iter()
                .map(|(stage, text, _, _)| (stage.as_str(), text.as_str()))
                .collect::<Vec<_>>(),
            vec![("image", "图片阶段快照"), ("video", "视频阶段快照")]
        );
        assert!(restored_stage_prompts
            .iter()
            .all(|(_, _, entry_id, version_id)| entry_id != "prm_backup"
                && version_id != "prv_backup_1"
                && version_id != "prv_backup_2"));
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM shot_generation_links WHERE shot_id = ?"
            )
            .bind(&restored_shot.0)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
        let restored_batch_item: (String, String, Option<String>) = sqlx::query_as(
            "SELECT i.id, i.batch_id, l.production_batch_item_id
             FROM production_batch_items i
             JOIN shot_generation_links l ON l.production_batch_item_id = i.id
             JOIN production_batches b ON b.id = i.batch_id
             WHERE b.project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_ne!(restored_batch_item.0, "pbi_backup");
        assert_ne!(restored_batch_item.1, "pbt_backup");
        assert_eq!(
            restored_batch_item.2.as_deref(),
            Some(restored_batch_item.0.as_str())
        );
    }

    #[tokio::test]
    async fn fixed_v1_fixture_inspects_and_restores_with_empty_organization() {
        let directory = tempdir().unwrap();
        let data_dirs = AppDataDirs::initialize(directory.path().join("AIStudioData")).unwrap();
        let pool = initialize(&data_dirs.database).await.unwrap();
        let archive_path = directory.path().join("legacy-v1.zip");
        let file = File::create(&archive_path).unwrap();
        let mut writer = ZipWriter::new(file);
        let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
        let manifest = json!({"format":"ai-studio-project-backup","version":1,"createdBy":"0.1.0","project":{"id":"legacy-project","name":"旧项目"}});
        let project = json!({
            "project":{"id":"legacy-project","name":"旧项目"},"description":null,
            "createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-01T00:00:00Z",
            "activeTasksExcluded":0,"incompleteTasksExcluded":0,"tasks":[],"taskEvents":[],"assets":[],
            "mappings":[],"snapshots":[],"presets":[],"batches":[],"items":[],"workflowRefs":[]
        });
        writer.start_file("manifest.json", options).unwrap();
        writer
            .write_all(serde_json::to_string(&manifest).unwrap().as_bytes())
            .unwrap();
        writer.start_file("project.json", options).unwrap();
        writer
            .write_all(serde_json::to_string(&project).unwrap().as_bytes())
            .unwrap();
        writer.finish().unwrap();
        let service = ProjectBackupService::new(
            pool.clone(),
            data_dirs.projects.clone(),
            data_dirs.cache.clone(),
        );
        let preview = service.inspect(archive_path).await.unwrap();
        let restored = service.restore(&preview.inspection_id).await.unwrap();
        let tags: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM asset_tags WHERE project_id = ?")
            .bind(&restored.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        let favorites: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM asset_favorites WHERE project_id = ?")
                .bind(&restored.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!((tags, favorites), (0, 0));
    }

    #[tokio::test]
    async fn fixed_v2_fixture_restores_with_empty_prompt_library() {
        let directory = tempdir().unwrap();
        let data_dirs = AppDataDirs::initialize(directory.path().join("AIStudioData")).unwrap();
        let pool = initialize(&data_dirs.database).await.unwrap();
        let archive_path = directory.path().join("legacy-v2.zip");
        let file = File::create(&archive_path).unwrap();
        let mut writer = ZipWriter::new(file);
        let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
        let manifest = json!({"format":"ai-studio-project-backup","version":2,"createdBy":"0.2.0","project":{"id":"legacy-v2-project","name":"旧项目 v2"}});
        let project = json!({
            "project":{"id":"legacy-v2-project","name":"旧项目 v2"},"description":null,
            "createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-01T00:00:00Z",
            "activeTasksExcluded":0,"incompleteTasksExcluded":0,"tasks":[],"taskEvents":[],"assets":[],
            "mappings":[],"snapshots":[],"presets":[],"batches":[],"items":[],"workflowRefs":[],
            "assetTags":[],"assetTagLinks":[],"assetFavorites":[]
        });
        writer.start_file("manifest.json", options).unwrap();
        writer
            .write_all(serde_json::to_string(&manifest).unwrap().as_bytes())
            .unwrap();
        writer.start_file("project.json", options).unwrap();
        writer
            .write_all(serde_json::to_string(&project).unwrap().as_bytes())
            .unwrap();
        writer.finish().unwrap();
        let service = ProjectBackupService::new(
            pool.clone(),
            data_dirs.projects.clone(),
            data_dirs.cache.clone(),
        );
        let preview = service.inspect(archive_path).await.unwrap();
        assert_eq!(preview.prompt_entries, 0);
        let restored = service.restore(&preview.inspection_id).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM prompt_entries WHERE project_id = ?",
            )
            .bind(restored.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn fixed_v3_fixture_restores_with_empty_shot_data() {
        let directory = tempdir().unwrap();
        let data_dirs = AppDataDirs::initialize(directory.path().join("AIStudioData")).unwrap();
        let pool = initialize(&data_dirs.database).await.unwrap();
        let archive_path = directory.path().join("legacy-v3.zip");
        let file = File::create(&archive_path).unwrap();
        let mut writer = ZipWriter::new(file);
        let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
        let manifest = json!({"format":"ai-studio-project-backup","version":3,"createdBy":"0.2.0","project":{"id":"legacy-v3-project","name":"旧项目 v3"}});
        let project = json!({
            "project":{"id":"legacy-v3-project","name":"旧项目 v3"},"description":null,
            "createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-01T00:00:00Z",
            "activeTasksExcluded":0,"incompleteTasksExcluded":0,"tasks":[],"taskEvents":[],"assets":[],
            "mappings":[],"snapshots":[],"presets":[],"batches":[],"items":[],"workflowRefs":[],
            "assetTags":[],"assetTagLinks":[],"assetFavorites":[]
        });
        writer.start_file("manifest.json", options).unwrap();
        writer
            .write_all(serde_json::to_string(&manifest).unwrap().as_bytes())
            .unwrap();
        writer.start_file("project.json", options).unwrap();
        writer
            .write_all(serde_json::to_string(&project).unwrap().as_bytes())
            .unwrap();
        writer.finish().unwrap();
        let service = ProjectBackupService::new(
            pool.clone(),
            data_dirs.projects.clone(),
            data_dirs.cache.clone(),
        );
        let preview = service.inspect(archive_path).await.unwrap();
        assert_eq!(preview.shots, 0);
        let restored = service.restore(&preview.inspection_id).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM shots WHERE project_id = ?")
                .bind(restored.id)
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn fixed_v4_fixture_restores_with_empty_shot_data() {
        let directory = tempdir().unwrap();
        let data_dirs = AppDataDirs::initialize(directory.path().join("AIStudioData")).unwrap();
        let pool = initialize(&data_dirs.database).await.unwrap();
        let archive_path = directory.path().join("legacy-v4.zip");
        let file = File::create(&archive_path).unwrap();
        let mut writer = ZipWriter::new(file);
        let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
        let manifest = json!({"format":"ai-studio-project-backup","version":4,"createdBy":"0.3.0","project":{"id":"legacy-v4-project","name":"旧项目 v4"}});
        let project = json!({
            "project":{"id":"legacy-v4-project","name":"旧项目 v4"},"description":null,
            "createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-01T00:00:00Z",
            "activeTasksExcluded":0,"incompleteTasksExcluded":0,"tasks":[],"taskEvents":[],"assets":[],
            "mappings":[],"snapshots":[],"presets":[],"batches":[],"items":[],"workflowRefs":[],
            "assetTags":[],"assetTagLinks":[],"assetFavorites":[]
        });
        writer.start_file("manifest.json", options).unwrap();
        writer
            .write_all(serde_json::to_string(&manifest).unwrap().as_bytes())
            .unwrap();
        writer.start_file("project.json", options).unwrap();
        writer
            .write_all(serde_json::to_string(&project).unwrap().as_bytes())
            .unwrap();
        writer.finish().unwrap();
        let service = ProjectBackupService::new(
            pool.clone(),
            data_dirs.projects.clone(),
            data_dirs.cache.clone(),
        );
        let preview = service.inspect(archive_path).await.unwrap();
        assert_eq!(preview.shots, 0);
        let restored = service.restore(&preview.inspection_id).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM shots WHERE project_id = ?")
                .bind(restored.id)
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn fixed_v5_through_v9_fixtures_restore_with_empty_later_data() {
        let directory = tempdir().unwrap();
        let data_dirs = AppDataDirs::initialize(directory.path().join("AIStudioData")).unwrap();
        let pool = initialize(&data_dirs.database).await.unwrap();
        let service = ProjectBackupService::new(
            pool.clone(),
            data_dirs.projects.clone(),
            data_dirs.cache.clone(),
        );

        for version in [5_u32, 6_u32, 7_u32, 8_u32, 9_u32] {
            let project_id = format!("legacy-v{version}-project");
            let project_name = format!("旧项目 v{version}");
            let archive_path = directory.path().join(format!("legacy-v{version}.zip"));
            let file = File::create(&archive_path).unwrap();
            let mut writer = ZipWriter::new(file);
            let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
            let manifest = json!({
                "format": "ai-studio-project-backup",
                "version": version,
                "createdBy": "0.3.0",
                "project": {"id": project_id, "name": project_name}
            });
            let project = json!({
                "project": {"id": project_id, "name": project_name},
                "description": null,
                "createdAt": "2026-01-01T00:00:00Z",
                "updatedAt": "2026-01-01T00:00:00Z",
                "activeTasksExcluded": 0,
                "incompleteTasksExcluded": 0,
                "tasks": [],
                "taskEvents": [],
                "assets": [],
                "mappings": [],
                "snapshots": [],
                "presets": [],
                "batches": [],
                "items": [],
                "workflowRefs": [],
                "assetTags": [],
                "assetTagLinks": [],
                "assetFavorites": [],
                "assetVideoPrompts": [],
                "productionItemReviews": [],
                "shots": [],
                "shotStageConfigs": [],
                "shotReferenceAssets": [],
                "shotGenerationLinks": []
            });
            writer.start_file("manifest.json", options).unwrap();
            writer
                .write_all(serde_json::to_string(&manifest).unwrap().as_bytes())
                .unwrap();
            writer.start_file("project.json", options).unwrap();
            writer
                .write_all(serde_json::to_string(&project).unwrap().as_bytes())
                .unwrap();
            writer.finish().unwrap();

            let preview = service.inspect(archive_path).await.unwrap();
            assert_eq!(preview.project_name, project_name);
            assert_eq!(preview.production_queues, 0);
            assert_eq!(preview.benchmarks, 0);
            let restored = service.restore(&preview.inspection_id).await.unwrap();
            let counts: (i64, i64, i64) = sqlx::query_as(
                "SELECT
                   (SELECT COUNT(*) FROM asset_video_prompts WHERE project_id = ?),
                   (SELECT COUNT(*) FROM production_item_reviews WHERE project_id = ?),
                   (SELECT COUNT(*) FROM benchmark_experiments WHERE project_id = ?)",
            )
            .bind(&restored.id)
            .bind(&restored.id)
            .bind(&restored.id)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(counts, (0, 0, 0));
        }
    }

    #[tokio::test]
    async fn snapshot_remap_failure_rolls_back_all_restore_rows() {
        let directory = tempdir().unwrap();
        let data_dirs = AppDataDirs::initialize(directory.path().join("AIStudioData")).unwrap();
        let pool = initialize(&data_dirs.database).await.unwrap();
        let service = ProjectBackupService::new(
            pool.clone(),
            data_dirs.projects.clone(),
            data_dirs.cache.clone(),
        );
        let now = Utc::now();
        let project_id = "prj_snapshot_remap_atomicity".to_owned();
        let task_id = "tsk_original".to_owned();
        let snapshot_id = "snp_original".to_owned();
        let document = BackupDocument {
            project: BackupProject {
                id: "project-original".to_owned(),
                name: "原始项目".to_owned(),
            },
            description: None,
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
            active_tasks_excluded: 0,
            incomplete_tasks_excluded: 0,
            tasks: vec![BackupTask {
                id: task_id.clone(),
                workflow_id: "workflow-1".to_owned(),
                workflow_version_id: "workflow-version-1".to_owned(),
                recipe_id: "recipe-1".to_owned(),
                app_version: None,
                build_commit: None,
                workflow_version: None,
                workflow_sha256: None,
                recipe_version: None,
                recipe_sha256: None,
                package_name: None,
                package_source_path: None,
                dynamic_binding_targets: None,
                generation_execution_id: None,
                compiled_workflow_sha256: None,
                runtime_profile: None,
                concurrency_class: None,
                prepare_started_at: None,
                prepared_at: None,
                submitted_at: None,
                execution_started_at: None,
                execution_finished_at: None,
                collection_finished_at: None,
                status: "SUCCEEDED".to_owned(),
                prompt_id: None,
                queue_number: None,
                progress_mode: "indeterminate".to_owned(),
                progress_current: None,
                progress_total: None,
                current_node_id: None,
                error_code: None,
                error_message: None,
                raw_error: None,
                created_at: now.to_rfc3339(),
                queued_at: None,
                started_at: None,
                finished_at: Some(now.to_rfc3339()),
            }],
            task_events: Vec::new(),
            assets: vec![BackupAsset {
                id: "ast_original_1".to_owned(),
                asset_type: "image".to_owned(),
                category: "source_image".to_owned(),
                name: "源图".to_owned(),
                original_name: "source.png".to_owned(),
                sha256: String::new(),
                mime_type: "image/png".to_owned(),
                width: 1,
                height: 1,
                duration_ms: None,
                file_size: 0,
                source_task_id: Some(task_id.clone()),
                metadata: json!({}),
                created_at: now.to_rfc3339(),
                updated_at: now.to_rfc3339(),
                content_path: "assets/source_image/image/source.png".to_owned(),
                thumbnail_path: None,
            }],
            mappings: Vec::new(),
            snapshots: vec![BackupSnapshot {
                id: snapshot_id.clone(),
                task_id: task_id.clone(),
                workflow: json!({}),
                recipe_yaml: "schema_version: 1\ninputs: {}\n".to_owned(),
                user_inputs: json!({
                    "reference_image": {
                        "type": "image_asset",
                        "assetId": "ast_original_1"
                    }
                }),
                resolved_inputs: json!({
                    "reference_image": {
                        "type": "image_asset",
                        "assetId": "ast_original_1"
                    }
                }),
                created_at: now.to_rfc3339(),
            }],
            presets: Vec::new(),
            prompt_entries: vec![BackupPromptEntry {
                id: "prm_atomic".to_owned(),
                project_id: "project-original".to_owned(),
                kind: "prompt".to_owned(),
                name: "原子提示词".to_owned(),
                normalized_name: "原子提示词".to_owned(),
                tags: Vec::new(),
                created_at: now.to_rfc3339(),
                updated_at: now.to_rfc3339(),
            }],
            prompt_versions: vec![BackupPromptVersion {
                id: "prv_atomic".to_owned(),
                project_id: "project-original".to_owned(),
                prompt_id: "prm_atomic".to_owned(),
                version: 1,
                text: "原子版本".to_owned(),
                created_at: now.to_rfc3339(),
            }],
            batches: Vec::new(),
            items: Vec::new(),
            workflow_refs: Vec::new(),
            asset_tags: Vec::new(),
            asset_tag_links: Vec::new(),
            asset_favorites: Vec::new(),
            asset_video_prompts: Vec::new(),
            production_item_reviews: Vec::new(),
            benchmark_experiments: Vec::new(),
            benchmark_candidates: Vec::new(),
            production_runs: Vec::new(),
            production_stages: Vec::new(),
            production_stage_items: Vec::new(),
            production_run_templates: Vec::new(),
            benchmark_runs: Vec::new(),
            benchmark_quality_scores: Vec::new(),
            shots: Vec::new(),
            shot_stage_configs: Vec::new(),
            shot_stage_prompts: Vec::new(),
            shot_reference_assets: Vec::new(),
            shot_generation_links: Vec::new(),
        };
        let project = ProjectRecord {
            id: project_id.clone(),
            name: "恢复项目".to_owned(),
            description: None,
            root_path: PathBuf::from("C:/restore/project"),
            created_at: now,
            updated_at: now,
        };
        let task_ids = HashMap::from([(task_id.clone(), "tsk_restored".to_owned())]);
        let asset_ids = HashMap::new();
        let snapshot_ids = HashMap::from([(snapshot_id, "snp_restored".to_owned())]);
        let preset_ids = HashMap::new();
        let prompt_ids = HashMap::from([("prm_atomic".to_owned(), "prm_restored".to_owned())]);
        let prompt_version_ids =
            HashMap::from([("prv_atomic".to_owned(), "prv_restored".to_owned())]);
        let batch_ids = HashMap::new();
        let item_ids = HashMap::new();

        let error = service
            .restore_rows(
                &project,
                &document,
                &task_ids,
                &asset_ids,
                &snapshot_ids,
                &preset_ids,
                &prompt_ids,
                &prompt_version_ids,
                &batch_ids,
                &item_ids,
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new(),
                &[],
            )
            .await
            .expect_err("incomplete snapshot mapping must abort restore");

        assert_eq!(error.code(), "BACKUP_SNAPSHOT_ASSET_REMAP_FAILED");
        let project_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projects WHERE id = ?")
            .bind(&project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        let task_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE project_id = ?")
            .bind(&project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        let snapshot_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM generation_snapshots WHERE task_id = 'tsk_restored'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let asset_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM assets WHERE project_id = ?")
                .bind(&project_id)
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(project_count, 0);
        assert_eq!(task_count, 0);
        assert_eq!(snapshot_count, 0);
        assert_eq!(asset_count, 0);
        let prompt_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM prompt_entries WHERE project_id = ?")
                .bind(&project_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(prompt_count, 0);
    }

    #[test]
    fn streaming_backup_writes_large_asset_without_building_an_in_memory_zip() {
        use std::fs::File;
        use zip::ZipArchive;

        const LARGE_ASSET_BYTES: u64 = 256 * 1024 * 1024;
        let directory = tempdir().unwrap();
        let source_path = directory.path().join("large.bin");
        let source = File::create(&source_path).unwrap();
        source.set_len(LARGE_ASSET_BYTES).unwrap();
        source.sync_all().unwrap();

        let mut hasher = Sha256::new();
        let zero_chunk = vec![0_u8; super::STREAM_CHUNK_BYTES];
        for _ in 0..(LARGE_ASSET_BYTES / super::STREAM_CHUNK_BYTES as u64) {
            hasher.update(&zero_chunk);
        }
        let expected_sha256 = hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let document = super::BackupDocument {
            project: super::BackupProject {
                id: "project-large".to_owned(),
                name: "大文件项目".to_owned(),
            },
            description: None,
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
            active_tasks_excluded: 0,
            incomplete_tasks_excluded: 0,
            tasks: Vec::new(),
            task_events: Vec::new(),
            assets: Vec::new(),
            mappings: Vec::new(),
            snapshots: Vec::new(),
            presets: Vec::new(),
            prompt_entries: Vec::new(),
            prompt_versions: Vec::new(),
            batches: Vec::new(),
            items: Vec::new(),
            workflow_refs: Vec::new(),
            asset_tags: Vec::new(),
            asset_tag_links: Vec::new(),
            asset_favorites: Vec::new(),
            asset_video_prompts: Vec::new(),
            production_item_reviews: Vec::new(),
            benchmark_experiments: Vec::new(),
            benchmark_candidates: Vec::new(),
            production_runs: Vec::new(),
            production_stages: Vec::new(),
            production_stage_items: Vec::new(),
            production_run_templates: Vec::new(),
            benchmark_runs: Vec::new(),
            benchmark_quality_scores: Vec::new(),
            shots: Vec::new(),
            shot_stage_configs: Vec::new(),
            shot_stage_prompts: Vec::new(),
            shot_reference_assets: Vec::new(),
            shot_generation_links: Vec::new(),
        };
        let files = [super::BackupFileSource {
            zip_path: "assets/ast_large/content.bin".to_owned(),
            source_path,
            expected_size: LARGE_ASSET_BYTES,
            expected_sha256: Some(expected_sha256),
        }];
        let archive_path = directory.path().join("large-backup.zip");
        super::write_zip_to_path(&document, &files, &archive_path).unwrap();

        let archive_size = std::fs::metadata(&archive_path).unwrap().len();
        assert!(archive_size < LARGE_ASSET_BYTES);
        let mut archive = ZipArchive::new(File::open(&archive_path).unwrap()).unwrap();
        let entry = archive.by_name("assets/ast_large/content.bin").unwrap();
        assert_eq!(entry.size(), LARGE_ASSET_BYTES);
    }

    #[test]
    fn zip_slip_archive_is_rejected_before_restore() {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::FileOptions::default();
        use std::io::Write;
        writer.start_file("manifest.json", options).unwrap();
        writer.write_all(b"{}").unwrap();
        writer.start_file("../escape.txt", options).unwrap();
        writer.write_all(b"blocked").unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let directory = tempdir().unwrap();
        let path = directory.path().join("unsafe.zip");
        std::fs::write(&path, bytes).unwrap();
        assert!(inspect_archive(&path).is_err());
    }
}
