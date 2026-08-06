use crate::application::ports::{
    ComfyAdapter, ComfyAdapterError, ComfyConnectionConfig, ComfyEventSubscription,
    ComfyExecutionEvent, ComfyHealth, ComfyHistory, ComfyNodeOutput, ComfyOutputData,
    ComfyOutputFile, DeviceInfo, PromptSubmission, SystemStats,
};
use crate::infrastructure::comfy::dto::{PromptRequestDto, PromptResponseDto, SystemStatsDto};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use reqwest::{header::CONTENT_TYPE, Client, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

const COMFY_HTTP_TIMEOUT: Duration = Duration::from_secs(5);
const COMFY_OUTPUT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_IMAGE_OUTPUT_BYTES: u64 = 256 * 1024 * 1024;

pub struct ComfyHttpAdapter {
    client: Client,
    config: ComfyConnectionConfig,
}

impl ComfyHttpAdapter {
    pub fn new(config: ComfyConnectionConfig) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .no_proxy()
            .timeout(COMFY_HTTP_TIMEOUT)
            .build()?;

        Ok(Self { client, config })
    }

    pub fn endpoint(&self) -> String {
        self.config.endpoint()
    }

    async fn get_json<T>(&self, route: &str) -> Result<T, ComfyAdapterError>
    where
        T: DeserializeOwned,
    {
        let endpoint = self.endpoint();
        let url = self.config.route_url(route);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|error| request_error("GET", &url, error))?;

        if !response.status().is_success() {
            return Err(http_status_error("GET", &url, response.status()));
        }

        response.json::<T>().await.map_err(|error| {
            ComfyAdapterError::Protocol(format!(
                "GET {endpoint}/{route} returned invalid JSON: {error}"
            ))
        })
    }

    async fn get_system_stats_internal(&self) -> Result<SystemStats, ComfyAdapterError> {
        let dto: SystemStatsDto = self.get_json("system_stats").await?;

        if dto.system.is_none() && dto.devices.is_empty() {
            return Err(ComfyAdapterError::Incompatible(
                "system_stats response has no system or device data".to_owned(),
            ));
        }

        let system = dto.system.unwrap_or_default();
        let devices = dto
            .devices
            .into_iter()
            .map(|device| DeviceInfo {
                name: device.name,
                device_type: device.device_type,
                vram_total: device.vram_total,
                vram_free: device.vram_free,
            })
            .collect();

        Ok(SystemStats {
            comfyui_version: system.comfyui_version,
            python_version: system.python_version,
            os: system.os,
            ram_total: system.ram_total,
            ram_free: system.ram_free,
            devices,
        })
    }

    async fn submit_workflow_internal(
        &self,
        client_id: &str,
        prompt_id: &str,
        workflow: Value,
    ) -> Result<PromptSubmission, ComfyAdapterError> {
        let url = self.config.route_url("prompt");
        let response = self
            .client
            .post(&url)
            .json(&PromptRequestDto {
                prompt: workflow,
                client_id: client_id.to_owned(),
                prompt_id: prompt_id.to_owned(),
            })
            .send()
            .await
            .map_err(|error| request_error("POST", &url, error))?;

        let status = response.status();
        let body = response.json::<Value>().await.map_err(|error| {
            ComfyAdapterError::Protocol(format!(
                "POST {url} returned invalid JSON with HTTP {status}: {error}"
            ))
        })?;
        let dto: PromptResponseDto = serde_json::from_value(body.clone()).map_err(|error| {
            ComfyAdapterError::Protocol(format!(
                "POST {url} returned an invalid prompt response: {error}"
            ))
        })?;

        if !status.is_success() {
            if status == StatusCode::BAD_REQUEST || dto.error.is_some() || dto.node_errors.is_some()
            {
                return Err(ComfyAdapterError::WorkflowValidation {
                    message: dto
                        .error
                        .as_ref()
                        .map(value_message)
                        .unwrap_or_else(|| "ComfyUI rejected the workflow".to_owned()),
                    node_errors: dto.node_errors.unwrap_or_else(|| serde_json::json!({})),
                });
            }
            return Err(http_status_error("POST", &url, status));
        }

        if let Some(error) = dto.error.as_ref() {
            return Err(ComfyAdapterError::WorkflowValidation {
                message: value_message(error),
                node_errors: dto.node_errors.unwrap_or_else(|| serde_json::json!({})),
            });
        }

        let response_prompt_id = dto.prompt_id.ok_or_else(|| {
            ComfyAdapterError::Protocol(
                "POST /prompt response did not contain prompt_id".to_owned(),
            )
        })?;
        if response_prompt_id != prompt_id {
            return Err(ComfyAdapterError::Protocol(format!(
                "POST /prompt prompt_id mismatch: requested {prompt_id}, received {response_prompt_id}"
            )));
        }

        Ok(PromptSubmission {
            prompt_id: response_prompt_id,
            number: number_to_i64(dto.number)?,
            node_errors: dto.node_errors.unwrap_or_else(|| serde_json::json!({})),
        })
    }

    async fn get_history_internal(
        &self,
        prompt_id: &str,
    ) -> Result<ComfyHistory, ComfyAdapterError> {
        let route = format!("history/{prompt_id}");
        let url = self.config.route_url(&route);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|error| request_error("GET", &url, error))?;
        if response.status() == StatusCode::NOT_FOUND {
            return Err(ComfyAdapterError::HistoryNotFound(prompt_id.to_owned()));
        }
        if !response.status().is_success() {
            return Err(http_status_error("GET", &url, response.status()));
        }
        let body = response.json::<Value>().await.map_err(|error| {
            ComfyAdapterError::Protocol(format!("GET {url} returned invalid history JSON: {error}"))
        })?;
        normalize_history(prompt_id, body)
    }

    async fn download_output_internal(
        &self,
        file: &ComfyOutputFile,
    ) -> Result<ComfyOutputData, ComfyAdapterError> {
        let url = self.config.route_url("view");
        let response = self
            .client
            .get(&url)
            .query(&[
                ("filename", file.filename.as_str()),
                ("subfolder", file.subfolder.as_str()),
                ("type", file.folder_type.as_str()),
            ])
            .timeout(COMFY_OUTPUT_TIMEOUT)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    ComfyAdapterError::OutputDownload(format!("GET {url} timed out: {error}"))
                } else {
                    ComfyAdapterError::OutputDownload(format!("GET {url} failed: {error}"))
                }
            })?;

        if !response.status().is_success() {
            return Err(ComfyAdapterError::OutputDownload(format!(
                "GET {url} returned HTTP {}",
                response.status()
            )));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_IMAGE_OUTPUT_BYTES)
        {
            return Err(ComfyAdapterError::OutputTooLarge(format!(
                "Content-Length exceeds {} bytes",
                MAX_IMAGE_OUTPUT_BYTES
            )));
        }

        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let bytes = response.bytes().await.map_err(|error| {
            ComfyAdapterError::OutputDownload(format!("GET {url} body read failed: {error}"))
        })?;
        if bytes.len() as u64 > MAX_IMAGE_OUTPUT_BYTES {
            return Err(ComfyAdapterError::OutputTooLarge(format!(
                "response body exceeds {} bytes",
                MAX_IMAGE_OUTPUT_BYTES
            )));
        }

        Ok(ComfyOutputData {
            bytes: bytes.to_vec(),
            content_type,
        })
    }
}

#[async_trait]
impl ComfyAdapter for ComfyHttpAdapter {
    async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
        Ok(ComfyHealth {
            system: self.get_system_stats().await?,
        })
    }

    async fn get_system_stats(&self) -> Result<SystemStats, ComfyAdapterError> {
        self.get_system_stats_internal().await
    }

    async fn get_object_info(&self) -> Result<Value, ComfyAdapterError> {
        let object_info: Value = self.get_json("object_info").await?;

        if !object_info.is_object() {
            return Err(ComfyAdapterError::Incompatible(
                "object_info response is not a JSON object".to_owned(),
            ));
        }

        Ok(object_info)
    }

    async fn get_history(&self, prompt_id: &str) -> Result<ComfyHistory, ComfyAdapterError> {
        self.get_history_internal(prompt_id).await
    }

    async fn download_output(
        &self,
        file: &ComfyOutputFile,
    ) -> Result<ComfyOutputData, ComfyAdapterError> {
        self.download_output_internal(file).await
    }

    async fn submit_workflow(
        &self,
        client_id: &str,
        prompt_id: &str,
        workflow: Value,
    ) -> Result<PromptSubmission, ComfyAdapterError> {
        self.submit_workflow_internal(client_id, prompt_id, workflow)
            .await
    }

    async fn subscribe_events(
        &self,
        client_id: &str,
    ) -> Result<Box<dyn ComfyEventSubscription>, ComfyAdapterError> {
        let url = self.config.websocket_url(client_id);
        let (stream, _) = connect_async(&url).await.map_err(|error| {
            ComfyAdapterError::StreamDisconnected(format!("connect {url}: {error}"))
        })?;
        Ok(Box::new(ComfyWsEventSubscription { stream }))
    }
}

fn normalize_history(prompt_id: &str, body: Value) -> Result<ComfyHistory, ComfyAdapterError> {
    let root = body.as_object().ok_or_else(|| {
        ComfyAdapterError::Protocol("GET /history response must be a JSON object".to_owned())
    })?;
    let history = root
        .get(prompt_id)
        .ok_or_else(|| ComfyAdapterError::HistoryNotFound(prompt_id.to_owned()))?;
    let history = history.as_object().ok_or_else(|| {
        ComfyAdapterError::Protocol("history prompt entry must be a JSON object".to_owned())
    })?;
    let outputs = history
        .get("outputs")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            ComfyAdapterError::Protocol("history outputs must be a JSON object".to_owned())
        })?;

    let mut normalized = std::collections::BTreeMap::new();
    for (node_id, node_value) in outputs {
        let node = node_value.as_object().ok_or_else(|| {
            ComfyAdapterError::Protocol(format!("history output node {node_id} must be an object"))
        })?;
        let Some(images_value) = node.get("images") else {
            normalized.insert(node_id.clone(), ComfyNodeOutput { images: Vec::new() });
            continue;
        };
        let images = images_value.as_array().ok_or_else(|| {
            ComfyAdapterError::Protocol(format!(
                "history output node {node_id} images must be an array"
            ))
        })?;
        let mut files = Vec::with_capacity(images.len());
        for image in images {
            let image = image.as_object().ok_or_else(|| {
                ComfyAdapterError::Protocol(format!(
                    "history output node {node_id} image must be an object"
                ))
            })?;
            let filename = image
                .get("filename")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    ComfyAdapterError::Protocol(format!(
                        "history output node {node_id} image filename is missing"
                    ))
                })?;
            files.push(ComfyOutputFile {
                filename: filename.to_owned(),
                subfolder: image
                    .get("subfolder")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                folder_type: image
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("output")
                    .to_owned(),
            });
        }
        normalized.insert(node_id.clone(), ComfyNodeOutput { images: files });
    }

    Ok(ComfyHistory {
        prompt_id: prompt_id.to_owned(),
        outputs: normalized,
    })
}

type ComfyWebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct ComfyWsEventSubscription {
    stream: ComfyWebSocket,
}

#[async_trait]
impl ComfyEventSubscription for ComfyWsEventSubscription {
    async fn next_event(&mut self) -> Result<Option<ComfyExecutionEvent>, ComfyAdapterError> {
        loop {
            let Some(message) = self.stream.next().await else {
                return Err(ComfyAdapterError::StreamDisconnected(
                    "ComfyUI WebSocket stream ended".to_owned(),
                ));
            };
            let message = message.map_err(|error| {
                ComfyAdapterError::StreamDisconnected(format!("WebSocket read failed: {error}"))
            })?;

            match message {
                Message::Text(text) => {
                    let value = serde_json::from_str::<Value>(text.as_ref()).map_err(|error| {
                        ComfyAdapterError::Protocol(format!(
                            "ComfyUI WebSocket returned malformed JSON: {error}"
                        ))
                    })?;
                    if let Some(event) = normalize_event(value)? {
                        return Ok(Some(event));
                    }
                }
                Message::Binary(_) => {
                    tracing::debug!("ignoring ComfyUI binary WebSocket frame");
                }
                Message::Ping(payload) => {
                    self.stream
                        .send(Message::Pong(payload))
                        .await
                        .map_err(|error| {
                            ComfyAdapterError::StreamDisconnected(format!(
                                "WebSocket pong failed: {error}"
                            ))
                        })?;
                }
                Message::Pong(_) => {}
                Message::Close(frame) => {
                    let message = frame
                        .map(|frame| format!("WebSocket close {}", frame.code))
                        .unwrap_or_else(|| "WebSocket closed by ComfyUI".to_owned());
                    return Err(ComfyAdapterError::StreamDisconnected(message));
                }
                Message::Frame(_) => {}
            }
        }
    }
}

fn normalize_event(value: Value) -> Result<Option<ComfyExecutionEvent>, ComfyAdapterError> {
    let event_type = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| ComfyAdapterError::Protocol("WebSocket event type is missing".to_owned()))?;

    if matches!(event_type, "status" | "execution_cached" | "executed") {
        tracing::debug!(event_type, "ignoring non-execution ComfyUI event");
        return Ok(None);
    }

    let data = match value.get("data") {
        Some(data) => data,
        None if matches!(
            event_type,
            "execution_start"
                | "executing"
                | "progress"
                | "execution_success"
                | "execution_error"
                | "execution_interrupted"
        ) =>
        {
            return Err(ComfyAdapterError::Protocol(format!(
                "WebSocket event {event_type} has no data"
            )));
        }
        None => {
            tracing::debug!(event_type, "ignoring unknown ComfyUI event without data");
            return Ok(None);
        }
    };

    match event_type {
        "execution_start" => Ok(Some(ComfyExecutionEvent::ExecutionStarted {
            prompt_id: required_prompt_id(data)?,
        })),
        "executing" => {
            let prompt_id = required_prompt_id(data)?;
            match data.get("node") {
                Some(Value::Null) | None => {
                    tracing::debug!(prompt_id, "ignoring ComfyUI executing completion hint");
                    Ok(None)
                }
                Some(Value::String(node_id)) => Ok(Some(ComfyExecutionEvent::NodeStarted {
                    prompt_id,
                    node_id: node_id.clone(),
                })),
                Some(_) => Err(ComfyAdapterError::Protocol(
                    "executing event node must be a string or null".to_owned(),
                )),
            }
        }
        "progress" => Ok(Some(ComfyExecutionEvent::Progress {
            prompt_id: required_prompt_id(data)?,
            node_id: optional_string(data, "node")?,
            current: required_u64(data, "value")?,
            total: required_u64(data, "max")?,
        })),
        "execution_success" => Ok(Some(ComfyExecutionEvent::ExecutionSucceeded {
            prompt_id: required_prompt_id(data)?,
        })),
        "execution_error" => Ok(Some(ComfyExecutionEvent::ExecutionError {
            prompt_id: required_prompt_id(data)?,
            node_id: optional_string_from(data, &["node_id", "node"])?,
            message: data
                .get("exception_message")
                .or_else(|| data.get("message"))
                .map(value_message)
                .unwrap_or_else(|| "ComfyUI execution error".to_owned()),
            raw: value,
        })),
        "execution_interrupted" => Ok(Some(ComfyExecutionEvent::ExecutionInterrupted {
            prompt_id: required_prompt_id(data)?,
            node_id: optional_string_from(data, &["node_id", "node"])?,
            raw: value,
        })),
        unknown => {
            tracing::debug!(event_type = unknown, "ignoring unknown ComfyUI event");
            Ok(None)
        }
    }
}

fn required_prompt_id(data: &Value) -> Result<String, ComfyAdapterError> {
    data.get("prompt_id")
        .and_then(Value::as_str)
        .filter(|prompt_id| !prompt_id.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            ComfyAdapterError::Protocol("WebSocket event prompt_id is missing".to_owned())
        })
}

fn optional_string(data: &Value, field: &str) -> Result<Option<String>, ComfyAdapterError> {
    match data.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(ComfyAdapterError::Protocol(format!(
            "WebSocket event {field} must be a string or null"
        ))),
    }
}

fn optional_string_from(
    data: &Value,
    fields: &[&str],
) -> Result<Option<String>, ComfyAdapterError> {
    for field in fields {
        if data.get(field).is_some() {
            return optional_string(data, field);
        }
    }
    Ok(None)
}

fn required_u64(data: &Value, field: &str) -> Result<u64, ComfyAdapterError> {
    data.get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| ComfyAdapterError::Protocol(format!("WebSocket event {field} is invalid")))
}

fn number_to_i64(value: Option<Value>) -> Result<Option<i64>, ComfyAdapterError> {
    let Some(value) = value else {
        return Ok(None);
    };
    match value {
        Value::Null => Ok(None),
        Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                return Ok(Some(value));
            }
            if let Some(value) = number.as_u64() {
                return i64::try_from(value).map(Some).map_err(|_| {
                    ComfyAdapterError::Protocol("prompt response number is out of range".to_owned())
                });
            }
            if let Some(value) = number.as_f64() {
                if value.is_finite()
                    && value.fract() == 0.0
                    && value >= i64::MIN as f64
                    && value <= i64::MAX as f64
                {
                    return Ok(Some(value as i64));
                }
            }
            Err(ComfyAdapterError::Protocol(
                "prompt response number is not an integer".to_owned(),
            ))
        }
        _ => Err(ComfyAdapterError::Protocol(
            "prompt response number is not numeric".to_owned(),
        )),
    }
}

fn value_message(value: &Value) -> String {
    match value {
        Value::String(message) => message.clone(),
        Value::Object(object) => object
            .get("message")
            .or_else(|| object.get("exception_message"))
            .or_else(|| object.get("type"))
            .map(value_message)
            .unwrap_or_else(|| value.to_string()),
        _ => value.to_string(),
    }
}

fn request_error(method: &str, url: &str, error: reqwest::Error) -> ComfyAdapterError {
    if error.is_timeout() {
        ComfyAdapterError::Timeout(format!("{method} {url}: {error}"))
    } else if error.is_connect() {
        ComfyAdapterError::Offline(format!("{method} {url}: {error}"))
    } else {
        ComfyAdapterError::Offline(format!("{method} {url}: {error}"))
    }
}

fn http_status_error(method: &str, url: &str, status: StatusCode) -> ComfyAdapterError {
    let message = format!("{method} {url} returned HTTP {status}");
    if matches!(
        status,
        StatusCode::BAD_GATEWAY | StatusCode::GATEWAY_TIMEOUT | StatusCode::SERVICE_UNAVAILABLE
    ) {
        ComfyAdapterError::Offline(message)
    } else {
        ComfyAdapterError::Incompatible(message)
    }
}

#[cfg(test)]
mod tests {
    use super::{ComfyHttpAdapter, MAX_IMAGE_OUTPUT_BYTES};
    use crate::application::ports::{
        ComfyAdapter, ComfyAdapterError, ComfyConnectionConfig, ComfyExecutionEvent,
        ComfyOutputFile,
    };
    use futures_util::SinkExt;
    use serde_json::json;
    use std::net::TcpListener;
    use tokio::io::AsyncWriteExt;
    use tokio_tungstenite::{accept_async, tungstenite::Message};
    use wiremock::{
        matchers::{body_json, method, path, query_param},
        Mock, MockServer, ResponseTemplate,
    };

    fn config_for(server: &MockServer) -> ComfyConnectionConfig {
        ComfyConnectionConfig::new("http", "127.0.0.1", server.address().port())
    }

    #[tokio::test]
    async fn parses_system_stats_with_device_vram() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/system_stats"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "system": {
                    "comfyui_version": "test-version",
                    "python_version": "3.12.0",
                    "os": "windows"
                },
                "devices": [{
                    "name": "Test GPU",
                    "type": "cuda",
                    "vram_total": 17179869184u64,
                    "vram_free": 8589934592u64
                }]
            })))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        let stats = adapter
            .get_system_stats()
            .await
            .expect("stats should parse");

        assert_eq!(stats.comfyui_version.as_deref(), Some("test-version"));
        assert_eq!(stats.devices[0].name.as_deref(), Some("Test GPU"));
        assert_eq!(stats.devices[0].vram_total, Some(17179869184));
        assert_eq!(stats.devices[0].vram_free, Some(8589934592));
    }

    #[tokio::test]
    async fn html_response_is_incompatible() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/system_stats"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>not comfyui</html>"))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        let error = adapter
            .get_system_stats()
            .await
            .expect_err("HTML should not parse as ComfyUI stats");

        assert!(matches!(error, ComfyAdapterError::Protocol(_)));
    }

    #[tokio::test]
    async fn connection_refused_or_timeout_is_offline() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("temporary port should bind");
        let port = listener
            .local_addr()
            .expect("local address should resolve")
            .port();
        drop(listener);

        let adapter = ComfyHttpAdapter::new(ComfyConnectionConfig::new("http", "127.0.0.1", port))
            .expect("client should build");
        let error = adapter
            .get_system_stats()
            .await
            .expect_err("closed port should be offline");

        assert!(matches!(
            error,
            ComfyAdapterError::Offline(_) | ComfyAdapterError::Timeout(_)
        ));
    }

    #[tokio::test]
    async fn object_info_is_read_as_backend_value() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/object_info"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "KSampler": {},
                "CLIPTextEncode": {},
                "SaveImage": {}
            })))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        let object_info = adapter
            .get_object_info()
            .await
            .expect("object info should parse");

        assert_eq!(object_info.as_object().expect("object expected").len(), 3);
    }

    #[tokio::test]
    async fn history_normalizes_multiple_images_and_ignores_unknown_fields() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/history/prompt-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "prompt-1": {
                    "outputs": {
                        "9": {
                            "images": [
                                {"filename": "a.png", "subfolder": "", "type": "output", "unknown": true},
                                {"filename": "b.png", "subfolder": "nested", "type": "temp"}
                            ],
                            "audio": [{"filename": "ignored.wav"}]
                        },
                        "3": {"text": ["ignored"]}
                    },
                    "status": {"completed": true}
                },
                "other-prompt": {"outputs": {}}
            })))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        let history = adapter
            .get_history("prompt-1")
            .await
            .expect("history should parse");
        assert_eq!(history.prompt_id, "prompt-1");
        assert_eq!(history.outputs["9"].images.len(), 2);
        assert_eq!(history.outputs["9"].images[1].subfolder, "nested");
        assert!(history.outputs["3"].images.is_empty());
    }

    #[tokio::test]
    async fn history_missing_prompt_is_history_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/history/missing"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"other": {"outputs": {}}})),
            )
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        assert!(matches!(
            adapter.get_history("missing").await,
            Err(ComfyAdapterError::HistoryNotFound(prompt_id)) if prompt_id == "missing"
        ));
    }

    #[tokio::test]
    async fn malformed_history_is_protocol_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/history/prompt-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "prompt-1": {"outputs": {"9": {"images": "not-array"}}}
            })))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        assert!(matches!(
            adapter.get_history("prompt-1").await,
            Err(ComfyAdapterError::Protocol(_))
        ));
    }

    #[tokio::test]
    async fn output_download_uses_view_query_and_returns_bytes_and_content_type() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/view"))
            .and(query_param("filename", "ComfyUI_00001.png"))
            .and(query_param("subfolder", "nested"))
            .and(query_param("type", "output"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "image/png")
                    .set_body_bytes(vec![1, 2, 3]),
            )
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        let output = adapter
            .download_output(&ComfyOutputFile {
                filename: "ComfyUI_00001.png".to_owned(),
                subfolder: "nested".to_owned(),
                folder_type: "output".to_owned(),
            })
            .await
            .expect("output should download");
        assert_eq!(output.bytes, vec![1, 2, 3]);
        assert_eq!(output.content_type.as_deref(), Some("image/png"));
    }

    #[tokio::test]
    async fn output_download_rejects_content_length_over_image_limit() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("listener should bind");
        let port = listener.local_addr().expect("listener address").port();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("request should arrive");
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: image/png\r\n\r\n",
                        MAX_IMAGE_OUTPUT_BYTES + 1
                    )
                    .as_bytes(),
                )
                .await
                .expect("headers should write");
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        });

        let adapter = ComfyHttpAdapter::new(ComfyConnectionConfig::new("http", "127.0.0.1", port))
            .expect("client should build");
        let error = adapter
            .download_output(&ComfyOutputFile {
                filename: "large.png".to_owned(),
                subfolder: String::new(),
                folder_type: "output".to_owned(),
            })
            .await
            .expect_err("large output should be rejected");
        assert!(matches!(error, ComfyAdapterError::OutputTooLarge(_)));
        server.await.expect("server should finish");
    }

    #[tokio::test]
    async fn submits_prompt_and_validates_requested_prompt_id() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/prompt"))
            .and(body_json(json!({
                "prompt": {"3": {}},
                "client_id": "client-1",
                "prompt_id": "prompt-1"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "prompt_id": "prompt-1",
                "number": 7,
                "node_errors": {}
            })))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        let submission = adapter
            .submit_workflow("client-1", "prompt-1", json!({"3": {}}))
            .await
            .expect("prompt should submit");

        assert_eq!(submission.prompt_id, "prompt-1");
        assert_eq!(submission.number, Some(7));
        assert_eq!(submission.node_errors, json!({}));
    }

    #[tokio::test]
    async fn maps_prompt_validation_failure_without_hiding_node_errors() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/prompt"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                "error": {"message": "missing model"},
                "node_errors": {"3": {"errors": ["model missing"]}}
            })))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        let error = adapter
            .submit_workflow("client-1", "prompt-1", json!({"3": {}}))
            .await
            .expect_err("validation failure should be returned");

        assert!(matches!(
            error,
            ComfyAdapterError::WorkflowValidation { node_errors, .. }
                if node_errors["3"]["errors"][0] == "model missing"
        ));
    }

    #[tokio::test]
    async fn rejects_prompt_id_mismatch_as_protocol_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/prompt"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "prompt_id": "server-prompt",
                "number": 1
            })))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        let error = adapter
            .submit_workflow("client-1", "requested-prompt", json!({"3": {}}))
            .await
            .expect_err("mismatched prompt id should fail");

        assert!(
            matches!(error, ComfyAdapterError::Protocol(message) if message.contains("mismatch"))
        );
    }

    #[test]
    fn websocket_url_translates_http_and_https_protocols() {
        assert_eq!(
            ComfyConnectionConfig::new("http", "localhost", 8188).websocket_url("client-1"),
            "ws://localhost:8188/ws?clientId=client-1"
        );
        assert_eq!(
            ComfyConnectionConfig::new("https", "example.test", 443).websocket_url("client-1"),
            "wss://example.test:443/ws?clientId=client-1"
        );
    }

    #[tokio::test]
    async fn websocket_mock_server_normalizes_execution_events() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("websocket listener should bind");
        let port = listener
            .local_addr()
            .expect("websocket address should resolve")
            .port();
        let server = tokio::spawn(async move {
            let (stream, _) = listener
                .accept()
                .await
                .expect("websocket client should connect");
            let mut socket = accept_async(stream)
                .await
                .expect("websocket handshake should succeed");
            socket
                .send(Message::Text(
                    json!({
                        "type": "execution_start",
                        "data": {"prompt_id": "prompt-1"}
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .expect("start event should send");
            socket
                .send(Message::Text(
                    json!({
                        "type": "progress",
                        "data": {
                            "prompt_id": "prompt-1",
                            "node": "3",
                            "value": 1,
                            "max": 20
                        }
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .expect("progress event should send");
        });

        let adapter = ComfyHttpAdapter::new(ComfyConnectionConfig::new("http", "127.0.0.1", port))
            .expect("client should build");
        let mut subscription = adapter
            .subscribe_events("client-1")
            .await
            .expect("websocket should connect");

        assert_eq!(
            subscription.next_event().await.expect("start should parse"),
            Some(ComfyExecutionEvent::ExecutionStarted {
                prompt_id: "prompt-1".to_owned()
            })
        );
        assert_eq!(
            subscription
                .next_event()
                .await
                .expect("progress should parse"),
            Some(ComfyExecutionEvent::Progress {
                prompt_id: "prompt-1".to_owned(),
                node_id: Some("3".to_owned()),
                current: 1,
                total: 20,
            })
        );

        server.await.expect("mock websocket server should finish");
    }

    #[tokio::test]
    async fn executed_and_unknown_events_are_ignored_but_malformed_json_is_protocol_error() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("websocket listener should bind");
        let port = listener
            .local_addr()
            .expect("websocket address should resolve")
            .port();
        let server = tokio::spawn(async move {
            let (stream, _) = listener
                .accept()
                .await
                .expect("websocket client should connect");
            let mut socket = accept_async(stream)
                .await
                .expect("websocket handshake should succeed");
            socket
                .send(Message::Text(
                    json!({"type": "executed", "data": {"prompt_id": "prompt-1"}})
                        .to_string()
                        .into(),
                ))
                .await
                .expect("executed event should send");
            socket
                .send(Message::Text(
                    json!({"type": "custom_node_event", "data": {}})
                        .to_string()
                        .into(),
                ))
                .await
                .expect("unknown event should send");
            socket
                .send(Message::Text("not-json".to_owned().into()))
                .await
                .expect("malformed event should send");
        });

        let adapter = ComfyHttpAdapter::new(ComfyConnectionConfig::new("http", "127.0.0.1", port))
            .expect("client should build");
        let mut subscription = adapter
            .subscribe_events("client-1")
            .await
            .expect("websocket should connect");
        let error = subscription
            .next_event()
            .await
            .expect_err("malformed JSON should be protocol error");

        assert!(matches!(error, ComfyAdapterError::Protocol(_)));
        server.await.expect("mock websocket server should finish");
    }

    #[test]
    fn executing_null_is_only_a_completion_hint_and_unknown_without_data_is_ignored() {
        assert_eq!(
            super::normalize_event(json!({
                "type": "executing",
                "data": {"prompt_id": "prompt-1", "node": null}
            }))
            .expect("executing null should normalize"),
            None
        );
        assert_eq!(
            super::normalize_event(json!({"type": "custom_event"}))
                .expect("unknown event should be ignored"),
            None
        );
    }
}
