use crate::error::AppError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub schema_version: u32,
    pub comfy: ComfySettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            comfy: ComfySettings::default(),
        }
    }
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
