use crate::application::ports::settings_store::{BatchWorkflowPreset, BatchWorkflowStagePreset};
use crate::application::ports::{AvailableGenerationDefinition, GenerationDefinitionRepository};
use crate::application::{
    project_template_service::{sanitize_values, ProjectTemplateError},
    settings_service::SettingsService,
};
use crate::error::AppError;
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::sync::Arc;
use uuid::Uuid;

const MAX_PRESETS: usize = 30;
const MAX_NAME_CHARS: usize = 80;
const MAX_DESCRIPTION_CHARS: usize = 500;
const UNAVAILABLE_REASON: &str = "WORKFLOW_UNAVAILABLE";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BatchWorkflowPresetInput {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub image: Option<BatchWorkflowStagePreset>,
    #[serde(default)]
    pub video: Option<BatchWorkflowStagePreset>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BatchWorkflowPresetView {
    #[serde(flatten)]
    pub preset: BatchWorkflowPreset,
    pub available: bool,
    pub reason: Option<String>,
}

pub struct BatchWorkflowPresetService {
    settings: Arc<SettingsService>,
    definitions: Arc<dyn GenerationDefinitionRepository>,
}

impl BatchWorkflowPresetService {
    pub fn new(
        settings: Arc<SettingsService>,
        definitions: Arc<dyn GenerationDefinitionRepository>,
    ) -> Self {
        Self {
            settings,
            definitions,
        }
    }

    pub async fn list(&self) -> Result<Vec<BatchWorkflowPresetView>, AppError> {
        let definitions = self.available_definitions().await?;
        Ok(self
            .settings
            .batch_workflow_presets()
            .into_iter()
            .map(|preset| view_with_availability(preset, &definitions))
            .collect())
    }

    pub async fn create(
        &self,
        input: BatchWorkflowPresetInput,
    ) -> Result<BatchWorkflowPresetView, AppError> {
        let definitions = self.available_definitions().await?;
        let mut presets = self.settings.batch_workflow_presets();
        if presets.len() >= MAX_PRESETS {
            return Err(AppError::invalid_input(
                "BATCH_WORKFLOW_PRESET_LIMIT_REACHED: 最多保存 30 个批量工作流预设。",
            ));
        }

        let (name, description) = normalize_name_and_description(&input)?;
        ensure_unique_name(&presets, &name, None)?;
        let (image, video) = sanitize_stages(input.image, input.video, &definitions)?;
        let now = now_string();
        let preset = BatchWorkflowPreset {
            id: format!("bwp_{}", Uuid::new_v4().simple()),
            name,
            description,
            image,
            video,
            created_at: now.clone(),
            updated_at: now,
        };
        presets.insert(0, preset.clone());
        self.settings.save_batch_workflow_presets(presets).await?;
        Ok(view_with_availability(preset, &definitions))
    }

    pub async fn update(
        &self,
        preset_id: &str,
        input: BatchWorkflowPresetInput,
    ) -> Result<BatchWorkflowPresetView, AppError> {
        let definitions = self.available_definitions().await?;
        let preset_id = require_id(preset_id, "BATCH_WORKFLOW_PRESET_ID")?;
        let mut presets = self.settings.batch_workflow_presets();
        let index = presets
            .iter()
            .position(|preset| preset.id == preset_id)
            .ok_or_else(|| {
                AppError::invalid_input(format!(
                    "BATCH_WORKFLOW_PRESET_NOT_FOUND: 预设 {} 不存在。",
                    preset_id
                ))
            })?;
        let (name, description) = normalize_name_and_description(&input)?;
        ensure_unique_name(&presets, &name, Some(&preset_id))?;
        let (image, video) = sanitize_stages(input.image, input.video, &definitions)?;
        let current = &presets[index];
        let updated = BatchWorkflowPreset {
            id: current.id.clone(),
            name,
            description,
            image,
            video,
            created_at: current.created_at.clone(),
            updated_at: now_string(),
        };
        presets[index] = updated.clone();
        self.settings.save_batch_workflow_presets(presets).await?;
        Ok(view_with_availability(updated, &definitions))
    }

    pub async fn delete(&self, preset_id: &str) -> Result<(), AppError> {
        let preset_id = require_id(preset_id, "BATCH_WORKFLOW_PRESET_ID")?;
        let mut presets = self.settings.batch_workflow_presets();
        let original_len = presets.len();
        presets.retain(|preset| preset.id != preset_id);
        if presets.len() == original_len {
            return Err(AppError::invalid_input(format!(
                "BATCH_WORKFLOW_PRESET_NOT_FOUND: 预设 {} 不存在。",
                preset_id
            )));
        }
        self.settings.save_batch_workflow_presets(presets).await
    }

    async fn available_definitions(&self) -> Result<Vec<AvailableGenerationDefinition>, AppError> {
        self.definitions
            .list_available()
            .await
            .map_err(|error| AppError::database(format!("工作流可用性读取失败：{error}")))
    }
}

fn normalize_name_and_description(
    input: &BatchWorkflowPresetInput,
) -> Result<(String, String), AppError> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARS {
        return Err(AppError::invalid_input(
            "BATCH_WORKFLOW_PRESET_NAME_INVALID: 名称必须为 1–80 个字符。",
        ));
    }
    if name.contains(['\r', '\n']) {
        return Err(AppError::invalid_input(
            "BATCH_WORKFLOW_PRESET_NAME_INVALID: 名称不能包含换行。",
        ));
    }
    let description = input.description.trim();
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(AppError::invalid_input(
            "BATCH_WORKFLOW_PRESET_DESCRIPTION_TOO_LONG: 描述最多 500 个字符。",
        ));
    }
    if input.image.is_none() && input.video.is_none() {
        return Err(AppError::invalid_input(
            "BATCH_WORKFLOW_PRESET_STAGE_REQUIRED: 至少需要 Image 或 Video 阶段。",
        ));
    }
    Ok((name.to_owned(), description.to_owned()))
}

fn ensure_unique_name(
    presets: &[BatchWorkflowPreset],
    name: &str,
    current_id: Option<&str>,
) -> Result<(), AppError> {
    let normalized = name.to_lowercase();
    if presets.iter().any(|preset| {
        Some(preset.id.as_str()) != current_id && preset.name.to_lowercase() == normalized
    }) {
        return Err(AppError::invalid_input(
            "BATCH_WORKFLOW_PRESET_NAME_CONFLICT: 预设名称不能重复。",
        ));
    }
    Ok(())
}

fn sanitize_stages(
    image: Option<BatchWorkflowStagePreset>,
    video: Option<BatchWorkflowStagePreset>,
    definitions: &[AvailableGenerationDefinition],
) -> Result<
    (
        Option<BatchWorkflowStagePreset>,
        Option<BatchWorkflowStagePreset>,
    ),
    AppError,
> {
    Ok((
        image
            .map(|stage| sanitize_stage(stage, definitions))
            .transpose()?,
        video
            .map(|stage| sanitize_stage(stage, definitions))
            .transpose()?,
    ))
}

fn sanitize_stage(
    mut stage: BatchWorkflowStagePreset,
    definitions: &[AvailableGenerationDefinition],
) -> Result<BatchWorkflowStagePreset, AppError> {
    stage.workflow_version_id = require_id(
        &stage.workflow_version_id,
        "BATCH_WORKFLOW_PRESET_WORKFLOW_VERSION",
    )?;
    stage.recipe_id = require_id(&stage.recipe_id, "BATCH_WORKFLOW_PRESET_RECIPE")?;
    let definition = definitions
        .iter()
        .find(|definition| {
            definition.workflow_version_id == stage.workflow_version_id
                && definition.recipe_id == stage.recipe_id
        })
        .ok_or_else(|| {
            AppError::invalid_input("WORKFLOW_UNAVAILABLE: Workflow Version 或 Recipe 当前不可用。")
        })?;
    let sanitized = sanitize_values(&definition.recipe_yaml, &stage.values)
        .map_err(project_template_error_to_app_error)?;
    stage.values = remove_project_owned_values(sanitized);
    Ok(stage)
}

fn project_template_error_to_app_error(error: ProjectTemplateError) -> AppError {
    AppError::invalid_input(format!("BATCH_WORKFLOW_PRESET_VALUES_INVALID: {error}"))
}

fn remove_project_owned_values(value: Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .into_iter()
                .filter(|(key, _)| !is_project_owned_key(key))
                .map(|(key, value)| (key, remove_project_owned_values(value)))
                .collect::<Map<_, _>>(),
        ),
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(remove_project_owned_values)
                .collect(),
        ),
        other => other,
    }
}

fn is_project_owned_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect::<String>();
    matches!(
        normalized.as_str(),
        "promptentryid"
            | "promptversionid"
            | "anchorid"
            | "assetid"
            | "assetids"
            | "sceneid"
            | "shotid"
            | "selectedasset"
            | "selectedassetid"
            | "selectedimage"
            | "selectedimageid"
            | "referenceassetid"
            | "referenceassetids"
            | "orderedreferenceassetids"
            | "imageassetid"
            | "videoassetid"
            | "audioassetid"
    )
}

fn view_with_availability(
    preset: BatchWorkflowPreset,
    definitions: &[AvailableGenerationDefinition],
) -> BatchWorkflowPresetView {
    let image_available = preset
        .image
        .as_ref()
        .is_none_or(|stage| stage_is_available(stage, definitions));
    let video_available = preset
        .video
        .as_ref()
        .is_none_or(|stage| stage_is_available(stage, definitions));
    let available = image_available && video_available;
    BatchWorkflowPresetView {
        preset,
        available,
        reason: (!available).then(|| UNAVAILABLE_REASON.to_owned()),
    }
}

fn stage_is_available(
    stage: &BatchWorkflowStagePreset,
    definitions: &[AvailableGenerationDefinition],
) -> bool {
    definitions.iter().any(|definition| {
        definition.workflow_version_id == stage.workflow_version_id
            && definition.recipe_id == stage.recipe_id
    })
}

fn require_id(value: &str, field: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::invalid_input(format!(
            "{field}_INVALID: ID 不能为空。"
        )));
    }
    Ok(value.to_owned())
}

fn now_string() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::{
        is_project_owned_key, remove_project_owned_values, sanitize_stage, view_with_availability,
        BatchWorkflowPresetInput, MAX_DESCRIPTION_CHARS, MAX_NAME_CHARS,
    };
    use crate::application::ports::settings_store::{
        BatchWorkflowPreset, BatchWorkflowStagePreset,
    };
    use crate::application::ports::{AppSettings, AvailableGenerationDefinition};
    use serde_json::json;

    #[test]
    fn reusable_values_drop_project_owned_and_selected_media_keys() {
        let value = remove_project_owned_values(json!({
            "steps": {"type": "integer", "value": 20},
            "promptEntryId": "prm_1",
            "anchor_id": "anc_1",
            "selectedImageId": "ast_1",
            "nested": {"shotId": "sht_1", "keep": true}
        }));
        assert_eq!(
            value,
            json!({
                "steps": {"type": "integer", "value": 20},
                "nested": {"keep": true}
            })
        );
    }

    #[test]
    fn forbidden_key_matching_is_explicit_not_a_broad_reference_filter() {
        assert!(is_project_owned_key("referenceAssetIds"));
        assert!(!is_project_owned_key("reference_strength"));
    }

    #[test]
    fn legacy_settings_default_to_empty_presets_without_schema_bump() {
        let settings: AppSettings = serde_json::from_value(json!({
            "schemaVersion": 1,
            "comfy": {"endpoint": "http://127.0.0.1:8188"}
        }))
        .unwrap();
        assert_eq!(settings.schema_version, 1);
        assert!(settings.batch_workflow_presets.is_empty());
    }

    #[test]
    fn input_serializes_stage_names_in_camel_case() {
        let input = BatchWorkflowPresetInput {
            name: "Batch".to_owned(),
            description: "Reusable".to_owned(),
            image: Some(BatchWorkflowStagePreset {
                workflow_version_id: "wfv_1".to_owned(),
                recipe_id: "rcp_1".to_owned(),
                values: json!({}),
            }),
            video: None,
        };
        let value = serde_json::to_value(input).unwrap();
        assert_eq!(value["image"]["workflowVersionId"], "wfv_1");
        assert_eq!(value["image"]["recipeId"], "rcp_1");
        assert!(MAX_NAME_CHARS >= 80);
        assert!(MAX_DESCRIPTION_CHARS >= 500);
    }

    #[test]
    fn stage_sanitization_reuses_recipe_scalar_rules_and_strips_media() {
        let definition = AvailableGenerationDefinition {
            workflow_id: "wf_1".to_owned(),
            workflow_version_id: "wfv_1".to_owned(),
            recipe_id: "rcp_1".to_owned(),
            recipe_version: "1".to_owned(),
            name: "Image".to_owned(),
            category: "generated_image".to_owned(),
            mode: "t2i".to_owned(),
            recipe_yaml: "schema_version: 1\nid: r\nname: R\nworkflow:\n  file: w.json\ninputs:\n  steps:\n    type: integer\n    label: Steps\n    default: 10\n  image:\n    type: image\n    label: Image\n    required: false\n  asset_id:\n    type: textarea\n    label: Asset\n    required: false\nbindings: []\noutputs: []\n".to_owned(),
        };
        let stage = sanitize_stage(
            BatchWorkflowStagePreset {
                workflow_version_id: " wfv_1 ".to_owned(),
                recipe_id: "rcp_1".to_owned(),
                values: json!({
                    "steps": {"type": "integer", "value": 16},
                    "image": {"type": "image_asset", "assetId": "ast_1"},
                    "asset_id": {"type": "string", "value": "ast_2"}
                }),
            },
            &[definition],
        )
        .unwrap();
        assert_eq!(stage.values["steps"]["value"], 16);
        assert!(stage.values.get("image").is_none());
        assert!(stage.values.get("asset_id").is_none());
    }

    #[test]
    fn unavailable_legacy_stage_is_reported_without_deleting_it() {
        let preset = BatchWorkflowPreset {
            id: "bwp_old".to_owned(),
            name: "旧预设".to_owned(),
            description: String::new(),
            image: Some(BatchWorkflowStagePreset {
                workflow_version_id: "missing".to_owned(),
                recipe_id: "missing".to_owned(),
                values: json!({}),
            }),
            video: None,
            created_at: "2026-08-18T00:00:00Z".to_owned(),
            updated_at: "2026-08-18T00:00:00Z".to_owned(),
        };
        let view = view_with_availability(preset.clone(), &[]);
        assert!(!view.available);
        assert_eq!(view.reason.as_deref(), Some("WORKFLOW_UNAVAILABLE"));
        assert_eq!(view.preset.id, preset.id);
    }
}
