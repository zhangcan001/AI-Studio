use async_trait::async_trait;
use serde_json::Value;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComfyConnectionConfig {
    pub protocol: String,
    pub host: String,
    pub port: u16,
}

impl ComfyConnectionConfig {
    pub fn new(protocol: impl Into<String>, host: impl Into<String>, port: u16) -> Self {
        Self {
            protocol: protocol.into(),
            host: host.into(),
            port,
        }
    }

    pub fn endpoint(&self) -> String {
        format!("{}://{}:{}", self.protocol, self.host, self.port)
    }

    pub fn route_url(&self, route: &str) -> String {
        format!(
            "{}/{}",
            self.endpoint().trim_end_matches('/'),
            route.trim_start_matches('/')
        )
    }
}

impl Default for ComfyConnectionConfig {
    fn default() -> Self {
        Self::new("http", "127.0.0.1", 8188)
    }
}

#[derive(Clone, Debug)]
pub struct ComfyHealth {
    pub system: SystemStats,
}

#[derive(Clone, Debug)]
pub struct SystemStats {
    pub comfyui_version: Option<String>,
    pub python_version: Option<String>,
    pub os: Option<String>,
    pub ram_total: Option<u64>,
    pub ram_free: Option<u64>,
    pub devices: Vec<DeviceInfo>,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vram_total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vram_free: Option<u64>,
}

#[derive(Clone, Debug)]
pub enum ComfyAdapterError {
    Offline(String),
    Timeout(String),
    Incompatible(String),
    Protocol(String),
}

impl ComfyAdapterError {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Offline(_) => "OFFLINE",
            Self::Timeout(_) => "TIMEOUT",
            Self::Incompatible(_) => "INCOMPATIBLE",
            Self::Protocol(_) => "PROTOCOL_ERROR",
        }
    }
}

impl fmt::Display for ComfyAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Offline(message) => write!(formatter, "ComfyUI is offline: {message}"),
            Self::Timeout(message) => write!(formatter, "ComfyUI request timed out: {message}"),
            Self::Incompatible(message) => {
                write!(formatter, "ComfyUI API is incompatible: {message}")
            }
            Self::Protocol(message) => {
                write!(formatter, "ComfyUI API response is invalid: {message}")
            }
        }
    }
}

impl std::error::Error for ComfyAdapterError {}

#[async_trait]
pub trait ComfyAdapter: Send + Sync {
    async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError>;

    async fn get_system_stats(&self) -> Result<SystemStats, ComfyAdapterError>;

    async fn get_object_info(&self) -> Result<Value, ComfyAdapterError>;
}
