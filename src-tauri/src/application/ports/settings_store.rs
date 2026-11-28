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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_state: Option<WindowGeometry>,
    #[serde(default)]
    pub desktop_preferences: DesktopPreferences,
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
            window_state: None,
            desktop_preferences: DesktopPreferences::default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub maximized: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DesktopPreferences {
    #[serde(default)]
    pub auto_free_vram_between_items: bool,
    #[serde(default)]
    pub transient_retry_limit: u8,
    #[serde(default = "default_true")]
    pub desktop_notifications: bool,
    #[serde(default)]
    pub onboarding_dismissed: bool,
}

fn default_true() -> bool {
    true
}

impl Default for DesktopPreferences {
    fn default() -> Self {
        Self {
            auto_free_vram_between_items: false,
            transient_retry_limit: 0,
            desktop_notifications: true,
            onboarding_dismissed: false,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_header: Option<String>,
    #[serde(default)]
    pub allow_system_proxy: bool,
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
#[serde(rename_all = "camelCase")]
pub struct ComfySettings {
    pub endpoint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_header: Option<String>,
    #[serde(default)]
    pub allow_system_proxy: bool,
}

impl Default for ComfySettings {
    fn default() -> Self {
        Self {
            endpoint: "http://127.0.0.1:8188".to_owned(),
            auth_header: None,
            allow_system_proxy: false,
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
