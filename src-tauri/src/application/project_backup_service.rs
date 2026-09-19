use crate::application::ports::{
    ProjectBackupRepository, ProjectBackupRepositorySource, ProjectBackupRestorePlan,
    ProjectBackupRestoreResult, ProjectRecord, RepositoryError,
};
use crate::domain::consistency::{
    BindingRole, InheritanceMode, ProfileRevisionStatus, ProfileType, ReferenceSetPurpose,
};
use crate::error::AppError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{self, BufWriter, Read, Write},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use uuid::Uuid;
use zip::{write::FileOptions, CompressionMethod, ZipArchive, ZipWriter};

const BACKUP_FORMAT: &str = "ai-studio-project-backup";
const BACKUP_VERSION: u32 = 20;
const MAX_ZIP_BYTES: u64 = 20 * 1024 * 1024 * 1024;
const MAX_ENTRY_BYTES: u64 = 10 * 1024 * 1024 * 1024;
const MAX_ENTRIES: usize = 100_000;
const INSPECTION_TTL: Duration = Duration::from_secs(20 * 60);
/// Root JSON entries written for v19+ packages: manifest, project, history,
/// presets, production_queue, preparation snapshots, provenance.
const PACKAGE_JSON_ENTRIES: usize = 7;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBackupManifest {
    pub format: String,
    pub version: u32,
    pub created_by: String,
    pub project: BackupProject,
    /// Inventory counts for inspect/preview. Absent on historical packages (serde default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inventory: Option<BackupInventoryCounts>,
    /// SHA-256 of the exact `project.json` bytes written into the package.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logical_snapshot_checksum: Option<String>,
    /// Required media content files (path + size + sha256). Thumbnails stay under
    /// `assets/<id>/thumbnail.*` and are not duplicated into a separate previews/ tree.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub media_inventory: Vec<BackupMediaInventoryEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupInventoryCounts {
    pub assets: usize,
    pub asset_versions: usize,
    pub relations: usize,
    pub prompts: usize,
    pub models: usize,
    pub tools: usize,
    pub lineage: usize,
    pub tasks: usize,
    /// Missing from manifests produced before Backup v20.
    #[serde(default)]
    pub artifact_reviews: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupMediaInventoryEntry {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

/// Deterministic sidecar for provenance rows. Logical authority remains `project.json`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupProvenanceDocument {
    generation_tool_usages: Vec<BackupGenerationToolUsage>,
    generation_asset_versions: Vec<BackupGenerationAssetVersion>,
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
    pub asset_versions: usize,
    pub asset_relations: usize,
    pub models: usize,
    pub model_versions: usize,
    pub tools: usize,
    pub tool_instances: usize,
    pub generation_tool_usages: usize,
    pub generation_asset_versions: usize,
    pub artifact_reviews: usize,
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
    pub status: String,
    pub backup_version: u32,
    pub assets: usize,
    pub versions: usize,
    pub generations: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_models: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_files: Vec<String>,
    pub restored_generation_tool_usages: usize,
    pub restored_generation_asset_versions: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unresolved_model_version_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unresolved_tool_instance_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unresolved_tool_version_ids: Vec<String>,
}

#[derive(Clone)]
struct Inspection {
    archive_path: PathBuf,
    expires_at: std::time::Instant,
}

fn map_repository_error(error: RepositoryError) -> AppError {
    match error {
        RepositoryError::Database { message } => AppError::database(message),
        RepositoryError::NotFound { entity, id } if entity == "project" => {
            AppError::project_not_found(id)
        }
        RepositoryError::Serialization { context, message } => {
            AppError::backup_invalid(format!("{context}: {message}"))
        }
        RepositoryError::Integrity { message } => AppError::backup_invalid(message),
        other => AppError::backup_invalid(other.to_string()),
    }
}

fn build_restore_warnings(
    backup_version: u32,
    expected_generation_tool_usages: usize,
    expected_generation_asset_versions: usize,
    result: &ProjectBackupRestoreResult,
) -> Vec<String> {
    let mut warnings = Vec::new();
    if backup_version < BACKUP_VERSION {
        warnings.push(format!(
            "已兼容恢复 Backup v{backup_version}；该归档可能不包含当前 v{BACKUP_VERSION} 的全部 v2 数据。"
        ));
    }
    if !result.unresolved_model_version_ids.is_empty() {
        warnings.push(format!(
            "以下 ModelVersion 未能解析，相关历史关系保留为 UNKNOWN：{}。",
            result.unresolved_model_version_ids.join(", ")
        ));
    }
    if !result.unresolved_tool_instance_ids.is_empty() {
        warnings.push(format!(
            "以下 ToolInstance 未能解析，相关工具溯源未伪造：{}。",
            result.unresolved_tool_instance_ids.join(", ")
        ));
    }
    if !result.unresolved_tool_version_ids.is_empty() {
        warnings.push(format!(
            "以下 ToolVersion 未能解析，相关生成记录保留但版本显示为 UNKNOWN：{}。",
            result.unresolved_tool_version_ids.join(", ")
        ));
    }
    if result.restored_generation_tool_usages < expected_generation_tool_usages {
        warnings.push(format!(
            "归档中的 {} 条工具使用溯源中有 {} 条未恢复；未根据名称、路径或时间猜测关系。",
            expected_generation_tool_usages,
            expected_generation_tool_usages - result.restored_generation_tool_usages
        ));
    }
    if result.restored_generation_asset_versions < expected_generation_asset_versions {
        warnings.push(format!(
            "归档中的 {} 条生成资产版本关系中有 {} 条未恢复；恢复未执行启发式修复。",
            expected_generation_asset_versions,
            expected_generation_asset_versions - result.restored_generation_asset_versions
        ));
    }
    warnings
}

pub struct ProjectBackupService {
    repository: Arc<dyn ProjectBackupRepository>,
    projects_dir: PathBuf,
    inspection_dir: PathBuf,
    inspections: Mutex<HashMap<String, Inspection>>,
}

impl ProjectBackupService {
    pub fn new<R: ProjectBackupRepositorySource>(
        repository: R,
        projects_dir: PathBuf,
        cache_dir: PathBuf,
    ) -> Self {
        Self {
            repository: repository.into_repository(),
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
                .unwrap_or("AI-Studio-Project.aiarchive")
                .to_owned(),
            bytes,
            entries: PACKAGE_JSON_ENTRIES + built.files.len(),
            active_tasks_excluded: built.document.active_tasks_excluded,
        })
    }

    pub async fn inspect(&self, source: PathBuf) -> Result<ProjectBackupPreviewView, AppError> {
        let (manifest, document, entry_names) = inspect_archive(&source)?;
        validate_document_entries(&document, &entry_names, manifest.version)?;
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
            asset_versions: document.asset_versions.len(),
            asset_relations: document.asset_relations.len(),
            models: document.models.len(),
            model_versions: document.model_versions.len(),
            tools: document.tools.len(),
            tool_instances: document.tool_instances.len(),
            generation_tool_usages: document.generation_tool_usages.len(),
            generation_asset_versions: document.generation_asset_versions.len(),
            artifact_reviews: document.artifact_reviews.len(),
            missing_workflows,
            active_tasks_excluded: document.active_tasks_excluded,
            warning: "项目归档包含项目历史、提示词、素材与溯源，请妥善保存。检查不会修改数据库。"
                .to_owned(),
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
        validate_document_entries(&document, &entry_names, manifest.version)?;

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
        let mut preparation_snapshot_ids = HashMap::new();
        for snapshot in &document.preparation_snapshots {
            preparation_snapshot_ids.insert(
                snapshot.id.clone(),
                format!("pps_{}", Uuid::new_v4().simple()),
            );
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
        let mut asset_version_ids = HashMap::new();
        for version in &document.asset_versions {
            asset_version_ids.insert(version.id.clone(), format!("asv_{}", Uuid::new_v4()));
        }
        let mut asset_relation_ids = HashMap::new();
        for relation in &document.asset_relations {
            asset_relation_ids.insert(relation.id.clone(), format!("rel_{}", Uuid::new_v4()));
        }
        let mut generation_tool_usage_ids = HashMap::new();
        for usage in &document.generation_tool_usages {
            generation_tool_usage_ids.insert(usage.id.clone(), format!("gtu_{}", Uuid::new_v4()));
        }
        let mut generation_asset_version_ids = HashMap::new();
        for link in &document.generation_asset_versions {
            generation_asset_version_ids.insert(link.id.clone(), format!("gav_{}", Uuid::new_v4()));
        }
        let mut tag_ids = HashMap::new();
        for tag in &document.asset_tags {
            tag_ids.insert(tag.id.clone(), format!("tag_{}", Uuid::new_v4()));
        }
        let mut reference_anchor_ids = HashMap::new();
        for anchor in &document.reference_anchors {
            reference_anchor_ids.insert(anchor.id.clone(), format!("anc_{}", Uuid::new_v4()));
        }
        let mut production_series_ids = HashMap::new();
        for series in &document.production_series {
            production_series_ids.insert(series.id.clone(), format!("ser_{}", Uuid::new_v4()));
        }
        let mut production_episode_ids = HashMap::new();
        for episode in &document.production_episodes {
            production_episode_ids.insert(episode.id.clone(), format!("ep_{}", Uuid::new_v4()));
        }
        let mut production_scene_ids = HashMap::new();
        for scene in &document.production_scenes {
            production_scene_ids.insert(scene.id.clone(), format!("scn_{}", Uuid::new_v4()));
        }
        let mut handoff_ids = HashMap::new();
        for handoff in &document.external_production_handoffs {
            handoff_ids.insert(handoff.id.clone(), format!("hnd_{}", Uuid::new_v4()));
        }
        let mut script_source_ids = HashMap::new();
        for source in &document.script_sources {
            script_source_ids.insert(source.id.clone(), format!("scr_{}", Uuid::new_v4()));
        }
        let mut script_draft_ids = HashMap::new();
        for revision in &document.script_draft_revisions {
            script_draft_ids
                .entry(revision.draft_id.clone())
                .or_insert_with(|| format!("drf_{}", Uuid::new_v4()));
        }
        let mut script_revision_ids = HashMap::new();
        for revision in &document.script_draft_revisions {
            script_revision_ids.insert(revision.id.clone(), format!("drev_{}", Uuid::new_v4()));
        }
        let mut consistency_ids = ConsistencyRestoreIds::default();
        for profile_id in document
            .character_profiles
            .iter()
            .map(|profile| &profile.id)
            .chain(document.scene_profiles.iter().map(|profile| &profile.id))
            .chain(document.prop_profiles.iter().map(|profile| &profile.id))
            .chain(document.style_profiles.iter().map(|profile| &profile.id))
        {
            consistency_ids
                .profiles
                .insert(profile_id.clone(), format!("cp_{}", Uuid::new_v4()));
        }
        for variant in &document.costume_variants {
            consistency_ids
                .costume_variants
                .insert(variant.id.clone(), format!("cv_{}", Uuid::new_v4()));
        }
        for revision in &document.profile_revisions {
            consistency_ids
                .profile_revisions
                .insert(revision.id.clone(), format!("prv_{}", Uuid::new_v4()));
        }
        for reference_set in &document.reference_sets {
            consistency_ids
                .reference_sets
                .insert(reference_set.id.clone(), format!("rs_{}", Uuid::new_v4()));
        }
        for binding in &document.shot_profile_bindings {
            consistency_ids
                .shot_profile_bindings
                .insert(binding.id.clone(), format!("spb_{}", Uuid::new_v4()));
        }
        for binding in &document.shot_reference_set_bindings {
            consistency_ids
                .shot_reference_set_bindings
                .insert(binding.id.clone(), format!("srb_{}", Uuid::new_v4()));
        }
        for binding in &document.scope_profile_bindings {
            consistency_ids
                .scope_profile_bindings
                .insert(binding.id.clone(), format!("hpb_{}", Uuid::new_v4()));
        }
        for binding in &document.scope_reference_set_bindings {
            consistency_ids
                .scope_reference_set_bindings
                .insert(binding.id.clone(), format!("hrb_{}", Uuid::new_v4()));
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

        let restored_project = project.clone();
        let restored_asset_count = document.assets.len();
        let restored_version_count = document.asset_versions.len();
        let restored_generation_count = document.tasks.len();
        let expected_generation_tool_usages = document.generation_tool_usages.len();
        let expected_generation_asset_versions = document.generation_asset_versions.len();
        let restored_snapshots = prepare_restored_snapshots(&document, &asset_ids)?;
        let restore_result = self
            .repository
            .restore_atomic(ProjectBackupRestorePlan {
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
                preparation_snapshot_ids,
                benchmark_experiment_ids,
                benchmark_candidate_ids,
                production_run_ids,
                production_stage_ids,
                production_stage_item_ids,
                production_run_template_ids,
                benchmark_run_ids,
                benchmark_quality_score_ids,
                tag_ids,
                reference_anchor_ids,
                production_structure_ids: ProductionStructureIds {
                    series: production_series_ids,
                    episodes: production_episode_ids,
                    scenes: production_scene_ids,
                },
                handoff_ids,
                script_source_ids,
                script_draft_ids,
                script_revision_ids,
                consistency_ids,
                shot_ids,
                shot_generation_link_ids,
                asset_version_ids,
                asset_relation_ids,
                generation_tool_usage_ids,
                generation_asset_version_ids,
                restored_assets,
                restored_snapshots,
            })
            .await
            .map_err(map_repository_error);
        let restore_meta = match restore_result {
            Ok(meta) => meta,
            Err(error) => {
                let _ = fs::remove_dir_all(&final_root);
                return Err(error);
            }
        };
        let _ = fs::remove_file(&inspection.archive_path);
        let missing_models = restore_meta.unresolved_model_version_ids.clone();
        let missing_tools = restore_meta.unresolved_tool_instance_ids.clone();
        let warnings = build_restore_warnings(
            manifest.version,
            expected_generation_tool_usages,
            expected_generation_asset_versions,
            &restore_meta,
        );
        Ok(RestoredProjectView {
            id: restored_project.id,
            name: restored_project.name,
            description: restored_project.description,
            created_at: restored_project.created_at,
            updated_at: restored_project.updated_at,
            status: "COMPLETE".to_owned(),
            backup_version: manifest.version,
            assets: restored_asset_count,
            versions: restored_version_count,
            generations: restored_generation_count,
            warnings,
            missing_tools,
            missing_models,
            missing_files: Vec::new(),
            restored_generation_tool_usages: restore_meta.restored_generation_tool_usages,
            restored_generation_asset_versions: restore_meta.restored_generation_asset_versions,
            unresolved_model_version_ids: restore_meta.unresolved_model_version_ids,
            unresolved_tool_instance_ids: restore_meta.unresolved_tool_instance_ids,
            unresolved_tool_version_ids: restore_meta.unresolved_tool_version_ids,
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
        self.repository
            .find_missing_workflows(document)
            .await
            .map_err(map_repository_error)
    }

    async fn build_backup(&self, project_id: &str) -> Result<BuiltBackup, AppError> {
        let snapshot = self
            .repository
            .load_export_snapshot(project_id)
            .await
            .map_err(map_repository_error)?;
        let mut document = snapshot.document;
        let mut files = Vec::new();
        let mut assets = Vec::new();
        for asset in snapshot.assets {
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
                asset_type: asset.asset_type,
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

        document.assets = assets;
        Ok(BuiltBackup { document, files })
    }

    #[cfg(test)]
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
        handoff_ids: &HashMap<String, String>,
        script_source_ids: &HashMap<String, String>,
        script_draft_ids: &HashMap<String, String>,
        script_revision_ids: &HashMap<String, String>,
        consistency_ids: &ConsistencyRestoreIds,
        shot_ids: &HashMap<String, String>,
        shot_generation_link_ids: &HashMap<String, String>,
        restored_assets: &[RestoredAsset],
    ) -> Result<(), AppError> {
        let restored_snapshots = prepare_restored_snapshots(document, asset_ids)?;
        self.repository
            .restore_atomic(ProjectBackupRestorePlan {
                project: project.clone(),
                document: document.clone(),
                task_ids: task_ids.clone(),
                asset_ids: asset_ids.clone(),
                snapshot_ids: snapshot_ids.clone(),
                preset_ids: preset_ids.clone(),
                prompt_ids: prompt_ids.clone(),
                prompt_version_ids: prompt_version_ids.clone(),
                batch_ids: batch_ids.clone(),
                item_ids: item_ids.clone(),
                preparation_snapshot_ids: preparation_snapshot_ids.clone(),
                benchmark_experiment_ids: benchmark_experiment_ids.clone(),
                benchmark_candidate_ids: benchmark_candidate_ids.clone(),
                production_run_ids: production_run_ids.clone(),
                production_stage_ids: production_stage_ids.clone(),
                production_stage_item_ids: production_stage_item_ids.clone(),
                production_run_template_ids: production_run_template_ids.clone(),
                benchmark_run_ids: benchmark_run_ids.clone(),
                benchmark_quality_score_ids: benchmark_quality_score_ids.clone(),
                tag_ids: tag_ids.clone(),
                reference_anchor_ids: reference_anchor_ids.clone(),
                production_structure_ids: ProductionStructureIds {
                    series: production_structure_ids.series.clone(),
                    episodes: production_structure_ids.episodes.clone(),
                    scenes: production_structure_ids.scenes.clone(),
                },
                handoff_ids: handoff_ids.clone(),
                script_source_ids: script_source_ids.clone(),
                script_draft_ids: script_draft_ids.clone(),
                script_revision_ids: script_revision_ids.clone(),
                consistency_ids: ConsistencyRestoreIds {
                    profiles: consistency_ids.profiles.clone(),
                    costume_variants: consistency_ids.costume_variants.clone(),
                    profile_revisions: consistency_ids.profile_revisions.clone(),
                    reference_sets: consistency_ids.reference_sets.clone(),
                    shot_profile_bindings: consistency_ids.shot_profile_bindings.clone(),
                    shot_reference_set_bindings: consistency_ids
                        .shot_reference_set_bindings
                        .clone(),
                    scope_profile_bindings: consistency_ids.scope_profile_bindings.clone(),
                    scope_reference_set_bindings: consistency_ids
                        .scope_reference_set_bindings
                        .clone(),
                },
                shot_ids: shot_ids.clone(),
                shot_generation_link_ids: shot_generation_link_ids.clone(),
                asset_version_ids: HashMap::new(),
                asset_relation_ids: HashMap::new(),
                generation_tool_usage_ids: HashMap::new(),
                generation_asset_version_ids: HashMap::new(),
                restored_assets: restored_assets.to_vec(),
                restored_snapshots,
            })
            .await
            .map_err(map_repository_error)
            .map(|_| ())
    }
}

#[derive(Clone)]
struct BuiltBackup {
    document: BackupDocument,
    files: Vec<BackupFileSource>,
}

#[derive(Clone)]
pub(crate) struct BackupFileSource {
    pub(crate) zip_path: String,
    pub(crate) source_path: PathBuf,
    pub(crate) expected_size: u64,
    pub(crate) expected_sha256: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupDocument {
    pub(crate) project: BackupProject,
    pub(crate) description: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    pub(crate) active_tasks_excluded: usize,
    pub(crate) incomplete_tasks_excluded: usize,
    pub(crate) tasks: Vec<BackupTask>,
    pub(crate) task_events: Vec<BackupTaskEvent>,
    pub(crate) assets: Vec<BackupAsset>,
    pub(crate) mappings: Vec<BackupMapping>,
    pub(crate) snapshots: Vec<BackupSnapshot>,
    pub(crate) presets: Vec<BackupPreset>,
    #[serde(default)]
    pub(crate) prompt_entries: Vec<BackupPromptEntry>,
    #[serde(default)]
    pub(crate) prompt_versions: Vec<BackupPromptVersion>,
    pub(crate) batches: Vec<BackupBatch>,
    pub(crate) items: Vec<BackupBatchItem>,
    #[serde(default)]
    pub(crate) preparation_snapshots: Vec<BackupProductionPreparationSnapshot>,
    pub(crate) workflow_refs: Vec<WorkflowReference>,
    #[serde(default)]
    pub(crate) project_workflow_bindings: Vec<BackupProjectWorkflowBinding>,
    #[serde(default)]
    pub(crate) workflow_registry: Option<BackupWorkflowRegistry>,
    #[serde(default)]
    pub(crate) asset_tags: Vec<BackupAssetTag>,
    #[serde(default)]
    pub(crate) asset_tag_links: Vec<BackupAssetTagLink>,
    #[serde(default)]
    pub(crate) asset_favorites: Vec<BackupAssetFavorite>,
    #[serde(default)]
    pub(crate) asset_video_prompts: Vec<BackupAssetVideoPrompt>,
    #[serde(default)]
    pub(crate) reference_anchors: Vec<BackupReferenceAnchor>,
    #[serde(default)]
    pub(crate) production_series: Vec<BackupProductionSeries>,
    #[serde(default)]
    pub(crate) production_episodes: Vec<BackupProductionEpisode>,
    #[serde(default)]
    pub(crate) production_scenes: Vec<BackupProductionScene>,
    #[serde(default)]
    pub(crate) shot_scene_assignments: Vec<BackupShotSceneAssignment>,
    #[serde(default)]
    pub(crate) script_sources: Vec<BackupScriptSource>,
    #[serde(default)]
    pub(crate) script_draft_revisions: Vec<BackupScriptDraftRevision>,
    #[serde(default)]
    pub(crate) production_item_reviews: Vec<BackupProductionItemReview>,
    /// Per-artifact review state added in Backup v20. Absent on v19 and earlier.
    #[serde(default)]
    pub(crate) artifact_reviews: Vec<BackupArtifactReview>,
    #[serde(default)]
    pub(crate) benchmark_experiments: Vec<BackupBenchmarkExperiment>,
    #[serde(default)]
    pub(crate) benchmark_candidates: Vec<BackupBenchmarkCandidate>,
    #[serde(default)]
    pub(crate) production_runs: Vec<BackupProductionRun>,
    #[serde(default)]
    pub(crate) production_stages: Vec<BackupProductionStage>,
    #[serde(default)]
    pub(crate) production_stage_items: Vec<BackupProductionStageItem>,
    #[serde(default)]
    pub(crate) production_run_templates: Vec<BackupProductionRunTemplate>,
    #[serde(default)]
    pub(crate) benchmark_runs: Vec<BackupBenchmarkRun>,
    #[serde(default)]
    pub(crate) benchmark_quality_scores: Vec<BackupBenchmarkQualityScore>,
    #[serde(default)]
    pub(crate) shots: Vec<BackupShot>,
    #[serde(default)]
    pub(crate) external_production_handoffs: Vec<BackupExternalProductionHandoff>,
    #[serde(default)]
    pub(crate) external_production_handoff_entities: Vec<BackupExternalProductionHandoffEntity>,

    #[serde(default)]
    pub(crate) shot_stage_configs: Vec<BackupShotStageConfig>,
    #[serde(default)]
    pub(crate) shot_stage_prompts: Vec<BackupShotStagePrompt>,
    #[serde(default)]
    pub(crate) shot_reference_assets: Vec<BackupShotReferenceAsset>,
    #[serde(default)]
    pub(crate) shot_generation_links: Vec<BackupShotGenerationLink>,
    #[serde(default)]
    pub(crate) character_profiles: Vec<BackupCharacterProfile>,
    #[serde(default)]
    pub(crate) scene_profiles: Vec<BackupSceneProfile>,
    #[serde(default)]
    pub(crate) prop_profiles: Vec<BackupPropProfile>,
    #[serde(default)]
    pub(crate) style_profiles: Vec<BackupStyleProfile>,
    #[serde(default)]
    pub(crate) costume_variants: Vec<BackupCostumeVariant>,
    #[serde(default)]
    pub(crate) profile_revisions: Vec<BackupProfileRevision>,
    #[serde(default)]
    pub(crate) reference_sets: Vec<BackupReferenceSet>,
    #[serde(default)]
    pub(crate) reference_set_items: Vec<BackupReferenceSetItem>,
    #[serde(default)]
    pub(crate) shot_profile_bindings: Vec<BackupShotProfileBinding>,
    #[serde(default)]
    pub(crate) shot_reference_set_bindings: Vec<BackupShotReferenceSetBinding>,
    #[serde(default)]
    pub(crate) scope_profile_bindings: Vec<BackupScopeProfileBinding>,
    #[serde(default)]
    pub(crate) scope_reference_set_bindings: Vec<BackupScopeReferenceSetBinding>,
    #[serde(default)]
    pub(crate) asset_versions: Vec<BackupAssetVersion>,
    #[serde(default)]
    pub(crate) asset_relations: Vec<BackupAssetRelation>,
    #[serde(default)]
    pub(crate) models: Vec<BackupModel>,
    #[serde(default)]
    pub(crate) model_versions: Vec<BackupModelVersion>,
    #[serde(default)]
    pub(crate) tools: Vec<BackupTool>,
    #[serde(default)]
    pub(crate) tool_versions: Vec<BackupToolVersion>,
    #[serde(default)]
    pub(crate) tool_capabilities: Vec<BackupToolCapability>,
    #[serde(default)]
    pub(crate) tool_instances: Vec<BackupToolInstance>,
    #[serde(default)]
    pub(crate) generation_tool_usages: Vec<BackupGenerationToolUsage>,
    #[serde(default)]
    pub(crate) generation_asset_versions: Vec<BackupGenerationAssetVersion>,
}

/// Registry metadata is part of a project backup because bindings and history
/// refer to global immutable workflow IDs. The optional field keeps Backup16
/// documents readable; current backups always write it.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupWorkflowRegistry {
    pub(crate) workflows: Vec<BackupWorkflow>,
    pub(crate) versions: Vec<BackupWorkflowVersion>,
    pub(crate) recipes: Vec<BackupWorkflowRecipe>,
    pub(crate) runtime_artifacts: Vec<BackupWorkflowRuntimeArtifact>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupWorkflow {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) category: String,
    pub(crate) mode: String,
    pub(crate) source_kind: String,
    pub(crate) library_state: String,
    pub(crate) current_version_id: Option<String>,
    pub(crate) removed_at: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupWorkflowVersion {
    pub(crate) id: String,
    pub(crate) workflow_id: String,
    pub(crate) version: String,
    pub(crate) api_workflow_json: String,
    pub(crate) workflow_sha256: String,
    pub(crate) package_name: Option<String>,
    pub(crate) package_source_path: Option<String>,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupWorkflowRecipe {
    pub(crate) id: String,
    pub(crate) workflow_version_id: String,
    pub(crate) version: String,
    pub(crate) schema_version: i64,
    pub(crate) recipe_yaml: String,
    pub(crate) recipe_sha256: String,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupWorkflowRuntimeArtifact {
    pub(crate) id: String,
    pub(crate) workflow_version_id: String,
    pub(crate) recipe_id: String,
    pub(crate) package_name: String,
    pub(crate) source_kind: String,
    pub(crate) package_source_path: Option<String>,
    pub(crate) workflow_sha256: String,
    pub(crate) recipe_sha256: String,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupCharacterProfile {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) canonical_prompt: String,
    pub(crate) negative_prompt: String,
    pub(crate) default_style_profile_id: Option<String>,
    pub(crate) default_reference_set_id: Option<String>,
    pub(crate) active_revision_id: Option<String>,
    pub(crate) metadata_json: String,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupSceneProfile {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) environment_prompt: String,
    pub(crate) lighting_prompt: Option<String>,
    pub(crate) negative_prompt: Option<String>,
    pub(crate) default_style_profile_id: Option<String>,
    pub(crate) default_reference_set_id: Option<String>,
    pub(crate) active_revision_id: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupPropProfile {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) canonical_prompt: String,
    pub(crate) material_prompt: Option<String>,
    pub(crate) scale_prompt: Option<String>,
    pub(crate) default_reference_set_id: Option<String>,
    pub(crate) active_revision_id: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupStyleProfile {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) name: String,
    pub(crate) style_prompt: String,
    pub(crate) color_prompt: Option<String>,
    pub(crate) line_prompt: Option<String>,
    pub(crate) negative_prompt: Option<String>,
    pub(crate) output_notes: Option<String>,
    pub(crate) active_revision_id: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupCostumeVariant {
    pub(crate) id: String,
    pub(crate) character_profile_id: String,
    pub(crate) name: String,
    pub(crate) prompt_fragment: String,
    pub(crate) reference_set_id: Option<String>,
    pub(crate) is_default: i64,
    pub(crate) ordinal: i64,
    pub(crate) active_revision_id: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupProfileRevision {
    pub(crate) id: String,
    pub(crate) profile_type: String,
    pub(crate) profile_id: String,
    pub(crate) revision_number: i64,
    pub(crate) content_json: String,
    pub(crate) content_sha256: String,
    pub(crate) status: String,
    pub(crate) created_at: String,
    pub(crate) created_by: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupReferenceSet {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) name: String,
    pub(crate) purpose: String,
    pub(crate) description: String,
    pub(crate) owner_profile_type: Option<String>,
    pub(crate) owner_profile_id: Option<String>,
    pub(crate) active_revision_id: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupReferenceSetItem {
    pub(crate) reference_set_id: String,
    pub(crate) asset_id: String,
    pub(crate) ordinal: i64,
    pub(crate) role: Option<String>,
    pub(crate) is_primary: i64,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupShotProfileBinding {
    pub(crate) id: String,
    pub(crate) shot_id: String,
    pub(crate) role: String,
    pub(crate) profile_type: String,
    pub(crate) profile_id: String,
    pub(crate) costume_variant_id: Option<String>,
    pub(crate) ordinal: i64,
    pub(crate) inheritance_mode: String,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupShotReferenceSetBinding {
    pub(crate) id: String,
    pub(crate) shot_id: String,
    pub(crate) role: String,
    pub(crate) reference_set_id: String,
    pub(crate) ordinal: i64,
    pub(crate) required: i64,
    pub(crate) inheritance_mode: String,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupScopeProfileBinding {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) scope_type: String,
    pub(crate) scope_id: String,
    pub(crate) role: String,
    pub(crate) profile_type: String,
    pub(crate) profile_id: String,
    pub(crate) costume_variant_id: Option<String>,
    pub(crate) ordinal: i64,
    pub(crate) inheritance_mode: String,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupScopeReferenceSetBinding {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) scope_type: String,
    pub(crate) scope_id: String,
    pub(crate) role: String,
    pub(crate) reference_set_id: String,
    pub(crate) ordinal: i64,
    pub(crate) required: i64,
    pub(crate) inheritance_mode: String,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupAssetVersion {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) asset_id: String,
    pub(crate) version_number: i64,
    pub(crate) metadata_snapshot: Value,
    pub(crate) location: String,
    pub(crate) checksum: String,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupAssetRelation {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) source_asset_id: String,
    pub(crate) target_asset_id: String,
    pub(crate) relation_type: String,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupModel {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) provider: String,
    pub(crate) model_type: String,
    pub(crate) description: String,
    pub(crate) metadata_json: Value,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupModelVersion {
    pub(crate) id: String,
    pub(crate) model_id: String,
    pub(crate) version: String,
    pub(crate) capabilities_json: Value,
    pub(crate) parameter_schema_json: Value,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupTool {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) tool_type: String,
    pub(crate) description: String,
    pub(crate) metadata_json: Value,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupToolVersion {
    pub(crate) id: String,
    pub(crate) tool_id: String,
    pub(crate) version: String,
    pub(crate) observed_at: String,
    pub(crate) metadata_json: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupToolCapability {
    pub(crate) tool_id: String,
    pub(crate) capability_name: String,
    pub(crate) metadata_json: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupToolInstance {
    pub(crate) id: String,
    pub(crate) tool_id: String,
    pub(crate) path: Option<String>,
    pub(crate) endpoint: Option<String>,
    pub(crate) status: String,
    pub(crate) last_checked: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupGenerationToolUsage {
    pub(crate) id: String,
    pub(crate) generation_id: String,
    pub(crate) tool_instance_id: String,
    pub(crate) tool_version_id: Option<String>,
    pub(crate) metadata_json: Value,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupGenerationAssetVersion {
    pub(crate) id: String,
    pub(crate) generation_id: String,
    pub(crate) output_id: String,
    pub(crate) ordinal: i64,
    pub(crate) asset_version_id: String,
    pub(crate) relation_type: String,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupPromptEntry {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) kind: String,
    pub(crate) name: String,
    pub(crate) normalized_name: String,
    pub(crate) tags: Vec<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupPromptVersion {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) prompt_id: String,
    pub(crate) version: i64,
    pub(crate) text: String,
    #[serde(default)]
    pub(crate) model_version_id: Option<String>,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupAssetTag {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) name: String,
    pub(crate) normalized_name: String,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupAssetTagLink {
    pub(crate) asset_id: String,
    pub(crate) tag_id: String,
    pub(crate) project_id: String,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupAssetFavorite {
    pub(crate) asset_id: String,
    pub(crate) project_id: String,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupAssetVideoPrompt {
    pub(crate) asset_id: String,
    pub(crate) project_id: String,
    pub(crate) prompt_text: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupReferenceAnchor {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) kind: String,
    pub(crate) name: String,
    pub(crate) normalized_name: String,
    pub(crate) description: String,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    pub(crate) assets: Vec<BackupReferenceAnchorAsset>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupReferenceAnchorAsset {
    pub(crate) asset_id: String,
    pub(crate) ordinal: i64,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupProductionSeries {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) ordinal: i64,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupProductionEpisode {
    pub(crate) id: String,
    pub(crate) series_id: String,
    pub(crate) ordinal: i64,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupProductionScene {
    pub(crate) id: String,
    pub(crate) episode_id: String,
    pub(crate) ordinal: i64,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupShotSceneAssignment {
    pub(crate) shot_id: String,
    pub(crate) scene_id: String,
    pub(crate) ordinal: i64,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupScriptSource {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) format: String,
    pub(crate) original_filename: Option<String>,
    pub(crate) source_checksum: String,
    pub(crate) source_bytes: i64,
    pub(crate) source_text: String,
    pub(crate) schema_version: i64,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupScriptDraftRevision {
    pub(crate) id: String,
    pub(crate) draft_id: String,
    pub(crate) project_id: String,
    pub(crate) source_id: String,
    pub(crate) revision: i64,
    pub(crate) previous_revision_id: Option<String>,
    pub(crate) schema_version: i64,
    pub(crate) revision_kind: String,
    pub(crate) parser_version: String,
    pub(crate) contract_version: i64,
    pub(crate) provider_kind: Option<String>,
    pub(crate) provider_model: Option<String>,
    pub(crate) provider_metadata_json: Option<String>,
    pub(crate) payload_checksum: String,
    pub(crate) summary_json: String,
    pub(crate) payload_json: String,
    pub(crate) created_at: String,
}

#[derive(Clone, Default)]
pub(crate) struct ProductionStructureIds {
    pub(crate) series: HashMap<String, String>,
    pub(crate) episodes: HashMap<String, String>,
    pub(crate) scenes: HashMap<String, String>,
}

#[derive(Clone, Default)]
pub(crate) struct ConsistencyRestoreIds {
    pub(crate) profiles: HashMap<String, String>,
    pub(crate) costume_variants: HashMap<String, String>,
    pub(crate) profile_revisions: HashMap<String, String>,
    pub(crate) reference_sets: HashMap<String, String>,
    pub(crate) shot_profile_bindings: HashMap<String, String>,
    pub(crate) shot_reference_set_bindings: HashMap<String, String>,
    pub(crate) scope_profile_bindings: HashMap<String, String>,
    pub(crate) scope_reference_set_bindings: HashMap<String, String>,
}

fn consistency_required_id<'a>(
    ids: &'a HashMap<String, String>,
    old_id: &str,
    label: &str,
) -> Result<&'a str, AppError> {
    ids.get(old_id)
        .map(String::as_str)
        .ok_or_else(|| AppError::backup_invalid(format!("{} ID 映射缺失", label)))
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
) -> Result<String, AppError> {
    match scope_type {
        "PROJECT" => {
            if scope_id == source_project_id {
                Ok(restored_project_id.to_owned())
            } else {
                Err(AppError::backup_invalid("一致性 Scope 项目 ID 不匹配"))
            }
        }
        "SERIES" => consistency_required_id(&structure_ids.series, scope_id, "Scope Series")
            .map(str::to_owned),
        "EPISODE" => consistency_required_id(&structure_ids.episodes, scope_id, "Scope Episode")
            .map(str::to_owned),
        "SCENE" => consistency_required_id(&structure_ids.scenes, scope_id, "Scope Scene")
            .map(str::to_owned),
        _ => Err(AppError::backup_invalid("一致性 Scope 类型无效")),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupTask {
    pub(crate) id: String,
    pub(crate) workflow_id: String,
    pub(crate) workflow_version_id: String,
    pub(crate) recipe_id: String,
    pub(crate) app_version: Option<String>,
    pub(crate) build_commit: Option<String>,
    pub(crate) workflow_version: Option<String>,
    pub(crate) workflow_sha256: Option<String>,
    pub(crate) recipe_version: Option<String>,
    pub(crate) recipe_sha256: Option<String>,
    pub(crate) package_name: Option<String>,
    pub(crate) package_source_path: Option<String>,
    pub(crate) dynamic_binding_targets: Option<Value>,
    #[serde(default)]
    pub(crate) generation_execution_id: Option<String>,
    #[serde(default)]
    pub(crate) compiled_workflow_sha256: Option<String>,
    #[serde(default)]
    pub(crate) runtime_profile: Option<String>,
    #[serde(default)]
    pub(crate) concurrency_class: Option<String>,
    #[serde(default)]
    pub(crate) prepare_started_at: Option<String>,
    #[serde(default)]
    pub(crate) prepared_at: Option<String>,
    #[serde(default)]
    pub(crate) submitted_at: Option<String>,
    #[serde(default)]
    pub(crate) execution_started_at: Option<String>,
    #[serde(default)]
    pub(crate) execution_finished_at: Option<String>,
    #[serde(default)]
    pub(crate) collection_finished_at: Option<String>,
    pub(crate) status: String,
    pub(crate) prompt_id: Option<String>,
    pub(crate) queue_number: Option<i64>,
    pub(crate) progress_mode: String,
    pub(crate) progress_current: Option<i64>,
    pub(crate) progress_total: Option<i64>,
    pub(crate) current_node_id: Option<String>,
    pub(crate) error_code: Option<String>,
    pub(crate) error_message: Option<String>,
    pub(crate) raw_error: Option<Value>,
    pub(crate) created_at: String,
    pub(crate) queued_at: Option<String>,
    pub(crate) started_at: Option<String>,
    pub(crate) finished_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupTaskEvent {
    pub(crate) id: String,
    pub(crate) task_id: String,
    pub(crate) sequence: i64,
    pub(crate) event_type: String,
    pub(crate) payload: Option<Value>,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupAsset {
    pub(crate) id: String,
    pub(crate) asset_type: String,
    pub(crate) category: String,
    pub(crate) name: String,
    pub(crate) original_name: String,
    pub(crate) sha256: String,
    pub(crate) mime_type: String,
    pub(crate) width: i64,
    pub(crate) height: i64,
    pub(crate) duration_ms: Option<i64>,
    pub(crate) file_size: i64,
    pub(crate) source_task_id: Option<String>,
    pub(crate) metadata: Value,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    pub(crate) content_path: String,
    pub(crate) thumbnail_path: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupMapping {
    pub(crate) task_id: String,
    pub(crate) output_id: String,
    pub(crate) ordinal: i64,

    pub(crate) asset_id: String,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupSnapshot {
    pub(crate) id: String,
    pub(crate) task_id: String,
    pub(crate) workflow: Value,
    pub(crate) recipe_yaml: String,
    pub(crate) user_inputs: Value,
    pub(crate) resolved_inputs: Value,
    #[serde(default)]
    pub(crate) model_version_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) prompt_version_id: Option<String>,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupPreset {
    pub(crate) id: String,
    pub(crate) workflow_version_id: String,
    pub(crate) recipe_id: String,
    pub(crate) name: String,
    pub(crate) values: Value,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupBatch {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) continue_on_failure: i64,
    pub(crate) archived_at: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupBatchItem {
    pub(crate) id: String,
    pub(crate) batch_id: String,
    pub(crate) ordinal: i64,
    pub(crate) workflow_version_id: String,
    pub(crate) recipe_id: String,
    pub(crate) values: Value,
    pub(crate) status: String,
    pub(crate) task_id: Option<String>,
    pub(crate) retry_of_item_id: Option<String>,
    pub(crate) error_code: Option<String>,
    pub(crate) error_message: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupProductionPreparationSnapshot {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) shot_id: String,
    pub(crate) stage: String,
    pub(crate) context_hash: String,
    pub(crate) production_batch_id: String,
    pub(crate) production_batch_item_id: String,
    /// Immutable historical evidence. Runtime relations use the outer IDs above.
    pub(crate) snapshot_json: String,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProductionPreparationSnapshotV1 {
    pub(crate) schema_version: u32,
    pub(crate) project_id: String,
    pub(crate) shot_id: String,
    pub(crate) stage: String,
    pub(crate) context_hash: String,
    pub(crate) resolved_at: String,
    pub(crate) prepared_at: String,
    pub(crate) structure: Value,
    pub(crate) profiles: Value,
    pub(crate) reference_sets: Value,
    pub(crate) reference_assets: Value,
    pub(crate) prompt: Value,
    pub(crate) workflow: Value,
    pub(crate) output_spec: Value,
    pub(crate) stage_input: Value,
    pub(crate) frozen_generation_values: Value,
    pub(crate) readiness: Value,
    pub(crate) comfy_capability_evidence: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupBenchmarkExperiment {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) media_type: String,
    pub(crate) status: String,
    pub(crate) base_values: Value,
    pub(crate) asset_ids: Vec<String>,
    pub(crate) winner_candidate_id: Option<String>,
    pub(crate) production_batch_id: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupBenchmarkCandidate {
    pub(crate) id: String,
    pub(crate) experiment_id: String,
    pub(crate) position: i64,
    pub(crate) workflow_version_id: String,
    pub(crate) recipe_id: String,
    pub(crate) preset_id: Option<String>,
    pub(crate) preset_name: Option<String>,
    pub(crate) label: String,
    pub(crate) values: Value,
    pub(crate) asset_ids: Vec<String>,
    pub(crate) production_batch_item_id: Option<String>,
    pub(crate) task_id: Option<String>,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupProductionRun {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) current_stage_ordinal: i64,
    pub(crate) template_id: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    pub(crate) started_at: Option<String>,
    pub(crate) finished_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupProductionStage {
    pub(crate) id: String,
    pub(crate) run_id: String,
    pub(crate) ordinal: i64,
    pub(crate) stage_type: String,
    pub(crate) status: String,
    pub(crate) workflow_version_id: Option<String>,
    pub(crate) recipe_id: Option<String>,
    pub(crate) production_batch_id: Option<String>,
    pub(crate) frozen_config: Value,
    pub(crate) prompt: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    pub(crate) started_at: Option<String>,
    pub(crate) finished_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupProductionStageItem {
    pub(crate) id: String,
    pub(crate) stage_id: String,
    pub(crate) ordinal: i64,
    pub(crate) status: String,
    pub(crate) production_batch_item_id: Option<String>,
    pub(crate) task_id: Option<String>,
    pub(crate) asset_id: Option<String>,
    pub(crate) source_asset_id: Option<String>,
    pub(crate) reference_index: Option<i64>,
    pub(crate) attempt: i64,
    pub(crate) submission_idempotency_key: Option<String>,
    pub(crate) parent_stage_item_id: Option<String>,
    pub(crate) frozen_values: Value,
    pub(crate) error_code: Option<String>,
    pub(crate) error_message: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupProductionRunTemplate {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) name: String,
    pub(crate) krea2_workflow_version_id: Option<String>,
    pub(crate) krea2_recipe_id: Option<String>,
    pub(crate) krea2_preset_id: Option<String>,
    pub(crate) default_image_count: i64,
    pub(crate) h3_workflow_version_id: Option<String>,
    pub(crate) h3_recipe_id: Option<String>,
    pub(crate) h3_profile: Option<String>,
    pub(crate) default_duration_seconds: Option<i64>,
    pub(crate) default_width: Option<i64>,
    pub(crate) default_height: Option<i64>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupBenchmarkRun {
    pub(crate) id: String,
    pub(crate) experiment_id: String,
    pub(crate) candidate_id: String,
    pub(crate) run_number: i64,
    pub(crate) production_batch_item_id: Option<String>,
    pub(crate) task_id: Option<String>,
    pub(crate) snapshot_id: Option<String>,
    pub(crate) output_asset_id: Option<String>,
    pub(crate) generation_execution_id: Option<String>,
    pub(crate) compiled_workflow_sha256: Option<String>,
    pub(crate) runtime_profile: Option<String>,
    pub(crate) concurrency_class: Option<String>,
    pub(crate) queue_wait_ms: Option<i64>,
    pub(crate) prepare_ms: Option<i64>,
    pub(crate) submit_ms: Option<i64>,
    pub(crate) comfy_execution_ms: Option<i64>,
    pub(crate) collect_ms: Option<i64>,
    pub(crate) total_ms: Option<i64>,
    pub(crate) status: Option<String>,
    pub(crate) error_code: Option<String>,
    pub(crate) output_file_size: Option<i64>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupBenchmarkQualityScore {
    pub(crate) id: String,
    pub(crate) candidate_id: String,
    pub(crate) prompt_adherence: Option<i64>,
    pub(crate) visual_quality: Option<i64>,
    pub(crate) motion_quality: Option<i64>,
    pub(crate) reference_consistency: Option<i64>,
    pub(crate) overall: Option<i64>,
    pub(crate) note: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupProductionItemReview {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) production_batch_id: String,
    pub(crate) production_batch_item_id: String,
    pub(crate) task_id: Option<String>,
    pub(crate) result_asset_id: Option<String>,
    pub(crate) review_status: String,
    pub(crate) review_note: String,
    pub(crate) version: i64,
    pub(crate) lineage_key: String,
    pub(crate) parent_batch_id: Option<String>,
    pub(crate) parent_item_id: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupArtifactReview {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) artifact_id: String,
    pub(crate) decision: String,
    pub(crate) comment: String,
    pub(crate) revision: i64,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupShot {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) ordinal: i64,
    pub(crate) name: String,
    pub(crate) prompt_text: String,
    pub(crate) prompt_entry_id: Option<String>,
    pub(crate) prompt_version_id: Option<String>,
    pub(crate) selected_image_asset_id: Option<String>,
    pub(crate) selected_video_asset_id: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupShotStageConfig {
    pub(crate) shot_id: String,
    pub(crate) stage: String,
    pub(crate) workflow_id: String,
    pub(crate) workflow_version_id: String,
    pub(crate) recipe_id: String,
    pub(crate) scalar_values: Value,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupShotStagePrompt {
    pub(crate) shot_id: String,
    pub(crate) stage: String,
    pub(crate) prompt_text: String,
    pub(crate) prompt_entry_id: Option<String>,
    pub(crate) prompt_version_id: Option<String>,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupShotReferenceAsset {
    pub(crate) shot_id: String,
    pub(crate) stage: String,
    pub(crate) asset_id: String,
    pub(crate) ordinal: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupShotGenerationLink {
    pub(crate) id: String,
    pub(crate) shot_id: String,
    pub(crate) stage: String,
    pub(crate) task_id: Option<String>,
    pub(crate) production_batch_item_id: Option<String>,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupExternalProductionHandoff {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) schema_version: i64,
    pub(crate) source_agent: String,
    pub(crate) source_revision: Option<String>,
    pub(crate) document_sha256: String,
    pub(crate) imported_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupExternalProductionHandoffEntity {
    pub(crate) handoff_id: String,
    pub(crate) entity_kind: String,
    pub(crate) external_id: String,
    pub(crate) formal_entity_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkflowReference {
    pub(crate) workflow_id: String,
    pub(crate) workflow_version_id: String,
    pub(crate) recipe_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupProjectWorkflowBinding {
    pub(crate) stage: String,
    pub(crate) mode: String,
    pub(crate) workflow_version_id: String,
    pub(crate) recipe_id: String,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    #[serde(default, skip_serializing)]
    pub(crate) workflow_id: Option<String>,
}

#[derive(Clone)]
pub(crate) struct RestoredAsset {
    pub(crate) old_id: String,
    pub(crate) new_id: String,
    pub(crate) storage_path: String,
    pub(crate) thumbnail_path: Option<String>,
}

fn remap_reference_anchor_assets(
    anchor: &BackupReferenceAnchor,
    asset_ids: &HashMap<String, String>,
) -> Result<Vec<BackupReferenceAnchorAsset>, AppError> {
    anchor
        .assets
        .iter()
        .map(|asset| {
            Ok(BackupReferenceAnchorAsset {
                asset_id: asset_ids
                    .get(&asset.asset_id)
                    .cloned()
                    .ok_or_else(|| AppError::backup_invalid("参考锚点素材缺少映射"))?,
                ordinal: asset.ordinal,
                created_at: asset.created_at.clone(),
            })
        })
        .collect()
}

impl BackupTask {
    pub(crate) fn is_terminal(&self) -> bool {
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

const STREAM_CHUNK_BYTES: usize = 1024 * 1024;

fn write_zip_to_path(
    document: &BackupDocument,
    files: &[BackupFileSource],
    destination: &Path,
) -> Result<(), AppError> {
    if files.len().saturating_add(PACKAGE_JSON_ENTRIES) > MAX_ENTRIES {
        return Err(AppError::backup_invalid("备份文件数量超过限制"));
    }
    let project_json_bytes = serde_json::to_vec_pretty(document)
        .map_err(|error| AppError::backup_invalid(format!("备份 JSON 生成失败：{error}")))?;
    let logical_snapshot_checksum = hash_bytes(&project_json_bytes);
    let media_inventory = files
        .iter()
        .filter_map(|file| {
            file.expected_sha256
                .as_ref()
                .map(|sha256| BackupMediaInventoryEntry {
                    path: file.zip_path.clone(),
                    size: file.expected_size,
                    sha256: sha256.clone(),
                })
        })
        .collect::<Vec<_>>();
    let provenance = build_provenance_document(document);
    let manifest = ProjectBackupManifest {
        format: BACKUP_FORMAT.to_owned(),
        version: BACKUP_VERSION,
        created_by: env!("CARGO_PKG_VERSION").to_owned(),
        project: document.project.clone(),
        inventory: Some(build_inventory_counts(document)),
        logical_snapshot_checksum: Some(logical_snapshot_checksum),
        media_inventory,
    };
    let file = File::create(destination)
        .map_err(|error| AppError::filesystem(format!("备份临时文件创建失败：{error}")))?;
    let mut writer = ZipWriter::new(BufWriter::with_capacity(STREAM_CHUNK_BYTES, file));
    let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
    write_zip_json(&mut writer, "manifest.json", &manifest, options)?;
    write_zip_bytes(&mut writer, "project.json", &project_json_bytes, options)?;
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
    write_zip_json(
        &mut writer,
        "production_preparation_snapshots.json",
        &document.preparation_snapshots,
        options,
    )?;
    write_zip_json(&mut writer, "provenance.json", &provenance, options)?;
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

fn build_inventory_counts(document: &BackupDocument) -> BackupInventoryCounts {
    BackupInventoryCounts {
        assets: document.assets.len(),
        asset_versions: document.asset_versions.len(),
        relations: document.asset_relations.len(),
        prompts: document.prompt_entries.len(),
        models: document.models.len(),
        tools: document.tools.len(),
        lineage: document
            .generation_tool_usages
            .len()
            .saturating_add(document.generation_asset_versions.len()),
        tasks: document.tasks.len(),
        artifact_reviews: document.artifact_reviews.len(),
    }
}

fn build_provenance_document(document: &BackupDocument) -> BackupProvenanceDocument {
    let mut generation_tool_usages = document.generation_tool_usages.clone();
    let mut generation_asset_versions = document.generation_asset_versions.clone();
    generation_tool_usages.sort_by(|a, b| a.id.cmp(&b.id));
    generation_asset_versions.sort_by(|a, b| a.id.cmp(&b.id));
    BackupProvenanceDocument {
        generation_tool_usages,
        generation_asset_versions,
    }
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
    write_zip_bytes(writer, path, &bytes, options)
}

fn write_zip_bytes<W: Write + io::Seek>(
    writer: &mut ZipWriter<W>,
    path: &str,
    bytes: &[u8],
    options: FileOptions,
) -> Result<(), AppError> {
    writer
        .start_file(path, options)
        .map_err(|error| AppError::backup_invalid(format!("备份目录写入失败：{error}")))?;
    writer
        .write_all(bytes)
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
        || !matches!(
            manifest.version,
            1 | 2
                | 3
                | 4
                | 5
                | 6
                | 7
                | 8
                | 9
                | 10
                | 11
                | 12
                | 13
                | 14
                | 15
                | 16
                | 17
                | 18
                | 19
                | 20
        )
    {
        return Err(AppError::backup_invalid("备份格式或版本不受支持"));
    }
    let project_json_bytes = read_zip_bytes_entry(&mut archive, "project.json")?;
    let document: BackupDocument = serde_json::from_slice(&project_json_bytes)
        .map_err(|error| AppError::backup_invalid(format!("备份 JSON 无效：{error}")))?;
    if document.project.id != manifest.project.id || document.project.name != manifest.project.name
    {
        return Err(AppError::backup_invalid("备份项目元数据与 manifest 不一致"));
    }
    validate_manifest_integrity(
        &mut archive,
        &manifest,
        &document,
        &project_json_bytes,
        &names,
    )?;
    Ok((manifest, document, names))
}

fn validate_manifest_integrity(
    archive: &mut ZipArchive<File>,
    manifest: &ProjectBackupManifest,
    document: &BackupDocument,
    project_json_bytes: &[u8],
    entry_names: &HashSet<String>,
) -> Result<(), AppError> {
    if let Some(expected) = &manifest.logical_snapshot_checksum {
        let actual = hash_bytes(project_json_bytes);
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(AppError::backup_invalid("逻辑快照校验值不匹配"));
        }
    }
    if let Some(inventory) = &manifest.inventory {
        let expected = build_inventory_counts(document);
        if inventory != &expected {
            return Err(AppError::backup_invalid("备份清单计数与逻辑快照不一致"));
        }
    }
    for media in &manifest.media_inventory {
        if !safe_zip_path(&media.path) || !entry_names.contains(&media.path) {
            return Err(AppError::backup_invalid(format!(
                "备份媒体清单缺少文件：{}",
                media.path
            )));
        }
        let (size, sha256) = hash_zip_entry(archive, &media.path)?;
        if size != media.size || !sha256.eq_ignore_ascii_case(&media.sha256) {
            return Err(AppError::backup_asset_hash_mismatch(format!(
                "备份媒体校验值不匹配：{}",
                media.path
            )));
        }
    }
    // provenance.json is optional for historical v18 / early v19 packages.
    if entry_names.contains("provenance.json") {
        let provenance: BackupProvenanceDocument = read_zip_json(archive, "provenance.json")?;
        if provenance.generation_tool_usages.len() != document.generation_tool_usages.len()
            || provenance.generation_asset_versions.len()
                != document.generation_asset_versions.len()
        {
            return Err(AppError::backup_invalid(
                "provenance.json 与 project.json 溯源条数不一致",
            ));
        }
    }
    Ok(())
}

fn hash_zip_entry(archive: &mut ZipArchive<File>, path: &str) -> Result<(u64, String), AppError> {
    let mut entry = archive
        .by_name(path)
        .map_err(|_| AppError::backup_invalid(format!("备份缺少 {path}")))?;
    if entry.size() > MAX_ENTRY_BYTES {
        return Err(AppError::backup_invalid("备份条目超过单文件大小限制"));
    }
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; STREAM_CHUNK_BYTES];
    let mut total = 0_u64;
    loop {
        let read = entry
            .read(&mut buffer)
            .map_err(|error| AppError::backup_invalid(format!("备份媒体读取失败：{error}")))?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or_else(|| AppError::backup_invalid("备份媒体大小溢出"))?;
        if total > MAX_ENTRY_BYTES {
            return Err(AppError::backup_invalid("备份条目超过单文件大小限制"));
        }
        hasher.update(&buffer[..read]);
    }
    let sha256 = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok((total, sha256))
}

fn read_zip_bytes_entry(archive: &mut ZipArchive<File>, path: &str) -> Result<Vec<u8>, AppError> {
    let entry = archive
        .by_name(path)
        .map_err(|_| AppError::backup_invalid(format!("备份缺少 {path}")))?;
    let mut bytes = Vec::new();
    entry
        .take(MAX_ENTRY_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|error| AppError::backup_invalid(format!("备份 JSON 读取失败：{error}")))?;
    Ok(bytes)
}

fn read_zip_json<T: for<'de> Deserialize<'de>>(
    archive: &mut ZipArchive<File>,
    path: &str,
) -> Result<T, AppError> {
    let bytes = read_zip_bytes_entry(archive, path)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| AppError::backup_invalid(format!("备份 JSON 无效：{error}")))
}

fn validate_document_entries(
    document: &BackupDocument,
    entry_names: &HashSet<String>,
    version: u32,
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
    validate_project_workflow_binding_document(document, version)?;
    validate_workflow_registry_document(document, version)?;
    validate_production_item_review_document(document)?;
    validate_artifact_review_document(document)?;
    validate_organization_document(document)?;
    validate_reference_anchor_document(document)?;
    validate_consistency_document(document, version)?;
    validate_production_structure_document(document, version)?;
    validate_external_production_handoff_document(document, version)?;
    validate_production_preparation_snapshot_document(document, version)?;
    validate_prompt_document(document)?;
    validate_benchmark_document(document)?;
    validate_production_orchestrator_document(document)?;
    validate_shot_document(document, version)?;
    validate_script_draft_document(document, version)?;
    Ok(())
}

fn validate_workflow_registry_document(
    document: &BackupDocument,
    version: u32,
) -> Result<(), AppError> {
    let Some(registry) = &document.workflow_registry else {
        return Ok(());
    };
    if version < 17 {
        if !registry.workflows.is_empty()
            || !registry.versions.is_empty()
            || !registry.recipes.is_empty()
            || !registry.runtime_artifacts.is_empty()
        {
            return Err(AppError::backup_invalid(
                "旧版备份不应包含 Workflow Registry 数据",
            ));
        }
        return Ok(());
    }

    let mut workflow_ids = HashSet::new();
    let mut workflow_versions = HashMap::new();
    for workflow in &registry.workflows {
        if workflow.id.trim().is_empty()
            || !workflow_ids.insert(workflow.id.as_str())
            || workflow.name.trim().is_empty()
            || !matches!(workflow.source_kind.as_str(), "PRODUCT" | "USER")
            || !matches!(workflow.library_state.as_str(), "ACTIVE" | "REMOVED")
            || workflow.created_at.trim().is_empty()
            || workflow.updated_at.trim().is_empty()
        {
            return Err(AppError::backup_invalid(
                "Workflow Registry 逻辑工作流数据无效",
            ));
        }
    }

    for workflow_version in &registry.versions {
        if workflow_version.id.trim().is_empty()
            || workflow_version.workflow_id.trim().is_empty()
            || !workflow_ids.contains(workflow_version.workflow_id.as_str())
            || workflow_version.version.trim().is_empty()
            || workflow_version.api_workflow_json.trim().is_empty()
            || workflow_version.workflow_sha256.trim().is_empty()
            || workflow_version.created_at.trim().is_empty()
            || workflow_versions
                .insert(workflow_version.id.as_str(), workflow_version)
                .is_some()
        {
            return Err(AppError::backup_invalid("Workflow Registry 版本数据无效"));
        }
    }

    for workflow in &registry.workflows {
        if let Some(current_version_id) = workflow.current_version_id.as_deref() {
            let Some(current_version) = workflow_versions.get(current_version_id) else {
                return Err(AppError::backup_invalid(
                    "Workflow Registry currentVersionId 不存在",
                ));
            };
            if current_version.workflow_id != workflow.id {
                return Err(AppError::backup_invalid(
                    "Workflow Registry currentVersionId 跨 Workflow",
                ));
            }
        }
    }

    let mut recipe_ids = HashMap::new();
    for recipe in &registry.recipes {
        if recipe.id.trim().is_empty()
            || recipe.workflow_version_id.trim().is_empty()
            || !workflow_versions.contains_key(recipe.workflow_version_id.as_str())
            || recipe.version.trim().is_empty()
            || recipe.schema_version <= 0
            || recipe.recipe_yaml.trim().is_empty()
            || recipe.recipe_sha256.trim().is_empty()
            || recipe.created_at.trim().is_empty()
            || recipe_ids.insert(recipe.id.as_str(), recipe).is_some()
        {
            return Err(AppError::backup_invalid(
                "Workflow Registry Recipe 数据无效",
            ));
        }
    }

    let mut artifact_ids = HashSet::new();
    let mut package_names = HashSet::new();
    let mut artifact_keys = HashSet::new();
    for artifact in &registry.runtime_artifacts {
        let Some(workflow_version) = workflow_versions.get(artifact.workflow_version_id.as_str())
        else {
            return Err(AppError::backup_invalid(
                "Workflow Runtime Artifact 引用了不存在的 WorkflowVersion",
            ));
        };
        let Some(recipe) = recipe_ids.get(artifact.recipe_id.as_str()) else {
            return Err(AppError::backup_invalid(
                "Workflow Runtime Artifact 引用了不存在的 Recipe",
            ));
        };
        if recipe.workflow_version_id != artifact.workflow_version_id
            || artifact.id.trim().is_empty()
            || !artifact_ids.insert(artifact.id.as_str())
            || artifact.package_name.trim().is_empty()
            || !package_names.insert(artifact.package_name.as_str())
            || !artifact_keys.insert((
                artifact.workflow_version_id.as_str(),
                artifact.recipe_id.as_str(),
                artifact.package_name.as_str(),
            ))
            || !matches!(artifact.source_kind.as_str(), "PRODUCT" | "USER")
            || artifact.workflow_sha256 != workflow_version.workflow_sha256
            || artifact.recipe_sha256 != recipe.recipe_sha256
            || artifact.created_at.trim().is_empty()
        {
            return Err(AppError::backup_invalid(
                "Workflow Runtime Artifact 精确映射数据无效",
            ));
        }
    }

    Ok(())
}

fn validate_project_workflow_binding_document(
    document: &BackupDocument,
    version: u32,
) -> Result<(), AppError> {
    if version < 16 {
        if !document.project_workflow_bindings.is_empty() {
            return Err(AppError::backup_invalid(
                "旧版备份不应包含项目工作流绑定数据",
            ));
        }
        return Ok(());
    }

    let mut keys = HashSet::new();
    for binding in &document.project_workflow_bindings {
        if binding.stage != "IMAGE" && binding.stage != "VIDEO" {
            return Err(AppError::backup_invalid("项目工作流绑定 stage 无效"));
        }
        let valid_mode = matches!(
            binding.mode.as_str(),
            "DEFAULT"
                | "FL2VA_TEXT_TO_VIDEO"
                | "FL2VA_IMAGE_TO_VIDEO"
                | "FL2VA_FIRST_LAST"
                | "REF2VA_IMAGE"
                | "REF2VA_AUDIO"
                | "REF2VA_IMAGE_AUDIO"
                | "REF2VA_VIDEO_IMAGE"
        );
        if !valid_mode
            || (binding.stage == "IMAGE" && binding.mode != "DEFAULT")
            || binding.workflow_version_id.trim().is_empty()
            || binding.recipe_id.trim().is_empty()
            || !keys.insert((binding.stage.as_str(), binding.mode.as_str()))
        {
            return Err(AppError::backup_invalid("项目工作流绑定数据无效"));
        }
    }
    Ok(())
}

fn validate_script_draft_document(document: &BackupDocument, version: u32) -> Result<(), AppError> {
    if version < 15 {
        if !document.script_sources.is_empty() || !document.script_draft_revisions.is_empty() {
            return Err(AppError::backup_invalid(
                "旧版备份不应包含 Script/Draft 数据",
            ));
        }
        return Ok(());
    }

    let mut source_ids = HashSet::new();
    for source in &document.script_sources {
        if !source_ids.insert(source.id.as_str())
            || source.project_id != document.project.id
            || source.id.trim().is_empty()
            || !matches!(source.format.as_str(), "TXT" | "MARKDOWN" | "JSON")
            || !is_lower_hex_64(&source.source_checksum)
            || source.source_bytes < 0
            || source.source_bytes as usize != source.source_text.len()
            || source.schema_version <= 0
        {
            return Err(AppError::backup_invalid("Script Source 元数据无效"));
        }
    }

    let mut revision_ids = HashSet::new();
    let mut draft_revisions = HashMap::new();
    for revision in &document.script_draft_revisions {
        if !revision_ids.insert(revision.id.as_str())
            || revision.id.trim().is_empty()
            || revision.draft_id.trim().is_empty()
            || revision.project_id != document.project.id
            || !source_ids.contains(revision.source_id.as_str())
            || revision.revision <= 0
            || revision.schema_version <= 0
            || revision.contract_version <= 0
            || !matches!(
                revision.revision_kind.as_str(),
                "PARSED" | "REPARSED" | "USER_EDIT" | "REVIEW" | "MERGE" | "SPLIT" | "REORDER"
            )
            || revision.parser_version.trim().is_empty()
            || revision
                .provider_kind
                .as_deref()
                .map(str::trim)
                .is_some_and(str::is_empty)
            || !is_lower_hex_64(&revision.payload_checksum)
            || serde_json::from_str::<Value>(&revision.summary_json).is_err()
            || serde_json::from_str::<Value>(&revision.payload_json).is_err()
            || hash_bytes(revision.payload_json.as_bytes()) != revision.payload_checksum
        {
            return Err(AppError::backup_invalid("Script Draft Revision 数据无效"));
        }
        if let Some(provider_metadata) = &revision.provider_metadata_json {
            if serde_json::from_str::<Value>(provider_metadata).is_err() {
                return Err(AppError::backup_invalid(
                    "Script Draft Provider metadata 无效",
                ));
            }
        }
        draft_revisions.insert(revision.id.as_str(), revision);
    }
    let mut draft_identity_revisions: HashMap<&str, HashSet<i64>> = HashMap::new();
    for revision in &document.script_draft_revisions {
        let numbers = draft_identity_revisions
            .entry(revision.draft_id.as_str())
            .or_default();
        if !numbers.insert(revision.revision) {
            return Err(AppError::backup_invalid(
                "Script Draft revision number 重复",
            ));
        }
        match (&revision.previous_revision_id, revision.revision) {
            (None, 1) => {}
            (Some(_), 1) => {
                return Err(AppError::backup_invalid(
                    "Script Draft revision 1 不应包含 previous revision",
                ));
            }
            (None, _) => {
                return Err(AppError::backup_invalid(
                    "Script Draft revision 缺少 immediate previous revision",
                ));
            }
            (Some(previous_id), _) => {
                let Some(previous) = draft_revisions.get(previous_id.as_str()) else {
                    return Err(AppError::backup_invalid(
                        "Script Draft previous revision 引用无效",
                    ));
                };
                if previous.draft_id != revision.draft_id
                    || previous.project_id != revision.project_id
                    || previous.revision != revision.revision - 1
                {
                    return Err(AppError::backup_invalid(
                        "Script Draft previous revision 链接无效",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn is_lower_hex_64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn remap_exact_string_ids(value: &mut Value, ids: &HashMap<String, String>) {
    match value {
        Value::String(text) => {
            if let Some(restored) = ids.get(text) {
                *text = restored.clone();
            }
        }
        Value::Array(values) => {
            for value in values {
                remap_exact_string_ids(value, ids);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                remap_exact_string_ids(value, ids);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn validate_production_preparation_snapshot_document(
    document: &BackupDocument,
    version: u32,
) -> Result<(), AppError> {
    if version < 14 {
        if !document.preparation_snapshots.is_empty() {
            return Err(AppError::backup_invalid(
                "旧版备份不应包含 Production Preparation Snapshot",
            ));
        }
        return Ok(());
    }

    let mut snapshot_ids = HashSet::new();
    for snapshot in &document.preparation_snapshots {
        if !snapshot_ids.insert(snapshot.id.as_str())
            || snapshot.project_id != document.project.id
            || snapshot.id.trim().is_empty()
            || snapshot.shot_id.trim().is_empty()
            || snapshot.context_hash.trim().is_empty()
            || snapshot.production_batch_id.trim().is_empty()
            || snapshot.production_batch_item_id.trim().is_empty()
            || !matches!(snapshot.stage.as_str(), "image" | "video")
        {
            return Err(AppError::backup_invalid(
                "Production Preparation Snapshot 外层关系无效",
            ));
        }
        if !document
            .shots
            .iter()
            .any(|shot| shot.id == snapshot.shot_id && shot.project_id == document.project.id)
        {
            return Err(AppError::backup_invalid(
                "Production Preparation Snapshot 镜头引用无效",
            ));
        }
        if !document
            .batches
            .iter()
            .any(|batch| batch.id == snapshot.production_batch_id)
        {
            return Err(AppError::backup_invalid(
                "Production Preparation Snapshot 批次引用无效",
            ));
        }
        if !document.items.iter().any(|item| {
            item.id == snapshot.production_batch_item_id
                && item.batch_id == snapshot.production_batch_id
        }) {
            return Err(AppError::backup_invalid(
                "Production Preparation Snapshot 批次项目引用无效",
            ));
        }
        validate_production_preparation_snapshot_json(&snapshot.snapshot_json)?;
    }
    Ok(())
}

fn validate_production_preparation_snapshot_json(snapshot_json: &str) -> Result<(), AppError> {
    let snapshot = serde_json::from_str::<ProductionPreparationSnapshotV1>(snapshot_json).map_err(
        |error| {
            AppError::backup_invalid(format!(
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
        return Err(AppError::backup_invalid(
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
            return Err(AppError::backup_invalid(
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
            return Err(AppError::backup_invalid(
                "Production Preparation Snapshot referenceAsset 字段无效",
            ));
        }
    }
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

fn validate_artifact_review_document(document: &BackupDocument) -> Result<(), AppError> {
    let artifact_ids = document
        .mappings
        .iter()
        .map(|mapping| mapping.asset_id.as_str())
        .collect::<HashSet<_>>();
    let mut review_ids = HashSet::new();
    let mut reviewed_artifacts = HashSet::new();
    for review in &document.artifact_reviews {
        if review.project_id != document.project.id
            || review.id.trim().is_empty()
            || !review_ids.insert(review.id.as_str())
            || !reviewed_artifacts.insert(review.artifact_id.as_str())
            || !artifact_ids.contains(review.artifact_id.as_str())
            || !matches!(
                review.decision.as_str(),
                "PENDING" | "APPROVED" | "REJECTED"
            )
            || review.revision < 0
            || review.comment.as_bytes().len() > 4 * 1024
        {
            return Err(AppError::backup_invalid(
                "备份产物审核状态无效或引用了未知产物",
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

fn validate_shot_document(document: &BackupDocument, version: u32) -> Result<(), AppError> {
    use crate::application::prompt_library_service::canonical_prompt_text;
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

    let mut stage_prompts = HashSet::new();
    let mut stage_prompt_counts = HashMap::<&str, usize>::new();
    for prompt in &document.shot_stage_prompts {
        if !shot_ids.contains(prompt.shot_id.as_str())
            || ShotStage::try_from_str(&prompt.stage).is_err()
            || !stage_prompts.insert((prompt.shot_id.as_str(), prompt.stage.as_str()))
        {
            return Err(AppError::backup_invalid("备份镜头阶段 Prompt 无效或重复"));
        }
        let canonical = canonical_prompt_text(&prompt.prompt_text)
            .map_err(|error| AppError::backup_invalid(format!("阶段 Prompt 无效：{error}")))?;
        if canonical != prompt.prompt_text {
            return Err(AppError::backup_invalid("备份镜头阶段 Prompt 未规范化"));
        }
        if prompt.prompt_entry_id.is_some() != prompt.prompt_version_id.is_some() {
            return Err(AppError::backup_invalid(
                "备份镜头阶段 Prompt provenance 不完整",
            ));
        }
        if let (Some(entry_id), Some(version_id)) =
            (&prompt.prompt_entry_id, &prompt.prompt_version_id)
        {
            if !prompt_entry_ids.contains(entry_id.as_str())
                || prompt_versions.get(version_id.as_str()).copied() != Some(entry_id.as_str())
            {
                return Err(AppError::backup_invalid(
                    "备份镜头阶段 Prompt provenance 引用无效",
                ));
            }
        }
        *stage_prompt_counts
            .entry(prompt.shot_id.as_str())
            .or_default() += 1;
    }
    if version >= 10
        && document
            .shots
            .iter()
            .any(|shot| stage_prompt_counts.get(shot.id.as_str()).copied() != Some(2))
    {
        return Err(AppError::backup_invalid(
            "v10 备份必须为每个镜头保存 image/video 阶段 Prompt",
        ));
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
    let prompt_version_ids = document
        .prompt_versions
        .iter()
        .map(|version| version.id.as_str())
        .collect::<HashSet<_>>();
    for snapshot in &document.snapshots {
        if snapshot
            .prompt_version_id
            .as_ref()
            .is_some_and(|id| !prompt_version_ids.contains(id.as_str()))
        {
            return Err(AppError::backup_invalid("备份生成快照引用了未知提示词版本"));
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

fn validate_reference_anchor_document(document: &BackupDocument) -> Result<(), AppError> {
    const MAX_ANCHORS: usize = 1000;
    const MAX_ASSETS_PER_ANCHOR: usize = 20;
    let asset_types = document
        .assets
        .iter()
        .map(|asset| (asset.id.as_str(), asset.asset_type.as_str()))
        .collect::<HashMap<_, _>>();
    if document.reference_anchors.len() > MAX_ANCHORS {
        return Err(AppError::backup_invalid("备份参考锚点数量超过限制"));
    }
    let mut anchor_ids = HashSet::new();
    let mut names = HashSet::new();
    for anchor in &document.reference_anchors {
        if anchor.project_id != document.project.id
            || anchor.id.trim().is_empty()
            || !anchor_ids.insert(anchor.id.as_str())
            || !matches!(
                anchor.kind.as_str(),
                "CHARACTER" | "SCENE" | "PROP" | "STYLE"
            )
            || anchor.description.chars().count() > 500
        {
            return Err(AppError::backup_invalid("备份参考锚点元数据无效"));
        }
        let (canonical_name, normalized_name) =
            crate::application::organization_service::normalize_name(
                &anchor.name,
                80,
                "REFERENCE_ANCHOR",
            )
            .map_err(|error| AppError::backup_invalid(format!("参考锚点名称无效：{error}")))?;
        if anchor.name != canonical_name || anchor.normalized_name != normalized_name {
            return Err(AppError::backup_invalid("备份参考锚点名称未规范化"));
        }
        if !names.insert((anchor.kind.as_str(), anchor.normalized_name.as_str())) {
            return Err(AppError::backup_invalid("备份包含重复参考锚点名称"));
        }
        if anchor.assets.len() > MAX_ASSETS_PER_ANCHOR {
            return Err(AppError::backup_invalid("参考锚点素材数量超过 20 个上限"));
        }
        let mut asset_ids = HashSet::new();
        let mut ordinals = Vec::with_capacity(anchor.assets.len());
        for asset in &anchor.assets {
            if !asset_ids.insert(asset.asset_id.as_str())
                || asset_types.get(asset.asset_id.as_str()).copied() != Some("image")
                || asset.ordinal < 0
            {
                return Err(AppError::backup_invalid("参考锚点素材引用无效"));
            }
            ordinals.push(asset.ordinal);
        }
        ordinals.sort_unstable();
        if ordinals
            .iter()
            .enumerate()
            .any(|(index, ordinal)| *ordinal != index as i64)
        {
            return Err(AppError::backup_invalid(
                "参考锚点素材序号必须从 0 连续排列",
            ));
        }
    }
    Ok(())
}

fn validate_consistency_document(document: &BackupDocument, version: u32) -> Result<(), AppError> {
    let has_consistency_data = !document.character_profiles.is_empty()
        || !document.scene_profiles.is_empty()
        || !document.prop_profiles.is_empty()
        || !document.style_profiles.is_empty()
        || !document.costume_variants.is_empty()
        || !document.profile_revisions.is_empty()
        || !document.reference_sets.is_empty()
        || !document.reference_set_items.is_empty()
        || !document.shot_profile_bindings.is_empty()
        || !document.shot_reference_set_bindings.is_empty()
        || !document.scope_profile_bindings.is_empty()
        || !document.scope_reference_set_bindings.is_empty();
    if version < 13 {
        if has_consistency_data {
            return Err(AppError::backup_invalid("一致性资产数据需要 Backup v13"));
        }
        return Ok(());
    }

    let mut profile_types = HashMap::<&str, &str>::new();
    let mut profile_names = HashSet::<String>::new();
    for profile in &document.character_profiles {
        register_consistency_profile(
            &mut profile_types,
            &mut profile_names,
            &profile.id,
            &profile.project_id,
            &profile.name,
            "CHARACTER",
        )?;
        if !valid_consistency_text(&profile.description, 4_000)
            || !valid_consistency_text(&profile.canonical_prompt, 20_000)
            || !valid_consistency_text(&profile.negative_prompt, 20_000)
            || !valid_consistency_metadata(&profile.metadata_json)
        {
            return Err(AppError::backup_invalid("Character Profile 内容无效"));
        }
    }
    for profile in &document.scene_profiles {
        register_consistency_profile(
            &mut profile_types,
            &mut profile_names,
            &profile.id,
            &profile.project_id,
            &profile.name,
            "SCENE",
        )?;
        if !valid_consistency_text(&profile.description, 4_000)
            || !valid_consistency_text(&profile.environment_prompt, 20_000)
            || !valid_consistency_optional_text(profile.lighting_prompt.as_deref(), 20_000)
            || !valid_consistency_optional_text(profile.negative_prompt.as_deref(), 20_000)
        {
            return Err(AppError::backup_invalid("Scene Profile 内容无效"));
        }
    }
    for profile in &document.prop_profiles {
        register_consistency_profile(
            &mut profile_types,
            &mut profile_names,
            &profile.id,
            &profile.project_id,
            &profile.name,
            "PROP",
        )?;
        if !valid_consistency_text(&profile.description, 4_000)
            || !valid_consistency_text(&profile.canonical_prompt, 20_000)
            || !valid_consistency_optional_text(profile.material_prompt.as_deref(), 20_000)
            || !valid_consistency_optional_text(profile.scale_prompt.as_deref(), 20_000)
        {
            return Err(AppError::backup_invalid("Prop Profile 内容无效"));
        }
    }
    for profile in &document.style_profiles {
        register_consistency_profile(
            &mut profile_types,
            &mut profile_names,
            &profile.id,
            &profile.project_id,
            &profile.name,
            "STYLE",
        )?;
        if !valid_consistency_text(&profile.style_prompt, 20_000)
            || !valid_consistency_optional_text(profile.color_prompt.as_deref(), 20_000)
            || !valid_consistency_optional_text(profile.line_prompt.as_deref(), 20_000)
            || !valid_consistency_optional_text(profile.negative_prompt.as_deref(), 20_000)
            || !valid_consistency_optional_text(profile.output_notes.as_deref(), 20_000)
        {
            return Err(AppError::backup_invalid("Style Profile 内容无效"));
        }
    }

    let mut reference_set_purposes = HashMap::<&str, &str>::new();
    let mut reference_set_names = HashSet::<String>::new();
    for reference_set in &document.reference_sets {
        if reference_set.project_id != document.project.id
            || !valid_consistency_id(&reference_set.id)
            || !valid_consistency_name(&reference_set.name)
            || !valid_consistency_text(&reference_set.description, 4_000)
            || reference_set_purpose(&reference_set.purpose).is_none()
            || reference_set_purposes
                .insert(&reference_set.id, &reference_set.purpose)
                .is_some()
            || !reference_set_names.insert(format!(
                "{}:{}",
                reference_set.purpose.to_ascii_uppercase(),
                reference_set.name.to_lowercase()
            ))
        {
            return Err(AppError::backup_invalid("Reference Set 元数据无效"));
        }
        let owner_pair_valid = match (
            reference_set.owner_profile_type.as_deref(),
            reference_set.owner_profile_id.as_deref(),
        ) {
            (None, None) => true,
            (Some(profile_type), Some(profile_id)) => {
                let Some(expected_type) = reference_set_owner_type(&reference_set.purpose) else {
                    return Err(AppError::backup_invalid(
                        "SHOT Reference Set 不能拥有 Profile",
                    ));
                };
                ProfileType::try_from_db(profile_type).is_ok()
                    && profile_type == expected_type
                    && profile_types.get(profile_id).copied() == Some(profile_type)
            }
            _ => false,
        };
        if !owner_pair_valid {
            return Err(AppError::backup_invalid("Reference Set Owner 无效"));
        }
    }
    for profile in &document.character_profiles {
        validate_consistency_profile_relation(
            "CHARACTER",
            &profile.id,
            profile.default_style_profile_id.as_ref(),
            profile.default_reference_set_id.as_ref(),
            &profile_types,
            &reference_set_purposes,
        )?;
    }
    for profile in &document.scene_profiles {
        validate_consistency_profile_relation(
            "SCENE",
            &profile.id,
            profile.default_style_profile_id.as_ref(),
            profile.default_reference_set_id.as_ref(),
            &profile_types,
            &reference_set_purposes,
        )?;
    }
    for profile in &document.prop_profiles {
        validate_consistency_profile_relation(
            "PROP",
            &profile.id,
            None,
            profile.default_reference_set_id.as_ref(),
            &profile_types,
            &reference_set_purposes,
        )?;
    }
    for profile in &document.style_profiles {
        validate_consistency_profile_relation(
            "STYLE",
            &profile.id,
            None,
            None,
            &profile_types,
            &reference_set_purposes,
        )?;
    }

    let mut costume_character_ids = HashMap::<&str, &str>::new();
    for variant in &document.costume_variants {
        if !valid_consistency_id(&variant.id)
            || costume_character_ids
                .insert(&variant.id, &variant.character_profile_id)
                .is_some()
            || profile_types
                .get(variant.character_profile_id.as_str())
                .copied()
                != Some("CHARACTER")
            || !valid_consistency_name(&variant.name)
            || !valid_consistency_text(&variant.prompt_fragment, 20_000)
            || variant.is_default != 0 && variant.is_default != 1
            || variant.ordinal < 0
            || variant
                .reference_set_id
                .as_ref()
                .is_some_and(|id| reference_set_purposes.get(id.as_str()) != Some(&"COSTUME"))
        {
            return Err(AppError::backup_invalid("Costume Variant 数据无效"));
        }
    }

    let mut revision_ids = HashMap::<&str, (&str, &str)>::new();
    let mut revision_keys = HashSet::<(&str, &str, i64)>::new();
    for revision in &document.profile_revisions {
        let valid_profile = profile_types.get(revision.profile_id.as_str()).copied()
            == Some(revision.profile_type.as_str());
        if !valid_consistency_id(&revision.id)
            || revision_ids
                .insert(&revision.id, (&revision.profile_type, &revision.profile_id))
                .is_some()
            || ProfileType::try_from_db(&revision.profile_type).is_err()
            || !valid_profile
            || revision.revision_number < 1
            || !revision_keys.insert((
                &revision.profile_type,
                &revision.profile_id,
                revision.revision_number,
            ))
            || serde_json::from_str::<Value>(&revision.content_json).is_err()
            || revision.content_sha256.trim().is_empty()
            || ProfileRevisionStatus::try_from_db(&revision.status).is_err()
        {
            return Err(AppError::backup_invalid("Profile Revision 数据无效"));
        }
    }
    for profile in &document.character_profiles {
        validate_active_revision(
            "CHARACTER",
            &profile.id,
            profile.active_revision_id.as_ref(),
            &revision_ids,
        )?;
    }
    for profile in &document.scene_profiles {
        validate_active_revision(
            "SCENE",
            &profile.id,
            profile.active_revision_id.as_ref(),
            &revision_ids,
        )?;
    }
    for profile in &document.prop_profiles {
        validate_active_revision(
            "PROP",
            &profile.id,
            profile.active_revision_id.as_ref(),
            &revision_ids,
        )?;
    }
    for profile in &document.style_profiles {
        validate_active_revision(
            "STYLE",
            &profile.id,
            profile.active_revision_id.as_ref(),
            &revision_ids,
        )?;
    }

    let asset_types = document
        .assets
        .iter()
        .map(|asset| (asset.id.as_str(), asset.asset_type.as_str()))
        .collect::<HashMap<_, _>>();
    let mut items_by_set = HashMap::<&str, Vec<&BackupReferenceSetItem>>::new();
    for item in &document.reference_set_items {
        if !reference_set_purposes.contains_key(item.reference_set_id.as_str())
            || asset_types.get(item.asset_id.as_str()).copied() != Some("image")
            || item.ordinal < 0
            || item.is_primary != 0 && item.is_primary != 1
            || item
                .role
                .as_deref()
                .is_some_and(|role| !valid_consistency_text(role, 120) || role.trim().is_empty())
        {
            return Err(AppError::backup_invalid("Reference Set Item 数据无效"));
        }
        items_by_set
            .entry(&item.reference_set_id)
            .or_default()
            .push(item);
    }
    for items in items_by_set.values() {
        if items.len() > 20 {
            return Err(AppError::backup_invalid(
                "Reference Set Item 数量超过 20 个上限",
            ));
        }
        let mut asset_ids = HashSet::new();
        let mut ordinals = HashSet::new();
        let mut primary_count = 0;
        for item in items {
            if !asset_ids.insert(item.asset_id.as_str())
                || !ordinals.insert(item.ordinal)
                || item.is_primary == 1 && {
                    primary_count += 1;
                    primary_count > 1
                }
            {
                return Err(AppError::backup_invalid(
                    "Reference Set Item 存在重复或多个 primary",
                ));
            }
        }
        let mut ordinals = ordinals.into_iter().collect::<Vec<_>>();
        ordinals.sort_unstable();
        if ordinals
            .iter()
            .enumerate()
            .any(|(index, ordinal)| *ordinal != index as i64)
        {
            return Err(AppError::backup_invalid(
                "Reference Set Item 序号必须从 0 连续排列",
            ));
        }
    }

    let shot_ids = document
        .shots
        .iter()
        .map(|shot| shot.id.as_str())
        .collect::<HashSet<_>>();
    let mut shot_profile_binding_ids = HashSet::new();
    let mut shot_profile_slots = HashSet::new();
    for binding in &document.shot_profile_bindings {
        if !shot_profile_binding_ids.insert(binding.id.as_str())
            || !shot_ids.contains(binding.shot_id.as_str())
            || !binding_profile_fields_valid(
                &binding.role,
                &binding.profile_type,
                &binding.profile_id,
                binding.costume_variant_id.as_ref(),
                &profile_types,
                &costume_character_ids,
            )
            || binding.ordinal < 0
            || !InheritanceMode::try_from_db(&binding.inheritance_mode).is_ok()
            || !shot_profile_slots.insert((
                binding.shot_id.as_str(),
                binding.role.as_str(),
                binding.ordinal,
                binding.profile_id.as_str(),
            ))
        {
            return Err(AppError::backup_invalid("Shot Profile Binding 数据无效"));
        }
    }
    let mut shot_reference_binding_ids = HashSet::new();
    let mut shot_reference_slots = HashSet::new();
    for binding in &document.shot_reference_set_bindings {
        if !shot_reference_binding_ids.insert(binding.id.as_str())
            || !shot_ids.contains(binding.shot_id.as_str())
            || !reference_set_purposes.contains_key(binding.reference_set_id.as_str())
            || BindingRole::try_from_db(&binding.role).is_err()
            || binding.ordinal < 0
            || binding.required != 0 && binding.required != 1
            || InheritanceMode::try_from_db(&binding.inheritance_mode).is_err()
            || !shot_reference_slots.insert((
                binding.shot_id.as_str(),
                binding.role.as_str(),
                binding.ordinal,
                binding.reference_set_id.as_str(),
            ))
        {
            return Err(AppError::backup_invalid(
                "Shot Reference Set Binding 数据无效",
            ));
        }
    }

    let series_ids = document
        .production_series
        .iter()
        .map(|series| series.id.as_str())
        .collect::<HashSet<_>>();
    let episode_ids = document
        .production_episodes
        .iter()
        .map(|episode| episode.id.as_str())
        .collect::<HashSet<_>>();
    let scene_ids = document
        .production_scenes
        .iter()
        .map(|scene| scene.id.as_str())
        .collect::<HashSet<_>>();
    let scope_is_valid = |scope_type: &str, scope_id: &str| match scope_type {
        "PROJECT" => scope_id == document.project.id,
        "SERIES" => series_ids.contains(scope_id),
        "EPISODE" => episode_ids.contains(scope_id),
        "SCENE" => scene_ids.contains(scope_id),
        _ => false,
    };
    let mut scope_profile_binding_ids = HashSet::new();
    let mut scope_profile_slots = HashSet::new();
    for binding in &document.scope_profile_bindings {
        if !scope_profile_binding_ids.insert(binding.id.as_str())
            || binding.project_id != document.project.id
            || !scope_is_valid(&binding.scope_type, &binding.scope_id)
            || !binding_profile_fields_valid(
                &binding.role,
                &binding.profile_type,
                &binding.profile_id,
                binding.costume_variant_id.as_ref(),
                &profile_types,
                &costume_character_ids,
            )
            || binding.ordinal < 0
            || InheritanceMode::try_from_db(&binding.inheritance_mode).is_err()
            || !scope_profile_slots.insert((
                binding.scope_type.as_str(),
                binding.scope_id.as_str(),
                binding.role.as_str(),
                binding.ordinal,
                binding.profile_id.as_str(),
            ))
        {
            return Err(AppError::backup_invalid("Scope Profile Binding 数据无效"));
        }
    }
    let mut scope_reference_binding_ids = HashSet::new();
    let mut scope_reference_slots = HashSet::new();
    for binding in &document.scope_reference_set_bindings {
        if !scope_reference_binding_ids.insert(binding.id.as_str())
            || binding.project_id != document.project.id
            || !scope_is_valid(&binding.scope_type, &binding.scope_id)
            || !reference_set_purposes.contains_key(binding.reference_set_id.as_str())
            || BindingRole::try_from_db(&binding.role).is_err()
            || binding.ordinal < 0
            || binding.required != 0 && binding.required != 1
            || InheritanceMode::try_from_db(&binding.inheritance_mode).is_err()
            || !scope_reference_slots.insert((
                binding.scope_type.as_str(),
                binding.scope_id.as_str(),
                binding.role.as_str(),
                binding.ordinal,
                binding.reference_set_id.as_str(),
            ))
        {
            return Err(AppError::backup_invalid(
                "Scope Reference Set Binding 数据无效",
            ));
        }
    }
    Ok(())
}

fn register_consistency_profile<'a>(
    profile_types: &mut HashMap<&'a str, &'static str>,
    profile_names: &mut HashSet<String>,
    id: &'a str,
    project_id: &str,
    name: &str,
    profile_type: &'static str,
) -> Result<(), AppError> {
    if project_id.trim().is_empty()
        || !valid_consistency_id(id)
        || !valid_consistency_name(name)
        || profile_types.insert(id, profile_type).is_some()
        || !profile_names.insert(format!("{}:{}", profile_type, name.trim().to_lowercase()))
    {
        return Err(AppError::backup_invalid("Profile 元数据无效"));
    }
    Ok(())
}

fn valid_consistency_id(value: &str) -> bool {
    !value.trim().is_empty() && value.chars().count() <= 200
}

fn valid_consistency_name(value: &str) -> bool {
    !value.trim().is_empty() && value.trim().chars().count() <= 120
}

fn valid_consistency_text(value: &str, max_chars: usize) -> bool {
    value.chars().count() <= max_chars
}

fn valid_consistency_optional_text(value: Option<&str>, max_chars: usize) -> bool {
    value.is_none_or(|value| valid_consistency_text(value, max_chars))
}

fn valid_consistency_metadata(value: &str) -> bool {
    value.len() <= 64 * 1024
        && serde_json::from_str::<Value>(value)
            .map(|value| value.is_object())
            .unwrap_or(false)
}

fn reference_set_purpose(value: &str) -> Option<&'static str> {
    ReferenceSetPurpose::try_from_db(value)
        .ok()
        .map(ReferenceSetPurpose::as_str)
}

fn reference_set_owner_type(purpose: &str) -> Option<&'static str> {
    match ReferenceSetPurpose::try_from_db(purpose).ok()? {
        ReferenceSetPurpose::Character | ReferenceSetPurpose::Costume => Some("CHARACTER"),
        ReferenceSetPurpose::Scene => Some("SCENE"),
        ReferenceSetPurpose::Prop => Some("PROP"),
        ReferenceSetPurpose::Style => Some("STYLE"),
        ReferenceSetPurpose::Shot => None,
    }
}

fn validate_consistency_profile_relation(
    profile_type: &str,
    profile_id: &str,
    default_style_profile_id: Option<&String>,
    default_reference_set_id: Option<&String>,
    profile_types: &HashMap<&str, &str>,
    reference_set_purposes: &HashMap<&str, &str>,
) -> Result<(), AppError> {
    if profile_types.get(profile_id).copied() != Some(profile_type)
        || default_style_profile_id
            .is_some_and(|id| profile_types.get(id.as_str()).copied() != Some("STYLE"))
        || default_reference_set_id.as_ref().is_some_and(|id| {
            id.trim().is_empty() || !reference_set_purposes.contains_key(id.as_str())
        })
    {
        return Err(AppError::backup_invalid("Profile 关系引用无效"));
    }
    Ok(())
}

fn validate_active_revision(
    profile_type: &str,
    profile_id: &str,
    active_revision_id: Option<&String>,
    revisions: &HashMap<&str, (&str, &str)>,
) -> Result<(), AppError> {
    if active_revision_id.is_some_and(|revision_id| {
        revisions
            .get(revision_id.as_str())
            .is_none_or(|(revision_type, revision_profile_id)| {
                *revision_type != profile_type || *revision_profile_id != profile_id
            })
    }) {
        return Err(AppError::backup_invalid(
            "Profile active_revision_id 引用无效",
        ));
    }
    Ok(())
}

fn binding_profile_fields_valid(
    role: &str,
    profile_type: &str,
    profile_id: &str,
    costume_variant_id: Option<&String>,
    profile_types: &HashMap<&str, &str>,
    costume_character_ids: &HashMap<&str, &str>,
) -> bool {
    let expected_type = match BindingRole::try_from_db(role).ok() {
        Some(BindingRole::Character) => Some("CHARACTER"),
        Some(BindingRole::Scene) => Some("SCENE"),
        Some(BindingRole::Prop) => Some("PROP"),
        Some(BindingRole::Style) => Some("STYLE"),
        Some(BindingRole::ShotReference) | None => None,
    };
    profile_types.get(profile_id).copied() == Some(profile_type)
        && expected_type == Some(profile_type)
        && costume_variant_id.is_none_or(|variant_id| {
            role == "CHARACTER"
                && costume_character_ids.get(variant_id.as_str()).copied() == Some(profile_id)
        })
}

fn validate_production_structure_document(
    document: &BackupDocument,
    version: u32,
) -> Result<(), AppError> {
    if version < 12
        && (!document.production_series.is_empty()
            || !document.production_episodes.is_empty()
            || !document.production_scenes.is_empty()
            || !document.shot_scene_assignments.is_empty())
    {
        return Err(AppError::backup_invalid("生产结构数据需要 Backup v12"));
    }

    let mut series_ids = HashSet::new();
    let mut series_ordinals = Vec::with_capacity(document.production_series.len());
    for series in &document.production_series {
        if series.project_id != document.project.id
            || series.id.trim().is_empty()
            || !series_ids.insert(series.id.as_str())
            || series.ordinal < 0
            || !valid_structure_name(&series.name)
            || series.description.chars().count() > 1000
        {
            return Err(AppError::backup_invalid("备份 Series 数据无效"));
        }
        series_ordinals.push(series.ordinal);
    }
    if !is_contiguous_ordinals(&mut series_ordinals) {
        return Err(AppError::backup_invalid(
            "备份 Series 序号必须从 0 连续排列",
        ));
    }

    let mut episode_ids = HashSet::new();
    let mut episode_ordinals = HashMap::<&str, Vec<i64>>::new();
    for episode in &document.production_episodes {
        if episode.id.trim().is_empty()
            || !episode_ids.insert(episode.id.as_str())
            || !series_ids.contains(episode.series_id.as_str())
            || episode.ordinal < 0
            || !valid_structure_name(&episode.name)
            || episode.description.chars().count() > 1000
        {
            return Err(AppError::backup_invalid("备份 Episode 数据无效"));
        }
        episode_ordinals
            .entry(episode.series_id.as_str())
            .or_default()
            .push(episode.ordinal);
    }
    if episode_ordinals
        .values_mut()
        .any(|ordinals| !is_contiguous_ordinals(ordinals))
    {
        return Err(AppError::backup_invalid("备份 Episode 序号必须连续"));
    }

    let mut scene_ids = HashSet::new();
    let mut scene_ordinals = HashMap::<&str, Vec<i64>>::new();
    for scene in &document.production_scenes {
        if scene.id.trim().is_empty()
            || !scene_ids.insert(scene.id.as_str())
            || !episode_ids.contains(scene.episode_id.as_str())
            || scene.ordinal < 0
            || !valid_structure_name(&scene.name)
            || scene.description.chars().count() > 1000
        {
            return Err(AppError::backup_invalid("备份 Scene 数据无效"));
        }
        scene_ordinals
            .entry(scene.episode_id.as_str())
            .or_default()
            .push(scene.ordinal);
    }
    if scene_ordinals
        .values_mut()
        .any(|ordinals| !is_contiguous_ordinals(ordinals))
    {
        return Err(AppError::backup_invalid("备份 Scene 序号必须连续"));
    }

    let shot_ids = document
        .shots
        .iter()
        .map(|shot| shot.id.as_str())
        .collect::<HashSet<_>>();
    let mut assigned_shots = HashSet::new();
    let mut assignment_ordinals = HashMap::<&str, Vec<i64>>::new();
    for assignment in &document.shot_scene_assignments {
        if assignment.shot_id.trim().is_empty()
            || !shot_ids.contains(assignment.shot_id.as_str())
            || !scene_ids.contains(assignment.scene_id.as_str())
            || !assigned_shots.insert(assignment.shot_id.as_str())
            || assignment.ordinal < 0
        {
            return Err(AppError::backup_invalid("备份镜头 Scene 归属无效"));
        }
        assignment_ordinals
            .entry(assignment.scene_id.as_str())
            .or_default()
            .push(assignment.ordinal);
    }
    if assignment_ordinals
        .values_mut()
        .any(|ordinals| !is_contiguous_ordinals(ordinals))
    {
        return Err(AppError::backup_invalid("备份 Scene 镜头序号必须连续"));
    }
    Ok(())
}

fn validate_external_production_handoff_document(
    document: &BackupDocument,
    version: u32,
) -> Result<(), AppError> {
    if version < 18
        && (!document.external_production_handoffs.is_empty()
            || !document.external_production_handoff_entities.is_empty())
    {
        return Err(AppError::backup_invalid(
            "External Production Handoff 数据需要 Backup v18",
        ));
    }
    if document.external_production_handoffs.is_empty()
        && !document.external_production_handoff_entities.is_empty()
    {
        return Err(AppError::backup_invalid(
            "External Production Handoff Entity 缺少 Handoff",
        ));
    }

    let handoff_ids = document
        .external_production_handoffs
        .iter()
        .map(|handoff| handoff.id.as_str())
        .collect::<HashSet<_>>();
    let series_ids = document
        .production_series
        .iter()
        .map(|series| series.id.as_str())
        .collect::<HashSet<_>>();
    let episode_ids = document
        .production_episodes
        .iter()
        .map(|episode| episode.id.as_str())
        .collect::<HashSet<_>>();
    let scene_ids = document
        .production_scenes
        .iter()
        .map(|scene| scene.id.as_str())
        .collect::<HashSet<_>>();
    let shot_ids = document
        .shots
        .iter()
        .map(|shot| shot.id.as_str())
        .collect::<HashSet<_>>();

    for handoff in &document.external_production_handoffs {
        let valid_sha = handoff.document_sha256.len() == 64
            && handoff
                .document_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit());
        if handoff.project_id != document.project.id
            || handoff.id.trim().is_empty()
            || handoff.schema_version != 1
            || handoff.source_agent.trim().is_empty()
            || handoff.source_agent.chars().count() > 200
            || handoff.source_revision.as_deref().is_some_and(|revision| {
                revision.trim().is_empty() || revision.chars().count() > 200
            })
            || handoff.imported_at.trim().is_empty()
            || !valid_sha
        {
            return Err(AppError::backup_invalid(
                "External Production Handoff 数据无效",
            ));
        }
    }
    if handoff_ids.len() != document.external_production_handoffs.len() {
        return Err(AppError::backup_invalid(
            "External Production Handoff ID 重复",
        ));
    }

    let mut external_keys = HashSet::new();
    let mut formal_keys = HashSet::new();
    for entity in &document.external_production_handoff_entities {
        let formal_exists = match entity.entity_kind.as_str() {
            "series" => series_ids.contains(entity.formal_entity_id.as_str()),
            "episode" => episode_ids.contains(entity.formal_entity_id.as_str()),
            "scene" => scene_ids.contains(entity.formal_entity_id.as_str()),
            "shot" => shot_ids.contains(entity.formal_entity_id.as_str()),
            _ => false,
        };
        if !handoff_ids.contains(entity.handoff_id.as_str())
            || !matches!(
                entity.entity_kind.as_str(),
                "series" | "episode" | "scene" | "shot"
            )
            || entity.external_id.trim().is_empty()
            || entity.formal_entity_id.trim().is_empty()
            || !formal_exists
            || !external_keys.insert((
                entity.handoff_id.as_str(),
                entity.entity_kind.as_str(),
                entity.external_id.as_str(),
            ))
            || !formal_keys.insert((
                entity.handoff_id.as_str(),
                entity.entity_kind.as_str(),
                entity.formal_entity_id.as_str(),
            ))
        {
            return Err(AppError::backup_invalid(
                "External Production Handoff Entity 数据无效",
            ));
        }
    }
    Ok(())
}

fn valid_structure_name(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed == value
        && !value.contains(['\r', '\n'])
        && value.chars().count() <= 100
}

fn is_contiguous_ordinals(ordinals: &mut [i64]) -> bool {
    ordinals.sort_unstable();
    ordinals
        .iter()
        .enumerate()
        .all(|(index, ordinal)| *ordinal == index as i64)
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

#[cfg(test)]
pub(crate) use crate::infrastructure::database::{
    assemble_reference_anchor_backups, DbReferenceAnchor, DbReferenceAnchorAsset,
};

#[cfg(test)]
mod tests {
    use super::{
        assemble_reference_anchor_backups, collect_exact_asset_id_references, hash_bytes,
        inspect_archive, remap_reference_anchor_assets, remap_snapshot_asset_references,
        restored_name, safe_zip_path, validate_asset_video_prompt_document,
        validate_organization_document, validate_production_structure_document,
        validate_prompt_document, validate_reference_anchor_document, write_zip_to_path,
        BackupArtifactReview, BackupAsset, BackupAssetRelation, BackupAssetTag, BackupAssetTagLink,
        BackupAssetVersion, BackupAssetVideoPrompt, BackupDocument, BackupFileSource,
        BackupGenerationAssetVersion, BackupGenerationToolUsage, BackupInventoryCounts,
        BackupMapping, BackupModel, BackupModelVersion, BackupProductionEpisode,
        BackupProductionScene, BackupProductionSeries, BackupProject, BackupPromptEntry,
        BackupPromptVersion, BackupReferenceAnchor, BackupReferenceAnchorAsset, BackupShot,
        BackupShotSceneAssignment, BackupSnapshot, BackupTask, BackupTool, BackupToolCapability,
        BackupToolInstance, BackupToolVersion, DbReferenceAnchor, DbReferenceAnchorAsset,
        ProductionStructureIds, ProjectBackupManifest, ProjectBackupService,
    };
    use crate::application::ports::ProjectRecord;
    use crate::infrastructure::{
        database::{initialize, SqliteProjectBackupRepository},
        filesystem::AppDataDirs,
    };
    use chrono::Utc;
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::collections::{HashMap, HashSet};
    use std::io::Read;
    use std::sync::Arc;
    use std::{
        fs::File,
        io::Write,
        path::{Path, PathBuf},
    };
    use tempfile::tempdir;
    use zip::{write::FileOptions, CompressionMethod, ZipWriter};

    fn test_service(
        pool: &sqlx::SqlitePool,
        projects_dir: PathBuf,
        cache_dir: PathBuf,
    ) -> ProjectBackupService {
        ProjectBackupService::new(
            Arc::new(SqliteProjectBackupRepository::new(pool.clone())),
            projects_dir,
            cache_dir,
        )
    }

    async fn seed_current_v20_legacy_shot_fixture(
        directory: &tempfile::TempDir,
    ) -> (sqlx::SqlitePool, ProjectBackupService, String, PathBuf) {
        let data_dirs = AppDataDirs::initialize(directory.path().join("AIStudioData")).unwrap();
        let pool = initialize(&data_dirs.database).await.unwrap();
        crate::infrastructure::database::repositories::test_support::seed_task_dependencies(&pool)
            .await;

        let project_id = "project-current-v20".to_owned();
        let project_root = data_dirs.projects.join(&project_id);
        std::fs::create_dir_all(&project_root).unwrap();
        sqlx::query(
            "INSERT INTO projects (id, name, description, root_path, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&project_id)
        .bind("Current v20 fixture")
        .bind("legacy shot stage prompt source")
        .bind(project_root.to_string_lossy().to_string())
        .bind("2026-01-01T00:00:00Z")
        .bind("2026-01-01T00:00:00Z")
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO project_workflow_bindings
             (project_id, stage, mode, workflow_version_id, recipe_id, created_at, updated_at)
             VALUES
             (?, 'IMAGE', 'DEFAULT', 'workflow-version-1', 'recipe-1', ?, ?),
             (?, 'VIDEO', 'DEFAULT', 'workflow-version-1', 'recipe-1', ?, ?)",
        )
        .bind(&project_id)
        .bind("2026-01-01T00:00:00Z")
        .bind("2026-01-01T00:00:00Z")
        .bind(&project_id)
        .bind("2026-01-01T00:00:01Z")
        .bind("2026-01-01T00:00:01Z")
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO prompt_entries
             (id, project_id, kind, name, normalized_name, tags_json, created_at, updated_at)
             VALUES (?, ?, 'prompt', ?, ?, '[]', ?, ?)",
        )
        .bind("prm_current_v20")
        .bind(&project_id)
        .bind("Harbor prompt")
        .bind("harbor prompt")
        .bind("2026-01-01T00:00:00Z")
        .bind("2026-01-01T00:00:00Z")
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO prompt_versions (id, prompt_id, version, text, created_at)
             VALUES (?, ?, 1, ?, ?)",
        )
        .bind("prv_current_v20")
        .bind("prm_current_v20")
        .bind("A quiet harbor at dawn")
        .bind("2026-01-01T00:00:00Z")
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO tasks
             (id, project_id, workflow_id, workflow_version_id, recipe_id, status,
              progress_mode, created_at, finished_at)
             VALUES (?, ?, 'workflow-1', 'workflow-version-1', 'recipe-1', 'SUCCEEDED',
                     'indeterminate', ?, ?)",
        )
        .bind("tsk_current_v20")
        .bind(&project_id)
        .bind("2026-01-01T00:01:00Z")
        .bind("2026-01-01T00:02:00Z")
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO generation_snapshots
             (id, task_id, workflow_json, recipe_yaml, user_inputs_json,
              resolved_inputs_json, created_at)
             VALUES (?, ?, '{}', 'schema_version: 1\ninputs: {}\n', ?, ?, ?)",
        )
        .bind("snp_current_v20")
        .bind("tsk_current_v20")
        .bind(r#"{"reference_image":"ast_current_v20"}"#)
        .bind(r#"{"reference_image":"ast_current_v20"}"#)
        .bind("2026-01-01T00:02:00Z")
        .execute(&pool)
        .await
        .unwrap();

        let bytes = b"current-v20-image-bytes";
        let sha = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let asset_path = project_root.join("generated.png");
        std::fs::write(&asset_path, bytes).unwrap();
        sqlx::query(
            "INSERT INTO assets
             (id, project_id, type, category, name, original_name, storage_path, sha256,
              mime_type, width, height, file_size, metadata_json, created_at, updated_at,
              source_task_id)
             VALUES (?, ?, 'image', 'generated_image', 'Generated harbor', 'generated.png', ?, ?,
                     'image/png', 1, 1, ?, '{}', ?, ?, ?)",
        )
        .bind("ast_current_v20")
        .bind(&project_id)
        .bind(asset_path.to_string_lossy().to_string())
        .bind(&sha)
        .bind(bytes.len() as i64)
        .bind("2026-01-01T00:02:00Z")
        .bind("2026-01-01T00:02:00Z")
        .bind("tsk_current_v20")
        .execute(&pool)
        .await
        .unwrap();
        let source_bytes = b"current-v20-source-bytes";
        let source_sha = Sha256::digest(source_bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let source_path = project_root.join("source.png");
        std::fs::write(&source_path, source_bytes).unwrap();
        sqlx::query(
            "INSERT INTO assets
             (id, project_id, type, category, name, original_name, storage_path, sha256,
              mime_type, width, height, file_size, metadata_json, created_at, updated_at)
             VALUES (?, ?, 'image', 'source_image', 'Source harbor', 'source.png', ?, ?,
                     'image/png', 1, 1, ?, '{}', ?, ?)",
        )
        .bind("ast_current_v20_source")
        .bind(&project_id)
        .bind(source_path.to_string_lossy().to_string())
        .bind(&source_sha)
        .bind(source_bytes.len() as i64)
        .bind("2026-01-01T00:02:01Z")
        .bind("2026-01-01T00:02:01Z")
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO task_output_assets
             (task_id, output_id, ordinal, asset_id, created_at)
             VALUES (?, 'output-current-v20', 0, ?, ?)",
        )
        .bind("tsk_current_v20")
        .bind("ast_current_v20")
        .bind("2026-01-01T00:02:00Z")
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE artifact_reviews
             SET decision = 'APPROVED', comment = 'accepted', revision = 1,
                 updated_at = '2026-01-01T00:02:30Z'
             WHERE project_id = ? AND artifact_id = 'ast_current_v20'",
        )
        .bind(&project_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO asset_versions
             (id, project_id, asset_id, version_number, metadata_snapshot, location,
              checksum, created_at)
             VALUES (?, ?, ?, 1, '{}', ?, ?, ?)",
        )
        .bind("asv_current_v20")
        .bind(&project_id)
        .bind("ast_current_v20")
        .bind(asset_path.to_string_lossy().to_string())
        .bind(&sha)
        .bind("2026-01-01T00:02:00Z")
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO asset_relations
             (id, project_id, source_asset_id, target_asset_id, relation_type, created_at)
             VALUES (?, ?, ?, ?, 'SOURCE_OF', ?)",
        )
        .bind("rel_current_v20")
        .bind(&project_id)
        .bind("ast_current_v20_source")
        .bind("ast_current_v20")
        .bind("2026-01-01T00:02:00Z")
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shots
             (id, project_id, ordinal, name, prompt_text, prompt_entry_id, prompt_version_id,
              selected_image_asset_id, selected_video_asset_id, created_at, updated_at)
             VALUES (?, ?, 0, 'Harbor opening',
                     'A quiet harbor at dawn, one red balloon drifting above the water, wide cinematic composition.',
                     'prm_current_v20', 'prv_current_v20', ?, NULL, ?, ?)",
        )
        .bind("sht_current_v20")
        .bind(&project_id)
        .bind("ast_current_v20")
        .bind("2026-01-01T00:03:00Z")
        .bind("2026-01-01T00:03:00Z")
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shot_stage_configs
             (shot_id, stage, workflow_version_id, recipe_id, scalar_values_json, updated_at)
             VALUES
             (?, 'image', 'workflow-version-1', 'recipe-1', '{\"steps\":{\"type\":\"integer\",\"value\":4}}', ?),
             (?, 'video', 'workflow-version-1', 'recipe-1', '{\"seed\":{\"type\":\"seed_random\"}}', ?)",
        )
        .bind("sht_current_v20")
        .bind("2026-01-01T00:03:00Z")
        .bind("sht_current_v20")
        .bind("2026-01-01T00:03:00Z")
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO production_batches
             (id, project_id, name, status, continue_on_failure, created_at, updated_at)
             VALUES (?, ?, 'Current v20 batch', 'COMPLETED', 0, ?, ?)",
        )
        .bind("pbt_current_v20")
        .bind(&project_id)
        .bind("2026-01-01T00:03:00Z")
        .bind("2026-01-01T00:03:00Z")
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO production_batch_items
             (id, batch_id, ordinal, workflow_version_id, recipe_id, values_json, status,
              task_id, created_at, updated_at)
             VALUES (?, ?, 0, 'workflow-version-1', 'recipe-1', '{}', 'SUCCEEDED', ?, ?, ?)",
        )
        .bind("pbi_current_v20")
        .bind("pbt_current_v20")
        .bind("tsk_current_v20")
        .bind("2026-01-01T00:03:00Z")
        .bind("2026-01-01T00:03:00Z")
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shot_generation_links
             (id, shot_id, stage, task_id, production_batch_item_id, created_at)
             VALUES (?, ?, 'image', ?, ?, ?)",
        )
        .bind("sgl_current_v20")
        .bind("sht_current_v20")
        .bind("tsk_current_v20")
        .bind("pbi_current_v20")
        .bind("2026-01-01T00:03:00Z")
        .execute(&pool)
        .await
        .unwrap();

        let service = test_service(&pool, data_dirs.projects.clone(), data_dirs.cache.clone());
        let archive = directory.path().join("current-v20.aiarchive");
        (pool, service, project_id, archive)
    }

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
            preparation_snapshots: Vec::new(),
            workflow_refs: Vec::new(),
            project_workflow_bindings: Vec::new(),
            workflow_registry: None,
            asset_tags: tags,
            asset_tag_links: links,
            asset_favorites: Vec::new(),
            asset_video_prompts: Vec::new(),
            reference_anchors: Vec::new(),
            production_series: Vec::new(),
            production_episodes: Vec::new(),
            production_scenes: Vec::new(),
            shot_scene_assignments: Vec::new(),
            script_sources: Vec::new(),
            script_draft_revisions: Vec::new(),
            production_item_reviews: Vec::new(),
            artifact_reviews: Vec::new(),
            benchmark_experiments: Vec::new(),
            benchmark_candidates: Vec::new(),
            production_runs: Vec::new(),
            production_stages: Vec::new(),
            production_stage_items: Vec::new(),
            production_run_templates: Vec::new(),
            benchmark_runs: Vec::new(),
            benchmark_quality_scores: Vec::new(),
            shots: Vec::new(),
            external_production_handoffs: Vec::new(),
            external_production_handoff_entities: Vec::new(),
            shot_stage_configs: Vec::new(),
            shot_stage_prompts: Vec::new(),
            shot_reference_assets: Vec::new(),
            shot_generation_links: Vec::new(),
            character_profiles: Vec::new(),
            scene_profiles: Vec::new(),
            prop_profiles: Vec::new(),
            style_profiles: Vec::new(),
            costume_variants: Vec::new(),
            profile_revisions: Vec::new(),
            reference_sets: Vec::new(),
            reference_set_items: Vec::new(),
            shot_profile_bindings: Vec::new(),
            shot_reference_set_bindings: Vec::new(),
            scope_profile_bindings: Vec::new(),
            scope_reference_set_bindings: Vec::new(),
            asset_versions: Vec::new(),
            asset_relations: Vec::new(),
            models: Vec::new(),
            model_versions: Vec::new(),
            tools: Vec::new(),
            tool_versions: Vec::new(),
            tool_capabilities: Vec::new(),
            tool_instances: Vec::new(),
            generation_tool_usages: Vec::new(),
            generation_asset_versions: Vec::new(),
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

    #[test]
    fn backup_reference_anchor_validation_preserves_image_memberships_only() {
        let mut valid = organization_document(Vec::new(), Vec::new());
        valid.reference_anchors = vec![BackupReferenceAnchor {
            id: "anc_character".to_owned(),
            project_id: valid.project.id.clone(),
            kind: "CHARACTER".to_owned(),
            name: "地藏菩萨".to_owned(),
            normalized_name: "地藏菩萨".to_owned(),
            description: "主参考".to_owned(),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
            assets: vec![BackupReferenceAnchorAsset {
                asset_id: "ast_organization".to_owned(),
                ordinal: 0,
                created_at: "2026-01-01T00:00:00Z".to_owned(),
            }],
        }];
        validate_reference_anchor_document(&valid).expect("valid anchor backup should pass");

        let mut video = valid.clone();
        video.assets[0].asset_type = "video".to_owned();
        assert!(validate_reference_anchor_document(&video).is_err());

        let mut duplicate_ordinal = valid.clone();
        duplicate_ordinal.reference_anchors[0]
            .assets
            .push(BackupReferenceAnchorAsset {
                asset_id: "ast_organization".to_owned(),
                ordinal: 0,
                created_at: "2026-01-01T00:00:00Z".to_owned(),
            });
        assert!(validate_reference_anchor_document(&duplicate_ordinal).is_err());
    }

    #[test]
    fn v10_backup_defaults_reference_anchors_to_empty() {
        let mut value =
            serde_json::to_value(organization_document(Vec::new(), Vec::new())).unwrap();
        value
            .as_object_mut()
            .expect("backup document is an object")
            .remove("referenceAnchors");
        let document: BackupDocument = serde_json::from_value(value).unwrap();
        assert!(document.reference_anchors.is_empty());
    }

    #[test]
    fn v18_backup_without_v2_collections_still_deserializes() {
        let legacy = json!({
            "project": {"id": "prj_legacy", "name": "旧备份"},
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
            "workflowRefs": []
        });
        let document: BackupDocument = serde_json::from_value(legacy).unwrap();
        assert!(document.asset_versions.is_empty());
        assert!(document.asset_relations.is_empty());
        assert!(document.models.is_empty());
        assert!(document.model_versions.is_empty());
        assert!(document.tools.is_empty());
        assert!(document.tool_versions.is_empty());
        assert!(document.tool_capabilities.is_empty());
        assert!(document.tool_instances.is_empty());
        assert!(document.generation_tool_usages.is_empty());
        assert!(document.generation_asset_versions.is_empty());
        assert!(document.artifact_reviews.is_empty());
        assert!(document.prompt_versions.is_empty());
    }

    #[test]
    fn v20_backup_document_round_trips_artifact_review_and_additive_collections() {
        let mut document = organization_document(Vec::new(), Vec::new());
        document.asset_versions = vec![BackupAssetVersion {
            id: "asv_1".to_owned(),
            project_id: document.project.id.clone(),
            asset_id: "ast_organization".to_owned(),
            version_number: 1,
            metadata_snapshot: json!({"name": "v1"}),
            location: "/tmp/ast.png".to_owned(),
            checksum: "abc".to_owned(),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
        }];
        document.asset_relations = vec![BackupAssetRelation {
            id: "rel_1".to_owned(),
            project_id: document.project.id.clone(),
            source_asset_id: "ast_organization".to_owned(),
            target_asset_id: "ast_organization".to_owned(),
            relation_type: "RELATED".to_owned(),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
        }];
        document.models = vec![BackupModel {
            id: "mdl_1".to_owned(),
            name: "H3".to_owned(),
            provider: "MiniMax".to_owned(),
            model_type: "video".to_owned(),
            description: "".to_owned(),
            metadata_json: json!({}),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
        }];
        document.model_versions = vec![BackupModelVersion {
            id: "mdv_1".to_owned(),
            model_id: "mdl_1".to_owned(),
            version: "2026-01".to_owned(),
            capabilities_json: json!([]),
            parameter_schema_json: json!({}),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
        }];
        document.mappings = vec![BackupMapping {
            task_id: "tsk_1".to_owned(),
            output_id: "out_1".to_owned(),
            ordinal: 0,
            asset_id: "ast_organization".to_owned(),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
        }];
        document.artifact_reviews = vec![BackupArtifactReview {
            id: "arv_1".to_owned(),
            project_id: document.project.id.clone(),
            artifact_id: "ast_organization".to_owned(),
            decision: "REJECTED".to_owned(),
            comment: "调整光线".to_owned(),
            revision: 3,
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:01:00Z".to_owned(),
        }];
        document.tools = vec![BackupTool {
            id: "tool_1".to_owned(),
            name: "ComfyUI".to_owned(),
            tool_type: "local".to_owned(),
            description: "".to_owned(),
            metadata_json: json!({}),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
        }];
        document.tool_instances = vec![BackupToolInstance {
            id: "tins_1".to_owned(),
            tool_id: "tool_1".to_owned(),
            path: Some("/opt/comfy".to_owned()),
            endpoint: Some("http://127.0.0.1:8188".to_owned()),
            status: "AVAILABLE".to_owned(),
            last_checked: Some("2026-01-01T00:00:00Z".to_owned()),
        }];
        document.generation_tool_usages = vec![BackupGenerationToolUsage {
            id: "gtu_1".to_owned(),
            generation_id: "tsk_1".to_owned(),
            tool_instance_id: "tins_1".to_owned(),
            tool_version_id: None,
            metadata_json: json!({}),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
        }];
        document.generation_asset_versions = vec![BackupGenerationAssetVersion {
            id: "gav_1".to_owned(),
            generation_id: "tsk_1".to_owned(),
            output_id: "out_1".to_owned(),
            ordinal: 0,
            asset_version_id: "asv_1".to_owned(),
            relation_type: "OUTPUT".to_owned(),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
        }];
        let encoded = serde_json::to_value(&document).unwrap();
        let decoded: BackupDocument = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded.asset_versions.len(), 1);
        assert_eq!(decoded.models[0].provider, "MiniMax");
        assert_eq!(decoded.tool_instances[0].status, "AVAILABLE");
        assert_eq!(decoded.generation_asset_versions[0].relation_type, "OUTPUT");
        assert_eq!(decoded.artifact_reviews[0].decision, "REJECTED");
        assert_eq!(decoded.artifact_reviews[0].artifact_id, "ast_organization");
    }

    #[test]
    fn v11_restore_defaults_structure_and_v12_structure_remaps_ids() {
        let mut legacy =
            serde_json::to_value(organization_document(Vec::new(), Vec::new())).unwrap();
        let legacy_object = legacy
            .as_object_mut()
            .expect("backup document is an object");
        for field in [
            "productionSeries",
            "productionEpisodes",
            "productionScenes",
            "shotSceneAssignments",
        ] {
            legacy_object.remove(field);
        }
        let legacy: BackupDocument = serde_json::from_value(legacy).unwrap();
        assert!(legacy.production_series.is_empty());
        assert!(legacy.production_episodes.is_empty());
        assert!(legacy.production_scenes.is_empty());
        assert!(legacy.shot_scene_assignments.is_empty());

        let mut document = organization_document(Vec::new(), Vec::new());
        document.shots = vec![BackupShot {
            id: "sht_old".to_owned(),
            project_id: document.project.id.clone(),
            ordinal: 0,
            name: "镜头 1".to_owned(),
            prompt_text: "prompt".to_owned(),
            prompt_entry_id: None,
            prompt_version_id: None,
            selected_image_asset_id: None,
            selected_video_asset_id: None,
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
        }];
        document.production_series = vec![BackupProductionSeries {
            id: "ser_old".to_owned(),
            project_id: document.project.id.clone(),
            ordinal: 0,
            name: "第一季".to_owned(),
            description: String::new(),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
        }];
        document.production_episodes = vec![BackupProductionEpisode {
            id: "ep_old".to_owned(),
            series_id: "ser_old".to_owned(),
            ordinal: 0,
            name: "第一集".to_owned(),
            description: String::new(),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
        }];
        document.production_scenes = vec![BackupProductionScene {
            id: "scn_old".to_owned(),
            episode_id: "ep_old".to_owned(),
            ordinal: 0,
            name: "开场".to_owned(),
            description: String::new(),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
        }];
        document.shot_scene_assignments = vec![BackupShotSceneAssignment {
            shot_id: "sht_old".to_owned(),
            scene_id: "scn_old".to_owned(),
            ordinal: 0,
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
        }];
        validate_production_structure_document(&document, 12)
            .expect("v12 structure should validate");

        let remaps = ProductionStructureIds {
            series: HashMap::from([("ser_old".to_owned(), "ser_new".to_owned())]),
            episodes: HashMap::from([("ep_old".to_owned(), "ep_new".to_owned())]),
            scenes: HashMap::from([("scn_old".to_owned(), "scn_new".to_owned())]),
        };
        assert_eq!(remaps.series["ser_old"], "ser_new");
        assert_eq!(remaps.episodes["ep_old"], "ep_new");
        assert_eq!(remaps.scenes["scn_old"], "scn_new");
    }

    #[test]
    fn reference_anchor_export_and_restore_keep_order_and_remap_assets() {
        let included = HashSet::from(["ast_a".to_owned(), "ast_b".to_owned()]);
        let anchors = vec![DbReferenceAnchor {
            id: "anc_old".to_owned(),
            project_id: "project-1".to_owned(),
            kind: "CHARACTER".to_owned(),
            name: "角色".to_owned(),
            normalized_name: "角色".to_owned(),
            description: "说明".to_owned(),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
        }];
        let memberships = vec![
            DbReferenceAnchorAsset {
                anchor_id: "anc_old".to_owned(),
                asset_id: "ast_b".to_owned(),
                ordinal: 1,
                created_at: "2026-01-01T00:00:00Z".to_owned(),
            },
            DbReferenceAnchorAsset {
                anchor_id: "anc_old".to_owned(),
                asset_id: "ast_a".to_owned(),
                ordinal: 0,
                created_at: "2026-01-01T00:00:00Z".to_owned(),
            },
        ];
        let exported = assemble_reference_anchor_backups(anchors, memberships, &included);
        assert_eq!(exported[0].assets[0].asset_id, "ast_a");
        assert_eq!(exported[0].assets[1].asset_id, "ast_b");

        let mapping = HashMap::from([
            ("ast_a".to_owned(), "ast_new_a".to_owned()),
            ("ast_b".to_owned(), "ast_new_b".to_owned()),
        ]);
        let restored = remap_reference_anchor_assets(&exported[0], &mapping).unwrap();
        assert_eq!(
            restored
                .iter()
                .map(|asset| asset.asset_id.as_str())
                .collect::<Vec<_>>(),
            vec!["ast_new_a", "ast_new_b"]
        );
        assert!(!restored.iter().any(|asset| asset.asset_id == "ast_a"));
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
            model_version_id: None,
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
        sqlx::query(
            "INSERT INTO project_workflow_bindings
             (project_id, stage, mode, workflow_version_id, recipe_id, created_at, updated_at)
             VALUES
             ('project-backup', 'IMAGE', 'DEFAULT', 'workflow-version-1', 'recipe-1',
              '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'),
             ('project-backup', 'VIDEO', 'FL2VA_TEXT_TO_VIDEO', 'workflow-version-1', 'recipe-1',
              '2026-01-01T00:00:01Z', '2026-01-01T00:00:01Z')",
        )
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
            "INSERT INTO production_series
             (id, project_id, ordinal, name, description, created_at, updated_at)
             VALUES ('ser_backup', 'project-backup', 0, '第一季', '', '2026-01-01T00:03:00Z', '2026-01-01T00:03:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO production_episodes
             (id, series_id, ordinal, name, description, created_at, updated_at)
             VALUES ('ep_backup', 'ser_backup', 0, '第一集', '', '2026-01-01T00:03:00Z', '2026-01-01T00:03:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO production_scenes
             (id, episode_id, ordinal, name, description, created_at, updated_at)
             VALUES ('scn_backup', 'ep_backup', 0, '开场', '', '2026-01-01T00:03:00Z', '2026-01-01T00:03:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shot_scene_assignments
             (shot_id, scene_id, ordinal, created_at, updated_at)
             VALUES ('sht_backup', 'scn_backup', 0, '2026-01-01T00:03:00Z', '2026-01-01T00:03:00Z')",
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
        let preparation_snapshot_json = serde_json::to_string(&json!({
            "schemaVersion": 1,
            "projectId": "project-backup",
            "shotId": "sht_backup",
            "stage": "image",
            "contextHash": "context-backup",
            "resolvedAt": "2026-01-01T00:03:00Z",
            "preparedAt": "2026-01-01T00:03:01Z",
            "structure": {"shotId": "sht_backup"},
            "profiles": [],
            "referenceSets": [],
            "referenceAssets": [{
                "assetId": "ast_backup",
                "sha256": "sha-backup",
                "role": "primary",
                "ordinal": 0
            }],
            "prompt": {
                "renderedText": "frozen prompt",
                "negativePrompt": "",
                "orderedSegments": []
            },
            "workflow": {
                "workflowVersionId": "workflow-version-1",
                "recipeId": "recipe-1"
            },
            "outputSpec": {"type": "image"},
            "stageInput": null,
            "frozenGenerationValues": {"prompt": "frozen prompt"},
            "readiness": {
                "status": "READY",
                "score": 100,
                "gates": [],
                "evaluatedAt": "2026-01-01T00:03:00Z"
            },
            "comfyCapabilityEvidence": {"status": "READY"}
        }))
        .unwrap();
        sqlx::query(
            "INSERT INTO production_preparation_snapshots
             (id, project_id, shot_id, stage, context_hash, production_batch_id,
              production_batch_item_id, snapshot_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("pps_backup")
        .bind("project-backup")
        .bind("sht_backup")
        .bind("image")
        .bind("context-backup")
        .bind("pbt_backup")
        .bind("pbi_backup")
        .bind(&preparation_snapshot_json)
        .bind("2026-01-01T00:03:01Z")
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
        sqlx::query(
            "INSERT INTO style_profiles
             (id, project_id, name, style_prompt, color_prompt, line_prompt,
              negative_prompt, output_notes, active_revision_id, created_at, updated_at)
             VALUES ('stp_backup_style', 'project-backup', 'Backup Style', 'ink anime',
                     'violet', 'precise', 'photo', 'keep faces clear',
                     'prv_backup_style', '2026-01-01T00:05:00Z', '2026-01-01T00:05:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO reference_sets
             (id, project_id, name, purpose, description, owner_profile_type,
              owner_profile_id, active_revision_id, created_at, updated_at)
             VALUES
             ('rs_backup_character', 'project-backup', 'Backup Character References',
              'CHARACTER', 'character references', 'CHARACTER', 'cp_backup_character',
              NULL, '2026-01-01T00:05:01Z', '2026-01-01T00:05:01Z'),
             ('rs_backup_costume', 'project-backup', 'Backup Costume References',
              'COSTUME', 'costume references', 'CHARACTER', 'cp_backup_character',
              NULL, '2026-01-01T00:05:02Z', '2026-01-01T00:05:02Z'),
             ('rs_backup_scene', 'project-backup', 'Backup Scene References',
              'SCENE', 'scene references', 'SCENE', 'scp_backup_scene',
              NULL, '2026-01-01T00:05:03Z', '2026-01-01T00:05:03Z'),
             ('rs_backup_prop', 'project-backup', 'Backup Prop References',
              'PROP', 'prop references', 'PROP', 'pp_backup_prop',
              NULL, '2026-01-01T00:05:04Z', '2026-01-01T00:05:04Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO character_profiles
             (id, project_id, name, description, canonical_prompt, negative_prompt,
              default_style_profile_id, default_reference_set_id, active_revision_id,
              metadata_json, created_at, updated_at)
             VALUES ('cp_backup_character', 'project-backup', 'Backup Character',
                     'character description', 'hero prompt', 'blurry',
                     'stp_backup_style', 'rs_backup_character', 'prv_backup_character',
                     '{\"species\":\"human\"}', '2026-01-01T00:05:05Z',
                     '2026-01-01T00:05:05Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO scene_profiles
             (id, project_id, name, description, environment_prompt, lighting_prompt,
              negative_prompt, default_style_profile_id, default_reference_set_id,
              active_revision_id, created_at, updated_at)
             VALUES ('scp_backup_scene', 'project-backup', 'Backup Scene',
                     'scene description', 'rainy street', 'neon',
                     'empty', 'stp_backup_style', 'rs_backup_scene', 'prv_backup_scene',
                     '2026-01-01T00:05:06Z', '2026-01-01T00:05:06Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO prop_profiles
             (id, project_id, name, description, canonical_prompt, material_prompt,
              scale_prompt, default_reference_set_id, active_revision_id,
              created_at, updated_at)
             VALUES ('pp_backup_prop', 'project-backup', 'Backup Prop',
                     'prop description', 'lantern', 'brass', 'hand-sized',
                     'rs_backup_prop', 'prv_backup_prop',
                     '2026-01-01T00:05:07Z', '2026-01-01T00:05:07Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO costume_variants
             (id, character_profile_id, name, prompt_fragment, reference_set_id,
              is_default, ordinal, active_revision_id, created_at, updated_at)
             VALUES ('cv_backup_costume', 'cp_backup_character', 'Travel Coat',
                     'dark travel coat', 'rs_backup_costume', 1, 7, NULL,
                     '2026-01-01T00:05:08Z', '2026-01-01T00:05:08Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        for (revision_id, profile_type, profile_id, content) in [
            (
                "prv_backup_character",
                "CHARACTER",
                "cp_backup_character",
                "{\"revision\":\"character\"}",
            ),
            (
                "prv_backup_scene",
                "SCENE",
                "scp_backup_scene",
                "{\"revision\":\"scene\"}",
            ),
            (
                "prv_backup_prop",
                "PROP",
                "pp_backup_prop",
                "{\"revision\":\"prop\"}",
            ),
            (
                "prv_backup_style",
                "STYLE",
                "stp_backup_style",
                "{\"revision\":\"style\"}",
            ),
        ] {
            sqlx::query(
                "INSERT INTO profile_revisions
                 (id, profile_type, profile_id, revision_number, content_json,
                  content_sha256, status, created_at, created_by)
                 VALUES (?, ?, ?, 1, ?, ?, 'ACTIVE', '2026-01-01T00:05:09Z', 'dev051')",
            )
            .bind(revision_id)
            .bind(profile_type)
            .bind(profile_id)
            .bind(content)
            .bind("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .execute(&pool)
            .await
            .unwrap();
        }
        sqlx::query(
            "INSERT INTO reference_set_items
             (reference_set_id, asset_id, ordinal, role, is_primary, created_at)
             VALUES
             ('rs_backup_character', 'ast_backup', 0, 'front', 1, '2026-01-01T00:05:10Z'),
             ('rs_backup_character', 'ast_source_backup', 1, 'side', 0, '2026-01-01T00:05:10Z'),
             ('rs_backup_costume', 'ast_ref_b', 0, 'coat', 1, '2026-01-01T00:05:11Z'),
             ('rs_backup_scene', 'ast_ref_c', 0, 'wide', 1, '2026-01-01T00:05:12Z'),
             ('rs_backup_prop', 'ast_backup', 0, 'detail', 1, '2026-01-01T00:05:13Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shot_profile_bindings
             (id, shot_id, role, profile_type, profile_id, costume_variant_id,
              ordinal, inheritance_mode, created_at, updated_at)
             VALUES
             ('spb_backup_character', 'sht_backup', 'CHARACTER', 'CHARACTER',
              'cp_backup_character', 'cv_backup_costume', 3, 'REPLACE',
              '2026-01-01T00:05:14Z', '2026-01-01T00:05:14Z'),
             ('spb_backup_scene', 'sht_backup', 'SCENE', 'SCENE',
              'scp_backup_scene', NULL, 0, 'INHERITED',
              '2026-01-01T00:05:15Z', '2026-01-01T00:05:15Z'),
             ('spb_backup_prop', 'sht_backup', 'PROP', 'PROP',
              'pp_backup_prop', NULL, 2, 'EXPLICIT',
              '2026-01-01T00:05:16Z', '2026-01-01T00:05:16Z'),
             ('spb_backup_style', 'sht_backup', 'STYLE', 'STYLE',
              'stp_backup_style', NULL, 1, 'REMOVE',
              '2026-01-01T00:05:17Z', '2026-01-01T00:05:17Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shot_reference_set_bindings
             (id, shot_id, role, reference_set_id, ordinal, required,

              inheritance_mode, created_at, updated_at)
             VALUES
             ('srb_backup_character', 'sht_backup', 'CHARACTER',
              'rs_backup_character', 5, 1, 'REPLACE',
              '2026-01-01T00:05:18Z', '2026-01-01T00:05:18Z'),
             ('srb_backup_scene', 'sht_backup', 'SCENE',
              'rs_backup_scene', 0, 0, 'INHERITED',
              '2026-01-01T00:05:19Z', '2026-01-01T00:05:19Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO consistency_scope_profile_bindings
             (id, project_id, scope_type, scope_id, role, profile_type, profile_id,
              costume_variant_id, ordinal, inheritance_mode, created_at, updated_at)
             VALUES
             ('hpb_backup_project', 'project-backup', 'PROJECT', 'project-backup',
              'CHARACTER', 'CHARACTER', 'cp_backup_character', NULL, 4, 'INHERITED',
              '2026-01-01T00:05:20Z', '2026-01-01T00:05:20Z'),
             ('hpb_backup_scene', 'project-backup', 'SCENE', 'scn_backup',
              'SCENE', 'SCENE', 'scp_backup_scene', NULL, 6, 'REPLACE',
              '2026-01-01T00:05:21Z', '2026-01-01T00:05:21Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO consistency_scope_reference_set_bindings
             (id, project_id, scope_type, scope_id, role, reference_set_id,
              ordinal, required, inheritance_mode, created_at, updated_at)
             VALUES
             ('hrb_backup_project', 'project-backup', 'PROJECT', 'project-backup',
              'CHARACTER', 'rs_backup_character', 3, 1, 'INHERITED',
              '2026-01-01T00:05:22Z', '2026-01-01T00:05:22Z'),
             ('hrb_backup_scene', 'project-backup', 'SCENE', 'scn_backup',
              'PROP', 'rs_backup_prop', 1, 0, 'REMOVE',
              '2026-01-01T00:05:23Z', '2026-01-01T00:05:23Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO project_templates (id, name, normalized_name, description, workflow_version_id, recipe_id, values_json, created_at, updated_at) VALUES ('ptm_global', '全局模板', '全局模板', NULL, 'workflow-version-1', 'recipe-1', '{}', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')").execute(&pool).await.unwrap();

        let service = test_service(&pool, data_dirs.projects.clone(), data_dirs.cache.clone());
        let archive_path = directory.path().join("backup.zip");
        let exported = service
            .export("project-backup", archive_path.clone())
            .await
            .unwrap();
        assert!(exported.entries >= 6);
        let (manifest, document, names) = inspect_archive(&archive_path).unwrap();
        assert_eq!(manifest.format, "ai-studio-project-backup");
        assert_eq!(manifest.version, 20);
        assert_eq!(document.project_workflow_bindings.len(), 2);
        assert_eq!(document.project_workflow_bindings[0].stage, "IMAGE");
        assert_eq!(
            document.project_workflow_bindings[1].mode,
            "FL2VA_TEXT_TO_VIDEO"
        );
        assert_eq!(exported.entries, names.len());
        assert!(names.contains("production_preparation_snapshots.json"));
        assert_eq!(document.preparation_snapshots.len(), 1);
        assert_eq!(
            document.preparation_snapshots[0].snapshot_json,
            preparation_snapshot_json
        );
        assert!(!names.contains("app.db"));
        assert!(!names.contains("workflow_api.json"));
        assert!(!names.contains("recipe.yaml"));
        let preview = service.inspect(archive_path).await.unwrap();
        assert_eq!(preview.image_count, 4);
        assert_eq!(preview.shots, 1);
        let restored = service.restore(&preview.inspection_id).await.unwrap();
        assert_ne!(restored.id, "project-backup");
        let restored_bindings: Vec<(String, String, String, String, String)> = sqlx::query_as(
            "SELECT project_id, stage, mode, workflow_version_id, recipe_id
             FROM project_workflow_bindings WHERE project_id = ? ORDER BY stage, mode",
        )
        .bind(&restored.id)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(restored_bindings.len(), 2);
        assert!(restored_bindings.iter().all(|row| row.0 == restored.id));
        assert_eq!(restored_bindings[0].1, "IMAGE");
        assert_eq!(restored_bindings[1].2, "FL2VA_TEXT_TO_VIDEO");
        let restored_preparation_snapshot: (
            String,
            String,
            String,
            String,
            String,
            String,
            String,
        ) = sqlx::query_as(
            "SELECT id, project_id, shot_id, context_hash, production_batch_id,
                        production_batch_item_id, snapshot_json
                 FROM production_preparation_snapshots WHERE project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_ne!(restored_preparation_snapshot.0, "pps_backup");
        assert_eq!(restored_preparation_snapshot.1, restored.id);
        assert_ne!(restored_preparation_snapshot.2, "sht_backup");
        assert_eq!(restored_preparation_snapshot.3, "context-backup");
        assert_ne!(restored_preparation_snapshot.4, "pbt_backup");
        assert_ne!(restored_preparation_snapshot.5, "pbi_backup");
        assert_eq!(restored_preparation_snapshot.6, preparation_snapshot_json);
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
        let restored_structure: (String, String, String, String) = sqlx::query_as(
            "SELECT s.id, e.id, c.id, a.shot_id
             FROM production_series s
             JOIN production_episodes e ON e.series_id = s.id
             JOIN production_scenes c ON c.episode_id = e.id
             JOIN shot_scene_assignments a ON a.scene_id = c.id
             WHERE s.project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_ne!(restored_structure.0, "ser_backup");
        assert_ne!(restored_structure.1, "ep_backup");
        assert_ne!(restored_structure.2, "scn_backup");
        assert_ne!(restored_structure.3, "sht_backup");
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
        let service = test_service(&pool, data_dirs.projects.clone(), data_dirs.cache.clone());
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
        let service = test_service(&pool, data_dirs.projects.clone(), data_dirs.cache.clone());
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
        let service = test_service(&pool, data_dirs.projects.clone(), data_dirs.cache.clone());
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
        let service = test_service(&pool, data_dirs.projects.clone(), data_dirs.cache.clone());
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
    async fn fixed_v5_through_v9_and_v12_and_v13_fixtures_restore_with_empty_later_data() {
        let directory = tempdir().unwrap();
        let data_dirs = AppDataDirs::initialize(directory.path().join("AIStudioData")).unwrap();
        let pool = initialize(&data_dirs.database).await.unwrap();
        let service = test_service(&pool, data_dirs.projects.clone(), data_dirs.cache.clone());

        for version in [5_u32, 6_u32, 7_u32, 8_u32, 9_u32, 12_u32, 13_u32, 17_u32] {
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
            let consistency_count: i64 = sqlx::query_scalar(
                "SELECT
                   (SELECT COUNT(*) FROM character_profiles)
                 + (SELECT COUNT(*) FROM scene_profiles)
                 + (SELECT COUNT(*) FROM prop_profiles)
                 + (SELECT COUNT(*) FROM style_profiles)
                 + (SELECT COUNT(*) FROM costume_variants)
                 + (SELECT COUNT(*) FROM profile_revisions)
                 + (SELECT COUNT(*) FROM reference_sets)
                 + (SELECT COUNT(*) FROM reference_set_items)
                 + (SELECT COUNT(*) FROM shot_profile_bindings)
                 + (SELECT COUNT(*) FROM shot_reference_set_bindings)
                 + (SELECT COUNT(*) FROM consistency_scope_profile_bindings)
                 + (SELECT COUNT(*) FROM consistency_scope_reference_set_bindings)",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(consistency_count, 0);
            let preparation_snapshot_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM production_preparation_snapshots WHERE project_id = ?",
            )
            .bind(&restored.id)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(preparation_snapshot_count, 0);
        }
    }

    #[tokio::test]
    async fn snapshot_remap_failure_rolls_back_all_restore_rows() {
        let directory = tempdir().unwrap();
        let data_dirs = AppDataDirs::initialize(directory.path().join("AIStudioData")).unwrap();
        let pool = initialize(&data_dirs.database).await.unwrap();
        let service = test_service(&pool, data_dirs.projects.clone(), data_dirs.cache.clone());
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
                model_version_id: None,
                prompt_version_id: None,
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
                model_version_id: None,
                created_at: now.to_rfc3339(),
            }],
            batches: Vec::new(),
            items: Vec::new(),
            preparation_snapshots: Vec::new(),
            workflow_refs: Vec::new(),
            project_workflow_bindings: Vec::new(),
            workflow_registry: None,
            asset_tags: Vec::new(),
            asset_tag_links: Vec::new(),
            asset_favorites: Vec::new(),
            asset_video_prompts: Vec::new(),
            reference_anchors: Vec::new(),
            production_series: Vec::new(),
            production_episodes: Vec::new(),
            production_scenes: Vec::new(),
            shot_scene_assignments: Vec::new(),
            script_sources: Vec::new(),
            script_draft_revisions: Vec::new(),
            production_item_reviews: Vec::new(),
            artifact_reviews: Vec::new(),
            benchmark_experiments: Vec::new(),
            benchmark_candidates: Vec::new(),
            production_runs: Vec::new(),
            production_stages: Vec::new(),
            production_stage_items: Vec::new(),
            production_run_templates: Vec::new(),
            benchmark_runs: Vec::new(),
            benchmark_quality_scores: Vec::new(),
            shots: Vec::new(),
            external_production_handoffs: Vec::new(),
            external_production_handoff_entities: Vec::new(),
            shot_stage_configs: Vec::new(),
            shot_stage_prompts: Vec::new(),
            shot_reference_assets: Vec::new(),
            shot_generation_links: Vec::new(),
            character_profiles: Vec::new(),
            scene_profiles: Vec::new(),
            prop_profiles: Vec::new(),
            style_profiles: Vec::new(),
            costume_variants: Vec::new(),
            profile_revisions: Vec::new(),
            reference_sets: Vec::new(),
            reference_set_items: Vec::new(),
            shot_profile_bindings: Vec::new(),
            shot_reference_set_bindings: Vec::new(),
            scope_profile_bindings: Vec::new(),
            scope_reference_set_bindings: Vec::new(),
            asset_versions: Vec::new(),
            asset_relations: Vec::new(),
            models: Vec::new(),
            model_versions: Vec::new(),
            tools: Vec::new(),
            tool_versions: Vec::new(),
            tool_capabilities: Vec::new(),
            tool_instances: Vec::new(),
            generation_tool_usages: Vec::new(),
            generation_asset_versions: Vec::new(),
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
                &ProductionStructureIds::default(),
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new(),
                &super::ConsistencyRestoreIds::default(),
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
            preparation_snapshots: Vec::new(),
            workflow_refs: Vec::new(),
            project_workflow_bindings: Vec::new(),
            workflow_registry: None,
            asset_tags: Vec::new(),
            asset_tag_links: Vec::new(),
            asset_favorites: Vec::new(),
            asset_video_prompts: Vec::new(),
            reference_anchors: Vec::new(),
            production_series: Vec::new(),
            production_episodes: Vec::new(),
            production_scenes: Vec::new(),
            shot_scene_assignments: Vec::new(),
            script_sources: Vec::new(),
            script_draft_revisions: Vec::new(),
            production_item_reviews: Vec::new(),
            artifact_reviews: Vec::new(),
            benchmark_experiments: Vec::new(),
            benchmark_candidates: Vec::new(),
            production_runs: Vec::new(),
            production_stages: Vec::new(),
            production_stage_items: Vec::new(),
            production_run_templates: Vec::new(),
            benchmark_runs: Vec::new(),
            benchmark_quality_scores: Vec::new(),
            shots: Vec::new(),
            external_production_handoffs: Vec::new(),
            external_production_handoff_entities: Vec::new(),
            shot_stage_configs: Vec::new(),
            shot_stage_prompts: Vec::new(),
            shot_reference_assets: Vec::new(),
            shot_generation_links: Vec::new(),
            character_profiles: Vec::new(),
            scene_profiles: Vec::new(),
            prop_profiles: Vec::new(),
            style_profiles: Vec::new(),
            costume_variants: Vec::new(),
            profile_revisions: Vec::new(),
            reference_sets: Vec::new(),
            reference_set_items: Vec::new(),
            shot_profile_bindings: Vec::new(),
            shot_reference_set_bindings: Vec::new(),
            scope_profile_bindings: Vec::new(),
            scope_reference_set_bindings: Vec::new(),
            asset_versions: Vec::new(),
            asset_relations: Vec::new(),
            models: Vec::new(),
            model_versions: Vec::new(),
            tools: Vec::new(),
            tool_versions: Vec::new(),
            tool_capabilities: Vec::new(),
            tool_instances: Vec::new(),
            generation_tool_usages: Vec::new(),
            generation_asset_versions: Vec::new(),
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

    #[tokio::test]
    async fn missing_project_workflow_dependencies_are_reported_but_restore_succeeds() {
        let directory = tempdir().unwrap();
        let data_dirs = AppDataDirs::initialize(directory.path().join("AIStudioData")).unwrap();
        let pool = initialize(&data_dirs.database).await.unwrap();
        let project_root = data_dirs.projects.join("stale-binding");
        std::fs::create_dir_all(&project_root).unwrap();
        sqlx::query(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at)
             VALUES ('stale-binding', '失效绑定项目', ?, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .bind(project_root.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO project_workflow_bindings
             (project_id, stage, mode, workflow_version_id, recipe_id, created_at, updated_at)
             VALUES ('stale-binding', 'VIDEO', 'DEFAULT', 'missing-version', 'missing-recipe',
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let service = test_service(&pool, data_dirs.projects.clone(), data_dirs.cache.clone());
        let archive_path = directory.path().join("stale-binding.zip");
        service
            .export("stale-binding", archive_path.clone())
            .await
            .unwrap();
        let preview = service.inspect(archive_path).await.unwrap();
        assert!(preview
            .missing_workflows
            .contains(&"missing-version".to_owned()));
        assert!(preview
            .missing_workflows
            .contains(&"missing-recipe".to_owned()));

        let restored = service.restore(&preview.inspection_id).await.unwrap();
        let restored_binding: (String, String, String, String) = sqlx::query_as(
            "SELECT stage, mode, workflow_version_id, recipe_id

             FROM project_workflow_bindings WHERE project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            restored_binding,
            (
                "VIDEO".to_owned(),
                "DEFAULT".to_owned(),
                "missing-version".to_owned(),
                "missing-recipe".to_owned()
            )
        );
    }

    #[tokio::test]
    async fn current_v20_export_is_immediately_inspectable() {
        let directory = tempdir().unwrap();
        let (_pool, service, project_id, archive) =
            seed_current_v20_legacy_shot_fixture(&directory).await;

        service
            .export(&project_id, archive.clone())
            .await
            .expect("current v20 export");
        let preview = service
            .inspect(archive)
            .await
            .expect("a current v20 export must be immediately inspectable");

        assert_eq!(preview.shots, 1);
        assert_eq!(preview.asset_versions, 1);
        assert_eq!(preview.asset_relations, 1);
        assert_eq!(preview.artifact_reviews, 1);
    }

    #[tokio::test]
    async fn current_v20_export_inspect_restore_round_trip() {
        let directory = tempdir().unwrap();
        let (pool, service, project_id, archive) =
            seed_current_v20_legacy_shot_fixture(&directory).await;

        service
            .export(&project_id, archive.clone())
            .await
            .expect("current v20 export");
        let preview = service
            .inspect(archive)
            .await
            .expect("current v20 export inspect");
        let restored = service
            .restore(&preview.inspection_id)
            .await
            .expect("current v20 restore");

        assert_eq!(restored.status, "COMPLETE");
        assert_eq!(restored.backup_version, 20);
        assert_eq!(restored.assets, 2);
        assert_eq!(restored.versions, 1);
        assert_eq!(restored.generations, 1);
        assert!(restored.warnings.is_empty());
        assert_ne!(restored.id, project_id);

        let restored_binding_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM project_workflow_bindings WHERE project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(restored_binding_count, 2);

        let restored_shot: (String, String, String, Option<String>) = sqlx::query_as(
            "SELECT id, prompt_entry_id, prompt_version_id, selected_image_asset_id
             FROM shots WHERE project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_ne!(restored_shot.0, "sht_current_v20");
        assert_ne!(restored_shot.1, "prm_current_v20");
        assert_ne!(restored_shot.2, "prv_current_v20");
        assert_ne!(restored_shot.3.as_deref(), Some("ast_current_v20"));

        let restored_stage_prompts: Vec<(String, String, Option<String>, Option<String>)> =
            sqlx::query_as(
                "SELECT stage, prompt_text, prompt_entry_id, prompt_version_id
                 FROM shot_stage_prompts WHERE shot_id = ? ORDER BY stage",
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
            vec![
                (
                    "image",
                    "A quiet harbor at dawn, one red balloon drifting above the water, wide cinematic composition."
                ),
                (
                    "video",
                    "A quiet harbor at dawn, one red balloon drifting above the water, wide cinematic composition."
                )
            ]
        );
        assert!(restored_stage_prompts
            .iter()
            .all(
                |(_, _, entry_id, version_id)| entry_id.as_deref() != Some("prm_current_v20")
                    && version_id.as_deref() != Some("prv_current_v20")
            ));

        let restored_task: String = sqlx::query_scalar("SELECT id FROM tasks WHERE project_id = ?")
            .bind(&restored.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_ne!(restored_task, "tsk_current_v20");
        let snapshot_inputs: String = sqlx::query_scalar(
            "SELECT user_inputs_json FROM generation_snapshots WHERE task_id = ?",
        )
        .bind(&restored_task)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!snapshot_inputs.contains("ast_current_v20"));
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM task_output_assets WHERE task_id = ?",
            )
            .bind(&restored_task)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM shot_generation_links WHERE shot_id = ?",
            )
            .bind(&restored_shot.0)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );

        let restored_asset: (String, String, String) = sqlx::query_as(
            "SELECT id, storage_path, sha256 FROM assets
             WHERE project_id = ? AND type = 'image' ORDER BY source_task_id IS NULL, id
             LIMIT 1",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_ne!(restored_asset.0, "ast_current_v20");
        assert_eq!(
            std::fs::read(&restored_asset.1).unwrap(),
            b"current-v20-image-bytes"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM artifact_reviews
                 WHERE project_id = ? AND artifact_id = ? AND decision = 'APPROVED'",
            )
            .bind(&restored.id)
            .bind(&restored_asset.0)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM asset_versions WHERE project_id = ?",
            )
            .bind(&restored.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM asset_relations WHERE project_id = ?",
            )
            .bind(&restored.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn backup_v20_exports_and_restores_artifact_reviews_with_remapped_assets() {
        let directory = tempdir().unwrap();
        let data_dirs = AppDataDirs::initialize(directory.path().join("AIStudioData")).unwrap();
        let pool = initialize(&data_dirs.database).await.unwrap();
        crate::infrastructure::database::repositories::test_support::seed_task_dependencies(&pool)
            .await;
        let project_root = data_dirs.projects.join("project-v19");
        std::fs::create_dir_all(&project_root).unwrap();
        sqlx::query(
            "INSERT INTO projects (id, name, description, root_path, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("project-v19")
        .bind("V19项目")
        .bind("test")
        .bind(project_root.to_string_lossy().to_string())
        .bind("2026-01-01T00:00:00Z")
        .bind("2026-01-01T00:00:00Z")
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO models (id, name, provider, type, description, metadata_json, created_at)
             VALUES ('mdl_existing', 'H3', 'MiniMax', 'video', '', '{}', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO model_versions
             (id, model_id, version, capabilities_json, parameter_schema_json, created_at)
             VALUES ('mdv_existing', 'mdl_existing', '2026-01', '[]', '{}', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        // Project-local prompt references the existing canonical model version.
        sqlx::query(
            "INSERT INTO prompt_entries
             (id, project_id, kind, name, normalized_name, tags_json, created_at, updated_at)
             VALUES ('prm_v19', 'project-v19', 'prompt', '提示', '提示', '[]',
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO prompt_versions (id, prompt_id, version, text, model_version_id, created_at)
             VALUES ('prv_v19', 'prm_v19', 1, 'text', 'mdv_existing', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO tasks
             (id, project_id, workflow_id, workflow_version_id, recipe_id, status, progress_mode,
              created_at, finished_at)
             VALUES ('tsk_v19', 'project-v19', 'workflow-1', 'workflow-version-1', 'recipe-1',
                     'SUCCEEDED', 'indeterminate', '2026-01-01T00:00:00Z', '2026-01-01T00:01:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO generation_snapshots
             (id, task_id, workflow_json, recipe_yaml, user_inputs_json, resolved_inputs_json,
              model_version_id, created_at)
             VALUES ('snp_v19', 'tsk_v19', '{}', 'schema_version: 1\ninputs: {}\n', '{}', '{}',
                     'mdv_existing', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let bytes = b"v19-asset-bytes";
        let sha = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let asset_path = project_root.join("asset.png");
        std::fs::write(&asset_path, bytes).unwrap();
        sqlx::query(
            "INSERT INTO assets
             (id, project_id, type, category, name, original_name, storage_path, sha256, mime_type,
              width, height, file_size, metadata_json, created_at, updated_at, source_task_id)
             VALUES ('ast_v19', 'project-v19', 'image', 'generated_image', '图', 'a.png', ?, ?,
                     'image/png', 1, 1, ?, '{}', '2026-01-01T00:01:00Z', '2026-01-01T00:01:00Z',
                     'tsk_v19')",
        )
        .bind(asset_path.to_string_lossy().to_string())
        .bind(&sha)
        .bind(bytes.len() as i64)
        .execute(&pool)
        .await
        .unwrap();
        let bytes2 = b"v19-source-bytes";
        let sha2 = Sha256::digest(bytes2)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let source_path = project_root.join("source.png");
        std::fs::write(&source_path, bytes2).unwrap();
        sqlx::query(
            "INSERT INTO assets
             (id, project_id, type, category, name, original_name, storage_path, sha256, mime_type,
              width, height, file_size, metadata_json, created_at, updated_at)
             VALUES ('ast_v19_src', 'project-v19', 'image', 'source_image', '源', 's.png', ?, ?,
                     'image/png', 1, 1, ?, '{}', '2026-01-01T00:00:30Z', '2026-01-01T00:00:30Z')",
        )
        .bind(source_path.to_string_lossy().to_string())
        .bind(&sha2)
        .bind(bytes2.len() as i64)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO task_output_assets (task_id, output_id, ordinal, asset_id, created_at)
             VALUES ('tsk_v19', 'out_1', 0, 'ast_v19', '2026-01-01T00:01:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO production_batches
             (id, project_id, name, status, continue_on_failure, created_at, updated_at)
             VALUES ('pbt_v19', 'project-v19', '旧批次', 'COMPLETE', 0,
                     '2026-01-01T00:01:00Z', '2026-01-01T00:01:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO production_batch_items
             (id, batch_id, ordinal, workflow_version_id, recipe_id, values_json, status,
              task_id, created_at, updated_at)
             VALUES ('pbi_v19', 'pbt_v19', 0, 'workflow-version-1', 'recipe-1', '{}',
                     'SUCCEEDED', 'tsk_v19', '2026-01-01T00:01:00Z', '2026-01-01T00:01:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO production_item_reviews
             (id, project_id, production_batch_id, production_batch_item_id, task_id,
              result_asset_id, review_status, review_note, version, lineage_key,
              created_at, updated_at)
             VALUES ('pri_v19', 'project-v19', 'pbt_v19', 'pbi_v19', 'tsk_v19',
                     'ast_v19', 'APPROVED', 'old approved', 1, 'pbi_v19',
                     '2026-01-01T00:01:30Z', '2026-01-01T00:01:30Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE artifact_reviews SET decision = 'REJECTED', comment = '需要调整',
             revision = 2, updated_at = '2026-01-01T00:02:00Z'
             WHERE project_id = 'project-v19' AND artifact_id = 'ast_v19'",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO asset_versions
             (id, project_id, asset_id, version_number, metadata_snapshot, location, checksum, created_at)
             VALUES ('asv_v19', 'project-v19', 'ast_v19', 1, '{}', ?, ?, '2026-01-01T00:01:00Z')",
        )
        .bind(asset_path.to_string_lossy().to_string())
        .bind(&sha)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO asset_relations
             (id, project_id, source_asset_id, target_asset_id, relation_type, created_at)
             VALUES ('rel_v19', 'project-v19', 'ast_v19_src', 'ast_v19', 'SOURCE_OF',
                     '2026-01-01T00:01:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO tools (id, name, type, description, metadata_json, created_at)
             VALUES ('tool_v19', 'ComfyUI', 'local', '', '{}', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO tool_versions (id, tool_id, version, observed_at, metadata_json)
             VALUES ('tver_v19', 'tool_v19', '0.1', '2026-01-01T00:00:00Z', '{}')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO tool_instances (id, tool_id, path, endpoint, status, last_checked)
             VALUES ('tins_v19', 'tool_v19', '/opt/comfy', 'http://127.0.0.1:8188', 'AVAILABLE',
                     '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO generation_tool_usages
             (id, generation_id, tool_instance_id, tool_version_id, metadata_json, created_at)
             VALUES ('gtu_v19', 'tsk_v19', 'tins_v19', 'tver_v19', '{}', '2026-01-01T00:01:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO generation_asset_versions
             (id, generation_id, output_id, ordinal, asset_version_id, relation_type, created_at)
             VALUES ('gav_v19', 'tsk_v19', 'out_1', 0, 'asv_v19', 'OUTPUT', '2026-01-01T00:01:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let service = test_service(&pool, data_dirs.projects.clone(), data_dirs.cache.clone());
        let archive = directory.path().join("v19.zip");
        service
            .export("project-v19", archive.clone())
            .await
            .expect("export v19");
        let (manifest, document, _) = inspect_archive(&archive).unwrap();
        assert_eq!(manifest.version, 20);
        assert_eq!(document.asset_versions.len(), 1);
        assert_eq!(document.asset_relations.len(), 1);
        assert_eq!(document.models.len(), 1);
        assert_eq!(document.model_versions.len(), 1);
        assert_eq!(document.tools.len(), 1);
        assert_eq!(document.tool_instances.len(), 1);
        assert_eq!(document.generation_tool_usages.len(), 1);
        assert_eq!(document.generation_asset_versions.len(), 1);
        assert_eq!(document.artifact_reviews.len(), 1);
        assert_eq!(document.artifact_reviews[0].decision, "REJECTED");
        assert_eq!(document.artifact_reviews[0].comment, "需要调整");
        assert_eq!(document.production_item_reviews.len(), 1);
        let legacy_archive = directory.path().join("v19-without-artifact-reviews.zip");
        rewrite_v20_archive_as_v19(&archive, &legacy_archive);

        // Keep the source project (restore always creates a new project). Remove only the
        // tool instance so restore re-inserts it as UNKNOWN while reusing the tool row.
        sqlx::query("DELETE FROM generation_tool_usages WHERE id = 'gtu_v19'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM tool_instances WHERE id = 'tins_v19'")
            .execute(&pool)
            .await
            .unwrap();

        let preview = service.inspect(archive).await.unwrap();
        assert_eq!(preview.asset_versions, 1);
        assert_eq!(preview.artifact_reviews, 1);
        assert_eq!(preview.tools, 1);
        let restored = service.restore(&preview.inspection_id).await.unwrap();
        assert_eq!(restored.status, "COMPLETE");
        assert_eq!(restored.backup_version, 20);
        assert_eq!(restored.assets, 2);
        assert_eq!(restored.versions, 1);
        assert_eq!(restored.generations, 1);
        assert_eq!(restored.restored_generation_tool_usages, 1);
        assert_eq!(restored.restored_generation_asset_versions, 1);
        assert!(restored.warnings.is_empty());

        let restored_review: (String, String, i64, String) = sqlx::query_as(
            "SELECT decision, comment, revision, artifact_id FROM artifact_reviews
             WHERE project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(restored_review.0, "REJECTED");
        assert_eq!(restored_review.1, "需要调整");
        assert_eq!(restored_review.2, 2);
        assert_ne!(restored_review.3, "ast_v19");

        let legacy_preview = service.inspect(legacy_archive).await.unwrap();
        assert_eq!(legacy_preview.artifact_reviews, 0);
        let legacy_restored = service
            .restore(&legacy_preview.inspection_id)
            .await
            .unwrap();
        assert_eq!(legacy_restored.backup_version, 19);
        let legacy_review: (String, String) = sqlx::query_as(
            "SELECT decision, artifact_id FROM artifact_reviews WHERE project_id = ?",
        )
        .bind(&legacy_restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(legacy_review.0, "APPROVED");
        let legacy_comment: String =
            sqlx::query_scalar("SELECT comment FROM artifact_reviews WHERE project_id = ?")
                .bind(&legacy_restored.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(legacy_comment, "old approved");
        assert_ne!(legacy_review.1, "ast_v19");

        let version_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM asset_versions WHERE project_id = ?")
                .bind(&restored.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(version_count, 1);
        let remapped_asset: Option<String> =
            sqlx::query_scalar("SELECT asset_id FROM asset_versions WHERE project_id = ?")
                .bind(&restored.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(remapped_asset.is_some());
        assert_ne!(remapped_asset.as_deref(), Some("ast_v19"));
        let relation_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM asset_relations WHERE project_id = ?")
                .bind(&restored.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(relation_count, 1);
        let lineage_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM generation_asset_versions gav
             JOIN tasks t ON t.id = gav.generation_id WHERE t.project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(lineage_count, 1);
        let tool_usage_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM generation_tool_usages gtu
             JOIN tasks t ON t.id = gtu.generation_id WHERE t.project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(tool_usage_count, 1);
        let tool_status: String =
            sqlx::query_scalar("SELECT status FROM tool_instances WHERE id = 'tins_v19'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(tool_status, "UNKNOWN");
        let model_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM models WHERE provider = 'MiniMax' AND name = 'H3'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            model_count, 1,
            "model registry must reuse UNIQUE(provider,name)"
        );
        let prompt_model: Option<String> = sqlx::query_scalar(
            "SELECT model_version_id FROM prompt_versions pv
             JOIN prompt_entries pe ON pe.id = pv.prompt_id WHERE pe.project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(prompt_model.as_deref(), Some("mdv_existing"));
    }

    async fn project_backup_v15_restores_without_project_workflow_bindings() {
        let directory = tempdir().unwrap();
        let data_dirs = AppDataDirs::initialize(directory.path().join("AIStudioData")).unwrap();
        let pool = initialize(&data_dirs.database).await.unwrap();
        let archive_path = directory.path().join("backup-v15.zip");
        let file = File::create(&archive_path).unwrap();
        let mut writer = ZipWriter::new(file);
        let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
        let manifest = json!({
            "format": "ai-studio-project-backup",
            "version": 15,
            "createdBy": "1.0.0",
            "project": { "id": "legacy-v15", "name": "旧项目 V15" }
        });
        let document = json!({
            "project": { "id": "legacy-v15", "name": "旧项目 V15" },
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
            "workflowRefs": []
        });
        assert!(!document
            .as_object()
            .unwrap()
            .contains_key("projectWorkflowBindings"));
        writer.start_file("manifest.json", options).unwrap();
        writer
            .write_all(serde_json::to_string(&manifest).unwrap().as_bytes())
            .unwrap();
        writer.start_file("project.json", options).unwrap();
        writer
            .write_all(serde_json::to_string(&document).unwrap().as_bytes())
            .unwrap();
        writer.finish().unwrap();

        let service = test_service(&pool, data_dirs.projects.clone(), data_dirs.cache.clone());
        let preview = service.inspect(archive_path).await.unwrap();
        let restored = service.restore(&preview.inspection_id).await.unwrap();
        assert_ne!(restored.id, "legacy-v15");
        assert_eq!(restored.name, "旧项目 V15（恢复）");
        let binding_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM project_workflow_bindings WHERE project_id = ?",
        )
        .bind(restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(binding_count, 0);
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

    fn empty_archive_document(project_id: &str, name: &str) -> BackupDocument {
        let mut document = organization_document(Vec::new(), Vec::new());
        document.project = BackupProject {
            id: project_id.to_owned(),
            name: name.to_owned(),
        };
        document.assets.clear();
        document
    }

    #[test]
    fn v19_write_includes_provenance_and_aiarchive_friendly_package() {
        let directory = tempdir().unwrap();
        let mut document = empty_archive_document("prj_aiarchive", "归档包");
        document.generation_tool_usages = vec![BackupGenerationToolUsage {
            id: "gtu_pkg".to_owned(),
            generation_id: "tsk_1".to_owned(),
            tool_instance_id: "tins_1".to_owned(),
            tool_version_id: None,
            metadata_json: json!({}),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
        }];
        document.generation_asset_versions = vec![BackupGenerationAssetVersion {
            id: "gav_pkg".to_owned(),
            generation_id: "tsk_1".to_owned(),
            output_id: "out_1".to_owned(),
            ordinal: 0,
            asset_version_id: "asv_1".to_owned(),
            relation_type: "OUTPUT".to_owned(),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
        }];
        let bytes = b"aiarchive-bytes";
        let sha = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let source_path = directory.path().join("content.bin");
        std::fs::write(&source_path, bytes).unwrap();
        document.assets = vec![BackupAsset {
            id: "ast_pkg".to_owned(),
            asset_type: "image".to_owned(),
            category: "source_image".to_owned(),
            name: "图".to_owned(),
            original_name: "a.bin".to_owned(),
            sha256: sha.clone(),
            mime_type: "application/octet-stream".to_owned(),
            width: 1,
            height: 1,
            duration_ms: None,
            file_size: bytes.len() as i64,
            source_task_id: None,
            metadata: json!({}),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
            content_path: "assets/ast_pkg/content.bin".to_owned(),
            thumbnail_path: None,
        }];
        let files = [BackupFileSource {
            zip_path: "assets/ast_pkg/content.bin".to_owned(),
            source_path,
            expected_size: bytes.len() as u64,
            expected_sha256: Some(sha.clone()),
        }];
        let archive_path = directory.path().join("AI-Studio-Project.aiarchive");
        write_zip_to_path(&document, &files, &archive_path).unwrap();

        let (manifest, loaded, names) = inspect_archive(&archive_path).unwrap();
        assert_eq!(manifest.version, 20);
        assert!(manifest.logical_snapshot_checksum.is_some());
        assert_eq!(manifest.media_inventory.len(), 1);
        assert_eq!(manifest.media_inventory[0].sha256, sha);
        assert!(names.contains("provenance.json"));
        assert!(names.contains("manifest.json"));
        assert!(names.contains("project.json"));
        assert!(!names.iter().any(|name| name.contains("metadata.sqlite")));
        assert_eq!(loaded.generation_tool_usages.len(), 1);
        assert_eq!(loaded.generation_asset_versions.len(), 1);
        let inventory = manifest.inventory.expect("inventory");
        assert_eq!(inventory.assets, 1);
        assert_eq!(inventory.lineage, 2);
    }

    #[test]
    fn inspect_accepts_historical_v18_zip_without_provenance() {
        let directory = tempdir().unwrap();
        let archive_path = directory.path().join("legacy-v18.zip");
        let file = File::create(&archive_path).unwrap();
        let mut writer = ZipWriter::new(file);
        let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
        let manifest = json!({
            "format": "ai-studio-project-backup",
            "version": 18,
            "createdBy": "1.3.1",
            "project": { "id": "legacy-v18", "name": "旧归档 V18" }
        });
        let document = json!({
            "project": { "id": "legacy-v18", "name": "旧归档 V18" },
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
            "workflowRefs": []
        });
        writer.start_file("manifest.json", options).unwrap();
        writer
            .write_all(serde_json::to_string(&manifest).unwrap().as_bytes())
            .unwrap();
        writer.start_file("project.json", options).unwrap();
        writer
            .write_all(serde_json::to_string(&document).unwrap().as_bytes())
            .unwrap();
        writer.finish().unwrap();

        let (loaded_manifest, loaded_document, names) = inspect_archive(&archive_path).unwrap();
        assert_eq!(loaded_manifest.version, 18);
        assert!(loaded_manifest.logical_snapshot_checksum.is_none());
        assert!(loaded_manifest.media_inventory.is_empty());
        assert!(!names.contains("provenance.json"));
        assert!(loaded_document.generation_tool_usages.is_empty());
    }

    #[tokio::test]
    async fn inspect_does_not_write_db() {
        let directory = tempdir().unwrap();
        let data_dirs = AppDataDirs::initialize(directory.path().join("AIStudioData")).unwrap();
        let pool = initialize(&data_dirs.database).await.unwrap();
        let archive_path = directory.path().join("inspect-only.zip");
        let file = File::create(&archive_path).unwrap();
        let mut writer = ZipWriter::new(file);
        let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
        let manifest = json!({
            "format": "ai-studio-project-backup",
            "version": 18,
            "createdBy": "1.3.1",
            "project": { "id": "inspect-only", "name": "仅检查" }
        });
        let document = json!({
            "project": { "id": "inspect-only", "name": "仅检查" },
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
            "workflowRefs": []
        });
        writer.start_file("manifest.json", options).unwrap();
        writer
            .write_all(serde_json::to_string(&manifest).unwrap().as_bytes())
            .unwrap();
        writer.start_file("project.json", options).unwrap();
        writer
            .write_all(serde_json::to_string(&document).unwrap().as_bytes())
            .unwrap();
        writer.finish().unwrap();

        let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projects")
            .fetch_one(&pool)
            .await
            .unwrap();
        let service = test_service(&pool, data_dirs.projects.clone(), data_dirs.cache.clone());
        let preview = service.inspect(archive_path).await.unwrap();
        assert_eq!(preview.project_name, "仅检查");
        let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projects")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(before, after);
        let tasks_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(tasks_after, 0);
    }

    #[tokio::test]
    async fn restore_from_aiarchive_creates_new_project_id() {
        let directory = tempdir().unwrap();
        let data_dirs = AppDataDirs::initialize(directory.path().join("AIStudioData")).unwrap();
        let pool = initialize(&data_dirs.database).await.unwrap();
        let document = empty_archive_document("source-aiarchive", "源归档");
        let archive_path = directory.path().join("AI-Studio-Project.aiarchive");
        write_zip_to_path(&document, &[], &archive_path).unwrap();
        let service = test_service(&pool, data_dirs.projects.clone(), data_dirs.cache.clone());
        let preview = service.inspect(archive_path).await.unwrap();
        let restored = service.restore(&preview.inspection_id).await.unwrap();
        assert_ne!(restored.id, "source-aiarchive");
        assert!(restored.name.contains("恢复"));
    }

    #[test]
    fn media_checksum_mismatch_fails_inspect() {
        let directory = tempdir().unwrap();
        let bytes = b"good-bytes";
        let good_sha = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let source_path = directory.path().join("content.bin");
        std::fs::write(&source_path, bytes).unwrap();
        let mut document = empty_archive_document("prj_bad_hash", "坏校验");
        document.assets = vec![BackupAsset {
            id: "ast_bad".to_owned(),
            asset_type: "image".to_owned(),
            category: "source_image".to_owned(),
            name: "图".to_owned(),
            original_name: "a.bin".to_owned(),
            sha256: good_sha.clone(),
            mime_type: "application/octet-stream".to_owned(),
            width: 1,
            height: 1,
            duration_ms: None,
            file_size: bytes.len() as i64,
            source_task_id: None,
            metadata: json!({}),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
            content_path: "assets/ast_bad/content.bin".to_owned(),
            thumbnail_path: None,
        }];
        let files = [BackupFileSource {
            zip_path: "assets/ast_bad/content.bin".to_owned(),
            source_path,
            expected_size: bytes.len() as u64,
            expected_sha256: Some(good_sha),
        }];
        let archive_path = directory.path().join("bad-hash.aiarchive");
        write_zip_to_path(&document, &files, &archive_path).unwrap();

        // Tamper with the published package: rewrite manifest mediaInventory sha256.
        let file = File::open(&archive_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut rewritten =
            zip::ZipWriter::new(File::create(directory.path().join("tampered.aiarchive")).unwrap());
        let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).unwrap();
            let name = entry.name().to_owned();
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut bytes).unwrap();
            if name == "manifest.json" {
                let mut manifest: ProjectBackupManifest = serde_json::from_slice(&bytes).unwrap();
                manifest.media_inventory[0].sha256 =
                    "0000000000000000000000000000000000000000000000000000000000000000".to_owned();
                rewritten.start_file("manifest.json", options).unwrap();
                rewritten
                    .write_all(&serde_json::to_vec_pretty(&manifest).unwrap())
                    .unwrap();
            } else {
                rewritten.start_file(&name, options).unwrap();
                rewritten.write_all(&bytes).unwrap();
            }
        }
        rewritten.finish().unwrap();
        let err = inspect_archive(&directory.path().join("tampered.aiarchive")).unwrap_err();
        assert!(
            err.to_string().contains("校验") || format!("{err:?}").contains("校验"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn export_blocks_required_media_checksum_mismatch() {
        let directory = tempdir().unwrap();
        let source_path = directory.path().join("content.bin");
        std::fs::write(&source_path, b"actual-bytes").unwrap();
        let document = empty_archive_document("prj_export_bad", "导出失败");
        let files = [BackupFileSource {
            zip_path: "assets/ast_x/content.bin".to_owned(),
            source_path,
            expected_size: 12,
            expected_sha256: Some(
                "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_owned(),
            ),
        }];
        let archive_path = directory.path().join("export-bad.aiarchive");
        let err = write_zip_to_path(&document, &files, &archive_path).unwrap_err();
        assert!(
            err.to_string().contains("校验") || format!("{err:?}").contains("校验"),
            "unexpected error: {err:?}"
        );
    }

    fn archive_media_asset(id: &str, bytes: &[u8], content_rel: &str) -> (BackupAsset, String) {
        let sha = hash_bytes(bytes);
        (
            BackupAsset {
                id: id.to_owned(),
                asset_type: "image".to_owned(),
                category: "source_image".to_owned(),
                name: format!("asset-{id}"),
                original_name: format!("{id}.bin"),
                sha256: sha.clone(),
                mime_type: "application/octet-stream".to_owned(),
                width: 1,
                height: 1,
                duration_ms: None,
                file_size: bytes.len() as i64,
                source_task_id: None,
                metadata: json!({}),
                created_at: "2026-01-01T00:00:00Z".to_owned(),
                updated_at: "2026-01-01T00:00:00Z".to_owned(),
                content_path: content_rel.to_owned(),
                thumbnail_path: None,
            },
            sha,
        )
    }

    fn terminal_backup_task(id: &str, prompt_id: Option<&str>, timestamp: &str) -> BackupTask {
        BackupTask {
            id: id.to_owned(),
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
            prompt_id: prompt_id.map(str::to_owned),
            queue_number: None,
            progress_mode: "indeterminate".to_owned(),
            progress_current: None,
            progress_total: None,
            current_node_id: None,
            error_code: None,
            error_message: None,
            raw_error: None,
            created_at: timestamp.to_owned(),
            queued_at: None,
            started_at: None,
            finished_at: Some(timestamp.to_owned()),
        }
    }

    fn rewrite_manifest_entry(
        source: &Path,
        destination: &Path,
        mutate: impl FnOnce(&mut ProjectBackupManifest),
    ) {
        let file = File::open(source).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut rewritten = ZipWriter::new(File::create(destination).unwrap());
        let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
        let mut mutator = Some(mutate);
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).unwrap();
            let name = entry.name().to_owned();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            if name == "manifest.json" {
                let mut manifest: ProjectBackupManifest = serde_json::from_slice(&bytes).unwrap();
                mutator.take().expect("manifest.json appears once")(&mut manifest);
                rewritten.start_file("manifest.json", options).unwrap();
                rewritten
                    .write_all(&serde_json::to_vec_pretty(&manifest).unwrap())
                    .unwrap();
            } else {
                rewritten.start_file(&name, options).unwrap();
                rewritten.write_all(&bytes).unwrap();
            }
        }
        rewritten.finish().unwrap();
    }

    fn rewrite_v20_archive_as_v19(source: &Path, destination: &Path) {
        let mut source_archive = zip::ZipArchive::new(File::open(source).unwrap()).unwrap();
        let mut project_bytes = Vec::new();
        source_archive
            .by_name("project.json")
            .unwrap()
            .read_to_end(&mut project_bytes)
            .unwrap();
        let mut project: serde_json::Value = serde_json::from_slice(&project_bytes).unwrap();
        project.as_object_mut().unwrap().remove("artifactReviews");
        let legacy_project_bytes = serde_json::to_vec_pretty(&project).unwrap();
        let checksum = hash_bytes(&legacy_project_bytes);

        let mut source_archive = zip::ZipArchive::new(File::open(source).unwrap()).unwrap();
        let mut rewritten = ZipWriter::new(File::create(destination).unwrap());
        let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
        for index in 0..source_archive.len() {
            let mut entry = source_archive.by_index(index).unwrap();
            let name = entry.name().to_owned();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            let bytes = match name.as_str() {
                "manifest.json" => {
                    let mut manifest: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                    manifest["version"] = json!(19);
                    manifest["logicalSnapshotChecksum"] = json!(checksum);
                    if let Some(inventory) = manifest
                        .get_mut("inventory")
                        .and_then(serde_json::Value::as_object_mut)
                    {
                        inventory.remove("artifactReviews");
                    }
                    serde_json::to_vec_pretty(&manifest).unwrap()
                }
                "project.json" => legacy_project_bytes.clone(),
                _ => bytes,
            };
            rewritten.start_file(name, options).unwrap();
            rewritten.write_all(&bytes).unwrap();
        }
        rewritten.finish().unwrap();
    }

    #[tokio::test]
    async fn truncated_corrupt_zip_fails_inspect_without_db_mutation() {
        let directory = tempdir().unwrap();
        let data_dirs = AppDataDirs::initialize(directory.path().join("AIStudioData")).unwrap();
        let pool = initialize(&data_dirs.database).await.unwrap();
        let document = empty_archive_document("prj_truncated", "截断包");
        let archive_path = directory.path().join("good.aiarchive");
        write_zip_to_path(&document, &[], &archive_path).unwrap();
        let good_bytes = std::fs::read(&archive_path).unwrap();
        assert!(good_bytes.len() > 32);
        let truncated_path = directory.path().join("truncated.aiarchive");
        std::fs::write(&truncated_path, &good_bytes[..good_bytes.len() / 2]).unwrap();

        let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projects")
            .fetch_one(&pool)
            .await
            .unwrap();
        let service = test_service(&pool, data_dirs.projects.clone(), data_dirs.cache.clone());
        let err = service.inspect(truncated_path).await.unwrap_err();
        assert_eq!(err.code(), "BACKUP_INVALID");
        let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projects")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(before, after);
        let tasks_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(tasks_after, 0);
    }

    #[test]
    fn logical_snapshot_checksum_mismatch_fails_inspect() {
        let directory = tempdir().unwrap();
        let document = empty_archive_document("prj_checksum", "校验失败");
        let archive_path = directory.path().join("checksum.aiarchive");
        write_zip_to_path(&document, &[], &archive_path).unwrap();
        let tampered = directory.path().join("checksum-bad.aiarchive");
        rewrite_manifest_entry(&archive_path, &tampered, |manifest| {
            manifest.logical_snapshot_checksum =
                Some("0000000000000000000000000000000000000000000000000000000000000000".to_owned());
        });
        let err = inspect_archive(&tampered).unwrap_err();
        assert_eq!(err.code(), "BACKUP_INVALID");
        assert!(
            err.to_string().contains("逻辑快照") || format!("{err:?}").contains("逻辑快照"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn inventory_count_mismatch_fails_inspect() {
        let directory = tempdir().unwrap();
        let document = empty_archive_document("prj_inventory", "清单失败");
        let archive_path = directory.path().join("inventory.aiarchive");
        write_zip_to_path(&document, &[], &archive_path).unwrap();
        let tampered = directory.path().join("inventory-bad.aiarchive");
        rewrite_manifest_entry(&archive_path, &tampered, |manifest| {
            let mut inventory = manifest.inventory.clone().unwrap_or(BackupInventoryCounts {
                assets: 0,
                asset_versions: 0,
                relations: 0,
                prompts: 0,
                models: 0,
                tools: 0,
                lineage: 0,
                tasks: 0,
                artifact_reviews: 0,
            });
            inventory.assets = inventory.assets.saturating_add(99);
            manifest.inventory = Some(inventory);
        });
        let err = inspect_archive(&tampered).unwrap_err();
        assert_eq!(err.code(), "BACKUP_INVALID");
        assert!(
            err.to_string().contains("清单") || format!("{err:?}").contains("清单"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn media_size_mismatch_fails_inspect() {
        let directory = tempdir().unwrap();
        let bytes = b"size-bytes";
        let source_path = directory.path().join("content.bin");
        std::fs::write(&source_path, bytes).unwrap();
        let (asset, sha) = archive_media_asset("ast_size", bytes, "assets/ast_size/content.bin");
        let mut document = empty_archive_document("prj_size", "大小失败");
        document.assets = vec![asset];
        let files = [BackupFileSource {
            zip_path: "assets/ast_size/content.bin".to_owned(),
            source_path,
            expected_size: bytes.len() as u64,
            expected_sha256: Some(sha),
        }];
        let archive_path = directory.path().join("size.aiarchive");
        write_zip_to_path(&document, &files, &archive_path).unwrap();
        let tampered = directory.path().join("size-bad.aiarchive");
        rewrite_manifest_entry(&archive_path, &tampered, |manifest| {
            manifest.media_inventory[0].size = bytes.len() as u64 + 1;
        });
        let err = inspect_archive(&tampered).unwrap_err();
        assert_eq!(err.code(), "BACKUP_ASSET_HASH_MISMATCH");
    }

    #[test]
    fn export_blocks_required_media_size_mismatch() {
        let directory = tempdir().unwrap();
        let source_path = directory.path().join("content.bin");
        std::fs::write(&source_path, b"twelve-bytes").unwrap();
        let document = empty_archive_document("prj_export_size", "导出大小失败");
        let files = [BackupFileSource {
            zip_path: "assets/ast_y/content.bin".to_owned(),
            source_path,
            expected_size: 999,
            expected_sha256: Some(hash_bytes(b"twelve-bytes")),
        }];
        let archive_path = directory.path().join("export-size-bad.aiarchive");
        let err = write_zip_to_path(&document, &files, &archive_path).unwrap_err();
        assert_eq!(err.code(), "BACKUP_ASSET_HASH_MISMATCH");
    }

    #[test]
    fn zip_path_traversal_regression_still_rejected() {
        // Keep the DEV-129-C zip-slip regression assertion for DEV-129-D hardening.
        assert!(!safe_zip_path("../escape.txt"));
        assert!(!safe_zip_path("assets/../../etc/passwd"));
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::FileOptions::default();
        writer.start_file("manifest.json", options).unwrap();
        writer.write_all(b"{}").unwrap();
        writer.start_file("../escape.txt", options).unwrap();
        writer.write_all(b"blocked").unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let directory = tempdir().unwrap();
        let path = directory.path().join("unsafe-regression.zip");
        std::fs::write(&path, bytes).unwrap();
        assert!(inspect_archive(&path).is_err());
    }

    #[tokio::test]
    async fn restore_writes_media_under_new_project_root_not_archived_host_paths() {
        let directory = tempdir().unwrap();
        let data_dirs = AppDataDirs::initialize(directory.path().join("AIStudioData")).unwrap();
        let pool = initialize(&data_dirs.database).await.unwrap();
        let old_host_root = directory.path().join("old-host-project");
        std::fs::create_dir_all(&old_host_root).unwrap();
        let bytes = b"path-change-bytes";
        let archived_absolute = old_host_root
            .join("assets/source_image/image/ast_old.bin")
            .to_string_lossy()
            .to_string();
        let source_path = directory.path().join("content.bin");
        std::fs::write(&source_path, bytes).unwrap();
        let (asset, sha) = archive_media_asset("ast_old", bytes, "assets/ast_old/content.bin");
        // Simulate an export that still recorded absolute host paths on versions.
        let mut document = empty_archive_document("source-path-change", "路径迁移");
        document.assets = vec![asset.clone()];
        document.asset_versions = vec![BackupAssetVersion {
            id: "asv_old".to_owned(),
            project_id: "source-path-change".to_owned(),
            asset_id: "ast_old".to_owned(),
            version_number: 1,
            metadata_snapshot: json!({}),
            location: archived_absolute.clone(),
            checksum: sha.clone(),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
        }];
        let files = [BackupFileSource {
            zip_path: asset.content_path.clone(),
            source_path,
            expected_size: bytes.len() as u64,
            expected_sha256: Some(sha),
        }];
        let archive_path = directory.path().join("path-change.aiarchive");
        write_zip_to_path(&document, &files, &archive_path).unwrap();

        let service = test_service(&pool, data_dirs.projects.clone(), data_dirs.cache.clone());
        let preview = service.inspect(archive_path).await.unwrap();
        let restored = service.restore(&preview.inspection_id).await.unwrap();
        assert_ne!(restored.id, "source-path-change");

        let storage_path: String =
            sqlx::query_scalar("SELECT storage_path FROM assets WHERE project_id = ?")
                .bind(&restored.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let version_location: String =
            sqlx::query_scalar("SELECT location FROM asset_versions WHERE project_id = ?")
                .bind(&restored.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let new_root = data_dirs.projects.join(&restored.id);
        let new_root_str = new_root.to_string_lossy().to_string();
        assert!(
            storage_path.starts_with(&new_root_str),
            "storage_path={storage_path} new_root={new_root_str}"
        );
        assert!(
            version_location.starts_with(&new_root_str),
            "version_location={version_location} new_root={new_root_str}"
        );
        assert!(!storage_path.contains("old-host-project"));
        assert!(!version_location.contains("old-host-project"));
        assert_ne!(version_location, archived_absolute);
        assert!(Path::new(&storage_path).is_file());
    }

    #[tokio::test]
    async fn restore_id_remap_keeps_versions_relations_lineage_in_new_project() {
        let directory = tempdir().unwrap();
        let data_dirs = AppDataDirs::initialize(directory.path().join("AIStudioData")).unwrap();
        let pool = initialize(&data_dirs.database).await.unwrap();
        crate::infrastructure::database::repositories::test_support::seed_task_dependencies(&pool)
            .await;

        let bytes_a = b"remap-a";
        let bytes_b = b"remap-b";
        let path_a = directory.path().join("a.bin");
        let path_b = directory.path().join("b.bin");
        std::fs::write(&path_a, bytes_a).unwrap();
        std::fs::write(&path_b, bytes_b).unwrap();
        let (asset_a, sha_a) =
            archive_media_asset("ast_src_a", bytes_a, "assets/ast_src_a/content.bin");
        let (asset_b, sha_b) =
            archive_media_asset("ast_src_b", bytes_b, "assets/ast_src_b/content.bin");
        let old_project = "source-remap-ids";
        let mut document = empty_archive_document(old_project, "重映射");
        document.assets = vec![asset_a.clone(), asset_b.clone()];
        document.tasks = vec![BackupTask {
            id: "tsk_src".to_owned(),
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
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            queued_at: None,
            started_at: None,
            finished_at: Some("2026-01-01T00:01:00Z".to_owned()),
        }];
        document.asset_versions = vec![BackupAssetVersion {
            id: "asv_src".to_owned(),
            project_id: old_project.to_owned(),
            asset_id: "ast_src_b".to_owned(),
            version_number: 1,
            metadata_snapshot: json!({}),
            location: "/old/host/ast_src_b.bin".to_owned(),
            checksum: sha_b.clone(),
            created_at: "2026-01-01T00:01:00Z".to_owned(),
        }];
        document.asset_relations = vec![BackupAssetRelation {
            id: "rel_src".to_owned(),
            project_id: old_project.to_owned(),
            source_asset_id: "ast_src_a".to_owned(),
            target_asset_id: "ast_src_b".to_owned(),
            relation_type: "SOURCE_OF".to_owned(),
            created_at: "2026-01-01T00:01:00Z".to_owned(),
        }];
        document.generation_asset_versions = vec![BackupGenerationAssetVersion {
            id: "gav_src".to_owned(),
            generation_id: "tsk_src".to_owned(),
            output_id: "out_1".to_owned(),
            ordinal: 0,
            asset_version_id: "asv_src".to_owned(),
            relation_type: "OUTPUT".to_owned(),
            created_at: "2026-01-01T00:01:00Z".to_owned(),
        }];
        document.mappings = vec![BackupMapping {
            task_id: "tsk_src".to_owned(),
            output_id: "out_1".to_owned(),
            ordinal: 0,
            asset_id: "ast_src_b".to_owned(),
            created_at: "2026-01-01T00:01:00Z".to_owned(),
        }];
        let files = [
            BackupFileSource {
                zip_path: asset_a.content_path.clone(),
                source_path: path_a,
                expected_size: bytes_a.len() as u64,
                expected_sha256: Some(sha_a),
            },
            BackupFileSource {
                zip_path: asset_b.content_path.clone(),
                source_path: path_b,
                expected_size: bytes_b.len() as u64,
                expected_sha256: Some(sha_b),
            },
        ];
        let archive_path = directory.path().join("remap.aiarchive");
        write_zip_to_path(&document, &files, &archive_path).unwrap();

        let service = test_service(&pool, data_dirs.projects.clone(), data_dirs.cache.clone());
        let preview = service.inspect(archive_path).await.unwrap();
        let restored = service.restore(&preview.inspection_id).await.unwrap();
        assert_ne!(restored.id, old_project);

        let leftover_versions: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM asset_versions WHERE project_id = ?")
                .bind(old_project)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(leftover_versions, 0);
        let leftover_relations: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM asset_relations WHERE project_id = ?")
                .bind(old_project)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(leftover_relations, 0);

        let version_row: (String, String) =
            sqlx::query_as("SELECT id, asset_id FROM asset_versions WHERE project_id = ?")
                .bind(&restored.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_ne!(version_row.0, "asv_src");
        assert_ne!(version_row.1, "ast_src_b");

        let relation_row: (String, String, String) = sqlx::query_as(
            "SELECT id, source_asset_id, target_asset_id FROM asset_relations WHERE project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_ne!(relation_row.0, "rel_src");
        assert_ne!(relation_row.1, "ast_src_a");
        assert_ne!(relation_row.2, "ast_src_b");

        let lineage: (String, String) = sqlx::query_as(
            "SELECT gav.id, gav.generation_id FROM generation_asset_versions gav
             JOIN tasks t ON t.id = gav.generation_id
             WHERE t.project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_ne!(lineage.0, "gav_src");
        assert_ne!(lineage.1, "tsk_src");

        let foreign_lineage: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM generation_asset_versions gav
             JOIN tasks t ON t.id = gav.generation_id
             WHERE gav.id = 'gav_src' OR t.project_id = ?",
        )
        .bind(old_project)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(foreign_lineage, 0);
    }

    #[tokio::test]
    async fn archive_v19_multimedia_round_trip_preserves_v2_lineage_and_report() {
        let directory = tempdir().unwrap();
        let data_dirs = AppDataDirs::initialize(directory.path().join("AIStudioData")).unwrap();
        let pool = initialize(&data_dirs.database).await.unwrap();
        crate::infrastructure::database::repositories::test_support::seed_task_dependencies(&pool)
            .await;

        let foreign_root = data_dirs.projects.join("project-foreign");
        std::fs::create_dir_all(&foreign_root).unwrap();
        sqlx::query(
            "INSERT INTO projects (id, name, description, root_path, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("project-foreign")
        .bind("Foreign project")
        .bind(Option::<String>::None)
        .bind(foreign_root.to_string_lossy().to_string())
        .bind("2026-01-01T00:00:00Z")
        .bind("2026-01-01T00:00:00Z")
        .execute(&pool)
        .await
        .unwrap();

        let old_project = "project-v19-multimedia";
        let timestamp = "2026-01-01T00:00:00Z";
        let mut document = empty_archive_document(old_project, "多媒体归档");
        document.tasks = vec![terminal_backup_task(
            "tsk_multimedia",
            Some("prm_multimedia"),
            timestamp,
        )];
        document.prompt_entries = vec![BackupPromptEntry {
            id: "prm_multimedia".to_owned(),
            project_id: old_project.to_owned(),
            kind: "prompt".to_owned(),
            name: "多媒体提示词".to_owned(),
            normalized_name: "多媒体提示词".to_owned(),
            tags: Vec::new(),
            created_at: timestamp.to_owned(),
            updated_at: timestamp.to_owned(),
        }];
        document.prompt_versions = vec![BackupPromptVersion {
            id: "prv_multimedia".to_owned(),
            project_id: old_project.to_owned(),
            prompt_id: "prm_multimedia".to_owned(),
            version: 1,
            text: "生成多媒体内容".to_owned(),
            model_version_id: Some("mdv_multimedia".to_owned()),
            created_at: timestamp.to_owned(),
        }];
        document.models = vec![BackupModel {
            id: "mdl_multimedia".to_owned(),
            name: "H3".to_owned(),
            provider: "MiniMax".to_owned(),
            model_type: "video".to_owned(),
            description: "multimedia test model".to_owned(),
            metadata_json: json!({"fixture": true}),
            created_at: timestamp.to_owned(),
        }];
        document.model_versions = vec![BackupModelVersion {
            id: "mdv_multimedia".to_owned(),
            model_id: "mdl_multimedia".to_owned(),
            version: "2026-01".to_owned(),
            capabilities_json: json!(["image", "video", "audio"]),
            parameter_schema_json: json!({"seed": "integer"}),
            created_at: timestamp.to_owned(),
        }];
        document.snapshots = vec![BackupSnapshot {
            id: "snp_multimedia".to_owned(),
            task_id: "tsk_multimedia".to_owned(),
            workflow: json!({"kind": "multimedia"}),
            recipe_yaml: "schema_version: 1\ninputs: {}\n".to_owned(),
            user_inputs: json!({}),
            resolved_inputs: json!({}),
            model_version_id: Some("mdv_multimedia".to_owned()),
            prompt_version_id: Some("prv_multimedia".to_owned()),
            created_at: timestamp.to_owned(),
        }];
        document.tools = vec![BackupTool {
            id: "tool_multimedia".to_owned(),
            name: "ComfyUI".to_owned(),
            tool_type: "local".to_owned(),
            description: "multimedia tool".to_owned(),
            metadata_json: json!({}),
            created_at: timestamp.to_owned(),
        }];
        document.tool_versions = vec![BackupToolVersion {
            id: "tver_multimedia".to_owned(),
            tool_id: "tool_multimedia".to_owned(),
            version: "0.1".to_owned(),
            observed_at: timestamp.to_owned(),
            metadata_json: json!({}),
        }];
        document.tool_capabilities = vec![BackupToolCapability {
            tool_id: "tool_multimedia".to_owned(),
            capability_name: "multimedia_generation".to_owned(),
            metadata_json: json!({}),
        }];
        document.tool_instances = vec![BackupToolInstance {
            id: "tins_multimedia".to_owned(),
            tool_id: "tool_multimedia".to_owned(),
            path: Some("/old/comfy".to_owned()),
            endpoint: Some("http://127.0.0.1:8188".to_owned()),
            status: "AVAILABLE".to_owned(),
            last_checked: Some(timestamp.to_owned()),
        }];
        document.generation_tool_usages = vec![BackupGenerationToolUsage {
            id: "gtu_multimedia".to_owned(),
            generation_id: "tsk_multimedia".to_owned(),
            tool_instance_id: "tins_multimedia".to_owned(),
            tool_version_id: Some("tver_multimedia".to_owned()),
            metadata_json: json!({"executor": "comfy"}),
            created_at: timestamp.to_owned(),
        }];

        let media_dir = directory.path().join("media");
        std::fs::create_dir_all(&media_dir).unwrap();
        let media = [
            ("image", "image/png", "source_image", None),
            ("video", "video/mp4", "source_video", Some(4_000_i64)),
            ("audio", "audio/mpeg", "source_audio", Some(8_000_i64)),
        ];
        let mut files = Vec::with_capacity(media.len());
        for (ordinal, (kind, mime_type, category, duration_ms)) in media.into_iter().enumerate() {
            let asset_id = format!("ast_{kind}");
            let bytes = format!("{kind}-media-bytes").into_bytes();
            let source_path = media_dir.join(format!("{asset_id}.bin"));
            std::fs::write(&source_path, &bytes).unwrap();
            let content_path = format!("assets/{asset_id}/content.bin");
            let (mut asset, sha) = archive_media_asset(&asset_id, &bytes, &content_path);
            asset.asset_type = kind.to_owned();
            asset.category = category.to_owned();
            asset.mime_type = mime_type.to_owned();
            asset.duration_ms = duration_ms;
            asset.source_task_id = Some("tsk_multimedia".to_owned());
            asset.original_name = format!("{kind}.media");
            document.assets.push(asset);

            for version_number in 1..=2 {
                document.asset_versions.push(BackupAssetVersion {
                    id: format!("asv_{kind}_v{version_number}"),
                    project_id: old_project.to_owned(),
                    asset_id: asset_id.clone(),
                    version_number,
                    metadata_snapshot: json!({"version": version_number}),
                    location: format!("/old/project/{kind}/v{version_number}.media"),
                    checksum: sha.clone(),
                    created_at: timestamp.to_owned(),
                });
            }
            let output_id = format!("out_{kind}");
            document.mappings.push(BackupMapping {
                task_id: "tsk_multimedia".to_owned(),
                output_id: output_id.clone(),
                ordinal: ordinal as i64,
                asset_id: asset_id.clone(),
                created_at: timestamp.to_owned(),
            });
            document
                .generation_asset_versions
                .push(BackupGenerationAssetVersion {
                    id: format!("gav_{kind}"),
                    generation_id: "tsk_multimedia".to_owned(),
                    output_id,
                    ordinal: ordinal as i64,
                    asset_version_id: format!("asv_{kind}_v2"),
                    relation_type: "OUTPUT".to_owned(),
                    created_at: timestamp.to_owned(),
                });
            files.push(BackupFileSource {
                zip_path: content_path,
                source_path,
                expected_size: bytes.len() as u64,
                expected_sha256: Some(sha),
            });
        }
        document.asset_relations = vec![
            BackupAssetRelation {
                id: "rel_image_video".to_owned(),
                project_id: old_project.to_owned(),
                source_asset_id: "ast_image".to_owned(),
                target_asset_id: "ast_video".to_owned(),
                relation_type: "SOURCE_OF".to_owned(),
                created_at: timestamp.to_owned(),
            },
            BackupAssetRelation {
                id: "rel_video_audio".to_owned(),
                project_id: old_project.to_owned(),
                source_asset_id: "ast_video".to_owned(),
                target_asset_id: "ast_audio".to_owned(),
                relation_type: "DERIVED_FROM".to_owned(),
                created_at: timestamp.to_owned(),
            },
        ];

        let archive_path = directory.path().join("multimedia-v20.aiarchive");
        write_zip_to_path(&document, &files, &archive_path).unwrap();
        let (manifest, loaded, _) = inspect_archive(&archive_path).unwrap();
        assert_eq!(manifest.version, 20);
        let inventory = manifest.inventory.expect("v20 inventory");
        assert_eq!(inventory.assets, 3);
        assert_eq!(inventory.asset_versions, 6);
        assert_eq!(inventory.relations, 2);
        assert_eq!(inventory.models, 1);
        assert_eq!(inventory.tools, 1);
        assert_eq!(inventory.lineage, 4);
        assert_eq!(loaded.assets.len(), 3);
        assert_eq!(loaded.asset_versions.len(), 6);
        assert_eq!(loaded.generation_tool_usages.len(), 1);
        assert_eq!(loaded.generation_asset_versions.len(), 3);

        let service = test_service(&pool, data_dirs.projects.clone(), data_dirs.cache.clone());
        let preview = service.inspect(archive_path).await.unwrap();
        assert_eq!(preview.image_count, 1);
        assert_eq!(preview.video_count, 1);
        assert_eq!(preview.audio_count, 1);
        assert_eq!(preview.asset_versions, 6);
        assert_eq!(preview.generation_tool_usages, 1);
        assert_eq!(preview.generation_asset_versions, 3);
        let restored = service.restore(&preview.inspection_id).await.unwrap();

        assert_eq!(restored.status, "COMPLETE");
        assert_eq!(restored.backup_version, 20);
        assert_eq!(restored.assets, 3);
        assert_eq!(restored.versions, 6);
        assert_eq!(restored.generations, 1);
        assert_eq!(restored.restored_generation_tool_usages, 1);
        assert_eq!(restored.restored_generation_asset_versions, 3);
        assert!(restored.warnings.is_empty());
        assert!(restored.missing_tools.is_empty());
        assert!(restored.missing_models.is_empty());
        assert!(restored.missing_files.is_empty());

        let restored_asset_ids: Vec<String> =
            sqlx::query_scalar("SELECT id FROM assets WHERE project_id = ? ORDER BY id")
                .bind(&restored.id)
                .fetch_all(&pool)
                .await
                .unwrap();
        let source_asset_ids = HashSet::from([
            "ast_image".to_owned(),
            "ast_video".to_owned(),
            "ast_audio".to_owned(),
        ]);
        let source_version_ids = HashSet::from([
            "asv_image_v1".to_owned(),
            "asv_image_v2".to_owned(),
            "asv_video_v1".to_owned(),
            "asv_video_v2".to_owned(),
            "asv_audio_v1".to_owned(),
            "asv_audio_v2".to_owned(),
        ]);
        assert_eq!(restored_asset_ids.len(), 3);
        assert!(restored_asset_ids
            .iter()
            .all(|id| !source_asset_ids.contains(id)));

        let version_rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT av.id, av.asset_id
             FROM asset_versions av
             JOIN assets a ON a.id = av.asset_id
             WHERE av.project_id = ? AND a.project_id = ?
             ORDER BY av.id",
        )
        .bind(&restored.id)
        .bind(&restored.id)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(version_rows.len(), 6);
        assert!(version_rows.iter().all(|(version_id, asset_id)| {
            !source_version_ids.contains(version_id) && restored_asset_ids.contains(asset_id)
        }));
        let restored_type_counts: (i64, i64, i64) = sqlx::query_as(
            "SELECT
               (SELECT COUNT(*) FROM assets WHERE project_id = ? AND type = 'image'),
               (SELECT COUNT(*) FROM assets WHERE project_id = ? AND type = 'video'),
               (SELECT COUNT(*) FROM assets WHERE project_id = ? AND type = 'audio')",
        )
        .bind(&restored.id)
        .bind(&restored.id)
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(restored_type_counts, (1, 1, 1));

        let relation_rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT id, source_asset_id, target_asset_id
             FROM asset_relations WHERE project_id = ? ORDER BY id",
        )
        .bind(&restored.id)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(relation_rows.len(), 2);
        let source_relation_ids =
            HashSet::from(["rel_image_video".to_owned(), "rel_video_audio".to_owned()]);
        assert!(relation_rows.iter().all(|(id, source, target)| {
            !source_relation_ids.contains(id)
                && restored_asset_ids.contains(source)
                && restored_asset_ids.contains(target)
        }));

        let lineage_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM generation_asset_versions gav
             JOIN tasks t ON t.id = gav.generation_id
             JOIN asset_versions av ON av.id = gav.asset_version_id
             JOIN assets a ON a.id = av.asset_id
             JOIN task_output_assets toa
               ON toa.task_id = gav.generation_id
              AND toa.output_id = gav.output_id
              AND toa.ordinal = gav.ordinal
              AND toa.asset_id = av.asset_id
             WHERE t.project_id = ? AND av.project_id = ? AND a.project_id = ?",
        )
        .bind(&restored.id)
        .bind(&restored.id)
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(lineage_count, 3);
        let tool_usage_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM generation_tool_usages gtu
             JOIN tasks t ON t.id = gtu.generation_id
             JOIN tool_instances ti ON ti.id = gtu.tool_instance_id
             JOIN tool_versions tv ON tv.id = gtu.tool_version_id
             WHERE t.project_id = ? AND ti.id = 'tins_multimedia' AND tv.id = 'tver_multimedia'",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(tool_usage_count, 1);

        let snapshot_model: String = sqlx::query_scalar(
            "SELECT model_version_id FROM generation_snapshots gs
             JOIN tasks t ON t.id = gs.task_id WHERE t.project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(snapshot_model, "mdv_multimedia");
        let snapshot_prompt: String = sqlx::query_scalar(
            "SELECT prompt_version_id FROM generation_snapshots gs
             JOIN tasks t ON t.id = gs.task_id WHERE t.project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let restored_prompt_version: String = sqlx::query_scalar(
            "SELECT pv.id FROM prompt_versions pv
             JOIN prompt_entries pe ON pe.id = pv.prompt_id WHERE pe.project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(snapshot_prompt, restored_prompt_version);
        assert_ne!(snapshot_prompt, "prv_multimedia");
        let prompt_model: String = sqlx::query_scalar(
            "SELECT model_version_id FROM prompt_versions pv
             JOIN prompt_entries pe ON pe.id = pv.prompt_id WHERE pe.project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(prompt_model, "mdv_multimedia");
        let restored_task_prompt: Option<String> =
            sqlx::query_scalar("SELECT prompt_id FROM tasks WHERE project_id = ?")
                .bind(&restored.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let restored_prompt_id: String =
            sqlx::query_scalar("SELECT id FROM prompt_entries WHERE project_id = ?")
                .bind(&restored.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            restored_task_prompt.as_deref(),
            Some(restored_prompt_id.as_str())
        );
        assert_ne!(restored_task_prompt.as_deref(), Some("prm_multimedia"));

        let locations: Vec<String> =
            sqlx::query_scalar("SELECT location FROM asset_versions WHERE project_id = ?")
                .bind(&restored.id)
                .fetch_all(&pool)
                .await
                .unwrap();
        let restored_root = data_dirs.projects.join(&restored.id);
        let restored_root_string = restored_root.to_string_lossy().to_string();
        assert!(locations.iter().all(|location| {
            location.starts_with(&restored_root_string) && Path::new(location).is_file()
        }));
        let foreign_projects: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM projects WHERE id = 'project-foreign'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(foreign_projects, 1);
        let foreign_assets: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM assets WHERE project_id = 'project-foreign'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(foreign_assets, 0);
    }

    #[tokio::test]
    async fn restore_report_surfaces_unknown_model_and_tool_without_guessing() {
        let directory = tempdir().unwrap();
        let data_dirs = AppDataDirs::initialize(directory.path().join("AIStudioData")).unwrap();
        let pool = initialize(&data_dirs.database).await.unwrap();
        crate::infrastructure::database::repositories::test_support::seed_task_dependencies(&pool)
            .await;

        let timestamp = "2026-01-01T00:00:00Z";
        let old_project = "project-unknown-registry";
        let mut document = empty_archive_document(old_project, "未知注册表");
        document.tasks = vec![terminal_backup_task(
            "tsk_unknown",
            Some("prm_unknown"),
            timestamp,
        )];
        document.prompt_entries = vec![BackupPromptEntry {
            id: "prm_unknown".to_owned(),
            project_id: old_project.to_owned(),
            kind: "prompt".to_owned(),
            name: "未知来源提示词".to_owned(),
            normalized_name: "未知来源提示词".to_owned(),
            tags: Vec::new(),
            created_at: timestamp.to_owned(),
            updated_at: timestamp.to_owned(),
        }];
        document.prompt_versions = vec![BackupPromptVersion {
            id: "prv_unknown".to_owned(),
            project_id: old_project.to_owned(),
            prompt_id: "prm_unknown".to_owned(),
            version: 1,
            text: "保留显式来源".to_owned(),
            model_version_id: Some("mdv_unknown".to_owned()),
            created_at: timestamp.to_owned(),
        }];
        document.snapshots = vec![BackupSnapshot {
            id: "snp_unknown".to_owned(),
            task_id: "tsk_unknown".to_owned(),
            workflow: json!({}),
            recipe_yaml: "schema_version: 1\ninputs: {}\n".to_owned(),
            user_inputs: json!({}),
            resolved_inputs: json!({}),
            model_version_id: Some("mdv_unknown".to_owned()),
            prompt_version_id: Some("prv_unknown".to_owned()),
            created_at: timestamp.to_owned(),
        }];
        document.models = vec![BackupModel {
            id: "mdl_unknown".to_owned(),
            name: "Archive Model".to_owned(),
            provider: "Archive Provider".to_owned(),
            model_type: "image".to_owned(),
            description: String::new(),
            metadata_json: json!({}),
            created_at: timestamp.to_owned(),
        }];
        document.model_versions = vec![BackupModelVersion {
            id: "mdv_unknown".to_owned(),
            model_id: "mdl_unknown".to_owned(),
            version: "archive-1".to_owned(),
            capabilities_json: json!([]),
            parameter_schema_json: json!({}),
            created_at: timestamp.to_owned(),
        }];
        document.tools = vec![BackupTool {
            id: "tool_unknown".to_owned(),
            name: "Archive Tool".to_owned(),
            tool_type: "local".to_owned(),
            description: String::new(),
            metadata_json: json!({}),
            created_at: timestamp.to_owned(),
        }];
        document.tool_versions = vec![BackupToolVersion {
            id: "tver_unknown".to_owned(),
            tool_id: "tool_unknown".to_owned(),
            version: "archive-1".to_owned(),
            observed_at: timestamp.to_owned(),
            metadata_json: json!({}),
        }];
        document.tool_instances = vec![BackupToolInstance {
            id: "tins_unknown".to_owned(),
            tool_id: "tool_unknown".to_owned(),
            path: Some("/archive/tool".to_owned()),
            endpoint: Some("http://127.0.0.1:9999".to_owned()),
            status: "AVAILABLE".to_owned(),
            last_checked: Some(timestamp.to_owned()),
        }];
        document.generation_tool_usages = vec![BackupGenerationToolUsage {
            id: "gtu_unknown".to_owned(),
            generation_id: "tsk_unknown".to_owned(),
            tool_instance_id: "tins_unknown".to_owned(),
            tool_version_id: Some("tver_unknown".to_owned()),
            metadata_json: json!({}),
            created_at: timestamp.to_owned(),
        }];

        // Deliberately create immutable identity conflicts. Restore must expose UNKNOWN,
        // not match these rows by name/path/time or fabricate a replacement relationship.
        sqlx::query(
            "INSERT INTO models (id, name, provider, type, description, metadata_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("mdl_unknown")
        .bind("Existing Model")
        .bind("Existing Provider")
        .bind("image")
        .bind("")
        .bind("{}")
        .bind(timestamp)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO tools (id, name, type, description, metadata_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("tool_unknown")
        .bind("Existing Tool")
        .bind("local")
        .bind("")
        .bind("{}")
        .bind(timestamp)
        .execute(&pool)
        .await
        .unwrap();

        let archive_path = directory.path().join("unknown-registry.aiarchive");
        write_zip_to_path(&document, &[], &archive_path).unwrap();
        let service = test_service(&pool, data_dirs.projects.clone(), data_dirs.cache.clone());
        let preview = service.inspect(archive_path).await.unwrap();
        let restored = service.restore(&preview.inspection_id).await.unwrap();

        assert_eq!(restored.status, "COMPLETE");
        assert_eq!(restored.backup_version, 20);
        assert_eq!(restored.generations, 1);
        assert_eq!(restored.missing_models, vec!["mdv_unknown".to_owned()]);
        assert_eq!(restored.missing_tools, vec!["tins_unknown".to_owned()]);
        assert_eq!(
            restored.unresolved_tool_version_ids,
            vec!["tver_unknown".to_owned()]
        );
        assert_eq!(restored.restored_generation_tool_usages, 0);
        assert!(restored
            .warnings
            .iter()
            .any(|warning| warning.contains("UNKNOWN")));
        assert!(restored
            .warnings
            .iter()
            .any(|warning| warning.contains("未根据名称") || warning.contains("未执行启发式")));

        let restored_usage_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM generation_tool_usages gtu
             JOIN tasks t ON t.id = gtu.generation_id WHERE t.project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(restored_usage_count, 0);
        let restored_prompt_model: Option<String> = sqlx::query_scalar(
            "SELECT model_version_id FROM prompt_versions pv
             JOIN prompt_entries pe ON pe.id = pv.prompt_id WHERE pe.project_id = ?",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(restored_prompt_model, None);
    }

    /// Bounded synthetic scale/integrity probe for DEV-129-D.
    /// Product planning target is Projects:100 / Assets:10000 / Versions:50000 /
    /// Relations:100000; this environment uses a smaller N that still exercises
    /// inventory checksum + restore remapping without multi-minute SQLite fixtures.
    #[tokio::test]
    async fn bounded_synthetic_scale_archive_integrity_round_trip() {
        const N_ASSETS: usize = 120;
        const N_VERSIONS: usize = 120;
        const N_RELATIONS: usize = 200;

        let directory = tempdir().unwrap();
        let data_dirs = AppDataDirs::initialize(directory.path().join("AIStudioData")).unwrap();
        let pool = initialize(&data_dirs.database).await.unwrap();

        let mut document = empty_archive_document("prj_scale", "规模完整性");
        let mut files = Vec::with_capacity(N_ASSETS);
        let media_dir = directory.path().join("media");
        std::fs::create_dir_all(&media_dir).unwrap();
        for index in 0..N_ASSETS {
            let id = format!("ast_scale_{index}");
            let bytes = format!("scale-bytes-{index}").into_bytes();
            let source_path = media_dir.join(format!("{id}.bin"));
            std::fs::write(&source_path, &bytes).unwrap();
            let content_path = format!("assets/{id}/content.bin");
            let (asset, sha) = archive_media_asset(&id, &bytes, &content_path);
            files.push(BackupFileSource {
                zip_path: content_path,
                source_path,
                expected_size: bytes.len() as u64,
                expected_sha256: Some(sha.clone()),
            });
            document.asset_versions.push(BackupAssetVersion {
                id: format!("asv_scale_{index}"),
                project_id: "prj_scale".to_owned(),
                asset_id: id.clone(),
                version_number: 1,
                metadata_snapshot: json!({}),
                location: format!("/old/host/scale/{id}.bin"),
                checksum: sha,
                created_at: "2026-01-01T00:00:00Z".to_owned(),
            });
            document.assets.push(asset);
        }
        assert_eq!(document.asset_versions.len(), N_VERSIONS);
        // Unique (source, target, type) pairs only — schema enforces UNIQUE on that tuple.
        let mut relation_index = 0usize;
        for distance in 1..=3 {
            for source_index in 0..N_ASSETS {
                if relation_index >= N_RELATIONS {
                    break;
                }
                let target_index = (source_index + distance) % N_ASSETS;
                if target_index == source_index {
                    continue;
                }
                document.asset_relations.push(BackupAssetRelation {
                    id: format!("rel_scale_{relation_index}"),
                    project_id: "prj_scale".to_owned(),
                    source_asset_id: format!("ast_scale_{source_index}"),
                    target_asset_id: format!("ast_scale_{target_index}"),
                    relation_type: "SOURCE_OF".to_owned(),
                    created_at: "2026-01-01T00:00:00Z".to_owned(),
                });
                relation_index += 1;
            }
        }
        assert_eq!(document.asset_relations.len(), N_RELATIONS);

        let archive_path = directory.path().join("scale.aiarchive");
        write_zip_to_path(&document, &files, &archive_path).unwrap();
        let (manifest, loaded, _) = inspect_archive(&archive_path).unwrap();
        let inventory = manifest.inventory.expect("inventory present for v19 write");
        assert_eq!(inventory.assets, N_ASSETS);
        assert_eq!(inventory.asset_versions, N_VERSIONS);
        assert_eq!(inventory.relations, N_RELATIONS);
        assert_eq!(loaded.assets.len(), N_ASSETS);
        assert_eq!(loaded.asset_versions.len(), N_VERSIONS);
        assert_eq!(loaded.asset_relations.len(), N_RELATIONS);
        assert_eq!(manifest.media_inventory.len(), N_ASSETS);

        let service = test_service(&pool, data_dirs.projects.clone(), data_dirs.cache.clone());
        let preview = service.inspect(archive_path).await.unwrap();
        assert_eq!(preview.asset_versions, N_VERSIONS);
        assert_eq!(preview.asset_relations, N_RELATIONS);
        let restored = service.restore(&preview.inspection_id).await.unwrap();

        let asset_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM assets WHERE project_id = ?")
                .bind(&restored.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let version_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM asset_versions WHERE project_id = ?")
                .bind(&restored.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let relation_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM asset_relations WHERE project_id = ?")
                .bind(&restored.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(asset_count, N_ASSETS as i64);
        assert_eq!(version_count, N_VERSIONS as i64);
        assert_eq!(relation_count, N_RELATIONS as i64);

        let old_path_leaks: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM asset_versions
             WHERE project_id = ? AND location LIKE '/old/host/%'",
        )
        .bind(&restored.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(old_path_leaks, 0);
        let leftover_old_project: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM asset_versions WHERE project_id = 'prj_scale'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(leftover_old_project, 0);
    }

    #[tokio::test]
    async fn v18_zip_compat_regression_restores_with_visible_warning() {
        // Explicit DEV-130.1-C regression pointer to historical v18 restore support.
        let directory = tempdir().unwrap();
        let archive_path = directory.path().join("legacy-v18-hardening.zip");
        let file = File::create(&archive_path).unwrap();
        let mut writer = ZipWriter::new(file);
        let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
        let manifest = json!({
            "format": "ai-studio-project-backup",
            "version": 18,
            "createdBy": "1.3.1",
            "project": { "id": "legacy-v18-hardening", "name": "旧归档硬化" }
        });
        let document = json!({
            "project": { "id": "legacy-v18-hardening", "name": "旧归档硬化" },
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
            "workflowRefs": []
        });
        writer.start_file("manifest.json", options).unwrap();
        writer
            .write_all(serde_json::to_string(&manifest).unwrap().as_bytes())
            .unwrap();
        writer.start_file("project.json", options).unwrap();
        writer
            .write_all(serde_json::to_string(&document).unwrap().as_bytes())
            .unwrap();
        writer.finish().unwrap();
        let (loaded_manifest, _, _) = inspect_archive(&archive_path).unwrap();
        assert_eq!(loaded_manifest.version, 18);
        assert!(loaded_manifest.logical_snapshot_checksum.is_none());
        assert!(loaded_manifest.inventory.is_none());

        let data_dirs = AppDataDirs::initialize(directory.path().join("AIStudioData")).unwrap();
        let pool = initialize(&data_dirs.database).await.unwrap();
        let service = test_service(&pool, data_dirs.projects.clone(), data_dirs.cache.clone());
        let preview = service.inspect(archive_path).await.unwrap();
        let restored = service.restore(&preview.inspection_id).await.unwrap();
        assert_eq!(restored.status, "COMPLETE");
        assert_eq!(restored.backup_version, 18);
        assert_eq!(restored.assets, 0);
        assert_eq!(restored.versions, 0);
        assert_eq!(restored.generations, 0);
        assert!(restored.missing_files.is_empty());
        assert!(restored
            .warnings
            .iter()
            .any(|warning| warning.contains("Backup v18")));
    }
}
