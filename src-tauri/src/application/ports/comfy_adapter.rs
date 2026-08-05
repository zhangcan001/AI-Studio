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

    pub fn websocket_url(&self, client_id: &str) -> String {
        let protocol = match self.protocol.as_str() {
            "http" => "ws",
            "https" => "wss",
            other => other,
        };
        format!(
            "{protocol}://{}:{}/ws?clientId={client_id}",
            self.host, self.port
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

#[derive(Clone, Debug, PartialEq)]
pub struct PromptSubmission {
    pub prompt_id: String,
    pub number: Option<i64>,
    pub node_errors: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ComfyExecutionEvent {
    ExecutionStarted {
        prompt_id: String,
    },
    NodeStarted {
        prompt_id: String,
        node_id: String,
    },
    Progress {
        prompt_id: String,
        node_id: Option<String>,
        current: u64,
        total: u64,
    },
    ExecutionSucceeded {
        prompt_id: String,
    },
    ExecutionError {
        prompt_id: String,
        node_id: Option<String>,
        message: String,
        raw: Value,
    },
    ExecutionInterrupted {
        prompt_id: String,
        node_id: Option<String>,
        raw: Value,
    },
}

impl ComfyExecutionEvent {
    pub fn prompt_id(&self) -> &str {
        match self {
            Self::ExecutionStarted { prompt_id }
            | Self::ExecutionSucceeded { prompt_id }
            | Self::NodeStarted { prompt_id, .. }
            | Self::Progress { prompt_id, .. }
            | Self::ExecutionError { prompt_id, .. }
            | Self::ExecutionInterrupted { prompt_id, .. } => prompt_id,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ComfyAdapterError {
    Offline(String),
    Timeout(String),
    Incompatible(String),
    Protocol(String),
    WorkflowValidation { message: String, node_errors: Value },
    StreamDisconnected(String),
}

impl ComfyAdapterError {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Offline(_) => "OFFLINE",
            Self::Timeout(_) => "TIMEOUT",
            Self::Incompatible(_) => "INCOMPATIBLE",
            Self::Protocol(_) => "PROTOCOL_ERROR",
            Self::WorkflowValidation { .. } => "WORKFLOW_VALIDATION",
            Self::StreamDisconnected(_) => "STREAM_DISCONNECTED",
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
            Self::WorkflowValidation {
                message,
                node_errors,
            } => write!(
                formatter,
                "ComfyUI rejected the workflow: {message}; node_errors={node_errors}"
            ),
            Self::StreamDisconnected(message) => {
                write!(formatter, "ComfyUI WebSocket disconnected: {message}")
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

    async fn submit_workflow(
        &self,
        client_id: &str,
        prompt_id: &str,
        workflow: Value,
    ) -> Result<PromptSubmission, ComfyAdapterError>;

    async fn subscribe_events(
        &self,
        client_id: &str,
    ) -> Result<Box<dyn ComfyEventSubscription>, ComfyAdapterError>;
}

#[async_trait]
pub trait ComfyEventSubscription: Send {
    async fn next_event(&mut self) -> Result<Option<ComfyExecutionEvent>, ComfyAdapterError>;
}
