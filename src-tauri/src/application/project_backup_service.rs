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
const BACKUP_VERSION: u32 = 2;
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
        let mut batch_ids = HashMap::new();
        for batch in &document.batches {
            batch_ids.insert(batch.id.clone(), format!("pbt_{}", Uuid::new_v4().simple()));
        }
        let mut item_ids = HashMap::new();
        for item in &document.items {
            item_ids.insert(item.id.clone(), format!("pbi_{}", Uuid::new_v4().simple()));
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
                &batch_ids,
                &item_ids,
                &tag_ids,
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
        let tasks = db_tasks
            .into_iter()
            .filter(|task| included_task_ids.contains(&task.id))
            .map(BackupTask::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let task_events = query_task_events(&mut transaction, &included_task_ids).await?;
        let snapshots = query_snapshots(&mut transaction, &included_task_ids).await?;
        let mappings = query_mappings(&mut transaction, &included_task_ids).await?;
        let presets = query_presets(&mut transaction, project_id).await?;
        let batches = query_batches(&mut transaction, project_id).await?;
        let items = query_batch_items(&mut transaction, &batches).await?;
        let asset_tags = sqlx::query_as::<_, BackupAssetTag>(
            "SELECT id, project_id, name, normalized_name, created_at, updated_at FROM asset_tags WHERE project_id = ? ORDER BY created_at, id",
        ).bind(project_id).fetch_all(&mut *transaction).await.map_err(|error| AppError::database(error.to_string()))?;
        let mut asset_tag_links = sqlx::query_as::<_, BackupAssetTagLink>(
            "SELECT asset_id, tag_id, project_id, created_at FROM asset_tag_links WHERE project_id = ? ORDER BY created_at, asset_id, tag_id",
        ).bind(project_id).fetch_all(&mut *transaction).await.map_err(|error| AppError::database(error.to_string()))?;
        let mut asset_favorites = sqlx::query_as::<_, BackupAssetFavorite>(
            "SELECT asset_id, project_id, created_at FROM asset_favorites WHERE project_id = ? ORDER BY created_at, asset_id",
        ).bind(project_id).fetch_all(&mut *transaction).await.map_err(|error| AppError::database(error.to_string()))?;
        let workflow_refs = collect_workflow_refs(&tasks);
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
            batches,
            items,
            workflow_refs,
            asset_tags,
            asset_tag_links,
            asset_favorites,
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
        batch_ids: &HashMap<String, String>,
        item_ids: &HashMap<String, String>,
        tag_ids: &HashMap<String, String>,
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
            batch_ids,
            item_ids,
            tag_ids,
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
    batches: Vec<BackupBatch>,
    items: Vec<BackupBatchItem>,
    workflow_refs: Vec<WorkflowReference>,
    #[serde(default)]
    asset_tags: Vec<BackupAssetTag>,
    #[serde(default)]
    asset_tag_links: Vec<BackupAssetTagLink>,
    #[serde(default)]
    asset_favorites: Vec<BackupAssetFavorite>,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupTask {
    id: String,
    workflow_id: String,
    workflow_version_id: String,
    recipe_id: String,
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
    if manifest.format != BACKUP_FORMAT || !matches!(manifest.version, 1 | 2) {
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
    validate_organization_document(document)?;
    Ok(())
}

fn validate_organization_document(document: &BackupDocument) -> Result<(), AppError> {
    let asset_ids = document
        .assets
        .iter()
        .map(|asset| asset.id.as_str())
        .collect::<HashSet<_>>();
    let tag_ids = document
        .asset_tags
        .iter()
        .map(|tag| tag.id.as_str())
        .collect::<HashSet<_>>();
    if tag_ids.len() != document.asset_tags.len() {
        return Err(AppError::backup_invalid("备份包含重复标签 ID"));
    }
    let normalized = document
        .asset_tags
        .iter()
        .map(|tag| tag.normalized_name.as_str())
        .collect::<HashSet<_>>();
    if normalized.len() != document.asset_tags.len() {
        return Err(AppError::backup_invalid("备份包含重复标签名称"));
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
    for link in &document.asset_tag_links {
        if !asset_ids.contains(link.asset_id.as_str()) || !tag_ids.contains(link.tag_id.as_str()) {
            return Err(AppError::backup_invalid("备份标签链接引用了未知素材或标签"));
        }
        if !links.insert((link.asset_id.as_str(), link.tag_id.as_str())) {
            return Err(AppError::backup_invalid("备份包含重复标签链接"));
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
    batch_ids: &HashMap<String, String>,
    item_ids: &HashMap<String, String>,
    tag_ids: &HashMap<String, String>,
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
            "INSERT INTO tasks (id, project_id, workflow_id, workflow_version_id, recipe_id, status,
             prompt_id, queue_number, progress_mode, progress_current, progress_total, current_node_id,
             error_code, error_message, raw_error_json, created_at, queued_at, started_at, finished_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(new_task_id)
        .bind(&project.id)
        .bind(&task.workflow_id)
        .bind(&task.workflow_version_id)
        .bind(&task.recipe_id)
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
        restored_name, safe_zip_path, BackupAsset, BackupDocument, BackupProject, BackupSnapshot,
        BackupTask, ProjectBackupService,
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
        sqlx::query(
            "INSERT INTO generation_snapshots (id, task_id, workflow_json, recipe_yaml, user_inputs_json, resolved_inputs_json, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("snp_backup")
        .bind("tsk_backup")
        .bind("{}")
        .bind("schema_version: 1\ninputs: {}\n")
        .bind(r#"{"reference_image":{"type":"image_asset","assetId":"ast_backup"}}"#)
        .bind(r#"{"reference_image":{"type":"image_asset","assetId":"ast_backup"}}"#)
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
        assert_eq!(manifest.version, 2);
        assert_eq!(exported.entries, names.len());
        assert!(!names.contains("app.db"));
        assert!(!names.contains("workflow_api.json"));
        assert!(!names.contains("recipe.yaml"));
        let preview = service.inspect(archive_path).await.unwrap();
        assert_eq!(preview.image_count, 1);
        let restored = service.restore(&preview.inspection_id).await.unwrap();
        assert_ne!(restored.id, "project-backup");
        let restored_count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM assets WHERE project_id = ?")
                .bind(&restored.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(restored_count, 2);
        let restored_path = sqlx::query_scalar::<_, String>(
            "SELECT storage_path FROM assets WHERE project_id = ? AND type = 'image'",
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
        let restored_asset_id: String =
            sqlx::query_scalar("SELECT id FROM assets WHERE project_id = ? AND type = 'image'")
                .bind(&restored.id)
                .fetch_one(&pool)
                .await
                .unwrap();
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
            user_inputs["reference_image"]["assetId"],
            restored_asset_id.as_str()
        );
        assert_eq!(
            resolved_inputs["reference_image"]["assetId"],
            restored_asset_id.as_str()
        );
        assert_ne!(restored_asset_id, "ast_backup");
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
            batches: Vec::new(),
            items: Vec::new(),
            workflow_refs: Vec::new(),
            asset_tags: Vec::new(),
            asset_tag_links: Vec::new(),
            asset_favorites: Vec::new(),
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
                &batch_ids,
                &item_ids,
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
            batches: Vec::new(),
            items: Vec::new(),
            workflow_refs: Vec::new(),
            asset_tags: Vec::new(),
            asset_tag_links: Vec::new(),
            asset_favorites: Vec::new(),
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
