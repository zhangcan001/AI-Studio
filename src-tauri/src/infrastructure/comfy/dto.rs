use serde::Deserialize;

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
