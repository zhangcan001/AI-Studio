use crate::error::AppError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub schema_version: u32,
    pub comfy: ComfySettings,
    #[serde(default)]
    pub workspace_resume: WorkspaceResume,
    #[serde(default)]
    pub preferred_presets: BTreeMap<String, String>,
    #[serde(default)]
    pub runtime_profiles: Vec<RuntimeParameterProfile>,
    #[serde(default)]
    pub production_queue_name_presets: Vec<String>,
    #[serde(default)]
    pub comfy_environment_profiles: Vec<ComfyEnvironmentProfile>,
    #[serde(default)]
    pub batch_workflow_presets: Vec<BatchWorkflowPreset>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            comfy: ComfySettings::default(),
            workspace_resume: WorkspaceResume::default(),
            preferred_presets: BTreeMap::new(),
            runtime_profiles: Vec::new(),
            production_queue_name_presets: vec![
                "第01集 图片".to_owned(),
                "第01集 视频".to_owned(),
                "角色测试".to_owned(),
                "场景实验".to_owned(),
            ],
            comfy_environment_profiles: Vec::new(),
            batch_workflow_presets: Vec::new(),
        }
    }
}

/// A reusable local production configuration. It intentionally contains no
/// project-owned identifiers, media assets, or prompt/reference selections.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BatchWorkflowPreset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub image: Option<BatchWorkflowStagePreset>,
    pub video: Option<BatchWorkflowStagePreset>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BatchWorkflowStagePreset {
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub values: Value,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceResume {
    #[serde(default)]
    pub last_project_id: Option<String>,
    #[serde(default)]
    pub last_workspace: Option<String>,
    #[serde(default)]
    pub last_shot_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeParameterProfile {
    pub id: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub name: String,
    pub values: BTreeMap<String, i64>,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ComfyEnvironmentProfile {
    pub id: String,
    pub name: String,
    pub endpoint: String,
    pub created_at: String,
    pub updated_at: String,
}

pub fn preferred_preset_key(
    project_id: &str,
    workflow_version_id: &str,
    recipe_id: &str,
) -> String {
    format!("{project_id}\u{001f}{workflow_version_id}\u{001f}{recipe_id}")
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ComfySettings {
    pub endpoint: String,
}

impl Default for ComfySettings {
    fn default() -> Self {
        Self {
            endpoint: "http://127.0.0.1:8188".to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedSettings {
    pub settings: AppSettings,
    pub warning: Option<String>,
}

#[async_trait]
pub trait SettingsStore: Send + Sync {
    async fn load(&self) -> LoadedSettings;

    async fn save(&self, settings: &AppSettings) -> Result<(), AppError>;
}
