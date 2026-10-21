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
    }
}
