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

        pub async fn composition_usage_for_shot(
            &self,
            project_id: &str,
            shot_id: &str,
        ) -> Result<crate::domain::story_entity::ShotCompositionUsage, AppError> {
            self.repo
                .composition_usage_for_shot(project_id, shot_id)
                .await
                .map_err(map_repo)
        }

        pub async fn set_composition_usage(
            &self,
            project_id: &str,
            shot_id: &str,
            usage: crate::domain::story_entity::ShotCompositionUsage,
        ) -> Result<crate::domain::story_entity::ShotCompositionUsage, AppError> {
            self.repo
                .set_composition_usage(project_id, shot_id, &usage)
                .await
                .map_err(map_repo)
        }

        pub async fn prompt_sections_for_shot(
            &self,
            project_id: &str,
            shot_id: &str,
        ) -> Result<crate::domain::story_entity::ShotPromptSections, AppError> {
            self.repo
                .prompt_sections_for_shot(project_id, shot_id)
                .await
                .map_err(map_repo)
        }

        pub async fn set_prompt_sections(
            &self,
            project_id: &str,
            shot_id: &str,
            sections: crate::domain::story_entity::ShotPromptSections,
        ) -> Result<crate::domain::story_entity::ShotPromptSections, AppError> {
            self.repo
                .set_prompt_sections(project_id, shot_id, &sections)
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
        /// Shot description (detailed_description body); `description` is accepted as an alias.
        #[serde(alias = "description")]
        pub prompt: String,
        /// Optional writer sections of the H3 Ref2VA prompt (English).
        #[serde(default)]
        pub style: Option<String>,
        #[serde(default)]
        pub summary: Option<String>,
        #[serde(default)]
        pub soundscape: Option<String>,
        #[serde(default)]
        pub music: Option<String>,
        pub first_frame: Option<String>,
        pub last_frame: Option<String>,
        #[serde(default)]
        pub references: Vec<String>,
        #[serde(default)]
        pub characters: Vec<String>,
        /// 构图用途: name of the composition entity governing the opening frame.
        #[serde(default)]
        pub opening_composition: Option<String>,
        /// 构图用途: name of the composition entity governing the final frame.
        #[serde(default)]
        pub ending_composition: Option<String>,
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
        /// Entity appearance text; `appearanceEn` is accepted as an alias.
        #[serde(alias = "appearanceEn")]
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
        pub opening_composition: Option<String>,
        pub ending_composition: Option<String>,
        #[serde(skip)]
        pub prompt_sections: crate::domain::story_entity::ShotPromptSections,
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
    /// Names the offending manifest field and its position; validation is unchanged.
    pub(crate) fn manifest_error_message(error: &serde_json::Error) -> String {
        let text = error.to_string();
        let field = |marker: &str| {
            text.split_once(marker)
                .and_then(|(_, rest)| rest.split_once('`'))
                .map(|(name, _)| name.to_owned())
        };
        let position = if error.line() > 0 {
            format!("（第 {} 行第 {} 列）", error.line(), error.column())
        } else {
            String::new()
        };
        if let Some(name) = field("missing field `") {
            format!("清单缺少必填字段 {name}{position}")
        } else if let Some(name) = field("unknown field `") {
            format!("清单含未知字段 {name}{position}")
        } else if let Some(name) = field("unknown variant `") {
            format!("清单含无法识别的取值 {name}{position}")
        } else if error.is_syntax() || error.is_eof() {
            format!("清单不是有效的 JSON{position}")
        } else {
            format!("清单字段类型或取值不正确{position}")
        }
    }
    pub(crate) fn parse_manifest(bytes: &[u8]) -> Result<Manifest, ImportFailure> {
        if bytes.len() > 1024 * 1024 {
            return Err(failure("SCAN_LIMIT_EXCEEDED", "manifest 最大为 1 MB"));
        }
        let manifest: Manifest = serde_json::from_slice(bytes)
            .map_err(|error| failure("INVALID_MANIFEST", manifest_error_message(&error)))?;
        if manifest.format != "ai-studio-shot-batch" || manifest.version != 1 {
            return Err(failure(
                "UNSUPPORTED_VERSION",
                "仅支持 ai-studio-shot-batch version 1",
            ));
        }
        if manifest.queue.name.as_ref().is_some_and(|name| {
            name.trim().is_empty()
                || name.trim().chars().count() > 120
                || name.contains(['\0', '\n', '\r'])
        }) {
            return Err(failure(
                "INVALID_MANIFEST",
                "批次名称必须为1–120字且不能含换行或 NUL",
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
                    .is_some_and(|k| !matches!(k, "character" | "scene" | "prop" | "composition"))
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
                let has_usage =
                    shot.opening_composition.is_some() || shot.ending_composition.is_some();
                if has_usage && mode != Mode::Reference {
                    errors.push(failure(
                        "COMPOSITION_USAGE_MODE",
                        "开场构图/结尾构图仅适用于混合参考模式的镜头",
                    ));
                }
                if let (Some(opening), Some(ending)) =
                    (&shot.opening_composition, &shot.ending_composition)
                {
                    if opening.trim().to_lowercase() == ending.trim().to_lowercase() {
                        errors.push(failure(
                            "COMPOSITION_USAGE_SAME",
                            "开场构图与结尾构图不能是同一构图",
                        ));
                    }
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
                if duration > super::product::h3_resolution::MAX_SHOT_DURATION_SECONDS {
                    errors.push(failure(
                        "DURATION_TOO_LONG",
                        super::product::h3_resolution::SHOT_DURATION_LIMIT_MESSAGE,
                    ));
                } else if duration < 1 {
                    errors.push(failure(
                        "DURATION_OUT_OF_RANGE",
                        "时长应为 1–11 的整数，且仍需通过 Recipe 正式限制",
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
                    prompt_sections: crate::domain::story_entity::ShotPromptSections {
                        style: shot.style.clone(),
                        summary: shot.summary.clone(),
                        soundscape: shot.soundscape.clone(),
                        music: shot.music.clone(),
                    }
                    .normalized(),
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
                    opening_composition: shot.opening_composition.clone(),
                    ending_composition: shot.ending_composition.clone(),
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
        pub legacy: Option<LegacyDefaults>,
    }
    #[derive(Clone, Debug, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct LegacyDefaults {
        pub selection_ref: String,
        pub duration_seconds: i64,
        pub width: i64,
        pub height: i64,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct LegacyImageRow {
        image: String,
        prompt: String,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct LegacyReferenceRow {
        #[serde(default)]
        images: Vec<String>,
        #[serde(default)]
        videos: Vec<String>,
        #[serde(default)]
        audios: Vec<String>,
        prompt: String,
    }
    fn legacy_manifest(
        bytes: &[u8],
        source: &str,
        options: &PreviewOptions,
    ) -> Result<Manifest, ImportFailure> {
        let mode = if source == "h3-batch" {
            Mode::Image
        } else {
            Mode::Reference
        };
        let rows: Vec<(Option<String>, Vec<String>, String)> = if mode == Mode::Image {
            let rows: Vec<LegacyImageRow> = serde_json::from_slice(bytes)
                .map_err(|_| failure("INVALID_MANIFEST", "旧版 h3-batch 清单格式无效"))?;
            rows.into_iter()
                .map(|r| (Some(r.image), vec![], r.prompt))
                .collect()
        } else {
            let rows: Vec<LegacyReferenceRow> = serde_json::from_slice(bytes)
                .map_err(|_| failure("INVALID_MANIFEST", "旧版 h3-omni-batch 清单格式无效"))?;
            if rows
                .iter()
                .any(|r| !r.videos.is_empty() || !r.audios.is_empty())
            {
                return Err(failure(
                    "UNSUPPORTED_LEGACY_MEDIA",
                    "旧版导入暂不支持视频/音频参考；不会忽略这些素材",
                ));
            }
            rows.into_iter()
                .map(|r| (None, r.images, r.prompt))
                .collect()
        };
        let mut generators = BTreeMap::new();
        let mut defaults = Parameters {
            mode: Some(mode.clone()),
            ..Default::default()
        };
        if let Some(selected) = &options.legacy {
            let pair = super::product::selection_ref::ExactGeneratorSelection::decode(
                &selected.selection_ref,
            )
            .map_err(|_| {
                failure(
                    "GENERATOR_UNAVAILABLE",
                    "请选择正式生成器，不能由名称猜测身份",
                )
            })?;
            generators.insert(
                mode.clone(),
                Generator {
                    workflow_version_id: pair.workflow_version_id,
                    recipe_id: pair.recipe_id,
                },
            );
            defaults.duration_seconds = Some(selected.duration_seconds);
            defaults.width = Some(selected.width);
            defaults.height = Some(selected.height);
        }
        let manifest = Manifest {
            format: "ai-studio-shot-batch".into(),
            version: 1,
            name: "旧版 H3 分镜导入".into(),
            generators,
            defaults,
            characters: vec![],
            queue: QueueOptions::default(),
            shots: rows
                .into_iter()
                .enumerate()
                .map(|(i, (first, references, prompt))| ManifestShot {
                    id: None,
                    name: Some(format!("镜头 {:03}", i + 1)),
                    mode: Some(mode.clone()),
                    prompt,
                    style: None,
                    summary: None,
                    soundscape: None,
                    music: None,
                    first_frame: first,
                    last_frame: None,
                    references,
                    characters: vec![],
                    opening_composition: None,
                    ending_composition: None,
                    duration_seconds: None,
                    aspect_ratio: None,
                    resolution_preset: None,
                    width: None,
                    height: None,
                    seed: None,
                })
                .collect(),
        };
        parse_manifest(
            &serde_json::to_vec(&manifest)
                .map_err(|_| failure("INVALID_MANIFEST", "旧版清单转换失败"))?,
        )
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
        state: SessionState,
    }
    pub struct FolderBatchImportService {
        sessions: Mutex<HashMap<String, Session>>,
        commit_services: Option<CommitServices>,
        project_locks: Arc<std::sync::Mutex<BTreeSet<String>>>,
        #[cfg(test)]
        faults: std::sync::Mutex<BTreeSet<&'static str>>,
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
                commit_services: None,
                project_locks: Arc::new(std::sync::Mutex::new(BTreeSet::new())),
                #[cfg(test)]
                faults: std::sync::Mutex::new(BTreeSet::new()),
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
                matches!(s.state, SessionState::Writing)
                    || self
                        .clock
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
                    state: SessionState::Previewed,
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
            if !matches!(session.state, SessionState::Previewed) {
                return Err(failure(
                    "SESSION_CONSUMED",
                    "写入中的或已结束的会话不能重新校验",
                ));
            }
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
            if !matches!(live.state, SessionState::Previewed)
                || live.plan.view.preview_version != session.plan.view.preview_version
            {
                return Err(failure("STALE_PREVIEW", "预览已被更新，请重新校验"));
            }
            live.plan = plan;
            live.created_at = self.clock.now();
            Ok(live.plan.view.clone())
        }
        pub async fn discard(&self, id: &str) -> Result<(), ImportFailure> {
            let mut sessions = self.sessions.lock().await;
            if sessions
                .get(id)
                .is_some_and(|s| matches!(s.state, SessionState::Writing))
            {
                return Err(failure("IMPORT_IN_PROGRESS", "写入中不能丢弃会话"));
            }
            sessions.remove(id);
            Ok(())
        }
        async fn session(&self, id: &str) -> Result<Session, ImportFailure> {
            let sessions = self.sessions.lock().await;
            let s = sessions
                .get(id)
                .ok_or_else(|| failure("SESSION_CONSUMED", "预览已关闭或过期，请重新选择文件夹"))?;
            if !matches!(s.state, SessionState::Writing)
                && self
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
        /// One image per reused entity per shot. A reused entity's declared
        /// package images listed in the row's `references` collapse to one:
        /// - same bytes as the project cover: the package path is dropped and the
        ///   existing asset is used (returns `false`, the caller adds it);
        /// - different bytes: the first declared package path is kept; commit
        ///   imports it and makes it the entity cover (old images are kept), so
        ///   the shot binds that new cover once (returns `true`).
        async fn dedupe_existing_entity_reference(
            &self,
            root: &Path,
            manifest: &Manifest,
            existing: &[StoryEntity],
            entity: &StoryEntity,
            row: &mut Row,
        ) -> Result<bool, ImportFailure> {
            if !existing.iter().any(|e| e.id == entity.id) {
                return Ok(false);
            }
            let declared = declared_paths(manifest, &entity.name);
            let listed: Vec<String> = declared
                .iter()
                .filter(|p| row.references.contains(p))
                .cloned()
                .collect();
            let Some(first) = listed.first().cloned() else {
                return Ok(false);
            };
            let managed = entity
                .cover_asset_id
                .as_ref()
                .or_else(|| entity.image_asset_ids.first())
                .and_then(|id| crate::domain::AssetId::parse(id).ok());
            let managed_sha = match managed {
                Some(id) => self
                    .assets
                    .find_by_id(&id)
                    .await
                    .map_err(|_| failure("IMAGE_INVALID", "无法检查实体素材"))?
                    .map(|asset| asset.sha256),
                None => None,
            };
            let package_sha = read_file(
                root,
                &first,
                super::source_asset_import_service::MAX_SOURCE_IMAGE_BYTES,
            )
            .ok()
            .map(|bytes| sha(&bytes));
            if managed_sha.is_some() && package_sha == managed_sha {
                row.references.retain(|p| !listed.contains(p));
                return Ok(false);
            }
            row.references
                .retain(|p| p == &first || !listed.contains(p));
            row.warnings
                .push(crate::application::shot_bulk_service::BulkIssue {
                    code: "ENTITY_COVER_FROM_PACKAGE".into(),
                    message: format!(
                        "「{}」将使用导入包中的新图片作为封面（原图片保留在资料库中）",
                        entity.name
                    ),
                    row_number: Some(row.row_number),
                    shot_id: None,
                });
            Ok(true)
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
            let has = |name: &str| {
                root.join(name)
                    .try_exists()
                    .map_err(|_| failure("FILE_NOT_FOUND", "无法检查清单文件"))
            };
            let modern = has("manifest.json")?;
            let (filename, format_source) = if modern {
                ("manifest.json", "manifest")
            } else {
                match (has("h3-batch.json")?, has("h3-omni-batch.json")?) {
                    (true, false) => ("h3-batch.json", "h3-batch"),
                    (false, true) => ("h3-omni-batch.json", "h3-omni-batch"),
                    (true, true) => {
                        return Err(failure(
                            "INVALID_MANIFEST",
                            "同时存在两种旧清单，请保留一种或提供 manifest.json",
                        ))
                    }
                    _ => {
                        return Err(failure(
                            "FILE_NOT_FOUND",
                            "文件夹缺少 manifest.json 或受支持的旧版 H3 清单",
                        ))
                    }
                }
            };
            let bytes = read_file(root, filename, 1024 * 1024)?;
            let mut manifest = if modern {
                if options.legacy.is_some() {
                    return Err(failure(
                        "INVALID_MANIFEST",
                        "现代 manifest 的生成器与参数不可由旧版选项覆盖",
                    ));
                }
                parse_manifest(&bytes)?
            } else {
                legacy_manifest(&bytes, format_source, options)?
            };
            if self
                .projects
                .find_by_id(project)
                .await
                .map_err(|_| failure("PROJECT_UNAVAILABLE", "无法读取项目"))?
                .is_none()
            {
                return Err(failure("PROJECT_UNAVAILABLE", "项目不存在"));
            }
            // A package written for an older version of a built-in workflow
            // follows that workflow's current version (same rule as the
            // registration-time selection migration); anything else is
            // validated exactly as written below.
            let mut generator_warnings: Vec<String> = Vec::new();
            for generator in manifest.generators.values_mut() {
                if let Some(current) = self
                    .definitions
                    .resolve_superseded_pair(&generator.workflow_version_id, &generator.recipe_id)
                    .await
                    .map_err(|_| failure("GENERATOR_UNAVAILABLE", "无法读取生成器"))?
                {
                    generator.workflow_version_id = current.workflow_version_id;
                    generator.recipe_id = current.recipe_id;
                    let warning = format!(
                        "导入包指定的工作流「{}」版本已更新，已改用最新版本 {}",
                        current.workflow_name, current.current_version
                    );
                    if !generator_warnings.contains(&warning) {
                        generator_warnings.push(warning);
                    }
                }
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
                let declared_kind = entity.kind.as_deref().unwrap_or("character");
                if existing
                    .iter()
                    .any(|e| e.name == entity.name && e.kind.as_str() != declared_kind)
                {
                    return Err(failure(
                        "INVALID_MANIFEST",
                        format!("实体 {} 与项目中已有的同名实体类型不同", entity.name),
                    ));
                }
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
                let listed_before = row.references.clone();
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
                            // One image per entity per shot: a reused entity whose
                            // declared package image is also listed in `references`
                            // is the same reference, not a second one.
                            // The package image replaces the cover at commit and is
                            // already counted once through `row.references`.
                            if self
                                .dedupe_existing_entity_reference(
                                    root, &manifest, &existing, entity, row,
                                )
                                .await?
                            {
                                continue;
                            }
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
                // Paths dropped as duplicates of a project image are not counted.
                reference_paths
                    .retain(|p| !listed_before.contains(p) || row.references.contains(p));
                for (label, name) in [
                    ("开场构图", row.opening_composition.clone()),
                    ("结尾构图", row.ending_composition.clone()),
                ] {
                    let Some(name) = name else { continue };
                    let key = name.trim().to_lowercase();
                    let named: Vec<_> = entities
                        .iter()
                        .filter(|e| e.name.trim().to_lowercase() == key)
                        .collect();
                    let composition = named.iter().find(|e| {
                        e.kind == crate::domain::story_entity::StoryEntityKind::Composition
                    });
                    match composition {
                        None if named.is_empty() => row_error(
                            row,
                            failure(
                                "COMPOSITION_USAGE_UNKNOWN",
                                format!("{label}「{name}」不存在，请在 characters 中声明 kind 为 composition 的实体，或使用项目中已有的构图"),
                            ),
                        ),
                        None => row_error(
                            row,
                            failure(
                                "COMPOSITION_USAGE_WRONG_KIND",
                                format!("{label}「{name}」不是构图类型，只能选择构图"),
                            ),
                        ),
                        Some(entity) => {
                            if !row.entity_names.contains(&entity.name) {
                                row_error(
                                    row,
                                    failure(
                                        "COMPOSITION_USAGE_UNBOUND",
                                        format!("{label}「{name}」需要在提示词中以 @{} 引用", entity.name),
                                    ),
                                );
                            } else if entity.name != name {
                                // Store the canonical entity name for commit lookup.
                                if label == "开场构图" {
                                    row.opening_composition = Some(entity.name.clone());
                                } else {
                                    row.ending_composition = Some(entity.name.clone());
                                }
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
            warnings.extend(generator_warnings);
            // Non-blocking: a composition picture from the package that looks
            // like a rendered scene may fight the scene picture's background.
            for entity in entities
                .iter()
                .filter(|e| e.kind == crate::domain::story_entity::StoryEntityKind::Composition)
            {
                let Some(path) = declared_paths(&manifest, &entity.name).into_iter().next() else {
                    continue;
                };
                if read_file(
                    root,
                    &path,
                    super::source_asset_import_service::MAX_SOURCE_IMAGE_BYTES,
                )
                .is_ok_and(|bytes| {
                    super::shot_video_input_service::composition_looks_detailed(&bytes)
                }) {
                    warnings.push(super::shot_video_input_service::composition_detail_hint(
                        &entity.name,
                    ));
                }
            }
            for declared in &manifest.characters {
                if let Some(old) = existing.iter().find(|e| e.name == declared.name) {
                    warnings.push(format!(
                        "复用实体 {}：同一镜头内该实体只计一张参考图；图片与项目中相同时直接复用已有素材。",
                        declared.name
                    ));
                    let given = declared.description.as_deref().unwrap_or("").trim();
                    if fill_description(&old.description, declared.description.as_deref()).is_some()
                    {
                        warnings.push(format!(
                            "「{}」已有描述为空或不是英文，导入时将填入清单中的英文外观描述。",
                            declared.name
                        ));
                    } else if !given.is_empty() && given != old.description.trim() {
                        warnings.push(format!(
                            "「{}」已有英文描述，保留原描述，未使用清单中的描述。",
                            declared.name
                        ));
                    }
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
            if !modern {
                warnings.push("使用旧版 H3 清单，建议迁移到 manifest.json。".into());
            }
            if manifest.queue.auto_start == Some(true) {
                warnings
                    .push("清单申请了自动开始，但仍需在确认区域明确勾选；默认不执行生产。".into());
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
                format_source: format_source.into(),
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
    /// English manifest appearance text for a reused entity whose description
    /// is empty or not English. English text is never overwritten.
    fn fill_description(current: &str, given: Option<&str>) -> Option<String> {
        use super::shot_video_input_service::mostly_english;
        let given = given?.trim();
        let current = current.trim();
        (!given.is_empty()
            && mostly_english(given)
            && given != current
            && (current.is_empty() || !mostly_english(current)))
        .then(|| given.to_string())
    }
    /// Cover first, then other declared images of a manifest entity.
    fn declared_paths(manifest: &Manifest, name: &str) -> Vec<String> {
        manifest
            .characters
            .iter()
            .find(|e| e.name == name)
            .map(|e| {
                let mut paths: Vec<String> = e.cover.iter().cloned().collect();
                for image in &e.images {
                    if !paths.contains(image) {
                        paths.push(image.clone());
                    }
                }
                paths
            })
            .unwrap_or_default()
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
            (Mode::Reference, ShotVideoInputMode::ReferenceImages { key, .. }) => {
                key == "reference_images"
            }
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
            if let crate::compiler::CompileError::InputOutOfRange { input, value, .. } = &e {
                if input == "duration_seconds"
                    && *value > super::product::h3_resolution::MAX_SHOT_DURATION_SECONDS
                {
                    return failure(
                        "DURATION_TOO_LONG",
                        super::product::h3_resolution::SHOT_DURATION_LIMIT_MESSAGE,
                    );
                }
            }
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

    #[derive(Clone)]
    enum SessionState {
        Previewed,
        Writing,
        Terminal(Box<ImportResult>),
    }
    #[derive(Clone, Debug, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct CommitRequest {
        pub session_id: String,
        pub preview_version: u64,
        pub manifest_sha256: String,
        pub include_row_keys: Vec<String>,
        pub create_queue: bool,
        pub auto_start: bool,
        pub name_suffix: Option<String>,
    }
    #[derive(Clone, Debug, Default, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Residual {
        pub unverified_file_cleanup: bool,
        pub shot_ids: Vec<String>,
        pub entity_ids: Vec<String>,
        pub asset_ids: Vec<String>,
        pub batch_id: Option<String>,
    }
    #[derive(Clone, Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct CompensationError {
        pub target: String,
        pub id: String,
        pub message: String,
    }
    #[derive(Clone, Debug, Default, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ImportResult {
        pub outcome: String,
        pub failure: Option<ImportFailure>,
        pub compensation_errors: Vec<CompensationError>,
        pub created_shot_ids: Vec<String>,
        pub reused_entity_ids: Vec<String>,
        pub created_entity_ids: Vec<String>,
        pub imported_asset_ids: Vec<String>,
        pub batch_id: Option<String>,
        pub item_count: usize,
        pub auto_started: bool,
        pub residual: Residual,
        pub warnings: Vec<String>,
        /// Reused entities as they were before this import changed their cover
        /// or description; compensation writes them back.
        #[serde(skip)]
        pub restored_entities: Vec<StoryEntity>,
    }
    /// Test seam for explicit Start only; the product adapter delegates to the existing admission/Queue authority.
    #[async_trait::async_trait]
    pub trait ImportStart: Send + Sync {
        async fn start(&self, project: &str, batch: &str) -> Result<(), ImportFailure>;
    }
    #[async_trait::async_trait]
    impl ImportStart for super::production_start_admission_service::ProductionStartAdmissionService {
        async fn start(&self, project: &str, batch: &str) -> Result<(), ImportFailure> {
            super::production_start_admission_service::ProductionStartAdmissionService::start(
                self, project, batch,
            )
            .await
            .map_err(|_| {
                failure(
                    "QUEUE_START_FAILED",
                    "正式启动失败；批次已保留，请到生产队列检查后显式启动",
                )
            })
        }
    }
    pub struct CommitServices {
        pub bulk: Arc<super::shot_bulk_service::ShotBulkService>,
        pub shots: Arc<super::shot_service::ShotService>,
        pub source: Arc<super::source_asset_import_service::SourceAssetImportService>,
        pub inputs: Arc<super::shot_video_input_service::ShotVideoInputService>,
        pub batch: Arc<super::shot_batch_service::ShotBatchService>,
        pub queue: Arc<super::production_queue_service::ProductionQueueService>,
        pub deletion: Arc<super::asset_deletion_service::AssetDeletionService>,
        pub start: Arc<dyn ImportStart>,
    }
    struct ProjectImportGuard {
        project: String,
        locks: Arc<std::sync::Mutex<BTreeSet<String>>>,
    }
    impl ProjectImportGuard {
        fn acquire(
            locks: Arc<std::sync::Mutex<BTreeSet<String>>>,
            project: &str,
        ) -> Result<Self, ImportFailure> {
            if !locks
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(project.into())
            {
                return Err(failure(
                    "IMPORT_IN_PROGRESS",
                    "当前项目正在提交另一份导入，请稍后查看结果",
                ));
            }
            Ok(Self {
                project: project.into(),
                locks,
            })
        }
    }
    impl Drop for ProjectImportGuard {
        fn drop(&mut self) {
            self.locks
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&self.project);
        }
    }
    /// User-facing Chinese reason for a ShotBatchServiceError: no bare codes or IDs.
    pub(crate) fn batch_error_reason(
        error: &super::shot_batch_service::ShotBatchServiceError,
    ) -> String {
        use super::shot_batch_service::ShotBatchServiceError as E;
        match error {
            E::InvalidInput(message) => {
                if message.starts_with(crate::application::minimax_video_product_policy::REQUIRED) {
                    "项目绑定的生成器版本不可用于新生成，请在项目生成器设置中重新选择".into()
                } else {
                    chinese_reason(message)
                }
            }
            E::NotFound(_) => "项目不存在或已被删除".into(),
            E::Repository(_) => "数据库读写失败，请稍后重试".into(),
        }
    }
    /// Keeps Chinese explanations, strips internal IDs; English-only text becomes a generic hint.
    pub(crate) fn chinese_reason(message: &str) -> String {
        let has_cjk = message
            .chars()
            .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
        if !has_cjk {
            return "正式输入校验未通过（参考素材或生成参数不符合生成器要求）".into();
        }
        let mut cleaned = message.to_owned();
        for prefix in [
            "ast_", "sht_", "prj_", "wfv_", "rcp_", "tsk_", "pbi_", "pbt_",
        ] {
            while let Some(start) = cleaned.find(prefix) {
                let end = cleaned[start..]
                    .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
                    .map_or(cleaned.len(), |n| start + n);
                cleaned.replace_range(start..end, "");
            }
        }
        let cleaned = cleaned
            .trim_start_matches("INVALID_INPUT:")
            .trim()
            .trim_end_matches(['：', ':', '/'])
            .trim();
        if cleaned.is_empty() {
            "正式输入校验未通过".into()
        } else {
            cleaned.to_owned()
        }
    }
    fn stage_failure(stage: &str, code: &str, message: &str) -> ImportFailure {
        ImportFailure {
            stage: stage.into(),
            code: code.into(),
            message: message.into(),
        }
    }
    fn source_failure(
        result: &mut ImportResult,
        error: super::source_asset_import_service::SourceAssetImportError,
    ) -> ImportFailure {
        if matches!(
            error,
            super::source_asset_import_service::SourceAssetImportError::AssetPersistence { .. }
        ) {
            // The existing importer owns file rollback but cannot attest every deletion.
            result.residual.unverified_file_cleanup = true;
            result.compensation_errors.push(CompensationError {
                target: "source-files".into(),
                id: "unconfirmed".into(),
                message:
                    "素材存储发生错误；底层文件清理未能确认，请检查导入日志，不要直接重复导入。"
                        .into(),
            });
        }
        stage_failure(
            "A",
            "SOURCE_IMPORT_FAILED",
            "素材导入失败，请检查项目存储和素材格式",
        )
    }
    fn selected_files(plan: &Plan, rows: &[Row]) -> Result<BTreeSet<String>, ImportFailure> {
        let mut files: BTreeSet<_> = rows
            .iter()
            .flat_map(|r| {
                r.first_frame
                    .iter()
                    .chain(r.last_frame.iter())
                    .chain(r.references.iter())
            })
            .cloned()
            .collect();
        for name in rows.iter().flat_map(|r| r.entity_names.iter()) {
            if plan
                .view
                .characters
                .iter()
                .any(|e| e.name == *name && e.action == "create")
            {
                let entity = plan
                    .manifest
                    .characters
                    .iter()
                    .find(|e| e.name == *name)
                    .ok_or_else(|| failure("STALE_PREVIEW", "实体计划不完整"))?;
                files.extend(entity.images.iter().cloned());
            }
        }
        if files.len() > 500 {
            return Err(failure(
                "REFERENCE_LIMIT",
                "本次选中镜头去重后的引用文件超过 500 个，请减少选择",
            ));
        }
        Ok(files)
    }
    impl FolderBatchImportService {
        pub fn with_commit_services(mut self, services: CommitServices) -> Self {
            self.commit_services = Some(services);
            self
        }
        fn injection(&self, stage: &str) -> Result<(), ImportFailure> {
            #[cfg(test)]
            {
                let faults = self.faults.lock().unwrap_or_else(|e| e.into_inner());
                if stage == "B" && faults.contains("B_PANIC") {
                    panic!("owned import panic fixture");
                }
                if faults.contains(stage) {
                    return Err(stage_failure(
                        stage,
                        "INJECTED_FAILURE",
                        "隔离测试注入的失败",
                    ));
                }
            }
            #[cfg(not(test))]
            let _ = stage;
            Ok(())
        }
        pub async fn commit(&self, request: CommitRequest) -> Result<ImportResult, ImportFailure> {
            let initial = self.session(&request.session_id).await?;
            let _guard =
                ProjectImportGuard::acquire(self.project_locks.clone(), &initial.project_id)?;
            let session = self.session(&request.session_id).await?;
            match &session.state {
                SessionState::Terminal(result) => {
                    let mut result = (**result).clone();
                    result
                        .warnings
                        .push("这次导入已经完成，以下是上次的结果，没有重复写入。".into());
                    return Ok(result);
                }
                SessionState::Writing => {
                    return Err(failure(
                        "SESSION_CONSUMED",
                        "上次写入未正常结案；不会再次执行，请检查残留后重新选择文件夹",
                    ))
                }
                SessionState::Previewed => {}
            }
            let preview = &session.plan.view;
            if request.preview_version != preview.preview_version
                || request.manifest_sha256 != preview.manifest_sha256
                || request.name_suffix != preview.name_suffix
            {
                return Err(failure(
                    "STALE_PREVIEW",
                    "名称后缀或预览版本已变化，请重新校验",
                ));
            }
            let keys: BTreeSet<_> = request.include_row_keys.iter().cloned().collect();
            if keys.is_empty()
                || keys.len() != request.include_row_keys.len()
                || keys
                    .iter()
                    .any(|key| !preview.rows.iter().any(|r| r.row_key == *key))
            {
                return Err(failure("INVALID_MANIFEST", "必须选择有效且不重复的会话行"));
            }
            let rows: Vec<_> = preview
                .rows
                .iter()
                .filter(|r| keys.contains(&r.row_key))
                .cloned()
                .collect();
            if rows.iter().any(|r| !r.errors.is_empty()) {
                return Err(failure("INVALID_MANIFEST", "错误行不可提交"));
            }
            if request.auto_start && !request.create_queue {
                return Err(failure(
                    "INVALID_MANIFEST",
                    "自动开始需要先创建正式生产批次",
                ));
            }
            if request.create_queue && rows.len() > super::shot_batch_service::MAX_SHOT_BATCH_ITEMS
            {
                return Err(failure(
                    "REFERENCE_LIMIT",
                    format!(
                        "正式单批上限为 {} 项；请选择更少镜头，或不创建队列",
                        super::shot_batch_service::MAX_SHOT_BATCH_ITEMS
                    ),
                ));
            }
            let files = selected_files(&session.plan, &rows)?;
            let filename = match preview.format_source.as_str() {
                "h3-batch" => "h3-batch.json",
                "h3-omni-batch" => "h3-omni-batch.json",
                _ => "manifest.json",
            };
            if sha(&read_file(&session.root, filename, 1024 * 1024)
                .map_err(|_| failure("STALE_PREVIEW", "清单已丢失或无法安全读取"))?)
                != preview.manifest_sha256
            {
                return Err(failure("STALE_PREVIEW", "清单已变化，请重新校验"));
            }
            for file in &files {
                let bytes = read_file(
                    &session.root,
                    file,
                    super::source_asset_import_service::MAX_SOURCE_IMAGE_BYTES,
                )
                .map_err(|_| failure("STALE_PREVIEW", "引用文件已丢失或无法安全读取"))?;
                if session.plan.file_sha256.get(file) != Some(&sha(&bytes)) {
                    return Err(failure("STALE_PREVIEW", "引用文件内容已变化，请重新校验"));
                }
            }
            let fresh = self
                .build(
                    &session.project_id,
                    &session.root,
                    &request.session_id,
                    preview.preview_version,
                    &session.plan.options,
                )
                .await?;
            for row in &rows {
                let next = fresh
                    .view
                    .rows
                    .iter()
                    .find(|r| r.row_key == row.row_key)
                    .ok_or_else(|| failure("STALE_PREVIEW", "镜头计划已变化"))?;
                if let Some(issue) = next.errors.first() {
                    return Err(failure(&issue.code, issue.message.clone()));
                }
                if serde_json::to_value(row).ok() != serde_json::to_value(next).ok() {
                    return Err(failure("STALE_PREVIEW", "镜头或实体计划已变化，请重新校验"));
                }
                let key = serde_json::to_string(
                    row.generator
                        .as_ref()
                        .ok_or_else(|| failure("GENERATOR_UNAVAILABLE", "生成器不存在"))?,
                )
                .unwrap();
                if session.plan.fingerprints.get(&key) != fresh.fingerprints.get(&key) {
                    return Err(failure("STALE_PREVIEW", "正式生成器版本内容已变化"));
                }
                for name in &row.entity_names {
                    let old = session.plan.entities.iter().find(|e| e.name == *name);
                    let new = fresh.entities.iter().find(|e| e.name == *name);
                    if serde_json::to_value(old).ok() != serde_json::to_value(new).ok() {
                        return Err(failure("STALE_PREVIEW", "实体或参考图已被更新，请重新校验"));
                    }
                }
            }
            let services = self
                .commit_services
                .as_ref()
                .ok_or_else(|| failure("IMPORT_UNAVAILABLE", "导入写入服务未配置"))?;
            {
                let mut sessions = self.sessions.lock().await;
                let live = sessions
                    .get_mut(&request.session_id)
                    .ok_or_else(|| failure("SESSION_CONSUMED", "会话已关闭"))?;
                if !matches!(live.state, SessionState::Previewed)
                    || live.plan.view.preview_version != request.preview_version
                {
                    return Err(failure("STALE_PREVIEW", "预览已更新"));
                }
                live.state = SessionState::Writing;
            }
            let mut result = ImportResult {
                outcome: "completed".into(),
                ..Default::default()
            };
            if let Err(error) = self
                .write(
                    services,
                    &session,
                    &fresh,
                    &rows,
                    &files,
                    &request,
                    &mut result,
                )
                .await
            {
                result.failure = Some(error);
                self.compensate(services, &session.project_id, &mut result)
                    .await;
            }
            if let Some(live) = self.sessions.lock().await.get_mut(&request.session_id) {
                live.state = SessionState::Terminal(Box::new(result.clone()));
            }
            Ok(result)
        }
        async fn write(
            &self,
            services: &CommitServices,
            session: &Session,
            plan: &Plan,
            rows: &[Row],
            files: &BTreeSet<String>,
            request: &CommitRequest,
            result: &mut ImportResult,
        ) -> Result<(), ImportFailure> {
            use crate::application::generation_input_preparer::GenerationInputValue;
            use crate::application::ports::{
                ShotRowEditRecord, ShotStageConfigRecord, ShotStagePromptRecord,
                ShotVideoInputAsset, ShotVideoInputScope,
            };
            use crate::domain::{SeedValue, ShotStage};
            let project = &session.project_id;
            let mut assets = BTreeMap::new();
            self.injection("A")?;
            for file in files {
                let bytes = read_file(
                    &session.root,
                    file,
                    super::source_asset_import_service::MAX_SOURCE_IMAGE_BYTES,
                )
                .map_err(|_| stage_failure("A", "STALE_PREVIEW", "素材已无法安全读取"))?;
                if plan.file_sha256.get(file) != Some(&sha(&bytes)) {
                    return Err(stage_failure(
                        "A",
                        "STALE_PREVIEW",
                        "素材在导入期间发生变化",
                    ));
                }
                let name = Path::new(file)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .ok_or_else(|| stage_failure("A", "IMAGE_INVALID", "素材文件名无效"))?;
                let asset = services
                    .source
                    .import_bytes(project, name, &bytes)
                    .await
                    .map_err(|error| source_failure(result, error))?;
                let id = asset.id.to_string();
                assets.insert(file.clone(), id.clone());
                result.imported_asset_ids.push(id);
            }
            self.injection("B")?;
            let mut entities = BTreeMap::new();
            for name in rows.iter().flat_map(|r| r.entity_names.iter()) {
                if entities.contains_key(name) {
                    continue;
                }
                let entity = plan
                    .entities
                    .iter()
                    .find(|e| e.name == *name)
                    .ok_or_else(|| stage_failure("B", "ENTITY_NOT_FOUND", "实体不可用"))?;
                let entity = if plan
                    .view
                    .characters
                    .iter()
                    .any(|e| e.name == *name && e.action == "create")
                {
                    let declaration = plan
                        .manifest
                        .characters
                        .iter()
                        .find(|e| e.name == *name)
                        .ok_or_else(|| stage_failure("B", "ENTITY_NOT_FOUND", "实体声明不可用"))?;
                    let ids = declaration
                        .images
                        .iter()
                        .map(|p| {
                            assets.get(p).cloned().ok_or_else(|| {
                                stage_failure("B", "IMAGE_INVALID", "实体图片尚未导入")
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let cover = declaration
                        .cover
                        .as_ref()
                        .and_then(|p| assets.get(p))
                        .cloned()
                        .or_else(|| ids.first().cloned());
                    let created = self
                        .entities
                        .create(
                            project,
                            entity.kind,
                            crate::domain::story_entity::StoryEntityUpsert {
                                name: name.clone(),
                                description: entity.description.clone(),
                                cover_asset_id: cover,
                                image_asset_ids: ids,
                            },
                        )
                        .await
                        .map_err(|_| stage_failure("B", "ENTITY_CREATE_FAILED", "实体创建失败"))?;
                    result.created_entity_ids.push(created.id.clone());
                    created
                } else {
                    if !result.reused_entity_ids.contains(&entity.id) {
                        result.reused_entity_ids.push(entity.id.clone());
                    }
                    let given = plan
                        .manifest
                        .characters
                        .iter()
                        .find(|e| e.name == *name)
                        .and_then(|e| e.description.as_deref());
                    // Only an empty or non-English description is filled.
                    let description = fill_description(&entity.description, given);
                    // A package image that preview kept (different bytes) was
                    // imported in stage A; it becomes the cover, old images stay.
                    let new_cover = declared_paths(&plan.manifest, name)
                        .iter()
                        .find_map(|p| assets.get(p).cloned())
                        .filter(|id| entity.cover_asset_id.as_ref() != Some(id));
                    if description.is_none() && new_cover.is_none() {
                        entity.clone()
                    } else {
                        let mut images = entity.image_asset_ids.clone();
                        if let Some(id) = &new_cover {
                            if !images.contains(id) {
                                images.push(id.clone());
                            }
                        }
                        result.restored_entities.push(entity.clone());
                        self.entities
                            .update(
                                project,
                                &entity.id,
                                crate::domain::story_entity::StoryEntityUpsert {
                                    name: entity.name.clone(),
                                    description: description
                                        .unwrap_or_else(|| entity.description.clone()),
                                    cover_asset_id: new_cover
                                        .or_else(|| entity.cover_asset_id.clone()),
                                    image_asset_ids: images,
                                },
                            )
                            .await
                            .map_err(|_| {
                                stage_failure("B", "ENTITY_UPDATE_FAILED", "实体更新失败")
                            })?
                    }
                };
                entities.insert(name.clone(), entity);
            }
            self.injection("C1")?;
            let content = serde_json::json!({"schemaVersion":1,"shots":rows.iter().map(|r|serde_json::json!({"name":r.name,"description":format!("导入自 {}",plan.view.display_root_name),"videoPrompt":r.prompt})).collect::<Vec<_>>()});
            let created = services
                .bulk
                .commit_import(&super::shot_bulk_service::ShotBulkImportRequest {
                    project_id: project.clone(),
                    format: super::shot_bulk_service::ShotBulkInputFormat::Json,
                    contents: content.to_string(),
                })
                .await
                .map_err(|_| {
                    stage_failure(
                        "C1",
                        "SHOT_CREATE_FAILED",
                        "镜头创建失败；可能发生同名或并发编辑冲突",
                    )
                })?;
            result.created_shot_ids = created.created.into_iter().map(|r| r.shot_id).collect();
            if result.created_shot_ids.len() != rows.len() {
                return Err(stage_failure(
                    "C1",
                    "SHOT_CREATE_FAILED",
                    "镜头创建数量不一致，拒绝继续",
                ));
            }
            self.injection("C2")?;
            let mut configs = Vec::new();
            let mut prompts = Vec::new();
            for (row, id) in rows.iter().zip(&result.created_shot_ids) {
                let pair = row
                    .generator
                    .as_ref()
                    .ok_or_else(|| stage_failure("C2", "GENERATOR_UNAVAILABLE", "生成器不可用"))?;
                let definition = self
                    .definitions
                    .find_active(&pair.workflow_version_id, &pair.recipe_id)
                    .await
                    .map_err(|_| stage_failure("C2", "GENERATOR_UNAVAILABLE", "生成器无法读取"))?
                    .ok_or_else(|| stage_failure("C2", "GENERATOR_UNAVAILABLE", "生成器不可用"))?;
                let mut values = BTreeMap::from([
                    ("width".into(), GenerationInputValue::Integer(row.width)),
                    ("height".into(), GenerationInputValue::Integer(row.height)),
                    (
                        "duration_seconds".into(),
                        GenerationInputValue::Integer(row.duration_seconds),
                    ),
                ]);
                if let Some(seed) = row.seed {
                    values.insert(
                        "seed".into(),
                        GenerationInputValue::Seed(SeedValue::Fixed(seed)),
                    );
                }
                let scalar_values = super::shot_service::validate_stage_config_values(
                    ShotStage::Video,
                    &definition,
                    &values,
                )
                .map_err(|_| {
                    stage_failure("C2", "INPUT_VALIDATION_FAILED", "阶段参数已不再有效")
                })?;
                configs.push(ShotStageConfigRecord {
                    shot_id: id.clone(),
                    stage: ShotStage::Video,
                    workflow_version_id: pair.workflow_version_id.clone(),
                    recipe_id: pair.recipe_id.clone(),
                    scalar_values,
                    updated_at: self.clock.now(),
                });
                prompts.push(ShotStagePromptRecord {
                    shot_id: id.clone(),
                    stage: ShotStage::Video,
                    prompt_text: super::prompt_library_service::canonical_prompt_text(&row.prompt)
                        .map_err(|_| stage_failure("C3", "INVALID_PROMPT", "提示词不合法"))?,
                    prompt_entry_id: None,
                    prompt_version_id: None,
                    updated_at: self.clock.now(),
                });
            }
            self.shots
                .upsert_stage_configs_atomic(project, &configs, &[])
                .await
                .map_err(|_| stage_failure("C2", "CONFIG_SAVE_FAILED", "阶段配置保存失败"))?;
            self.injection("C3")?;
            self.shots
                .update_stage_prompts_atomic(project, &prompts)
                .await
                .map_err(|_| stage_failure("C3", "PROMPT_SAVE_FAILED", "提示词保存失败"))?;
            self.injection("C4")?;
            for (row, id) in rows.iter().zip(&result.created_shot_ids) {
                let ids = row
                    .entity_names
                    .iter()
                    .filter_map(|n| entities.get(n).map(|e| e.id.clone()))
                    .collect();
                let usage_id = |name: &Option<String>| {
                    name.as_ref()
                        .and_then(|n| entities.get(n))
                        .map(|e| e.id.clone())
                };
                let composition_usage = (row.opening_composition.is_some()
                    || row.ending_composition.is_some())
                .then(|| crate::domain::story_entity::ShotCompositionUsage {
                    opening_entity_id: usage_id(&row.opening_composition),
                    ending_entity_id: usage_id(&row.ending_composition),
                });
                services
                    .shots
                    .save_row_atomic(ShotRowEditRecord {
                        project_id: project.clone(),
                        shot_id: id.clone(),
                        stage: ShotStage::Video,
                        name: None,
                        prompt_text: None,
                        config: None,
                        entity_ids: Some(ids),
                        composition_usage,
                        updated_at: self.clock.now(),
                    })
                    .await
                    .map_err(|_| stage_failure("C4", "ENTITY_BIND_FAILED", "镜头实体绑定失败"))?;
                if !row.prompt_sections.is_empty() {
                    self.entities
                        .set_prompt_sections(project, id, row.prompt_sections.clone())
                        .await
                        .map_err(|_| {
                            stage_failure(
                                "C4",
                                "PROMPT_SECTIONS_SAVE_FAILED",
                                "镜头提示词分段保存失败",
                            )
                        })?;
                }
            }
            self.injection("D")?;
            for (row, id) in rows.iter().zip(&result.created_shot_ids) {
                let mut inputs = Vec::new();
                for (key, path) in [
                    ("first_frame", row.first_frame.as_ref()),
                    ("last_frame", row.last_frame.as_ref()),
                ] {
                    if let Some(path) = path {
                        inputs.push(ShotVideoInputAsset {
                            input_key: key.into(),
                            ordinal: 0,
                            asset_id: assets.get(path).cloned().ok_or_else(|| {
                                stage_failure("D", "IMAGE_INVALID", "图片未成功导入")
                            })?,
                        });
                    }
                }
                if row.mode == Mode::Reference {
                    let mut refs: Vec<_> = row
                        .references
                        .iter()
                        .filter_map(|p| assets.get(p).cloned())
                        .collect();
                    let bound: Vec<_> = entities.values().cloned().collect();
                    for entity in super::shot_video_input_service::explicit_entity_mentions(
                        &row.prompt,
                        project,
                        &bound,
                    )
                    .map_err(|_| stage_failure("D", "ENTITY_REFERENCE_UNKNOWN", "实体引用不可用"))?
                    {
                        let cover = entity
                            .cover_asset_id
                            .as_ref()
                            .or_else(|| entity.image_asset_ids.first())
                            .ok_or_else(|| stage_failure("D", "IMAGE_INVALID", "实体图片不可用"))?;
                        if !refs.contains(cover) {
                            refs.push(cover.clone());
                        }
                    }
                    let mut unique = Vec::new();
                    for id in refs {
                        if !unique.contains(&id) {
                            unique.push(id);
                        }
                    }
                    inputs.extend(unique.into_iter().enumerate().map(|(i, id)| {
                        ShotVideoInputAsset {
                            input_key: "reference_images".into(),
                            ordinal: i as i64,
                            asset_id: id,
                        }
                    }));
                }
                let pair = row.generator.as_ref().unwrap();
                let input_scope = ShotVideoInputScope {
                    project_id: project.clone(),
                    shot_id: id.clone(),
                    workflow_version_id: pair.workflow_version_id.clone(),
                    recipe_id: pair.recipe_id.clone(),
                };
                #[cfg(test)]
                {
                    let inject_occ = self
                        .faults
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .contains("D_OCC");
                    if inject_occ {
                        services
                            .inputs
                            .save(&input_scope, None, &inputs[..1])
                            .await
                            .unwrap();
                    }
                }
                let saved = services.inputs.save(&input_scope, None, &inputs).await;
                if saved.is_err() {
                    result.residual.shot_ids.push(id.clone());
                    return Err(stage_failure(
                        "D",
                        "INPUT_SAVE_FAILED",
                        "正式输入保存被拒绝；保留可能被并发编辑的镜头",
                    ));
                }
            }
            self.injection("E")?;
            if request.create_queue {
                let batch_plan = services
                    .batch
                    .plan_for_shots(project, ShotStage::Video, &result.created_shot_ids)
                    .await
                    .map_err(|e| {
                        stage_failure(
                            "E",
                            "BATCH_BLOCKED",
                            &format!("正式批量准备不可用：{}", batch_error_reason(&e)),
                        )
                    })?;
                if let Some(row) = batch_plan
                    .rows
                    .iter()
                    .find(|r| !r.blocking_reasons.is_empty() || !r.eligible)
                {
                    let reason = if row.blocking_reasons.is_empty() {
                        "镜头暂不满足入队条件".to_owned()
                    } else {
                        row.blocking_reasons.join("；")
                    };
                    return Err(stage_failure(
                        "E",
                        "BATCH_BLOCKED",
                        &format!(
                            "正式预检阻断了批次；未启动生产：{}",
                            chinese_reason(&reason)
                        ),
                    ));
                }
                let batch = services
                    .batch
                    .create_named(
                        super::shot_batch_service::CreateShotBatchRequest {
                            project_id: project.clone(),
                            stage: ShotStage::Video,
                            shot_ids: result.created_shot_ids.clone(),
                        },
                        plan.manifest.queue.name.clone(),
                    )
                    .await
                    .map_err(|e| {
                        stage_failure(
                            "E",
                            "BATCH_BLOCKED",
                            &format!("批次创建失败，未启动生产：{}", batch_error_reason(&e)),
                        )
                    })?;
                result.batch_id = Some(batch.batch.id.as_str().to_owned());
                result.item_count = batch.items.len();
                if !batch.batch.continue_on_failure {
                    return Err(stage_failure(
                        "E",
                        "BATCH_POLICY_MISMATCH",
                        "正式批次策略不匹配",
                    ));
                }
                self.injection("E_AFTER")?;
                if request.auto_start {
                    match services
                        .start
                        .start(project, result.batch_id.as_deref().unwrap())
                        .await
                    {
                        Ok(()) => result.auto_started = true,
                        Err(error) => result.warnings.push(error.to_string()),
                    }
                }
            }
            Ok(())
        }
        async fn compensate(
            &self,
            services: &CommitServices,
            project: &str,
            result: &mut ImportResult,
        ) {
            if let Some(batch) = &result.batch_id {
                let owned_pending =
                    services
                        .queue
                        .get(project, batch)
                        .await
                        .ok()
                        .is_some_and(|d| {
                            d.batch.status == crate::domain::ProductionBatchStatus::Ready
                                && d.items.len() == result.item_count
                                && d.items.iter().all(|i| {
                                    i.status == crate::domain::ProductionBatchItemStatus::Pending
                                        && i.task_id.is_none()
                                })
                        });
                if !owned_pending
                    || services.queue.archive(project, batch).await.is_err()
                    || services.queue.delete(project, batch).await.is_err()
                {
                    result.residual.batch_id = Some(batch.clone());
                    result.compensation_errors.push(CompensationError {
                        target: "batch".into(),
                        id: batch.clone(),
                        message: "批次不再是从未启动的待执行状态，或归档/删除失败；已保留".into(),
                    });
                }
            }
            for id in result.created_shot_ids.iter().rev() {
                if result.residual.shot_ids.contains(id)
                    || result.residual.batch_id.is_some()
                    || services.shots.delete(project, id).await.is_err()
                {
                    if !result.residual.shot_ids.contains(id) {
                        result.residual.shot_ids.push(id.clone());
                    }
                    result.compensation_errors.push(CompensationError {
                        target: "shot".into(),
                        id: id.clone(),
                        message: "保留未清理批次的镜头关联，或镜头受保护/删除失败".into(),
                    });
                }
            }
            for id in result.created_entity_ids.iter().rev() {
                if !result.residual.shot_ids.is_empty()
                    || self.entities.delete(project, id).await.is_err()
                {
                    result.residual.entity_ids.push(id.clone());
                    result.compensation_errors.push(CompensationError {
                        target: "entity".into(),
                        id: id.clone(),
                        message: "保留仍有关联或未能清理的实体".into(),
                    });
                }
            }
            for old in result.restored_entities.clone().iter().rev() {
                let restored = self
                    .entities
                    .update(
                        project,
                        &old.id,
                        crate::domain::story_entity::StoryEntityUpsert {
                            name: old.name.clone(),
                            description: old.description.clone(),
                            cover_asset_id: old.cover_asset_id.clone(),
                            image_asset_ids: old.image_asset_ids.clone(),
                        },
                    )
                    .await;
                if restored.is_err() {
                    result.residual.entity_ids.push(old.id.clone());
                    result.compensation_errors.push(CompensationError {
                        target: "entity".into(),
                        id: old.id.clone(),
                        message: "未能恢复复用实体的原封面/描述".into(),
                    });
                }
            }
            for id in result.imported_asset_ids.iter().rev() {
                if self.injection("DELETE_ASSET").is_err()
                    || services
                        .deletion
                        .delete(project, std::slice::from_ref(id))
                        .await
                        .is_err()
                {
                    result.residual.asset_ids.push(id.clone());
                    result.compensation_errors.push(CompensationError {
                        target: "asset".into(),
                        id: id.clone(),
                        message: "素材受保护或删除失败".into(),
                    });
                }
            }
            result.outcome = if !result.residual.unverified_file_cleanup
                && result.residual.batch_id.is_none()
                && result.residual.shot_ids.is_empty()
                && result.residual.entity_ids.is_empty()
                && result.residual.asset_ids.is_empty()
            {
                "compensated"
            } else {
                "partial"
            }
            .into();
            tracing::warn!(outcome=%result.outcome,stage=?result.failure.as_ref().map(|e|&e.stage),"folder import compensation completed");
        }
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn manifest_errors_name_the_offending_field_and_position() {
            let missing = parse_manifest(br#"{"format":"ai-studio-shot-batch","version":1,"name":"x",
"defaults":{"durationSeconds":8,"aspectRatio":"16:9","resolutionPreset":"0.98"},"characters":[],"shots":[],"queue":{"create":false,"autoStart":false}}"#)
                .err()
                .unwrap();
            assert_eq!(missing.code, "INVALID_MANIFEST");
            assert!(
                missing
                    .message
                    .starts_with("清单缺少必填字段 generators（第 2 行"),
                "{}",
                missing.message
            );
            let unknown = parse_manifest(br#"{"format":"ai-studio-shot-batch","bogusField":1}"#)
                .err()
                .unwrap();
            assert!(
                unknown
                    .message
                    .starts_with("清单含未知字段 bogusField（第 1 行"),
                "{}",
                unknown.message
            );
            let syntax = parse_manifest(b"{").err().unwrap();
            assert!(
                syntax.message.starts_with("清单不是有效的 JSON"),
                "{}",
                syntax.message
            );
        }
        use super::*;
        fn multi_image_definition(key: &str) -> crate::application::ports::GenerationDefinition {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("runtime_packages/minimax_h3_reference_video_quality_2_2_0");
            let mut recipe: serde_json::Value =
                yaml_serde::from_str(&std::fs::read_to_string(path.join("recipe.yaml")).unwrap())
                    .unwrap();
            let inputs = recipe["inputs"].as_object_mut().unwrap();
            inputs.remove("reference_videos");
            inputs.remove("reference_audios");
            let mut images = inputs.remove("reference_images").unwrap();
            images["min_items"] = 1.into();
            images["max_items"] = 8.into();
            inputs.insert(key.into(), images);
            recipe["bindings"]
                .as_array_mut()
                .unwrap()
                .retain_mut(|binding| match binding["source"].as_str().unwrap() {
                    "reference_videos" | "reference_audios" => false,
                    "reference_images" => {
                        binding["source"] = key.into();
                        binding["item"].as_u64().is_some_and(|item| item < 8)
                    }
                    _ => true,
                });
            let yaml = yaml_serde::to_string(&recipe).unwrap();
            let workflow = std::fs::read(path.join("workflow_api.json")).unwrap();
            crate::application::ports::GenerationDefinition {
                workflow_id: "wfl_custom_multi_image".into(),
                workflow_version_id: "multi-image-version".into(),
                recipe_id: recipe["id"].as_str().unwrap().into(),
                workflow_version: "1.0.0".into(),
                workflow_sha256: sha(&workflow),
                recipe_version: "1.0.0".into(),
                recipe_sha256: sha(yaml.as_bytes()),
                package_name: None,
                package_source_path: None,
                workflow_json: serde_json::from_slice(&workflow).unwrap(),
                recipe_yaml: yaml,
            }
        }
        #[test]
        fn default_multi_image_reference_compiles_and_checks_bounds() {
            let row = normalize(&manifest(), None, &[]).remove(1);
            for key in ["reference_images", "pictures"] {
                let definition = multi_image_definition(key);
                let references: Vec<_> = (0..6).map(|i| format!("images/{i}.png")).collect();
                if key != "reference_images" {
                    // Import persistence uses the formal reference_images slot.
                    assert_eq!(
                        validate_definition(&row, &definition, &references)
                            .unwrap_err()
                            .code,
                        "MODE_MISMATCH"
                    );
                    continue;
                }
                validate_definition(&row, &definition, &references).unwrap();
                assert!(validate_definition(&row, &definition, &[]).is_err());
                let excess = vec!["images/a.png".into(); 9];
                assert!(validate_definition(&row, &definition, &excess).is_err());
                let image_row = normalize(&manifest(), None, &[]).remove(0);
                assert_eq!(
                    validate_definition(&image_row, &definition, &references)
                        .unwrap_err()
                        .code,
                    "MODE_MISMATCH"
                );
            }
        }
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
        fn manifest_accepts_h3_writer_sections_and_aliases() {
            let shot: super::ManifestShot = serde_json::from_value(serde_json::json!({
                "description": "@A walks.",
                "style": "Warm lamplight.",
                "summary": "@A walks home.",
                "soundscape": "Rain.",
                "music": "N/A"
            }))
            .unwrap();
            assert_eq!(shot.prompt, "@A walks.");
            assert_eq!(shot.style.as_deref(), Some("Warm lamplight."));
            assert_eq!(shot.music.as_deref(), Some("N/A"));
            let old: super::ManifestShot =
                serde_json::from_value(serde_json::json!({ "prompt": "旧写法" })).unwrap();
            assert!(old.style.is_none() && old.summary.is_none());
            let entity: super::ManifestEntity = serde_json::from_value(serde_json::json!({
                "name": "A", "appearanceEn": "a tall man", "images": []
            }))
            .unwrap();
            assert_eq!(entity.description.as_deref(), Some("a tall man"));
        }

        #[test]
        fn bounds_and_duplicate_names_require_formal_suffix_revalidation() {
            let mut m = manifest();
            m.shots[0].duration_seconds = Some(0);
            assert!(normalize(&m, None, &[])[0]
                .errors
                .iter()
                .any(|e| e.code == "DURATION_OUT_OF_RANGE"));
            for duration in [12, 16] {
                m.shots[0].duration_seconds = Some(duration);
                let row = &normalize(&m, None, &[])[0];
                assert!(row.errors.iter().any(|e| e.code == "DURATION_TOO_LONG"
                    && e.message.contains("镜头时长最长 11 秒（显存限制）")));
            }
            for duration in [1, 11] {
                m.shots[0].duration_seconds = Some(duration);
                assert!(!normalize(&m, None, &[])[0].errors.iter().any(|e| {
                    e.code == "DURATION_OUT_OF_RANGE" || e.code == "DURATION_TOO_LONG"
                }));
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
            async fn resolve_superseded_pair(
                &self,
                version: &str,
                recipe: &str,
            ) -> Result<
                Option<crate::application::ports::SupersededGenerationPair>,
                crate::application::ports::RepositoryError,
            > {
                // Mirrors the built-in Veda 1.1.0 pins carried by older packages.
                if ![
                    (SUPERSEDED_VEDA_VERSION, SUPERSEDED_VEDA_RECIPE),
                    (SUPERSEDED_VEDA_V1_2_VERSION, SUPERSEDED_VEDA_V1_2_RECIPE),
                ]
                .contains(&(version, recipe))
                {
                    return Ok(None);
                }
                Ok(self
                    .0
                    .iter()
                    .find(|d| d.workflow_version_id == "ref-version")
                    .map(|d| crate::application::ports::SupersededGenerationPair {
                        workflow_version_id: d.workflow_version_id.clone(),
                        recipe_id: d.recipe_id.clone(),
                        workflow_name: "Veda".into(),
                        current_version: "1.3.0".into(),
                    }))
            }
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
                Ok(self
                    .0
                    .iter()
                    .map(
                        |d| crate::application::ports::AvailableGenerationDefinition {
                            workflow_id: d.workflow_id.clone(),
                            workflow_version_id: d.workflow_version_id.clone(),
                            recipe_id: d.recipe_id.clone(),
                            recipe_version: d.recipe_version.clone(),
                            name: d.workflow_id.clone(),
                            category: "video".into(),
                            mode: "video".into(),
                            recipe_yaml: d.recipe_yaml.clone(),
                        },
                    )
                    .collect())
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
        const TEST_PROJECT: &str = "prj_00000000-0000-4000-8000-000000000001";
        struct PreviewFixture {
            start: Arc<CountStart>,
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
                (
                    Mode::Image,
                    "minimax_h3_fl2va_i2v_quality_2_2_0",
                    "image-version",
                    "wfl_minimax_h3_fl2va_i2v_quality",
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
            for d in &definitions {
                sqlx::query("INSERT INTO workflows(id,name,category,mode,created_at,updated_at) VALUES (?,?,'video','video',?,?)").bind(&d.workflow_id).bind(&d.workflow_id).bind(chrono::Utc::now().to_rfc3339()).bind(chrono::Utc::now().to_rfc3339()).execute(&pool).await.unwrap();
                sqlx::query("INSERT INTO workflow_versions(id,workflow_id,version,api_workflow_json,workflow_sha256,created_at) VALUES (?,?,?,?,?,?)").bind(&d.workflow_version_id).bind(&d.workflow_id).bind(&d.workflow_version).bind(d.workflow_json.to_string()).bind(&d.workflow_sha256).bind(chrono::Utc::now().to_rfc3339()).execute(&pool).await.unwrap();
                sqlx::query("INSERT INTO recipes(id,workflow_version_id,version,schema_version,recipe_yaml,recipe_sha256,created_at) VALUES (?,?,?,1,?,?,?)").bind(&d.recipe_id).bind(&d.workflow_version_id).bind(&d.recipe_version).bind(&d.recipe_yaml).bind(&d.recipe_sha256).bind(chrono::Utc::now().to_rfc3339()).execute(&pool).await.unwrap();
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
            let definitions = Arc::new(PreviewDefinitions(definitions));
            let entity_service = Arc::new(StoryEntityService::new(Arc::new(
                SqliteStoryEntityRepository::new(pool.clone()),
            )));
            let start = Arc::new(CountStart::default());
            let commits = commit_services(
                &pool,
                definitions.clone(),
                entity_service.clone(),
                start.clone(),
            );
            let service = FolderBatchImportService::new(
                definitions,
                Arc::new(PreviewAdmission),
                Arc::new(SqliteShotRepository::new(pool.clone())),
                entity_service,
                Arc::new(SqliteAssetRepository::new(pool.clone())),
                Arc::new(SqliteProjectRepository::new(pool.clone())),
                Arc::new(PreviewClock),
            )
            .with_commit_services(commits);
            PreviewFixture {
                start,
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
        const SUPERSEDED_VEDA_VERSION: &str = "wfv_e087c778-e559-490b-a0aa-90b8210e6d86";
        const SUPERSEDED_VEDA_RECIPE: &str = "rcp_54473f3e-2831-4011-afa0-cbb2a2754da2";
        const SUPERSEDED_VEDA_V1_2_VERSION: &str = "wfv_ca037b72-864a-4863-95cf-62d2bc819ebb";
        const SUPERSEDED_VEDA_V1_2_RECIPE: &str = "rcp_d3ee526b-3341-4409-b8cb-083fa27f1eb6";
        #[test]
        fn expression_test_v0_2_manifest_parses_with_its_pinned_generator() {
            let m = parse_manifest(r#"{"format":"ai-studio-shot-batch","version":1,"name":"第一集｜人物表情测试V0.2","generators":{"reference":{"workflowVersionId":"wfv_e087c778-e559-490b-a0aa-90b8210e6d86","recipeId":"rcp_54473f3e-2831-4011-afa0-cbb2a2754da2"}},"defaults":{"durationSeconds":8,"aspectRatio":"16:9","resolutionPreset":"0.98"},"characters":[{"name":"沈簿","kind":"character","images":["images/人物/沈簿.png"],"cover":"images/人物/沈簿.png","appearanceEn":"A lean middle-aged man."},{"name":"躲手与收手","kind":"composition","images":["images/构图/躲手与收手.png"],"cover":"images/构图/躲手与收手.png"}],"shots":[{"id":"E01-023","name":"E01-023 躲手与收手 表情测试V0.2","mode":"reference","durationSeconds":8,"references":["images/构图/躲手与收手.png","images/人物/沈簿.png"],"openingComposition":"躲手与收手","style":"natural","summary":"@沈簿 withdraws.","description":"[Shot 1] Positioned as in @躲手与收手, @沈簿 withdraws his hand.","soundscape":"Uneven breathing.","music":"N/A"}],"queue":{"create":false,"name":"第一集人物表情测试V0.2","autoStart":false}}"#.as_bytes()).unwrap();
            let g = &m.generators[&Mode::Reference];
            assert_eq!(
                (g.workflow_version_id.as_str(), g.recipe_id.as_str()),
                (SUPERSEDED_VEDA_VERSION, SUPERSEDED_VEDA_RECIPE)
            );
        }
        #[tokio::test]
        async fn superseded_builtin_pin_follows_current_version_with_a_warning() {
            let f = preview_fixture().await;
            let path = f.root.join("manifest.json");
            let mut value: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            value["generators"]["reference"] = serde_json::json!({
                "workflowVersionId": SUPERSEDED_VEDA_VERSION,
                "recipeId": SUPERSEDED_VEDA_RECIPE
            });
            std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
            let p = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            assert_eq!((p.valid_count, p.error_count), (2, 0), "{:?}", p.rows);
            let warning = "导入包指定的工作流「Veda」版本已更新，已改用最新版本 1.3.0";
            assert_eq!(
                p.warnings.iter().filter(|w| *w == warning).count(),
                1,
                "{:?}",
                p.warnings
            );
            let r = f.service.commit(request(&p, false)).await.unwrap();
            assert_eq!(r.outcome, "completed", "{r:?}");
            let pinned: Vec<String> = sqlx::query_scalar(
                "SELECT DISTINCT workflow_version_id FROM shot_stage_configs WHERE shot_id = ?",
            )
            .bind(&r.created_shot_ids[1])
            .fetch_all(&f.pool)
            .await
            .unwrap();
            assert_eq!(pinned, vec!["ref-version".to_string()]);
            f.pool.close().await;
        }
        #[tokio::test]
        async fn superseded_v1_2_pin_follows_current_1_3_0_with_a_warning() {
            let f = preview_fixture().await;
            let path = f.root.join("manifest.json");
            let mut value: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            value["generators"]["reference"] = serde_json::json!({
                "workflowVersionId": SUPERSEDED_VEDA_V1_2_VERSION,
                "recipeId": SUPERSEDED_VEDA_V1_2_RECIPE
            });
            std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
            let p = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            assert_eq!((p.valid_count, p.error_count), (2, 0), "{:?}", p.rows);
            let warning = "导入包指定的工作流「Veda」版本已更新，已改用最新版本 1.3.0";
            assert_eq!(
                p.warnings.iter().filter(|w| *w == warning).count(),
                1,
                "{:?}",
                p.warnings
            );
            let r = f.service.commit(request(&p, false)).await.unwrap();
            assert_eq!(r.outcome, "completed", "{r:?}");
            let pinned: Vec<String> = sqlx::query_scalar(
                "SELECT DISTINCT workflow_version_id FROM shot_stage_configs WHERE shot_id = ?",
            )
            .bind(&r.created_shot_ids[1])
            .fetch_all(&f.pool)
            .await
            .unwrap();
            assert_eq!(pinned, vec!["ref-version".to_string()]);
            f.pool.close().await;
        }
        #[tokio::test]
        async fn unknown_pins_keep_failing_without_a_version_warning() {
            let f = preview_fixture().await;
            let path = f.root.join("manifest.json");
            let mut value: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            value["generators"]["reference"]["workflowVersionId"] = SUPERSEDED_VEDA_VERSION.into();
            std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
            let p = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            assert_eq!(p.error_count, 1, "{:?}", p.rows);
            assert!(
                !p.warnings.iter().any(|w| w.contains("版本已更新")),
                "{:?}",
                p.warnings
            );
            f.pool.close().await;
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
                        legacy: None,
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
        struct NoExecution;
        #[async_trait::async_trait]
        impl crate::application::ports::ComfyAdapter for NoExecution {
            async fn health_check(
                &self,
            ) -> Result<
                crate::application::ports::ComfyHealth,
                crate::application::ports::ComfyAdapterError,
            > {
                panic!("import must not contact ComfyUI")
            }
            async fn get_system_stats(
                &self,
            ) -> Result<
                crate::application::ports::SystemStats,
                crate::application::ports::ComfyAdapterError,
            > {
                panic!("import must not contact ComfyUI")
            }
            async fn get_object_info(
                &self,
            ) -> Result<serde_json::Value, crate::application::ports::ComfyAdapterError>
            {
                panic!("import must not contact ComfyUI")
            }
            async fn get_history(
                &self,
                _: &str,
            ) -> Result<
                crate::application::ports::ComfyHistory,
                crate::application::ports::ComfyAdapterError,
            > {
                panic!("import must not contact ComfyUI")
            }
            async fn download_output(
                &self,
                _: &crate::application::ports::ComfyOutputFile,
            ) -> Result<
                crate::application::ports::ComfyOutputData,
                crate::application::ports::ComfyAdapterError,
            > {
                panic!("import must not download")
            }
            async fn submit_workflow(
                &self,
                _: &str,
                _: &str,
                _: serde_json::Value,
            ) -> Result<
                crate::application::ports::PromptSubmission,
                crate::application::ports::ComfyAdapterError,
            > {
                panic!("import must not submit GPU")
            }
            async fn subscribe_events(
                &self,
                _: &str,
            ) -> Result<
                Box<dyn crate::application::ports::ComfyEventSubscription>,
                crate::application::ports::ComfyAdapterError,
            > {
                panic!("import must not subscribe")
            }
        }
        #[derive(Default)]
        struct CountStart(std::sync::atomic::AtomicUsize);
        #[async_trait::async_trait]
        impl ImportStart for CountStart {
            async fn start(&self, _: &str, _: &str) -> Result<(), ImportFailure> {
                self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            }
        }
        fn commit_services(
            pool: &sqlx::SqlitePool,
            definitions: Arc<dyn GenerationDefinitionRepository>,
            entities: Arc<StoryEntityService>,
            start: Arc<CountStart>,
        ) -> CommitServices {
            use crate::application::*;
            use crate::infrastructure::database::repositories::{
                story_entity::SqliteShotRowEditRepository,
                story_entity::SqliteStoryEntityRepository, SqliteShotVideoInputRepository,
            };
            use crate::infrastructure::database::*;
            let clock = Arc::new(PreviewClock);
            let shots = Arc::new(SqliteShotRepository::new(pool.clone()));
            let assets = Arc::new(SqliteAssetRepository::new(pool.clone()));
            let projects = Arc::new(SqliteProjectRepository::new(pool.clone()));
            let tasks = Arc::new(SqliteTaskRepository::new(pool.clone()));
            let snapshots = Arc::new(SqliteGenerationSnapshotRepository::new(pool.clone()));
            let queue_repo = Arc::new(SqliteProductionQueueRepository::new(pool.clone()));
            let prompts = Arc::new(SqlitePromptLibraryRepository::new(pool.clone()));
            let store = Arc::new(crate::infrastructure::filesystem::FileSystemAssetStore::new());
            let comfy = Arc::new(NoExecution);
            let admission = Arc::new(PreviewAdmission);
            let preparer = Arc::new(generation_input_preparer::GenerationInputPreparer::new(
                assets.clone(),
                store.clone(),
                projects.clone(),
                comfy.clone(),
            ));
            let inputs = Arc::new(
                shot_video_input_service::ShotVideoInputService::new(
                    assets.clone(),
                    Arc::new(SqliteShotVideoInputRepository::new(pool.clone())),
                    shots.clone(),
                    definitions.clone(),
                    admission.clone(),
                    preparer,
                    clock.clone(),
                )
                .with_story_entities(Arc::new(SqliteStoryEntityRepository::new(pool.clone()))),
            );
            let generation = Arc::new(generation_service::GenerationService::new(
                tasks.clone(),
                snapshots.clone(),
                definitions.clone(),
                comfy.clone(),
                projects.clone(),
                store.clone(),
                assets.clone(),
                clock.clone(),
            ));
            let recovery = Arc::new(task_recovery_service::TaskRecoveryService::new(
                tasks.clone(),
                snapshots,
                assets.clone(),
                comfy,
                projects.clone(),
                store.clone(),
                clock.clone(),
                Arc::new(ports::NoopTaskUpdateSink),
            ));
            let queue = Arc::new(
                production_queue_service::ProductionQueueService::new(
                    queue_repo.clone(),
                    tasks.clone(),
                    definitions.clone(),
                    generation,
                    queue_repo.clone(),
                    recovery,
                    clock.clone(),
                )
                .with_new_generation_admission(admission.clone()),
            );
            let query = Arc::new(task_query_service::TaskQueryService::new(
                tasks.clone(),
                assets.clone(),
                definitions.clone(),
            ));
            let shot = Arc::new(
                shot_service::ShotService::new(
                    shots.clone(),
                    tasks.clone(),
                    assets.clone(),
                    definitions.clone(),
                    prompts.clone(),
                    query,
                    queue_repo.clone(),
                    clock.clone(),
                )
                .with_row_edit_repository(Arc::new(SqliteShotRowEditRepository::new(pool.clone())))
                .with_stage_prompt_repository(shots.clone())
                .with_video_inputs(inputs.clone()),
            );
            let batch = Arc::new(
                shot_batch_service::ShotBatchService::new(
                    shots.clone(),
                    queue_repo,
                    tasks,
                    assets.clone(),
                    definitions.clone(),
                    projects.clone(),
                    clock.clone(),
                )
                .with_stage_prompt_repository(shots.clone())
                .with_video_inputs(inputs.clone())
                .with_new_generation_admission(admission),
            );
            let _ = entities;
            CommitServices {
                bulk: Arc::new(shot_bulk_service::ShotBulkService::new(
                    shots,
                    definitions,
                    prompts,
                    clock.clone(),
                )),
                shots: shot,
                source: Arc::new(source_asset_import_service::SourceAssetImportService::new(
                    projects.clone(),
                    store.clone(),
                    assets.clone(),
                    clock,
                )),
                inputs,
                batch,
                queue,
                deletion: Arc::new(asset_deletion_service::AssetDeletionService::new(
                    assets,
                    Arc::new(SqliteAssetDeletionRepository::new(pool.clone())),
                    projects,
                    store,
                )),
                start,
            }
        }
        fn request(preview: &Preview, queue: bool) -> CommitRequest {
            CommitRequest {
                session_id: preview.session_id.clone(),
                preview_version: preview.preview_version,
                manifest_sha256: preview.manifest_sha256.clone(),
                include_row_keys: preview
                    .rows
                    .iter()
                    .filter(|r| r.errors.is_empty())
                    .map(|r| r.row_key.clone())
                    .collect(),
                create_queue: queue,
                auto_start: false,
                name_suffix: preview.name_suffix.clone(),
            }
        }
        async fn count(pool: &sqlx::SqlitePool, table: &str) -> i64 {
            sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(pool)
                .await
                .unwrap()
        }
        async fn assert_no_execution(pool: &sqlx::SqlitePool) {
            assert_eq!(count(pool, "tasks").await, 0);
            // Authoritative Prepare creates NULL-task links to pending items, not results.
            let invalid_links:i64=sqlx::query_scalar("SELECT COUNT(*) FROM shot_generation_links l LEFT JOIN production_batch_items i ON i.id=l.production_batch_item_id WHERE l.task_id IS NOT NULL OR i.id IS NULL OR i.status!='PENDING'").fetch_one(pool).await.unwrap();
            assert_eq!(invalid_links, 0);
            let generated: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM assets WHERE source_task_id IS NOT NULL OR category!='source_image'")
                    .fetch_one(pool)
                    .await
                    .unwrap();
            assert_eq!(generated, 0);
            assert!(sqlx::query("PRAGMA foreign_key_check")
                .fetch_all(pool)
                .await
                .unwrap()
                .is_empty());
        }
        #[tokio::test]
        async fn import_while_project_video_task_runs_is_blocked_by_precheck_with_chinese_reason() {
            // Repro: a second folder import while an earlier shot's video task is RUNNING.
            // The scoped precheck used to ignore the project-wide strict-serial guard,
            // so create_named failed with an unexplained "批次创建失败".
            let f = preview_fixture().await;
            let preview = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            let first = f.service.commit(request(&preview, true)).await.unwrap();
            assert_eq!(first.outcome, "completed", "{first:?}");
            let now = chrono::Utc::now().to_rfc3339();
            sqlx::query("INSERT INTO tasks(id,project_id,workflow_id,workflow_version_id,recipe_id,status,progress_mode,progress_current,progress_total,prompt_id,created_at,queued_at,started_at) VALUES ('tsk_00000000-0000-4000-8000-0000000000aa',?1,'wfl_minimax_h3_reference_video_quality','ref-version',(SELECT recipe_id FROM production_batch_items LIMIT 1),'RUNNING','indeterminate',NULL,NULL,'p1',?2,?2,?2)")
                .bind(TEST_PROJECT).bind(&now).execute(&f.pool).await.unwrap();
            sqlx::query("UPDATE shot_generation_links SET task_id='tsk_00000000-0000-4000-8000-0000000000aa' WHERE shot_id=? AND stage='video'")
                .bind(&first.created_shot_ids[0]).execute(&f.pool).await.unwrap();
            let second = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            let again = f
                .service
                .rescan(
                    &second.session_id,
                    PreviewOptions {
                        name_suffix: Some(" 二".into()),
                        legacy: None,
                    },
                )
                .await
                .unwrap();
            let r = f.service.commit(request(&again, true)).await.unwrap();
            let failure = r.failure.expect("must be blocked");
            assert!(
                failure.message.starts_with("正式预检阻断了批次")
                    && failure.message.contains("视频阶段严格串行"),
                "{failure:?}"
            );
            assert!(!failure.message.contains("tsk_") && !failure.message.contains("sht_"));
            f.pool.close().await;
        }
        #[test]
        fn batch_error_reason_is_chinese_without_ids() {
            use crate::application::shot_batch_service::ShotBatchServiceError as E;
            assert_eq!(
                batch_error_reason(&E::InvalidInput("REF2VA 参考图重复：ast_00000000-1".into())),
                "REF2VA 参考图重复"
            );
            assert!(
                batch_error_reason(&E::InvalidInput("generation definition missing".into()))
                    .starts_with("正式输入校验未通过")
            );
            assert!(batch_error_reason(&E::InvalidInput(format!(
                "{}: wfv_x/rcp_y",
                crate::application::minimax_video_product_policy::REQUIRED
            )))
            .contains("生成器版本"));
            assert_eq!(
                batch_error_reason(&E::NotFound("prj_x".into())),
                "项目不存在或已被删除"
            );
        }
        #[tokio::test]
        async fn commit_real_sources_independent_slots_entities_and_frozen_queue_without_execution()
        {
            let f = preview_fixture().await;
            let preview = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            assert_eq!(preview.valid_count, 2);
            let req = request(&preview, true);
            let result = f.service.commit(req.clone()).await.unwrap();
            assert_eq!(result.outcome, "completed", "{result:?}");
            assert_eq!(result.created_shot_ids.len(), 2);
            assert_eq!(result.imported_asset_ids.len(), 4);
            assert_eq!(result.created_entity_ids.len(), 1);
            assert!(!result.auto_started);
            let services = f.service.commit_services.as_ref().unwrap();
            let batch = services
                .queue
                .get(TEST_PROJECT, result.batch_id.as_ref().unwrap())
                .await
                .unwrap();
            assert_eq!(
                batch.batch.status,
                crate::domain::ProductionBatchStatus::Ready
            );
            assert_eq!(batch.items.len(), 2);
            assert!(batch.items.iter().all(|i| i.status
                == crate::domain::ProductionBatchItemStatus::Pending
                && i.task_id.is_none()));
            let input_rows:Vec<(String,String)>=sqlx::query_as("SELECT input_key,asset_id FROM shot_video_input_assets WHERE shot_id=? ORDER BY input_key,ordinal").bind(&result.created_shot_ids[0]).fetch_all(&f.pool).await.unwrap();
            assert_eq!(input_rows.len(), 2);
            assert_eq!(input_rows[0].0, "first_frame");
            assert_eq!(input_rows[1].0, "last_frame");
            assert_ne!(input_rows[0].1, input_rows[1].1);
            let names:Vec<String>=sqlx::query_scalar("SELECT a.original_name FROM shot_video_input_assets i JOIN assets a ON a.id=i.asset_id WHERE i.shot_id=? ORDER BY i.input_key,i.ordinal").bind(&result.created_shot_ids[0]).fetch_all(&f.pool).await.unwrap();
            assert_eq!(names, vec!["a.png", "b.png"]);
            let names:Vec<String>=sqlx::query_scalar("SELECT a.original_name FROM shot_video_input_assets i JOIN assets a ON a.id=i.asset_id WHERE i.shot_id=? ORDER BY i.ordinal").bind(&result.created_shot_ids[1]).fetch_all(&f.pool).await.unwrap();
            assert_eq!(names, vec!["c.png", "alice.png"]);
            let refs:i64=sqlx::query_scalar("SELECT COUNT(*) FROM shot_video_input_assets WHERE shot_id=? AND input_key='reference_images'").bind(&result.created_shot_ids[1]).fetch_one(&f.pool).await.unwrap();
            assert_eq!(refs, 2);
            let before: Vec<String> = sqlx::query_scalar(
                "SELECT values_json FROM production_batch_items ORDER BY ordinal",
            )
            .fetch_all(&f.pool)
            .await
            .unwrap();
            let prompts = vec![crate::application::ports::ShotStagePromptRecord {
                shot_id: result.created_shot_ids[0].clone(),
                stage: crate::domain::ShotStage::Video,
                prompt_text: "later edit".into(),
                prompt_entry_id: None,
                prompt_version_id: None,
                updated_at: chrono::Utc::now(),
            }];
            f.service
                .shots
                .update_stage_prompts_atomic(TEST_PROJECT, &prompts)
                .await
                .unwrap();
            let after: Vec<String> = sqlx::query_scalar(
                "SELECT values_json FROM production_batch_items ORDER BY ordinal",
            )
            .fetch_all(&f.pool)
            .await
            .unwrap();
            assert_eq!(before, after);
            let again = f.service.commit(req).await.unwrap();
            assert_eq!(again.created_shot_ids, result.created_shot_ids);
            assert!(again.warnings.iter().any(|w| w.contains("没有重复写入")));
            assert_eq!(count(&f.pool, "shots").await, 2);
            assert_eq!(count(&f.pool, "assets").await, 4);
            assert_no_execution(&f.pool).await;
            let duplicate = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            assert_eq!(duplicate.valid_count, 0);
            let suffixed = f
                .service
                .rescan(
                    &duplicate.session_id,
                    PreviewOptions {
                        name_suffix: Some(" copy".into()),
                        legacy: None,
                    },
                )
                .await
                .unwrap();
            assert_eq!(suffixed.valid_count, 2);
            f.pool.close().await;
        }
        /// Regression (project '111'): reused entities listed both as reference
        /// paths and @ mentions were counted and bound twice (managed:{asset} plus
        /// the manifest path), pushing shots past the 8-image limit.
        async fn reimport_with_existing_entity_paths_and_mentions(
            same_bytes: bool,
        ) -> (PreviewFixture, Preview, ImportResult, String) {
            let f = preview_fixture().await;
            let first = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            let done = f.service.commit(request(&first, true)).await.unwrap();
            assert_eq!(done.outcome, "completed", "{done:?}");
            let (old_cover, _) = alice(&f).await;
            let mut m: Manifest =
                serde_json::from_slice(&std::fs::read(f.root.join("manifest.json")).unwrap())
                    .unwrap();
            let png = std::fs::read(f.root.join("images/alice.png")).unwrap();
            let mut refs = vec!["images/c.png".to_string()];
            for i in 0..6 {
                let mut other = Vec::new();
                image::DynamicImage::new_rgb8(32 + i, 32)
                    .write_to(
                        &mut std::io::Cursor::new(&mut other),
                        image::ImageFormat::Png,
                    )
                    .unwrap();
                let name = format!("images/d{i}.png");
                std::fs::write(f.root.join(&name), other).unwrap();
                refs.push(name);
            }
            refs.push("images/alice.png".into());
            m.shots[1].references = refs;
            m.characters[0].description = Some("Tall woman in a red wool coat".into());
            std::fs::write(
                f.root.join("manifest.json"),
                serde_json::to_vec(&m).unwrap(),
            )
            .unwrap();
            if !same_bytes {
                let mut other = Vec::new();
                image::DynamicImage::new_rgb8(40, 40)
                    .write_to(
                        &mut std::io::Cursor::new(&mut other),
                        image::ImageFormat::Png,
                    )
                    .unwrap();
                assert_ne!(other, png);
                std::fs::write(f.root.join("images/alice.png"), other).unwrap();
            }
            let again = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            let preview = f
                .service
                .rescan(
                    &again.session_id,
                    PreviewOptions {
                        name_suffix: Some(" copy".into()),
                        legacy: None,
                    },
                )
                .await
                .unwrap();
            let row = &preview.rows[1];
            assert!(row.errors.is_empty(), "{:?}", row.errors);
            // c + d0..d5 + Alice once = 8, within the limit (was 9 before).
            assert_eq!(row.reference_count, 8);
            assert_eq!(preview.valid_count, 2);
            let result = f.service.commit(request(&preview, true)).await.unwrap();
            assert_eq!(result.outcome, "completed", "{result:?}");
            (f, preview, result, old_cover)
        }
        async fn bound_reference_assets(f: &PreviewFixture, shot: &str) -> Vec<String> {
            sqlx::query_scalar("SELECT asset_id FROM shot_video_input_assets WHERE shot_id=? AND input_key='reference_images' ORDER BY ordinal").bind(shot).fetch_all(&f.pool).await.unwrap()
        }
        async fn alice(f: &PreviewFixture) -> (String, String) {
            sqlx::query_as(
                "SELECT cover_asset_id, description FROM story_entities WHERE name='Alice'",
            )
            .fetch_one(&f.pool)
            .await
            .unwrap()
        }
        #[tokio::test]
        async fn reused_entity_same_bytes_counts_and_binds_one_project_image() {
            let (f, preview, result, old_cover) =
                reimport_with_existing_entity_paths_and_mentions(true).await;
            let refs = bound_reference_assets(&f, &result.created_shot_ids[1]).await;
            assert_eq!(refs.len(), 8);
            assert_eq!(refs.iter().collect::<BTreeSet<_>>().len(), 8);
            let (cover, description) = alice(&f).await;
            assert_eq!(cover, old_cover);
            assert_eq!(refs.iter().filter(|r| **r == cover).count(), 1);
            assert_eq!(description, "Tall woman in a red wool coat");
            assert!(!preview.rows[1]
                .warnings
                .iter()
                .any(|w| w.message.contains("导入包中的新图片")));
            assert_eq!(count(&f.pool, "story_entities").await, 1);
            assert_no_execution(&f.pool).await;
            f.pool.close().await;
        }
        #[tokio::test]
        async fn reused_entity_different_bytes_package_image_becomes_cover_once() {
            let (f, preview, result, old_cover) =
                reimport_with_existing_entity_paths_and_mentions(false).await;
            assert!(preview.rows[1]
                .warnings
                .iter()
                .any(|w| w.message
                    == "「Alice」将使用导入包中的新图片作为封面（原图片保留在资料库中）"));
            let refs = bound_reference_assets(&f, &result.created_shot_ids[1]).await;
            assert_eq!(refs.len(), 8);
            assert_eq!(refs.iter().collect::<BTreeSet<_>>().len(), 8);
            let (cover, _) = alice(&f).await;
            assert_ne!(cover, old_cover);
            assert_eq!(refs.iter().filter(|r| **r == cover).count(), 1);
            assert!(!refs.contains(&old_cover));
            let images: Vec<String> = sqlx::query_scalar("SELECT i.asset_id FROM story_entity_images i JOIN story_entities e ON e.id=i.entity_id WHERE e.name='Alice'")
                .fetch_all(&f.pool)
                .await
                .unwrap();
            assert!(
                images.contains(&old_cover) && images.contains(&cover),
                "{images:?}"
            );
            assert_eq!(count(&f.pool, "story_entities").await, 1);
            assert_no_execution(&f.pool).await;
            f.pool.close().await;
        }
        #[tokio::test]
        async fn reused_entity_cover_change_is_restored_on_compensation() {
            let f = preview_fixture().await;
            let first = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            assert_eq!(
                f.service
                    .commit(request(&first, true))
                    .await
                    .unwrap()
                    .outcome,
                "completed"
            );
            let before = alice(&f).await;
            let mut m: Manifest =
                serde_json::from_slice(&std::fs::read(f.root.join("manifest.json")).unwrap())
                    .unwrap();
            m.shots[1].references.push("images/alice.png".into());
            m.characters[0].description = Some("Tall woman in a red wool coat".into());
            std::fs::write(
                f.root.join("manifest.json"),
                serde_json::to_vec(&m).unwrap(),
            )
            .unwrap();
            let mut other = Vec::new();
            image::DynamicImage::new_rgb8(40, 40)
                .write_to(
                    &mut std::io::Cursor::new(&mut other),
                    image::ImageFormat::Png,
                )
                .unwrap();
            std::fs::write(f.root.join("images/alice.png"), other).unwrap();
            let again = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            let preview = f
                .service
                .rescan(
                    &again.session_id,
                    PreviewOptions {
                        name_suffix: Some(" copy".into()),
                        legacy: None,
                    },
                )
                .await
                .unwrap();
            f.service.faults.lock().unwrap().insert("C1");
            let r = f.service.commit(request(&preview, true)).await.unwrap();
            assert_eq!(r.outcome, "compensated", "{r:?}");
            assert_eq!(alice(&f).await, before);
            f.pool.close().await;
        }
        #[test]
        fn reused_entity_description_fills_only_empty_or_non_english() {
            let en = Some("Tall woman in a red coat");
            assert_eq!(fill_description("", en).as_deref(), en);
            assert_eq!(fill_description("红衣高个女子", en).as_deref(), en);
            assert_eq!(fill_description("A short man", en), None);
            assert_eq!(fill_description("", Some("红衣女子")), None);
            assert_eq!(fill_description("", None), None);
        }
        #[tokio::test]
        async fn all_committed_substeps_compensate_and_terminal_repeat_never_writes() {
            for stage in ["A", "B", "C1", "C2", "C3", "C4", "D", "E", "E_AFTER"] {
                let f = preview_fixture().await;
                let p = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
                let req = request(&p, true);
                f.service.faults.lock().unwrap().insert(stage);
                let r = f.service.commit(req.clone()).await.unwrap();
                assert_eq!(r.outcome, "compensated", "stage={stage}: {r:?}");
                assert_eq!(r.failure.as_ref().unwrap().stage, stage);
                for table in ["shots", "story_entities", "assets", "production_batches"] {
                    assert_eq!(count(&f.pool, table).await, 0, "{stage}/{table}: {r:?}");
                }
                let repeat = f.service.commit(req).await.unwrap();
                assert_eq!(repeat.outcome, r.outcome);
                assert_no_execution(&f.pool).await;
                f.pool.close().await;
            }
        }
        #[tokio::test]
        async fn failed_cleanup_reports_partial_and_never_retries_writes() {
            let f = preview_fixture().await;
            let p = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            let req = request(&p, false);
            f.service
                .faults
                .lock()
                .unwrap()
                .extend(["C3", "DELETE_ASSET"]);
            let r = f.service.commit(req.clone()).await.unwrap();
            assert_eq!(r.outcome, "partial");
            assert_eq!(r.failure.unwrap().stage, "C3");
            assert_eq!(r.residual.asset_ids.len(), 4);
            assert!(!r.compensation_errors.is_empty());
            f.service.faults.lock().unwrap().clear();
            let r2 = f.service.commit(req).await.unwrap();
            assert_eq!(r2.outcome, "partial");
            assert_eq!(count(&f.pool, "assets").await, 4);
            assert_no_execution(&f.pool).await;
            f.pool.close().await;
        }
        #[tokio::test]
        async fn stale_tokens_changed_bytes_and_duplicate_selection_reject_before_writes() {
            let f = preview_fixture().await;
            let p = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            let mut req = request(&p, false);
            req.preview_version += 1;
            assert_eq!(
                f.service.commit(req).await.unwrap_err().code,
                "STALE_PREVIEW"
            );
            let mut req = request(&p, false);
            req.include_row_keys.push(req.include_row_keys[0].clone());
            assert!(f.service.commit(req).await.is_err());
            std::fs::write(f.root.join("images/a.png"), b"changed").unwrap();
            assert_eq!(
                f.service.commit(request(&p, false)).await.unwrap_err().code,
                "STALE_PREVIEW"
            );
            assert_eq!(count(&f.pool, "assets").await, 0);
            assert_eq!(count(&f.pool, "shots").await, 0);
            assert_no_execution(&f.pool).await;
            f.pool.close().await;
        }
        #[tokio::test]
        async fn selection_imports_only_used_files_and_explicit_start_uses_existing_admission_seam()
        {
            let f = preview_fixture().await;
            let p = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            let mut req = request(&p, true);
            req.include_row_keys.truncate(1);
            req.auto_start = true;
            let r = f.service.commit(req).await.unwrap();
            assert_eq!(r.outcome, "completed", "{r:?}");
            assert!(r.auto_started);
            assert_eq!(r.imported_asset_ids.len(), 2);
            assert!(r.created_entity_ids.is_empty());
            assert_eq!(r.item_count, 1);
            assert_eq!(f.start.0.load(std::sync::atomic::Ordering::SeqCst), 1);
            assert_no_execution(&f.pool).await;
            f.pool.close().await;
        }
        #[tokio::test]
        async fn project_raii_lock_writing_state_and_panic_cannot_repeat_import() {
            let f = preview_fixture().await;
            let p = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            let guard =
                ProjectImportGuard::acquire(f.service.project_locks.clone(), TEST_PROJECT).unwrap();
            assert_eq!(
                f.service.commit(request(&p, false)).await.unwrap_err().code,
                "IMPORT_IN_PROGRESS"
            );
            drop(guard);
            let service = Arc::new(f.service);
            service.faults.lock().unwrap().insert("B_PANIC");
            let req = request(&p, false);
            let other = req.clone();
            let task = {
                let service = service.clone();
                tokio::spawn(async move { service.commit(req).await })
            };
            assert!(task.await.unwrap_err().is_panic());
            assert!(service.project_locks.lock().unwrap().is_empty());
            assert_eq!(
                service.commit(other).await.unwrap_err().code,
                "SESSION_CONSUMED"
            );
            assert!(service
                .rescan(&p.session_id, PreviewOptions::default())
                .await
                .is_err());
            assert!(service.discard(&p.session_id).await.is_err());
            assert_eq!(count(&f.pool, "assets").await, 4);
            assert_no_execution(&f.pool).await;
            f.pool.close().await;
        }
        #[tokio::test]
        async fn overlapping_commits_and_stale_suffix_cannot_duplicate_imports() {
            let f = preview_fixture().await;
            let p = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            let old = request(&p, false);
            let p = f
                .service
                .rescan(
                    &p.session_id,
                    PreviewOptions {
                        name_suffix: Some(" v2".into()),
                        legacy: None,
                    },
                )
                .await
                .unwrap();
            assert_eq!(
                f.service.commit(old).await.unwrap_err().code,
                "STALE_PREVIEW"
            );
            let req = request(&p, false);
            let (a, b) = tokio::join!(f.service.commit(req.clone()), f.service.commit(req));
            assert!(
                a.as_ref().is_ok_and(|r| r.outcome == "completed")
                    || b.as_ref().is_ok_and(|r| r.outcome == "completed")
            );
            for r in [a, b] {
                if let Err(e) = r {
                    assert_eq!(e.code, "IMPORT_IN_PROGRESS");
                }
            }
            assert_eq!(count(&f.pool, "shots").await, 2);
            assert_eq!(count(&f.pool, "assets").await, 4);
            assert_eq!(count(&f.pool, "story_entities").await, 1);
            assert_no_execution(&f.pool).await;
            f.pool.close().await;
        }
        fn add_prop_entity(f: &PreviewFixture, kind: &str) {
            let path = f.root.join("manifest.json");
            let mut value: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            value["characters"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "name": "调阅签", "kind": kind, "images": ["images/prop.png"]
                }));
            value["shots"][1]["prompt"] = "@Alice 拿起 @调阅签".into();
            std::fs::copy(f.root.join("images/a.png"), f.root.join("images/prop.png")).unwrap();
            std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        }
        #[tokio::test]
        async fn manifest_prop_entity_is_created_bound_and_invalid_kind_rejected() {
            let f = preview_fixture().await;
            add_prop_entity(&f, "prop");
            let p = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            assert_eq!(p.valid_count, 2, "{:?}", p.rows);
            let prop = p.characters.iter().find(|e| e.name == "调阅签").unwrap();
            assert_eq!(
                (prop.kind.as_str(), prop.action.as_str()),
                ("prop", "create")
            );
            assert_eq!(p.rows[1].entity_names, vec!["Alice", "调阅签"]);
            let r = f.service.commit(request(&p, false)).await.unwrap();
            assert_eq!(r.outcome, "completed", "{r:?}");
            assert_eq!(r.created_entity_ids.len(), 2);
            let props = f
                .service
                .entities
                .list(
                    TEST_PROJECT,
                    Some(crate::domain::story_entity::StoryEntityKind::Prop),
                )
                .await
                .unwrap();
            assert_eq!(props.len(), 1);
            assert_eq!(props[0].name, "调阅签");
            assert_eq!(props[0].image_asset_ids.len(), 1);
            let names:Vec<String>=sqlx::query_scalar("SELECT a.original_name FROM shot_video_input_assets i JOIN assets a ON a.id=i.asset_id WHERE i.shot_id=? AND i.input_key='reference_images' ORDER BY i.ordinal").bind(&r.created_shot_ids[1]).fetch_all(&f.pool).await.unwrap();
            assert!(names.contains(&"prop.png".to_string()), "{names:?}");
            assert_no_execution(&f.pool).await;
            f.pool.close().await;
            let f = preview_fixture().await;
            add_prop_entity(&f, "item");
            let err = f
                .service
                .pick(TEST_PROJECT, f.root.clone())
                .await
                .unwrap_err();
            assert_eq!(err.code, "INVALID_MANIFEST");
            assert_eq!(count(&f.pool, "story_entities").await, 0);
            f.pool.close().await;
        }
        #[tokio::test]
        async fn manifest_composition_entity_is_created_and_bound_in_reference_mode() {
            let f = preview_fixture().await;
            let path = f.root.join("manifest.json");
            let mut value: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            value["characters"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "name": "剧情01", "kind": "composition", "images": ["images/layout01.png"]
                }));
            value["shots"][1]["prompt"] = "@Alice 按 @剧情01 构图走向镜头".into();
            std::fs::copy(
                f.root.join("images/a.png"),
                f.root.join("images/layout01.png"),
            )
            .unwrap();
            std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
            let p = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            assert_eq!(p.valid_count, 2, "{:?}", p.rows);
            let c = p.characters.iter().find(|e| e.name == "剧情01").unwrap();
            assert_eq!(
                (c.kind.as_str(), c.action.as_str()),
                ("composition", "create")
            );
            let r = f.service.commit(request(&p, false)).await.unwrap();
            assert_eq!(r.outcome, "completed", "{r:?}");
            let list = f
                .service
                .entities
                .list(
                    TEST_PROJECT,
                    Some(crate::domain::story_entity::StoryEntityKind::Composition),
                )
                .await
                .unwrap();
            assert_eq!(list.len(), 1);
            assert_eq!(list[0].name, "剧情01");
            let names:Vec<String>=sqlx::query_scalar("SELECT a.original_name FROM shot_video_input_assets i JOIN assets a ON a.id=i.asset_id WHERE i.shot_id=? AND i.input_key='reference_images' ORDER BY i.ordinal").bind(&r.created_shot_ids[1]).fetch_all(&f.pool).await.unwrap();
            assert!(names.contains(&"layout01.png".to_string()), "{names:?}");
            assert_no_execution(&f.pool).await;
            f.pool.close().await;
        }
        #[tokio::test]
        async fn detailed_composition_picture_hints_in_preview_and_readiness_without_blocking() {
            let f = preview_fixture().await;
            let path = f.root.join("manifest.json");
            let mut value: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            value["characters"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "name": "剧情01", "kind": "composition", "images": ["images/layout01.png"]
                }));
            value["shots"][1]["prompt"] = "@Alice 按 @剧情01 构图走向镜头".into();
            std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
            std::fs::write(
                f.root.join("images/layout01.png"),
                crate::application::shot_video_input_service::tests::noisy_png(),
            )
            .unwrap();
            let hint = "构图图「剧情01」细节较多，可能干扰场景，建议改用纯色背景、线稿或剪影";
            let p = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            assert_eq!(p.valid_count, 2, "{:?}", p.rows);
            assert!(p.warnings.iter().any(|w| w == hint), "{:?}", p.warnings);
            let r = f.service.commit(request(&p, false)).await.unwrap();
            assert_eq!(r.outcome, "completed", "{r:?}");
            let rows: Vec<(String, i64, String)> = sqlx::query_as("SELECT i.asset_id, i.ordinal, a.sha256 FROM shot_video_input_assets i JOIN assets a ON a.id=i.asset_id WHERE i.shot_id=? AND i.input_key='reference_images' ORDER BY i.ordinal").bind(&r.created_shot_ids[1]).fetch_all(&f.pool).await.unwrap();
            let set = crate::domain::shot_context::ResolvedVideoInputSet {
                workflow_version_id: "ref-version".into(),
                recipe_id: "recipe".into(),
                instance_id: None,
                revision: None,
                inputs: rows
                    .into_iter()
                    .map(|(asset_id, ordinal, sha256)| {
                        crate::domain::shot_context::ResolvedVideoInputAsset {
                            input_key: "reference_images".into(),
                            ordinal,
                            asset_id,
                            media_type: "image".into(),
                            sha256,
                            duration_ms: None,
                        }
                    })
                    .collect(),
            };
            let services = f.service.commit_services.as_ref().unwrap();
            assert_eq!(
                services
                    .inputs
                    .composition_detail_hints(TEST_PROJECT, &set)
                    .await,
                vec![hint.to_string()]
            );
            // Cached by content hash; still the same non-blocking hint.
            assert_eq!(
                services
                    .inputs
                    .composition_detail_hints(TEST_PROJECT, &set)
                    .await,
                vec![hint.to_string()]
            );
            assert_no_execution(&f.pool).await;
            f.pool.close().await;
        }
        fn add_composition_usage(f: &PreviewFixture, opening: &str, ending: &str, prompt: &str) {
            let path = f.root.join("manifest.json");
            let mut value: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            for name in ["开场", "结尾"] {
                value["characters"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({
                        "name": name, "kind": "composition", "images": [format!("images/{name}.png")]
                    }));
                std::fs::copy(
                    f.root.join("images/a.png"),
                    f.root.join(format!("images/{name}.png")),
                )
                .unwrap();
            }
            value["shots"][1]["prompt"] = prompt.into();
            value["shots"][1]["openingComposition"] = opening.into();
            value["shots"][1]["endingComposition"] = ending.into();
            std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        }
        #[tokio::test]
        async fn manifest_composition_usage_is_saved_and_invalid_names_rejected() {
            let f = preview_fixture().await;
            add_composition_usage(
                &f,
                "开场",
                "结尾",
                "按 @开场 的构图，@Alice 走近，最后停在 @结尾 的构图",
            );
            let p = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            assert_eq!(p.valid_count, 2, "{:?}", p.rows);
            assert_eq!(p.rows[1].opening_composition.as_deref(), Some("开场"));
            assert_eq!(p.rows[1].ending_composition.as_deref(), Some("结尾"));
            let r = f.service.commit(request(&p, false)).await.unwrap();
            assert_eq!(r.outcome, "completed", "{r:?}");
            let usage: Vec<(String, String)> = sqlx::query_as("SELECT e.name, b.composition_usage FROM shot_story_entity_bindings b JOIN story_entities e ON e.id=b.entity_id WHERE b.shot_id=? AND b.composition_usage IS NOT NULL ORDER BY b.composition_usage DESC")
                .bind(&r.created_shot_ids[1]).fetch_all(&f.pool).await.unwrap();
            assert_eq!(
                usage,
                vec![
                    ("开场".to_string(), "opening".to_string()),
                    ("结尾".to_string(), "ending".to_string())
                ]
            );
            assert_no_execution(&f.pool).await;
            f.pool.close().await;
            for (opening, ending, prompt, code) in [
                (
                    "不存在",
                    "结尾",
                    "@开场 @Alice @结尾",
                    "COMPOSITION_USAGE_UNKNOWN",
                ),
                (
                    "Alice",
                    "结尾",
                    "@开场 @Alice @结尾",
                    "COMPOSITION_USAGE_WRONG_KIND",
                ),
                (
                    "开场",
                    "开场",
                    "@开场 @Alice @结尾",
                    "COMPOSITION_USAGE_SAME",
                ),
                ("开场", "结尾", "@开场 @Alice", "COMPOSITION_USAGE_UNBOUND"),
            ] {
                let f = preview_fixture().await;
                add_composition_usage(&f, opening, ending, prompt);
                let p = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
                assert!(
                    p.rows[1].errors.iter().any(|e| e.code == code),
                    "{code}: {:?}",
                    p.rows[1].errors
                );
                assert!(
                    p.rows[1]
                        .errors
                        .iter()
                        .all(|e| !e.message.contains("角色")
                            || code == "COMPOSITION_USAGE_WRONG_KIND")
                );
                f.pool.close().await;
            }
        }
        #[tokio::test]
        async fn existing_same_name_entity_with_other_kind_is_rejected_before_writes() {
            let f = preview_fixture().await;
            f.service
                .entities
                .create(
                    TEST_PROJECT,
                    crate::domain::story_entity::StoryEntityKind::Character,
                    crate::domain::story_entity::StoryEntityUpsert {
                        name: "调阅签".into(),
                        description: String::new(),
                        cover_asset_id: None,
                        image_asset_ids: vec![],
                    },
                )
                .await
                .unwrap();
            add_prop_entity(&f, "prop");
            let err = f
                .service
                .pick(TEST_PROJECT, f.root.clone())
                .await
                .unwrap_err();
            assert_eq!(err.code, "INVALID_MANIFEST");
            assert_eq!(count(&f.pool, "shots").await, 0);
            assert_eq!(count(&f.pool, "assets").await, 0);
            f.pool.close().await;
        }
        #[tokio::test]
        async fn reused_entity_is_readonly_and_ignored_entity_images_are_never_imported() {
            let f = preview_fixture().await;
            let p = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            let r = f.service.commit(request(&p, false)).await.unwrap();
            assert_eq!(r.outcome, "completed");
            let before = f.service.entities.list(TEST_PROJECT, None).await.unwrap();
            std::fs::remove_file(f.root.join("images/alice.png")).unwrap();
            let p = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            let p = f
                .service
                .rescan(
                    &p.session_id,
                    PreviewOptions {
                        name_suffix: Some(" second".into()),
                        legacy: None,
                    },
                )
                .await
                .unwrap();
            assert_eq!(p.valid_count, 2);
            let r = f.service.commit(request(&p, false)).await.unwrap();
            assert_eq!(r.outcome, "completed", "{r:?}");
            assert!(r.created_entity_ids.is_empty());
            assert_eq!(r.reused_entity_ids.len(), 1);
            assert_eq!(r.imported_asset_ids.len(), 3);
            let after = f.service.entities.list(TEST_PROJECT, None).await.unwrap();
            assert_eq!(
                serde_json::to_value(before).unwrap(),
                serde_json::to_value(after).unwrap()
            );
            assert_no_execution(&f.pool).await;
            f.pool.close().await;
        }
        #[tokio::test]
        async fn formal_single_batch_limit_is_checked_before_any_writes() {
            let f = preview_fixture().await;
            let path = f.root.join("manifest.json");
            let mut m: Manifest = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            let row = m.shots[0].clone();
            m.shots = (0..101)
                .map(|i| {
                    let mut r = row.clone();
                    r.id = Some(format!("id{i}"));
                    r.name = Some(format!("shot{i}"));
                    r
                })
                .collect();
            std::fs::write(&path, serde_json::to_vec(&m).unwrap()).unwrap();
            let p = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            assert_eq!(p.valid_count, 101);
            assert_eq!(p.max_queue_items, 100);
            assert_eq!(
                f.service.commit(request(&p, true)).await.unwrap_err().code,
                "REFERENCE_LIMIT"
            );
            for table in ["shots", "assets", "story_entities", "production_batches"] {
                assert_eq!(count(&f.pool, table).await, 0);
            }
            assert_no_execution(&f.pool).await;
            f.pool.close().await;
        }
        #[test]
        fn source_storage_error_cannot_claim_verified_file_compensation() {
            let mut result = ImportResult::default();
            let e=source_failure(&mut result,super::super::source_asset_import_service::SourceAssetImportError::AssetPersistence{message:"private path".into()});
            assert!(result.residual.unverified_file_cleanup);
            assert_eq!(result.compensation_errors.len(), 1);
            assert_eq!(e.stage, "A");
            assert!(!e.message.contains("private"));
            let mut m = manifest();
            m.queue.name = Some("\n".into());
            assert!(parse_manifest(&serde_json::to_vec(&m).unwrap()).is_err());
        }
        fn legacy_options(pair: &Generator, width: i64, height: i64) -> PreviewOptions {
            let selection = crate::application::product::selection_ref::ExactGeneratorSelection {
                workflow_version_id: pair.workflow_version_id.clone(),
                recipe_id: pair.recipe_id.clone(),
            }
            .encode()
            .unwrap();
            PreviewOptions {
                name_suffix: None,
                legacy: Some(LegacyDefaults {
                    selection_ref: selection,
                    duration_seconds: 6,
                    width,
                    height,
                }),
            }
        }
        #[tokio::test]
        async fn legacy_image_and_reference_require_explicit_exact_generator_and_recheck() {
            for (file, mode, data) in [
                (
                    "h3-batch.json",
                    Mode::Image,
                    serde_json::json!([{"image":"images/a.png","prompt":"slow move"}]),
                ),
                (
                    "h3-omni-batch.json",
                    Mode::Reference,
                    serde_json::json!([{"images":["images/c.png"],"videos":[],"audios":[],"prompt":"slow move"}]),
                ),
            ] {
                let f = preview_fixture().await;
                let path = f.root.join("manifest.json");
                let manifest: Manifest =
                    serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
                std::fs::remove_file(&path).unwrap();
                std::fs::write(f.root.join(file), serde_json::to_vec(&data).unwrap()).unwrap();
                let p = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
                assert_ne!(p.format_source, "manifest");
                assert_eq!(p.valid_count, 0);
                assert!(p.warnings.iter().any(|w| w.contains("旧版 H3")));
                assert_eq!(count(&f.pool, "assets").await, 0);
                let row = &p.rows[0];
                let options = legacy_options(&manifest.generators[&mode], row.width, row.height);
                let p = f.service.rescan(&p.session_id, options).await.unwrap();
                assert_eq!(p.valid_count, 1, "{:?}", p.rows);
                let r = f.service.commit(request(&p, false)).await.unwrap();
                assert_eq!(r.outcome, "completed", "{r:?}");
                assert_eq!(r.created_shot_ids.len(), 1);
                assert_eq!(r.imported_asset_ids.len(), 1);
                assert_no_execution(&f.pool).await;
                f.pool.close().await;
            }
        }
        #[tokio::test]
        async fn legacy_media_ambiguity_and_modern_override_fail_closed() {
            let f = preview_fixture().await;
            let path = f.root.join("manifest.json");
            let original = std::fs::read(&path).unwrap();
            std::fs::remove_file(&path).unwrap();
            std::fs::write(f.root.join("h3-omni-batch.json"),br#"[{"images":["images/c.png"],"videos":["clip.mp4"],"audios":[],"prompt":"move"}]"#).unwrap();
            assert_eq!(
                f.service
                    .pick(TEST_PROJECT, f.root.clone())
                    .await
                    .unwrap_err()
                    .code,
                "UNSUPPORTED_LEGACY_MEDIA"
            );
            std::fs::write(
                f.root.join("h3-batch.json"),
                br#"[{"image":"images/a.png","prompt":"move"}]"#,
            )
            .unwrap();
            assert_eq!(
                f.service
                    .pick(TEST_PROJECT, f.root.clone())
                    .await
                    .unwrap_err()
                    .code,
                "INVALID_MANIFEST"
            );
            std::fs::write(&path, &original).unwrap();
            let p = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            assert_eq!(p.format_source, "manifest");
            assert_eq!(p.valid_count, 2);
            let m: Manifest = serde_json::from_slice(&original).unwrap();
            let options = legacy_options(
                &m.generators[&Mode::Image],
                p.rows[0].width,
                p.rows[0].height,
            );
            assert_eq!(
                f.service
                    .rescan(&p.session_id, options)
                    .await
                    .unwrap_err()
                    .code,
                "INVALID_MANIFEST"
            );
            std::fs::write(&path, b"{}").unwrap();
            assert_eq!(
                f.service
                    .pick(TEST_PROJECT, f.root.clone())
                    .await
                    .unwrap_err()
                    .code,
                "INVALID_MANIFEST"
            );
            assert_eq!(count(&f.pool, "assets").await, 0);
            assert_no_execution(&f.pool).await;
            f.pool.close().await;
        }
        struct RejectStart;
        #[async_trait::async_trait]
        impl ImportStart for RejectStart {
            async fn start(&self, _: &str, _: &str) -> Result<(), ImportFailure> {
                Err(failure("QUEUE_START_FAILED", "正式启动未准入"))
            }
        }
        #[tokio::test]
        async fn start_rejection_keeps_prepared_batch_and_never_compensates_or_executes() {
            let mut f = preview_fixture().await;
            f.service.commit_services.as_mut().unwrap().start = Arc::new(RejectStart);
            let p = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            let mut req = request(&p, true);
            req.auto_start = true;
            let r = f.service.commit(req).await.unwrap();
            assert_eq!(r.outcome, "completed");
            assert!(!r.auto_started);
            assert!(r.warnings.iter().any(|w| w.contains("QUEUE_START_FAILED")));
            assert!(r.batch_id.is_some());
            assert_eq!(count(&f.pool, "shots").await, 2);
            assert_no_execution(&f.pool).await;
            f.pool.close().await;
        }
        #[tokio::test]
        async fn occ_conflict_retains_the_concurrent_input_and_never_deletes_its_shot() {
            let f = preview_fixture().await;
            let p = f.service.pick(TEST_PROJECT, f.root.clone()).await.unwrap();
            f.service.faults.lock().unwrap().insert("D_OCC");
            let req = request(&p, true);
            let r = f.service.commit(req.clone()).await.unwrap();
            assert_eq!(r.outcome, "partial");
            assert_eq!(r.failure.unwrap().stage, "D");
            assert_eq!(r.residual.shot_ids, vec![r.created_shot_ids[0].clone()]);
            assert!(r.batch_id.is_none());
            let kept: Vec<String> = sqlx::query_scalar(
                "SELECT input_key FROM shot_video_input_assets WHERE shot_id=? ORDER BY ordinal",
            )
            .bind(&r.created_shot_ids[0])
            .fetch_all(&f.pool)
            .await
            .unwrap();
            assert_eq!(kept, vec!["first_frame"]);
            assert_eq!(count(&f.pool, "shots").await, 1);
            assert_no_execution(&f.pool).await;
            let again = f.service.commit(req).await.unwrap();
            assert_eq!(again.outcome, "partial");
            assert_eq!(again.created_shot_ids, r.created_shot_ids);
            assert_eq!(count(&f.pool, "shots").await, 1);
            f.pool.close().await;
        }
    }
}
