pub mod artifact_service;
pub mod asset_data_service;
pub mod asset_deletion_service;
pub mod asset_import_service;
pub mod asset_library_service;
pub mod asset_query_service;
pub mod asset_usage_service;
pub mod asset_video_prompt_service;
pub mod batch_workflow_preset_service;
pub mod build_info;
pub mod builtin_runtime_packages;
#[cfg(test)]
mod cancellation_e2e;
mod comfy_execution_failure;
pub mod comfy_memory_service;
pub mod comfy_preflight_service;
pub mod comfy_service;
pub mod consistency_profile_service;
pub mod consistency_scope_binding_service;
#[cfg(test)]
mod dev027_e2e;
#[cfg(test)]
mod dev031_e2e;
pub mod diagnostics_service;
pub mod episode_production_service;
pub mod external_production_handoff_service;
pub mod generation_catalog_service;
#[cfg(test)]
mod generation_e2e;
pub mod generation_input_preparer;
pub mod generation_service;
pub mod h3_local_import_service;
pub(crate) mod image_inspection;
pub mod media_probe;
pub mod media_protocol;
pub mod minimax_video_product_policy;
pub mod model_service;
pub(crate) mod ordered_reference_binding;
pub mod organization_service;
pub mod output_collector;
pub mod pagination;
pub mod ports;
pub mod preset_service;
pub mod product;
pub mod product_runtime_scope;
pub mod production_audit_service;
pub mod production_batch_runbook_service;
pub mod production_orchestrator_service;
pub mod production_package_discovery_service;
pub mod production_package_inspector;
pub mod production_package_service;
pub mod production_preparation_service;
pub mod production_queue_service;
pub mod production_start_admission_service;
pub mod production_structure_service;
pub mod project_backup_service;
pub mod project_bootstrap;
pub mod project_command_center_service;
pub mod project_manifest_service;
pub mod project_service;
pub mod project_template_service;
pub mod project_workflow_binding_service;
pub mod prompt_context_builder;
pub mod prompt_library_service;
pub mod provenance_lineage_service;
pub mod recipe_history_query_service;
pub mod reference_anchor_service;
pub mod reference_set_service;
pub mod repair_jobs;
pub mod scene_production_service;
pub mod scheduler;
pub mod series_production_service;
pub mod settings_service;
pub mod shot_batch_service;
pub mod shot_bulk_service;
pub mod shot_consistency_binding_service;
pub mod shot_context_resolver;
pub mod shot_readiness_evaluator;
pub mod shot_readiness_service;
pub mod shot_reference_pack_builder;
pub mod shot_service;
pub mod shot_video_input_service;
pub(crate) mod shot_workflow_compatibility;
pub mod source_asset_import_service;
// Inlined (no new backend source files under the frozen Phase2/3 aggregate).
pub mod story_entity_service {
    use crate::application::ports::{RepositoryError, StoryEntityRepository};
    use crate::domain::story_entity::{StoryEntity, StoryEntityKind, StoryEntityUpsert};
    use crate::error::AppError;
    use std::sync::Arc;

    pub struct StoryEntityService {
        repo: Arc<dyn StoryEntityRepository>,
    }

    impl StoryEntityService {
        pub fn new(repo: Arc<dyn StoryEntityRepository>) -> Self {
            Self { repo }
        }

        pub async fn list(
            &self,
            project_id: &str,
            kind: Option<StoryEntityKind>,
        ) -> Result<Vec<StoryEntity>, AppError> {
            self.repo.list(project_id, kind).await.map_err(map_repo)
        }

        pub async fn get(&self, project_id: &str, id: &str) -> Result<StoryEntity, AppError> {
            match self.repo.get(project_id, id).await.map_err(map_repo)? {
                Some(entity) => Ok(entity),
                None => Err(AppError::invalid_input(format!(
                    "story entity not found: {id}"
                ))),
            }
        }

        pub async fn create(
            &self,
            project_id: &str,
            kind: StoryEntityKind,
            input: StoryEntityUpsert,
        ) -> Result<StoryEntity, AppError> {
            self.repo
                .create(project_id, kind, input)
                .await
                .map_err(map_repo)
        }

        pub async fn update(
            &self,
            project_id: &str,
            id: &str,
            input: StoryEntityUpsert,
        ) -> Result<StoryEntity, AppError> {
            self.repo
                .update(project_id, id, input)
                .await
                .map_err(map_repo)
        }

        pub async fn delete(&self, project_id: &str, id: &str) -> Result<(), AppError> {
            self.repo.delete(project_id, id).await.map_err(map_repo)
        }

        pub async fn list_for_shot(
            &self,
            project_id: &str,
            shot_id: &str,
        ) -> Result<Vec<StoryEntity>, AppError> {
            self.repo
                .list_for_shot(project_id, shot_id)
                .await
                .map_err(map_repo)
        }

        pub async fn replace_shot_bindings(
            &self,
            project_id: &str,
            shot_id: &str,
            entity_ids: Vec<String>,
        ) -> Result<Vec<StoryEntity>, AppError> {
            self.repo
                .replace_shot_bindings(project_id, shot_id, &entity_ids)
                .await
                .map_err(map_repo)
        }
    }

    fn map_repo(error: RepositoryError) -> AppError {
        match error {
            RepositoryError::NotFound { entity, id } => {
                AppError::invalid_input(format!("{entity} not found: {id}"))
            }
            RepositoryError::Integrity { message } => AppError::invalid_input(message),
            RepositoryError::Database { message } => AppError::database(message),
            RepositoryError::Serialization { context, message } => {
                AppError::database(format!("{context}: {message}"))
            }
            other => AppError::database(format!("{other:?}")),
        }
    }
}
pub mod task_cancellation_service;
pub mod task_execution_registry;
pub mod task_history_service;
pub mod task_query_service;
pub mod task_recovery_service;
pub mod tool_service;
pub mod workflow_analysis_service;
pub mod workflow_benchmark_service;
pub mod workflow_graph_analysis;
pub mod workflow_library_service;
pub mod workflow_lifecycle_coordinator;
pub mod workflow_lifecycle_service;
pub mod workflow_manifest;
pub mod workflow_onboarding_service;
pub mod workflow_recognition_provenance;
pub mod workflow_recognition_schema;
pub mod workflow_recognition_service;
#[cfg(test)]
mod workflow_recognition_v3_replay;
pub mod workflow_registry_service;
pub mod workflow_semantic_graph;
pub mod workflow_semantic_identity;
pub mod workflow_ui_compatibility;
pub mod workflow_ui_normalizer;
pub mod workflow_ui_serialization;
pub mod workflow_workspace_query_service;

pub mod workflow_file_service;

/// Folder import coordinates existing authorities; it is not a production executor.
pub mod folder_batch_import_service {
    use serde::{Deserialize, Serialize};
    use sha2::{Digest, Sha256};
    use std::collections::{BTreeMap, BTreeSet};

    #[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
    #[serde(rename_all = "camelCase")]
    pub enum Mode {
        FirstLast,
        Reference,
        Image,
    }
    #[derive(Clone, Debug, Deserialize, Serialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct Generator {
        pub workflow_version_id: String,
        pub recipe_id: String,
    }
    #[derive(Clone, Debug, Default, Deserialize, Serialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct Parameters {
        pub stage: Option<String>,
        pub mode: Option<Mode>,
        pub duration_seconds: Option<i64>,
        pub aspect_ratio: Option<String>,
        pub resolution_preset: Option<String>,
        pub width: Option<i64>,
        pub height: Option<i64>,
        pub seed: Option<u64>,
    }
    #[derive(Clone, Debug, Deserialize, Serialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct ManifestShot {
        pub id: Option<String>,
        pub name: Option<String>,
        pub mode: Option<Mode>,
        pub prompt: String,
        pub first_frame: Option<String>,
        pub last_frame: Option<String>,
        #[serde(default)]
        pub references: Vec<String>,
        #[serde(default)]
        pub characters: Vec<String>,
        pub duration_seconds: Option<i64>,
        pub aspect_ratio: Option<String>,
        pub resolution_preset: Option<String>,
        pub width: Option<i64>,
        pub height: Option<i64>,
        pub seed: Option<u64>,
    }
    #[derive(Clone, Debug, Deserialize, Serialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct ManifestEntity {
        pub name: String,
        pub kind: Option<String>,
        pub description: Option<String>,
        pub images: Vec<String>,
        pub cover: Option<String>,
    }
    #[derive(Clone, Debug, Default, Deserialize, Serialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct QueueOptions {
        pub create: Option<bool>,
        pub name: Option<String>,
        pub auto_start: Option<bool>,
    }
    #[derive(Clone, Debug, Deserialize, Serialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct Manifest {
        pub format: String,
        pub version: u32,
        pub name: String,
        pub generators: BTreeMap<Mode, Generator>,
        #[serde(default)]
        pub defaults: Parameters,
        #[serde(default)]
        pub characters: Vec<ManifestEntity>,
        pub shots: Vec<ManifestShot>,
        #[serde(default)]
        pub queue: QueueOptions,
    }
    #[derive(Clone, Debug, Serialize, PartialEq, Eq)]
    #[serde(rename_all = "camelCase")]
    pub struct ImportFailure {
        pub stage: String,
        pub code: String,
        pub message: String,
    }
    impl std::fmt::Display for ImportFailure {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}: {}", self.code, self.message)
        }
    }
    impl std::error::Error for ImportFailure {}
    pub(crate) fn failure(code: &str, message: impl Into<String>) -> ImportFailure {
        ImportFailure {
            stage: "PREVIEW".into(),
            code: code.into(),
            message: message.into(),
        }
    }
    #[derive(Clone, Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Row {
        pub row_key: String,
        pub row_number: usize,
        pub manifest_id: Option<String>,
        pub mode: Mode,
        pub name: String,
        pub prompt_preview: String,
        #[serde(skip)]
        pub prompt: String,
        pub duration_seconds: i64,
        pub width: i64,
        pub height: i64,
        #[serde(skip)]
        pub seed: Option<u64>,
        #[serde(skip)]
        pub generator: Option<Generator>,
        pub generator_label: String,
        pub first_frame: Option<String>,
        pub last_frame: Option<String>,
        pub reference_count: usize,
        pub entity_names: Vec<String>,
        #[serde(skip)]
        pub references: Vec<String>,
        pub errors: Vec<crate::application::shot_bulk_service::BulkIssue>,
        pub warnings: Vec<crate::application::shot_bulk_service::BulkIssue>,
    }
    pub(crate) fn sha(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }
    pub(crate) fn safe_relative_path(path: &str) -> bool {
        !path.is_empty()
            && !path.starts_with('/')
            && !path.contains(['\\', ':', '\0'])
            && path
                .split('/')
                .all(|p| !p.is_empty() && p != "." && p != "..")
    }
    pub(crate) fn parse_manifest(bytes: &[u8]) -> Result<Manifest, ImportFailure> {
        if bytes.len() > 1024 * 1024 {
            return Err(failure("SCAN_LIMIT_EXCEEDED", "manifest 最大为 1 MB"));
        }
        let manifest: Manifest = serde_json::from_slice(bytes)
            .map_err(|_| failure("INVALID_MANIFEST", "清单格式错误或含未知字段"))?;
        if manifest.format != "ai-studio-shot-batch" || manifest.version != 1 {
            return Err(failure(
                "UNSUPPORTED_VERSION",
                "仅支持 ai-studio-shot-batch version 1",
            ));
        }
        if manifest.name.trim().is_empty()
            || manifest.shots.is_empty()
            || manifest.shots.len() > 500
            || manifest.characters.len() > 100
        {
            return Err(failure(
                "INVALID_MANIFEST",
                "清单需要名称、1–500 个镜头，实体最多 100 个",
            ));
        }
        if manifest
            .defaults
            .stage
            .as_deref()
            .is_some_and(|s| s != "video")
        {
            return Err(failure("INVALID_MANIFEST", "仅支持 video 阶段"));
        }
        let mut names = BTreeSet::new();
        for entity in &manifest.characters {
            if entity.name.trim().is_empty()
                || entity.name.trim() != entity.name
                || !names.insert(entity.name.to_lowercase())
                || entity.images.is_empty()
                || entity
                    .kind
                    .as_deref()
                    .is_some_and(|k| !matches!(k, "character" | "scene"))
                || entity
                    .cover
                    .as_ref()
                    .is_some_and(|p| !entity.images.contains(p))
            {
                return Err(failure(
                    "INVALID_MANIFEST",
                    "实体名称、类型、图片或封面无效/重复",
                ));
            }
        }
        let mut ids = BTreeSet::new();
        if manifest
            .shots
            .iter()
            .filter_map(|s| s.id.as_ref())
            .any(|id| id.trim().is_empty() || !ids.insert(id))
        {
            return Err(failure("INVALID_MANIFEST", "镜头 id 必须非空且唯一"));
        }
        Ok(manifest)
    }
    fn resolution(defaults: &Parameters, shot: &ManifestShot) -> Result<(i64, i64), ImportFailure> {
        let bad = || {
            failure(
                "RESOLUTION_UNSUPPORTED",
                "请选择正式 MP 档位和画幅，或完整精确宽高；同层两种写法不能混用",
            )
        };
        if (defaults.width.is_some() || defaults.height.is_some())
            && (defaults.aspect_ratio.is_some() || defaults.resolution_preset.is_some())
        {
            return Err(bad());
        }
        let explicit = shot.width.is_some() || shot.height.is_some();
        if explicit && (shot.aspect_ratio.is_some() || shot.resolution_preset.is_some()) {
            return Err(bad());
        }
        let pair = if explicit {
            shot.width.zip(shot.height)
        } else if shot.aspect_ratio.is_none()
            && shot.resolution_preset.is_none()
            && (defaults.width.is_some() || defaults.height.is_some())
        {
            defaults.width.zip(defaults.height)
        } else {
            let tag = shot
                .resolution_preset
                .as_deref()
                .or(defaults.resolution_preset.as_deref())
                .unwrap_or("0.98");
            let aspect = shot
                .aspect_ratio
                .as_deref()
                .or(defaults.aspect_ratio.as_deref())
                .unwrap_or("16:9");
            let id = format!("mp-{}-{}", tag.replace('.', "_"), aspect.replace(':', "x"));
            super::product::h3_resolution::h3_base_presets()
                .iter()
                .find(|p| p.id == id)
                .map(|p| (p.width, p.height))
        };
        pair.filter(|(w, h)| super::product::h3_resolution::is_supported_h3_base_resolution(*w, *h))
            .ok_or_else(bad)
    }
    pub(crate) fn normalize(
        manifest: &Manifest,
        suffix: Option<&str>,
        existing_names: &[String],
    ) -> Vec<Row> {
        let mut names: BTreeSet<_> = existing_names
            .iter()
            .map(|n| n.trim().to_lowercase())
            .collect();
        manifest
            .shots
            .iter()
            .enumerate()
            .map(|(index, shot)| {
                let mut errors = Vec::new();
                let mode = shot
                    .mode
                    .clone()
                    .or(manifest.defaults.mode.clone())
                    .unwrap_or_else(|| {
                        errors.push(failure(
                            "MODE_MISMATCH",
                            "每行必须指定 mode 或 defaults.mode",
                        ));
                        Mode::Image
                    });
                let name = format!(
                    "{}{}",
                    shot.name
                        .as_deref()
                        .map(str::trim)
                        .filter(|n| !n.is_empty())
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("{} S{:02}", manifest.name.trim(), index + 1)),
                    suffix.unwrap_or("")
                );
                if !names.insert(name.trim().to_lowercase()) {
                    errors.push(failure(
                        "DUPLICATE_SHOT_NAME",
                        "存在同名镜头；可填写后缀并重新校验",
                    ));
                }
                if shot.prompt.trim().is_empty() || shot.prompt.chars().count() > 4000 {
                    errors.push(failure("INVALID_MANIFEST", "提示词应为 1–4000 字"));
                }
                let generator = manifest.generators.get(&mode).cloned();
                if generator.as_ref().is_none_or(|g| {
                    g.workflow_version_id.trim().is_empty() || g.recipe_id.trim().is_empty()
                }) {
                    errors.push(failure(
                        "GENERATOR_UNAVAILABLE",
                        "需要该模式准确的 WorkflowVersion + Recipe",
                    ));
                }
                let has_at = shot.prompt.contains('@');
                let valid = match mode {
                    Mode::FirstLast => {
                        shot.first_frame.is_some()
                            && shot.last_frame.is_some()
                            && shot.references.is_empty()
                            && shot.characters.is_empty()
                            && !has_at
                    }
                    Mode::Image => {
                        shot.first_frame.is_some()
                            && shot.last_frame.is_none()
                            && shot.references.is_empty()
                            && shot.characters.is_empty()
                            && !has_at
                    }
                    Mode::Reference => {
                        shot.first_frame.is_none()
                            && shot.last_frame.is_none()
                            && (!shot.references.is_empty() || has_at)
                    }
                };
                if !valid {
                    errors.push(failure(
                        "MODE_MISMATCH",
                        "输入字段与声明模式不一致或缺少必需输入",
                    ));
                }
                if shot
                    .first_frame
                    .iter()
                    .chain(shot.last_frame.iter())
                    .chain(shot.references.iter())
                    .any(|p| !safe_relative_path(p))
                {
                    errors.push(failure("PATH_OUTSIDE_ROOT", "仅接受根目录内安全的相对路径"));
                }
                let duration = shot
                    .duration_seconds
                    .or(manifest.defaults.duration_seconds)
                    .unwrap_or(5);
                if !(1..=15).contains(&duration) {
                    errors.push(failure(
                        "DURATION_OUT_OF_RANGE",
                        "时长应为 1–15 的整数，且仍需通过 Recipe 正式限制",
                    ));
                }
                let (width, height) = resolution(&manifest.defaults, shot).unwrap_or_else(|e| {
                    errors.push(e);
                    (0, 0)
                });
                Row {
                    row_key: format!(
                        "r{}-{}",
                        index + 1,
                        &sha(&serde_json::to_vec(shot).expect("serializable shot"))[..8]
                    ),
                    row_number: index + 1,
                    manifest_id: shot.id.clone(),
                    mode,
                    name,
                    prompt_preview: shot.prompt.chars().take(240).collect(),
                    prompt: shot.prompt.clone(),
                    duration_seconds: duration,
                    width,
                    height,
                    seed: shot.seed.or(manifest.defaults.seed),
                    generator_label: generator
                        .as_ref()
                        .map(|g| format!("{} / {}", g.workflow_version_id, g.recipe_id))
                        .unwrap_or_default(),
                    generator,
                    first_frame: shot.first_frame.clone(),
                    last_frame: shot.last_frame.clone(),
                    reference_count: shot.references.len(),
                    entity_names: shot.characters.clone(),
                    references: shot.references.clone(),
                    errors: errors
                        .into_iter()
                        .map(|e| crate::application::shot_bulk_service::BulkIssue {
                            code: e.code,
                            message: e.message,
                            row_number: Some(index + 1),
                            shot_id: None,
                        })
                        .collect(),
                    warnings: Vec::new(),
                }
            })
            .collect()
    }

    use crate::application::{
        generation_service::NewGenerationAdmission,
        ports::{
            AssetRepository, Clock, GenerationDefinitionRepository, ProjectRepository,
            ShotBulkRepository,
        },
        story_entity_service::StoryEntityService,
    };
    use crate::domain::story_entity::StoryEntity;
    use std::{
        collections::HashMap,
        io::Read,
        path::{Path, PathBuf},
        sync::Arc,
    };
    use tokio::sync::Mutex;

    #[derive(Clone, Debug, Default, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct PreviewOptions {
        pub name_suffix: Option<String>,
    }
    #[derive(Clone, Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct EntityPreview {
        pub name: String,
        pub kind: String,
        pub action: String,
        pub image_count: usize,
        pub existing_entity_id: Option<String>,
    }
    #[derive(Clone, Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Preview {
        pub session_id: String,
        pub preview_version: u64,
        pub name_suffix: Option<String>,
        pub display_root_name: String,
        pub manifest_sha256: String,
        pub format_source: String,
        pub shot_count: usize,
        pub valid_count: usize,
        pub error_count: usize,
        pub warning_count: usize,
        pub max_queue_items: usize,
        pub create_queue_default: bool,
        pub characters: Vec<EntityPreview>,
        pub rows: Vec<Row>,
        pub errors: Vec<crate::application::shot_bulk_service::BulkIssue>,
        pub warnings: Vec<String>,
    }
    #[derive(Clone)]
    struct Plan {
        manifest: Manifest,
        view: Preview,
        file_sha256: BTreeMap<String, String>,
        entities: Vec<StoryEntity>,
        fingerprints: BTreeMap<String, String>,
        options: PreviewOptions,
    }
    #[derive(Clone)]
    struct Session {
        project_id: String,
        root: PathBuf,
        created_at: chrono::DateTime<chrono::Utc>,
        plan: Plan,
    }
    pub struct FolderBatchImportService {
        sessions: Mutex<HashMap<String, Session>>,
        definitions: Arc<dyn GenerationDefinitionRepository>,
        admission: Arc<dyn NewGenerationAdmission>,
        shots: Arc<dyn ShotBulkRepository>,
        entities: Arc<StoryEntityService>,
        assets: Arc<dyn AssetRepository>,
        projects: Arc<dyn ProjectRepository>,
        clock: Arc<dyn Clock>,
    }
    impl FolderBatchImportService {
        pub fn new(
            definitions: Arc<dyn GenerationDefinitionRepository>,
            admission: Arc<dyn NewGenerationAdmission>,
            shots: Arc<dyn ShotBulkRepository>,
            entities: Arc<StoryEntityService>,
            assets: Arc<dyn AssetRepository>,
            projects: Arc<dyn ProjectRepository>,
            clock: Arc<dyn Clock>,
        ) -> Self {
            Self {
                sessions: Mutex::new(HashMap::new()),
                definitions,
                admission,
                shots,
                entities,
                assets,
                projects,
                clock,
            }
        }
        pub async fn pick(
            &self,
            project_id: &str,
            root: PathBuf,
        ) -> Result<Preview, ImportFailure> {
            crate::domain::validate_project_id(project_id)
                .map_err(|_| failure("INVALID_MANIFEST", "项目标识无效"))?;
            let id = format!("fbi_{}", uuid::Uuid::new_v4());
            let plan = self
                .build(project_id, &root, &id, 1, &PreviewOptions::default())
                .await?;
            let mut sessions = self.sessions.lock().await;
            sessions.retain(|_, s| {
                self.clock
                    .now()
                    .signed_duration_since(s.created_at)
                    .num_minutes()
                    < 30
            });
            if sessions.len() >= 32 {
                return Err(failure(
                    "IMPORT_IN_PROGRESS",
                    "导入会话过多，请先关闭未使用的预览",
                ));
            }
            let view = plan.view.clone();
            sessions.insert(
                id,
                Session {
                    project_id: project_id.into(),
                    root,
                    created_at: self.clock.now(),
                    plan,
                },
            );
            Ok(view)
        }
        pub async fn rescan(
            &self,
            id: &str,
            options: PreviewOptions,
        ) -> Result<Preview, ImportFailure> {
            let session = self.session(id).await?;
            if options
                .name_suffix
                .as_ref()
                .is_some_and(|s| s.chars().count() > 80 || s.contains(['\n', '\r', '\0']))
            {
                return Err(failure(
                    "INVALID_MANIFEST",
                    "名称后缀最长 80 字，不能含换行或 NUL",
                ));
            }
            let version = session
                .plan
                .view
                .preview_version
                .checked_add(1)
                .ok_or_else(|| failure("STALE_PREVIEW", "请重新选择文件夹"))?;
            let plan = self
                .build(&session.project_id, &session.root, id, version, &options)
                .await?;
            let mut sessions = self.sessions.lock().await;
            let live = sessions
                .get_mut(id)
                .ok_or_else(|| failure("SESSION_CONSUMED", "会话已关闭"))?;
            if live.plan.view.preview_version != session.plan.view.preview_version {
                return Err(failure("STALE_PREVIEW", "预览已被更新，请重新校验"));
            }
            live.plan = plan;
            live.created_at = self.clock.now();
            Ok(live.plan.view.clone())
        }
        pub async fn discard(&self, id: &str) -> Result<(), ImportFailure> {
            self.sessions.lock().await.remove(id);
            Ok(())
        }
        async fn session(&self, id: &str) -> Result<Session, ImportFailure> {
            let sessions = self.sessions.lock().await;
            let s = sessions
                .get(id)
                .ok_or_else(|| failure("SESSION_CONSUMED", "预览已关闭或过期，请重新选择文件夹"))?;
            if self
                .clock
                .now()
                .signed_duration_since(s.created_at)
                .num_minutes()
                >= 30
            {
                return Err(failure("SESSION_CONSUMED", "预览已过期，请重新选择文件夹"));
            }
            Ok(s.clone())
        }
        async fn build(
            &self,
            project: &str,
            root: &Path,
            id: &str,
            version: u64,
            options: &PreviewOptions,
        ) -> Result<Plan, ImportFailure> {
            let scanned =
                super::source_asset_import_service::scan_import_folder(root).map_err(|_| {
                    failure(
                        "LINK_REJECTED",
                        "目录不可安全扫描或超过 4 层/10,000 条目限制",
                    )
                })?;
            let bytes = read_file(root, "manifest.json", 1024 * 1024)?;
            let manifest = parse_manifest(&bytes)?;
            if self
                .projects
                .find_by_id(project)
                .await
                .map_err(|_| failure("PROJECT_UNAVAILABLE", "无法读取项目"))?
                .is_none()
            {
                return Err(failure("PROJECT_UNAVAILABLE", "项目不存在"));
            }
            let names = self
                .shots
                .list_bulk_data(project)
                .await
                .map_err(|_| failure("PROJECT_UNAVAILABLE", "无法读取当前项目镜头"))?
                .into_iter()
                .map(|s| s.shot.name)
                .collect::<Vec<_>>();
            let existing = self
                .entities
                .list(project, None)
                .await
                .map_err(|_| failure("PROJECT_UNAVAILABLE", "无法读取当前项目实体"))?;
            let mut entities = existing.clone();
            for (i, entity) in manifest.characters.iter().enumerate() {
                if !existing.iter().any(|e| {
                    e.name == entity.name
                        && e.kind.as_str() == entity.kind.as_deref().unwrap_or("character")
                }) {
                    entities.push(StoryEntity {
                        id: format!("manifest-{i}"),
                        project_id: project.into(),
                        name: entity.name.clone(),
                        kind: crate::domain::story_entity::StoryEntityKind::parse(
                            entity.kind.as_deref().unwrap_or("character"),
                        )
                        .map_err(|_| failure("INVALID_MANIFEST", "实体类型无效"))?,
                        description: entity.description.clone().unwrap_or_default(),
                        cover_asset_id: None,
                        image_asset_ids: vec![],
                        created_at: String::new(),
                        updated_at: String::new(),
                    });
                }
            }
            let mut rows = normalize(&manifest, options.name_suffix.as_deref(), &names);
            let mut file_sha256 = BTreeMap::new();
            let mut fingerprints = BTreeMap::new();
            let mut used = BTreeSet::new();
            for row in &mut rows {
                if crate::domain::canonical_shot_name(&row.name).is_err() {
                    row_error(
                        row,
                        failure("INVALID_MANIFEST", "镜头名称不符合正式命名规则"),
                    );
                }
                let mentions = super::shot_video_input_service::explicit_entity_mentions(
                    &row.prompt,
                    project,
                    &entities,
                );
                let mut reference_paths = row.references.clone();
                match mentions {
                    Err(error) => {
                        row_error(row, failure("ENTITY_REFERENCE_UNKNOWN", error.to_string()))
                    }
                    Ok(mentions) => {
                        for entity in &mentions {
                            if !row.entity_names.contains(&entity.name) {
                                row.entity_names.push(entity.name.clone());
                            }
                        }
                        for entity in mentions {
                            match entity_image(&manifest, &existing, entity) {
                                Ok(path) => {
                                    if !reference_paths.contains(&path) {
                                        reference_paths.push(path);
                                    }
                                }
                                Err(error) => row_error(row, error),
                            }
                        }
                    }
                }
                let mut files: BTreeSet<String> = row
                    .first_frame
                    .iter()
                    .chain(row.last_frame.iter())
                    .chain(row.references.iter())
                    .cloned()
                    .collect();
                for name in row.entity_names.clone() {
                    let matches: Vec<_> = entities.iter().filter(|e| e.name == name).collect();
                    if matches.len() != 1 {
                        row_error(
                            row,
                            failure(
                                "ENTITY_REFERENCE_AMBIGUOUS",
                                "实体不存在或同名，请使用唯一实体名称",
                            ),
                        );
                        continue;
                    }
                    let entity = matches[0];
                    used.insert(entity.id.clone());
                    if !existing.iter().any(|e| e.id == entity.id) {
                        if let Some(declared) = manifest.characters.iter().find(|e| e.name == name)
                        {
                            files.extend(declared.images.iter().cloned());
                        }
                    } else {
                        let cover = entity
                            .cover_asset_id
                            .as_ref()
                            .or_else(|| entity.image_asset_ids.first());
                        if let Some(asset) = cover {
                            match crate::domain::AssetId::parse(asset) {
                                Err(_) => row_error(
                                    row,
                                    failure("ENTITY_REFERENCE_MISSING_IMAGE", "实体参考素材不可用"),
                                ),
                                Ok(asset) => {
                                    let available =
                                        self.assets.find_by_id(&asset).await.map_err(|_| {
                                            failure("IMAGE_INVALID", "无法检查实体素材")
                                        })?;
                                    let receipt = self
                                        .assets
                                        .find_external_import(project, &asset)
                                        .await
                                        .map_err(|_| {
                                            failure("IMAGE_INVALID", "无法检查素材收据")
                                        })?;
                                    if available.as_ref().is_none_or(|a| {
                                        a.project_id != project
                                            || a.source_task_id.is_some()
                                            || a.category != crate::domain::SOURCE_IMAGE_CATEGORY
                                    }) || receipt.is_none()
                                    {
                                        row_error(row,failure("IMAGE_INVALID","实体图片必须是当前项目具有导入收据的外部 SOURCE 图片"));
                                    }
                                }
                            }
                        } else {
                            row_error(
                                row,
                                failure("ENTITY_REFERENCE_MISSING_IMAGE", "实体没有关联图片"),
                            );
                        }
                    }
                }
                for file in files {
                    if file_sha256.contains_key(&file) {
                        continue;
                    }
                    match read_file(
                        root,
                        &file,
                        super::source_asset_import_service::MAX_SOURCE_IMAGE_BYTES,
                    ) {
                        Ok(bytes) => {
                            let name = Path::new(&file)
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("");
                            if !scanned.contains(&file)
                                || super::source_asset_import_service::inspect_import_image(
                                    name, &bytes,
                                )
                                .is_err()
                            {
                                row_error(
                                    row,
                                    failure(
                                        "IMAGE_INVALID",
                                        "参考文件必须是扩展名匹配、可解码的 PNG/JPEG/WebP 图片",
                                    ),
                                );
                            } else {
                                file_sha256.insert(file, sha(&bytes));
                            }
                        }
                        Err(error) => row_error(row, error),
                    }
                }
                row.reference_count = reference_paths.iter().collect::<BTreeSet<_>>().len();
                if let Some(generator) = &row.generator {
                    let generator = generator.clone();
                    let pair = serde_json::to_string(&generator).expect("serializable generator");
                    let definition = self
                        .definitions
                        .find_active(&generator.workflow_version_id, &generator.recipe_id)
                        .await
                        .map_err(|_| failure("GENERATOR_UNAVAILABLE", "无法读取生成器"))?;
                    let admitted = self
                        .admission
                        .is_available_for_new_generation(
                            &generator.workflow_version_id,
                            &generator.recipe_id,
                        )
                        .await
                        .map_err(|_| failure("GENERATOR_UNAVAILABLE", "无法检查生成器准入"))?;
                    match definition.filter(|_| admitted) {
                        None => row_error(
                            row,
                            failure("GENERATOR_UNAVAILABLE", "所选生成器不允许新的视频生产"),
                        ),
                        Some(definition) => {
                            fingerprints.insert(
                                pair,
                                format!(
                                    "{}:{}",
                                    definition.workflow_sha256, definition.recipe_sha256
                                ),
                            );
                            if let Err(error) =
                                validate_definition(row, &definition, &reference_paths)
                            {
                                row_error(row, error);
                            }
                        }
                    }
                }
            }
            let characters = entities
                .iter()
                .filter(|e| used.contains(&e.id))
                .map(|e| EntityPreview {
                    name: e.name.clone(),
                    kind: e.kind.as_str().into(),
                    action: if existing.iter().any(|old| old.id == e.id) {
                        "reuse"
                    } else {
                        "create"
                    }
                    .into(),
                    image_count: if existing.iter().any(|old| old.id == e.id) {
                        e.image_asset_ids.len()
                    } else {
                        manifest
                            .characters
                            .iter()
                            .find(|m| m.name == e.name)
                            .map(|m| m.images.len())
                            .unwrap_or(0)
                    },
                    existing_entity_id: existing
                        .iter()
                        .find(|old| old.id == e.id)
                        .map(|old| old.id.clone()),
                })
                .collect();
            let mut warnings = if scanned.iter().any(|p| !file_sha256.contains_key(p)) {
                vec!["目录含未引用图片；只会导入选中有效行实际使用的文件。".into()]
            } else {
                vec![]
            };
            for declared in &manifest.characters {
                if existing.iter().any(|e| e.name == declared.name) {
                    warnings.push(format!(
                        "复用实体 {}：清单内该实体图片将被忽略，不修改已有实体。",
                        declared.name
                    ));
                } else if existing
                    .iter()
                    .any(|e| e.name.to_lowercase() == declared.name.to_lowercase())
                {
                    warnings.push(format!(
                        "实体 {} 与已有名称仅大小写不同；@ 引用仍精确区分大小写。",
                        declared.name
                    ));
                }
            }
            let view = Preview {
                session_id: id.into(),
                preview_version: version,
                name_suffix: options.name_suffix.clone(),
                display_root_name: root
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "所选目录".into()),
                manifest_sha256: sha(&bytes),
                format_source: "manifest".into(),
                shot_count: rows.len(),
                valid_count: rows.iter().filter(|r| r.errors.is_empty()).count(),
                error_count: rows.iter().filter(|r| !r.errors.is_empty()).count(),
                warning_count: warnings.len(),
                max_queue_items: super::shot_batch_service::MAX_SHOT_BATCH_ITEMS,
                create_queue_default: manifest.queue.create.unwrap_or(true),
                characters,
                rows,
                errors: vec![],
                warnings,
            };
            Ok(Plan {
                manifest,
                view,
                file_sha256,
                entities,
                fingerprints,
                options: options.clone(),
            })
        }
    }
    fn row_error(row: &mut Row, error: ImportFailure) {
        row.errors
            .push(crate::application::shot_bulk_service::BulkIssue {
                code: error.code,
                message: error.message,
                row_number: Some(row.row_number),
                shot_id: None,
            });
    }
    fn read_file(root: &Path, relative: &str, limit: u64) -> Result<Vec<u8>, ImportFailure> {
        if !safe_relative_path(relative) {
            return Err(failure("PATH_OUTSIDE_ROOT", "拒绝不安全相对路径"));
        }
        let file = super::source_asset_import_service::open_import_file(root, relative)
            .map_err(|_| failure("FILE_NOT_FOUND", "文件不可安全读取、丢失或已被替换"))?;
        let size = file
            .metadata()
            .map_err(|_| failure("IMAGE_INVALID", "无法检查文件"))?
            .len();
        if size > limit {
            return Err(failure("SCAN_LIMIT_EXCEEDED", "文件超过安全大小限制"));
        }
        let mut bytes = Vec::new();
        file.take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| failure("IMAGE_INVALID", "文件读取失败"))?;
        if bytes.len() as u64 != size {
            return Err(failure("STALE_PREVIEW", "文件在读取期间发生变化"));
        }
        Ok(bytes)
    }
    fn entity_image(
        manifest: &Manifest,
        existing: &[StoryEntity],
        entity: &StoryEntity,
    ) -> Result<String, ImportFailure> {
        if existing.iter().any(|e| e.id == entity.id) {
            entity
                .cover_asset_id
                .as_ref()
                .or_else(|| entity.image_asset_ids.first())
                .map(|id| format!("managed:{id}"))
                .ok_or_else(|| failure("ENTITY_REFERENCE_MISSING_IMAGE", "实体缺少参考图片"))
        } else {
            manifest
                .characters
                .iter()
                .find(|e| e.name == entity.name)
                .and_then(|e| e.cover.as_ref().or_else(|| e.images.first()))
                .cloned()
                .ok_or_else(|| failure("ENTITY_REFERENCE_MISSING_IMAGE", "新实体缺少参考图片"))
        }
    }
    fn validate_definition(
        row: &Row,
        definition: &crate::application::ports::GenerationDefinition,
        references: &[String],
    ) -> Result<(), ImportFailure> {
        use super::shot_workflow_compatibility::{classify_shot_recipe, ShotVideoInputMode};
        use crate::domain::{CompileRequest, InputValue, SeedValue, ShotStage, WorkflowDocument};
        let recipe = crate::compiler::RecipeParser::parse(&definition.recipe_yaml)
            .map_err(|_| failure("GENERATOR_UNAVAILABLE", "Recipe 无效"))?;
        let compatibility =
            classify_shot_recipe(ShotStage::Video, &definition.workflow_id, &recipe)
                .map_err(|_| failure("MODE_MISMATCH", "Recipe 不支持镜头视频输入"))?;
        let compatible = match (&row.mode, &compatibility.input_mode) {
            (Mode::FirstLast, ShotVideoInputMode::FirstLast)
            | (Mode::Reference, ShotVideoInputMode::ReferenceMedia) => true,
            (Mode::Image, ShotVideoInputMode::SingleImage { key }) => key == "first_frame",
            _ => false,
        };
        if !compatible {
            return Err(failure(
                "MODE_MISMATCH",
                "所选 Recipe 的正式输入模式与该行不一致",
            ));
        }
        let mut values = BTreeMap::from([
            ("prompt".into(), InputValue::String(row.prompt.clone())),
            ("width".into(), InputValue::Integer(row.width)),
            ("height".into(), InputValue::Integer(row.height)),
            (
                "duration_seconds".into(),
                InputValue::Integer(row.duration_seconds),
            ),
        ]);
        if let Some(seed) = row.seed {
            values.insert("seed".into(), InputValue::Seed(SeedValue::Fixed(seed)));
        }
        if let Some(path) = &row.first_frame {
            values.insert("first_frame".into(), InputValue::Image(path.clone()));
        }
        if let Some(path) = &row.last_frame {
            values.insert("last_frame".into(), InputValue::Image(path.clone()));
        }
        if row.mode == Mode::Reference {
            values.insert(
                "reference_images".into(),
                InputValue::Images(references.to_vec()),
            );
        }
        let workflow = WorkflowDocument::parse(definition.workflow_json.clone())
            .map_err(|_| failure("GENERATOR_UNAVAILABLE", "工作流无效"))?;
        super::product::h3_resolution::compile_checked(
            &crate::compiler::WorkflowCompiler,
            &workflow,
            &recipe,
            &CompileRequest::new(values),
        )
        .map_err(|e| {
            let code = match &e {
                crate::compiler::CompileError::InputOutOfRange { input, .. }
                    if input == "duration_seconds" =>
                {
                    "DURATION_OUT_OF_RANGE"
                }
                crate::compiler::CompileError::InputOutOfRange { input, .. }
                    if input == "width" || input == "height" =>
                {
                    "RESOLUTION_UNSUPPORTED"
                }
                _ => "INPUT_VALIDATION_FAILED",
            };
            failure(code, e.to_string())
        })?;
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        fn manifest() -> Manifest {
            parse_manifest(br#"{"format":"ai-studio-shot-batch","version":1,"name":"test","generators":{"firstLast":{"workflowVersionId":"wv","recipeId":"recipe"},"reference":{"workflowVersionId":"ref","recipeId":"recipe"}},"defaults":{"durationSeconds":5,"aspectRatio":"16:9","resolutionPreset":"0.98"},"shots":[{"prompt":"open door","mode":"firstLast","firstFrame":"images/a.png","lastFrame":"images/b.png"},{"prompt":"@Alice turn","mode":"reference","references":["images/c.png"],"aspectRatio":"9:16"}]}"#).unwrap()
        }
        #[test]
        fn normalizes_independent_slots_order_defaults_and_stable_missing_ids() {
            let m = manifest();
            let rows = normalize(&m, None, &[]);
            assert!(rows.iter().all(|r| r.errors.is_empty()));
            assert_eq!((rows[0].width, rows[0].height), (1344, 768));
            assert_eq!((rows[1].width, rows[1].height), (768, 1344));
            assert_ne!(rows[0].first_frame, rows[0].last_frame);
            assert_ne!(rows[0].row_key, rows[1].row_key);
            assert_eq!(
                rows[0].row_key,
                normalize(&m, Some(" copy"), &[])[0].row_key
            );
        }
        #[test]
        fn rejects_unknown_schema_versions_duplicate_ids_and_entities() {
            let mut value = serde_json::to_value(manifest()).unwrap();
            value["unknown"] = true.into();
            assert!(parse_manifest(&serde_json::to_vec(&value).unwrap()).is_err());
            value.as_object_mut().unwrap().remove("unknown");
            value["version"] = 2.into();
            assert!(parse_manifest(&serde_json::to_vec(&value).unwrap()).is_err());
            value["version"] = 1.into();
            value["shots"][0]["id"] = "same".into();
            value["shots"][1]["id"] = "same".into();
            assert!(parse_manifest(&serde_json::to_vec(&value).unwrap()).is_err());
            assert!(parse_manifest(&vec![0; 1024 * 1024 + 1]).is_err());
        }
        #[test]
        fn blocks_mixed_modes_bad_paths_and_resolution_conflicts() {
            let mut m = manifest();
            m.shots[0].prompt = "@Alice".into();
            assert!(!normalize(&m, None, &[])[0].errors.is_empty());
            m = manifest();
            m.shots[0].first_frame = None;
            assert!(!normalize(&m, None, &[])[0].errors.is_empty());
            m = manifest();
            m.shots[0].width = Some(1280);
            m.shots[0].height = Some(720);
            assert!(normalize(&m, None, &[])[0]
                .errors
                .iter()
                .any(|e| e.code == "RESOLUTION_UNSUPPORTED"));
            for p in [
                "../x.png",
                "/x.png",
                "C:/x.png",
                "a\\x.png",
                "a/../x.png",
                "a//b.png",
                "a\0.png",
                "a:stream",
            ] {
                assert!(!safe_relative_path(p), "{p:?}");
            }
        }
        #[test]
        fn bounds_and_duplicate_names_require_formal_suffix_revalidation() {
            let mut m = manifest();
            for duration in [0, 16] {
                m.shots[0].duration_seconds = Some(duration);
                assert!(normalize(&m, None, &[])[0]
                    .errors
                    .iter()
                    .any(|e| e.code == "DURATION_OUT_OF_RANGE"));
            }
            for duration in [1, 15] {
                m.shots[0].duration_seconds = Some(duration);
                assert!(!normalize(&m, None, &[])[0]
                    .errors
                    .iter()
                    .any(|e| e.code == "DURATION_OUT_OF_RANGE"));
            }
            assert!(normalize(&m, None, &["TEST S01".into()])[0]
                .errors
                .iter()
                .any(|e| e.code == "DUPLICATE_SHOT_NAME"));
            assert!(normalize(&m, Some(" copy"), &["test S01".into()])[0]
                .errors
                .is_empty());
        }
        #[test]
        fn shares_exact_entity_fixtures_with_frontend_and_rejects_other_project() {
            use crate::domain::story_entity::{StoryEntity, StoryEntityKind};
            let cases: serde_json::Value = serde_json::from_str(include_str!(
                "../../../src/features/shots/__fixtures__/folderEntityMentions.json"
            ))
            .unwrap();
            for case in cases.as_array().unwrap() {
                let entities: Vec<_> = case["names"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .enumerate()
                    .map(|(i, n)| StoryEntity {
                        id: format!("entity-{i}"),
                        project_id: "project".into(),
                        name: n.as_str().unwrap().into(),
                        kind: StoryEntityKind::Character,
                        description: String::new(),
                        cover_asset_id: None,
                        image_asset_ids: vec![],
                        created_at: String::new(),
                        updated_at: String::new(),
                    })
                    .collect();
                let found = crate::application::shot_video_input_service::explicit_entity_mentions(
                    case["prompt"].as_str().unwrap(),
                    "project",
                    &entities,
                );
                if case["expected"].is_null() {
                    assert!(found.is_err());
                } else {
                    let actual: Vec<_> = found.unwrap().iter().map(|e| e.name.as_str()).collect();
                    assert_eq!(serde_json::to_value(actual).unwrap(), case["expected"]);
                }
                if case["prompt"].as_str().unwrap().contains('@') {
                    assert!(
                        crate::application::shot_video_input_service::explicit_entity_mentions(
                            case["prompt"].as_str().unwrap(),
                            "other",
                            &entities
                        )
                        .is_err()
                    );
                }
            }
        }
        #[test]
        fn folder_scan_has_separate_budgets_and_cannot_open_traversal() {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path();
            for i in 0..501 {
                std::fs::write(root.join(format!("{i}.png")), b"fixture").unwrap();
            }
            assert_eq!(
                crate::application::source_asset_import_service::scan_import_folder(root)
                    .unwrap()
                    .len(),
                501
            );
            assert!(
                crate::application::source_asset_import_service::open_import_file(
                    root,
                    "../other.png"
                )
                .is_err()
            );
            std::fs::create_dir_all(root.join("a/b/c/d/e")).unwrap();
            assert!(
                crate::application::source_asset_import_service::scan_import_folder(root).is_err()
            );
        }
        struct PreviewClock;
        impl crate::application::ports::Clock for PreviewClock {
            fn now(&self) -> chrono::DateTime<chrono::Utc> {
                chrono::Utc::now()
            }
        }
        struct PreviewDefinitions(Vec<crate::application::ports::GenerationDefinition>);
        #[async_trait::async_trait]
        impl crate::application::ports::GenerationDefinitionRepository for PreviewDefinitions {
            async fn find(
                &self,
                version: &str,
                recipe: &str,
            ) -> Result<
                Option<crate::application::ports::GenerationDefinition>,
                crate::application::ports::RepositoryError,
            > {
                Ok(self
                    .0
                    .iter()
                    .find(|d| d.workflow_version_id == version && d.recipe_id == recipe)
                    .cloned())
            }
            async fn list_available(
                &self,
            ) -> Result<
                Vec<crate::application::ports::AvailableGenerationDefinition>,
                crate::application::ports::RepositoryError,
            > {
                Ok(vec![])
            }
        }
        struct PreviewAdmission;
        #[async_trait::async_trait]
        impl crate::application::generation_service::NewGenerationAdmission for PreviewAdmission {
            async fn is_available_for_new_generation(
                &self,
                _: &str,
                _: &str,
            ) -> Result<bool, crate::application::ports::RepositoryError> {
                Ok(true)
            }
        }
        struct PreviewFixture {
            _temp: tempfile::TempDir,
            root: std::path::PathBuf,
            pool: sqlx::SqlitePool,
            service: FolderBatchImportService,
        }
        async fn preview_fixture() -> PreviewFixture {
            use crate::infrastructure::database::repositories::story_entity::SqliteStoryEntityRepository;
            use crate::infrastructure::database::{
                SqliteAssetRepository, SqliteProjectRepository, SqliteShotRepository,
            };
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("sample");
            std::fs::create_dir_all(root.join("images")).unwrap();
            let pool = crate::infrastructure::database::initialize(&temp.path().join("test.db"))
                .await
                .unwrap();
            sqlx::query("INSERT INTO projects(id,name,root_path,created_at,updated_at) VALUES ('prj_00000000-0000-4000-8000-000000000001','Import test',?,'2026-10-21T00:00:00Z','2026-10-21T00:00:00Z')")
                .bind(temp.path().join("project").to_string_lossy().to_string()).execute(&pool).await.unwrap();
            let mut definitions = Vec::new();
            let mut m = manifest();
            for (mode, package, version, workflow_id) in [
                (
                    Mode::FirstLast,
                    "minimax_h3_fl2va_first_last_quality_2_2_0",
                    "first-version",
                    "wfl_minimax_h3_fl2va_first_last_quality",
                ),
                (
                    Mode::Reference,
                    "minimax_h3_reference_video_quality_2_2_0",
                    "ref-version",
                    "wfl_minimax_h3_reference_video_quality",
                ),
            ] {
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("runtime_packages")
                    .join(package);
                let yaml = std::fs::read_to_string(path.join("recipe.yaml")).unwrap();
                let workflow = std::fs::read(path.join("workflow_api.json")).unwrap();
                let recipe = crate::compiler::RecipeParser::parse(&yaml).unwrap();
                m.generators.insert(
                    mode,
                    Generator {
                        workflow_version_id: version.into(),
                        recipe_id: recipe.id.clone(),
                    },
                );
                definitions.push(crate::application::ports::GenerationDefinition {
                    workflow_id: workflow_id.into(),
                    workflow_version_id: version.into(),
                    recipe_id: recipe.id,
                    workflow_version: "2.2.0".into(),
                    workflow_sha256: sha(&workflow),
                    recipe_version: "2.2.0".into(),
                    recipe_sha256: sha(yaml.as_bytes()),
                    package_name: None,
                    package_source_path: None,
                    workflow_json: serde_json::from_slice(&workflow).unwrap(),
                    recipe_yaml: yaml,
                });
            }
            m.characters.push(ManifestEntity {
                name: "Alice".into(),
                kind: None,
                description: None,
                images: vec!["images/alice.png".into()],
                cover: None,
            });
            let mut png = Vec::new();
            image::DynamicImage::new_rgb8(32, 32)
                .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
                .unwrap();
            for name in ["a.png", "b.png", "c.png", "alice.png"] {
                std::fs::write(root.join("images").join(name), &png).unwrap();
            }
            std::fs::write(root.join("manifest.json"), serde_json::to_vec(&m).unwrap()).unwrap();
            let service = FolderBatchImportService::new(
                Arc::new(PreviewDefinitions(definitions)),
                Arc::new(PreviewAdmission),
                Arc::new(SqliteShotRepository::new(pool.clone())),
                Arc::new(StoryEntityService::new(Arc::new(
                    SqliteStoryEntityRepository::new(pool.clone()),
                ))),
                Arc::new(SqliteAssetRepository::new(pool.clone())),
                Arc::new(SqliteProjectRepository::new(pool.clone())),
                Arc::new(PreviewClock),
            );
            PreviewFixture {
                _temp: temp,
                root,
                pool,
                service,
            }
        }
        #[tokio::test]
        async fn previews_real_folder_and_formal_recipe_without_any_writes() {
            let fixture = preview_fixture().await;
            let view = fixture
                .service
                .pick(
                    "prj_00000000-0000-4000-8000-000000000001",
                    fixture.root.clone(),
                )
                .await
                .unwrap();
            assert_eq!(
                (view.shot_count, view.valid_count, view.error_count),
                (2, 2, 0),
                "{:?}",
                view.rows
            );
            assert_eq!(view.rows[1].reference_count, 2);
            assert_eq!(view.characters[0].action, "create");
            assert!(!serde_json::to_string(&view)
                .unwrap()
                .contains(&fixture.root.to_string_lossy().to_string()));
            for table in [
                "shots",
                "tasks",
                "production_batches",
                "shot_generation_links",
                "assets",
                "story_entities",
            ] {
                let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
                    .fetch_one(&fixture.pool)
                    .await
                    .unwrap();
                assert_eq!(count, 0, "{table}");
            }
            fixture.pool.close().await;
        }
        #[tokio::test]
        async fn rescan_has_versioned_suffix_and_discard_or_expiry_cannot_reopen() {
            let fixture = preview_fixture().await;
            let first = fixture
                .service
                .pick(
                    "prj_00000000-0000-4000-8000-000000000001",
                    fixture.root.clone(),
                )
                .await
                .unwrap();
            let next = fixture
                .service
                .rescan(
                    &first.session_id,
                    PreviewOptions {
                        name_suffix: Some(" copy".into()),
                    },
                )
                .await
                .unwrap();
            assert_eq!(next.preview_version, first.preview_version + 1);
            assert!(next.rows[0].name.ends_with(" copy"));
            fixture.service.discard(&first.session_id).await.unwrap();
            assert!(fixture
                .service
                .rescan(&first.session_id, PreviewOptions::default())
                .await
                .is_err());
            let view = fixture
                .service
                .pick(
                    "prj_00000000-0000-4000-8000-000000000001",
                    fixture.root.clone(),
                )
                .await
                .unwrap();
            fixture
                .service
                .sessions
                .lock()
                .await
                .get_mut(&view.session_id)
                .unwrap()
                .created_at -= chrono::Duration::minutes(31);
            assert!(fixture
                .service
                .rescan(&view.session_id, PreviewOptions::default())
                .await
                .is_err());
            assert!(fixture
                .service
                .pick(
                    "prj_00000000-0000-4000-8000-000000000002",
                    fixture.root.clone()
                )
                .await
                .is_err());
            fixture.pool.close().await;
        }
        #[tokio::test]
        async fn formal_constraints_and_missing_or_bad_images_are_row_errors() {
            let fixture = preview_fixture().await;
            std::fs::write(fixture.root.join("images/a.png"), b"not image").unwrap();
            let view = fixture
                .service
                .pick(
                    "prj_00000000-0000-4000-8000-000000000001",
                    fixture.root.clone(),
                )
                .await
                .unwrap();
            assert_eq!(view.valid_count, 1);
            let mut m: Manifest =
                serde_json::from_slice(&std::fs::read(fixture.root.join("manifest.json")).unwrap())
                    .unwrap();
            m.shots[1].duration_seconds = Some(1);
            std::fs::write(
                fixture.root.join("manifest.json"),
                serde_json::to_vec(&m).unwrap(),
            )
            .unwrap();
            let view = fixture
                .service
                .rescan(&view.session_id, PreviewOptions::default())
                .await
                .unwrap();
            assert_eq!(view.valid_count, 0);
            assert!(view.rows[1]
                .errors
                .iter()
                .any(|e| e.code == "DURATION_OUT_OF_RANGE"));
            fixture.pool.close().await;
        }
    }
}
