use async_trait::async_trait;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fmt,
    sync::{Arc, RwLock},
};
use url::Url;

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

    pub fn from_endpoint(endpoint: &str) -> Result<Self, ComfyEndpointError> {
        let endpoint = endpoint.trim();
        if endpoint.is_empty() {
            return Err(ComfyEndpointError::invalid("地址不能为空".to_owned()));
        }
        let parsed = Url::parse(endpoint)
            .map_err(|error| ComfyEndpointError::invalid(format!("地址格式无效：{error}")))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(ComfyEndpointError::invalid(
                "仅支持 http:// 或 https:// 地址".to_owned(),
            ));
        }
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(ComfyEndpointError::invalid(
                "ComfyUI 地址不能包含用户名或密码".to_owned(),
            ));
        }
        if parsed.query().is_some() || parsed.fragment().is_some() {
            return Err(ComfyEndpointError::invalid(
                "ComfyUI 地址不能包含查询参数或片段".to_owned(),
            ));
        }
        if !parsed.path().is_empty() && parsed.path() != "/" {
            return Err(ComfyEndpointError::invalid(
                "ComfyUI 地址不能包含 API 路径".to_owned(),
            ));
        }
        let host = parsed
            .host_str()
            .filter(|host| !host.trim().is_empty())
            .ok_or_else(|| ComfyEndpointError::invalid("ComfyUI 地址缺少主机名".to_owned()))?;
        let port = parsed
            .port_or_known_default()
            .ok_or_else(|| ComfyEndpointError::invalid("ComfyUI 地址缺少有效端口".to_owned()))?;
        let host = if host.contains(':') && !host.starts_with('[') {
            format!("[{host}]")
        } else {
            host.to_owned()
        };
        Ok(Self::new(parsed.scheme(), host, port))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComfyEndpointError {
    message: String,
}

impl fmt::Display for ComfyEndpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ComfyEndpointError {}

impl ComfyEndpointError {
    fn invalid(message: String) -> Self {
        Self { message }
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CancelPromptResult {
    CancellationRequested,
    NotFoundOrAlreadyFinished,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComfyQueueState {
    pub running_prompt_ids: Vec<String>,
    pub pending_prompt_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComfyHistoryStatus {
    pub status_str: Option<String>,
    pub completed: Option<bool>,
    pub messages: Option<Value>,
}

impl Default for ComfyHistoryStatus {
    fn default() -> Self {
        Self {
            status_str: None,
            completed: None,
            messages: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComfyHistory {
    pub prompt_id: String,
    pub status: ComfyHistoryStatus,
    pub outputs: BTreeMap<String, ComfyNodeOutput>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComfyNodeOutput {
    pub images: Vec<ComfyOutputFile>,
    pub saved_results: Vec<ComfySavedResult>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComfySavedResult {
    pub file: ComfyOutputFile,
    pub animated: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComfyOutputFile {
    pub filename: String,
    pub subfolder: String,
    pub folder_type: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComfyOutputData {
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
}

#[async_trait]
pub trait ComfyOutputStream: Send {
    fn content_type(&self) -> Option<&str>;
    fn content_length(&self) -> Option<u64>;
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, ComfyAdapterError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComfyImageUpload {
    pub bytes: Vec<u8>,
    pub upload_name: String,
    pub content_type: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComfyUploadedInput {
    pub name: String,
    pub subfolder: String,
    pub folder_type: String,
}

pub type ComfyUploadedImage = ComfyUploadedInput;

#[async_trait]
pub trait ComfyInputStream: Send {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, String>;
}

pub struct ComfyInputUpload {
    pub filename: String,
    pub content_type: String,
    pub content_length: Option<u64>,
    pub stream: Box<dyn ComfyInputStream>,
}

struct InMemoryComfyInputStream {
    bytes: Option<Vec<u8>>,
}

#[async_trait]
impl ComfyInputStream for InMemoryComfyInputStream {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, String> {
        Ok(self.bytes.take())
    }
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
    WorkflowValidation {
        message: String,
        node_errors: Value,
    },
    StreamDisconnected(String),
    HistoryNotFound(String),
    OutputDownload(String),
    OutputTooLarge(String),
    #[allow(dead_code)]
    ImageUpload(String),
    InputUpload(String),
    InputUploadTooLarge(String),
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
            Self::HistoryNotFound(_) => "HISTORY_NOT_FOUND",
            Self::OutputDownload(_) => "OUTPUT_DOWNLOAD_FAILED",
            Self::OutputTooLarge(_) => "OUTPUT_TOO_LARGE",
            Self::ImageUpload(_) => "IMAGE_UPLOAD_FAILED",
            Self::InputUpload(_) => "INPUT_UPLOAD_FAILED",
            Self::InputUploadTooLarge(_) => "INPUT_UPLOAD_TOO_LARGE",
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
            Self::HistoryNotFound(prompt_id) => {
                write!(
                    formatter,
                    "ComfyUI history was not found for prompt {prompt_id}"
                )
            }
            Self::OutputDownload(message) => {
                write!(formatter, "ComfyUI output download failed: {message}")
            }
            Self::OutputTooLarge(message) => {
                write!(formatter, "ComfyUI output is too large: {message}")
            }
            Self::ImageUpload(message) => {
                write!(formatter, "ComfyUI image upload failed: {message}")
            }
            Self::InputUpload(message) => {
                write!(formatter, "ComfyUI input upload failed: {message}")
            }
            Self::InputUploadTooLarge(message) => {
                write!(formatter, "ComfyUI input upload is too large: {message}")
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

    async fn upload_input_file(
        &self,
        upload: ComfyInputUpload,
    ) -> Result<ComfyUploadedInput, ComfyAdapterError> {
        let _ = upload;
        Err(ComfyAdapterError::Incompatible(
            "generic input upload is not supported by this adapter".to_owned(),
        ))
    }

    async fn upload_image(
        &self,
        upload: ComfyImageUpload,
    ) -> Result<ComfyUploadedImage, ComfyAdapterError> {
        let content_length = upload.bytes.len() as u64;
        self.upload_input_file(ComfyInputUpload {
            filename: upload.upload_name,
            content_type: upload.content_type,
            content_length: Some(content_length),
            stream: Box::new(InMemoryComfyInputStream {
                bytes: Some(upload.bytes),
            }),
        })
        .await
    }

    async fn cancel_prompt(
        &self,
        prompt_id: &str,
    ) -> Result<CancelPromptResult, ComfyAdapterError> {
        let _ = prompt_id;
        Err(ComfyAdapterError::Incompatible(
            "prompt cancellation is not supported by this adapter".to_owned(),
        ))
    }

    async fn get_queue_state(&self) -> Result<ComfyQueueState, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible(
            "queue inspection is not supported by this adapter".to_owned(),
        ))
    }

    async fn get_history(&self, prompt_id: &str) -> Result<ComfyHistory, ComfyAdapterError>;

    async fn download_output(
        &self,
        file: &ComfyOutputFile,
    ) -> Result<ComfyOutputData, ComfyAdapterError>;

    async fn open_output_stream(
        &self,
        _file: &ComfyOutputFile,
    ) -> Result<Box<dyn ComfyOutputStream>, ComfyAdapterError> {
        Err(ComfyAdapterError::Incompatible(
            "streaming output download is not supported by this adapter".to_owned(),
        ))
    }

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

pub trait ComfyAdapterFactory: Send + Sync {
    fn create(
        &self,
        config: ComfyConnectionConfig,
    ) -> Result<Arc<dyn ComfyAdapter>, ComfyAdapterError>;
}

#[async_trait]
pub trait ComfyEventSubscription: Send {
    async fn next_event(&mut self) -> Result<Option<ComfyExecutionEvent>, ComfyAdapterError>;
}

/// A stable adapter reference shared by every application service.
///
/// Requests clone the current adapter before awaiting, so replacing the
/// endpoint never holds a lock across an HTTP or WebSocket operation.
pub struct ComfyAdapterHandle {
    current: RwLock<Arc<dyn ComfyAdapter>>,
}

impl ComfyAdapterHandle {
    pub fn new(adapter: Arc<dyn ComfyAdapter>) -> Self {
        Self {
            current: RwLock::new(adapter),
        }
    }

    pub fn replace(&self, adapter: Arc<dyn ComfyAdapter>) {
        *self
            .current
            .write()
            .unwrap_or_else(|error| error.into_inner()) = adapter;
    }

    pub fn current(&self) -> Arc<dyn ComfyAdapter> {
        self.current
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
}

#[async_trait]
impl ComfyAdapter for ComfyAdapterHandle {
    async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
        self.current().health_check().await
    }

    async fn get_system_stats(&self) -> Result<SystemStats, ComfyAdapterError> {
        self.current().get_system_stats().await
    }

    async fn get_object_info(&self) -> Result<Value, ComfyAdapterError> {
        self.current().get_object_info().await
    }

    async fn upload_input_file(
        &self,
        upload: ComfyInputUpload,
    ) -> Result<ComfyUploadedInput, ComfyAdapterError> {
        self.current().upload_input_file(upload).await
    }

    async fn upload_image(
        &self,
        upload: ComfyImageUpload,
    ) -> Result<ComfyUploadedImage, ComfyAdapterError> {
        self.current().upload_image(upload).await
    }

    async fn cancel_prompt(
        &self,
        prompt_id: &str,
    ) -> Result<CancelPromptResult, ComfyAdapterError> {
        self.current().cancel_prompt(prompt_id).await
    }

    async fn get_queue_state(&self) -> Result<ComfyQueueState, ComfyAdapterError> {
        self.current().get_queue_state().await
    }

    async fn get_history(&self, prompt_id: &str) -> Result<ComfyHistory, ComfyAdapterError> {
        self.current().get_history(prompt_id).await
    }

    async fn download_output(
        &self,
        file: &ComfyOutputFile,
    ) -> Result<ComfyOutputData, ComfyAdapterError> {
        self.current().download_output(file).await
    }

    async fn open_output_stream(
        &self,
        file: &ComfyOutputFile,
    ) -> Result<Box<dyn ComfyOutputStream>, ComfyAdapterError> {
        self.current().open_output_stream(file).await
    }

    async fn submit_workflow(
        &self,
        client_id: &str,
        prompt_id: &str,
        workflow: Value,
    ) -> Result<PromptSubmission, ComfyAdapterError> {
        self.current()
            .submit_workflow(client_id, prompt_id, workflow)
            .await
    }

    async fn subscribe_events(
        &self,
        client_id: &str,
    ) -> Result<Box<dyn ComfyEventSubscription>, ComfyAdapterError> {
        self.current().subscribe_events(client_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TaggedAdapter(&'static str);

    #[async_trait]
    impl ComfyAdapter for TaggedAdapter {
        async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
            Ok(ComfyHealth {
                system: SystemStats {
                    comfyui_version: Some(self.0.to_owned()),
                    python_version: None,
                    os: None,
                    ram_total: None,
                    ram_free: None,
                    devices: Vec::new(),
                },
            })
        }

        async fn get_system_stats(&self) -> Result<SystemStats, ComfyAdapterError> {
            Ok(self.health_check().await?.system)
        }

        async fn get_object_info(&self) -> Result<Value, ComfyAdapterError> {
            Ok(serde_json::json!({ self.0: {} }))
        }

        async fn get_history(&self, _prompt_id: &str) -> Result<ComfyHistory, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
        }

        async fn download_output(
            &self,
            _file: &ComfyOutputFile,
        ) -> Result<ComfyOutputData, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
        }

        async fn submit_workflow(
            &self,
            _client_id: &str,
            _prompt_id: &str,
            _workflow: Value,
        ) -> Result<PromptSubmission, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
        }

        async fn subscribe_events(
            &self,
            _client_id: &str,
        ) -> Result<Box<dyn ComfyEventSubscription>, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
        }
    }

    #[tokio::test]
    async fn shared_handle_routes_each_request_to_the_current_adapter() {
        let handle = ComfyAdapterHandle::new(Arc::new(TaggedAdapter("A")));
        assert_eq!(
            handle.health_check().await.unwrap().system.comfyui_version,
            Some("A".to_owned())
        );
        handle.replace(Arc::new(TaggedAdapter("B")));
        assert_eq!(
            handle.health_check().await.unwrap().system.comfyui_version,
            Some("B".to_owned())
        );
        assert!(handle.get_object_info().await.unwrap().get("B").is_some());
    }
}
