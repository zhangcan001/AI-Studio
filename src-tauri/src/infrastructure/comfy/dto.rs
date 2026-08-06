use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Default)]
pub struct SystemStatsDto {
    #[serde(default)]
    pub system: Option<SystemInfoDto>,
    #[serde(default)]
    pub devices: Vec<DeviceInfoDto>,
}

#[derive(Debug, Deserialize, Default)]
pub struct SystemInfoDto {
    #[serde(default)]
    pub comfyui_version: Option<String>,
    #[serde(default)]
    pub python_version: Option<String>,
    #[serde(default)]
    pub os: Option<String>,
    #[serde(default)]
    pub ram_total: Option<u64>,
    #[serde(default)]
    pub ram_free: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct DeviceInfoDto {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(rename = "type", default)]
    pub device_type: Option<String>,
    #[serde(default)]
    pub vram_total: Option<u64>,
    #[serde(default)]
    pub vram_free: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct PromptRequestDto {
    pub prompt: Value,
    pub client_id: String,
    pub prompt_id: String,
}

#[derive(Debug, Deserialize)]
pub struct PromptResponseDto {
    #[serde(default)]
    pub prompt_id: Option<String>,
    #[serde(default)]
    pub number: Option<Value>,
    #[serde(default)]
    pub node_errors: Option<Value>,
    #[serde(default)]
    pub error: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct UploadResponseDto {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub subfolder: Option<String>,
    #[serde(rename = "type", default)]
    pub folder_type: Option<String>,
}
