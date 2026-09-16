use crate::application::ports::{
    CancelPromptResult, ComfyAdapter, ComfyAdapterError, ComfyAdapterFactory,
    ComfyConnectionConfig, ComfyEventSubscription, ComfyExecutionEvent, ComfyHealth, ComfyHistory,
    ComfyHistoryStatus, ComfyImageUpload, ComfyInputUpload, ComfyNodeOutput, ComfyOutputData,
    ComfyOutputFile, ComfyOutputStream, ComfyQueueState, ComfySavedResult, ComfyUploadContext,
    ComfyUploadedInput, DeviceInfo, PromptSubmission, SystemStats,
};
use crate::infrastructure::comfy::dto::{
    CancelResponseDto, PromptRequestDto, PromptResponseDto, SystemStatsDto, UploadResponseDto,
};
use async_trait::async_trait;
use futures_util::{SinkExt, Stream, StreamExt};
use reqwest::{
    header::{CONTENT_TYPE, RETRY_AFTER},
    multipart::{Form, Part},
    Body, Client, StatusCode,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

const COMFY_HTTP_TIMEOUT: Duration = Duration::from_secs(5);
// Queue inspection and model-memory control can briefly block while ComfyUI
// finishes bookkeeping or unloads a large model. Keep ordinary requests
// responsive, but give these control-plane operations a wider window.
const COMFY_CONTROL_TIMEOUT: Duration = Duration::from_secs(30);
// GPU telemetry can briefly block while ComfyUI is loading or releasing a
// model; health checks should not classify a live endpoint as offline after
// the ordinary request timeout.
const COMFY_HEALTH_TIMEOUT: Duration = Duration::from_secs(30);
const COMFY_OUTPUT_TIMEOUT: Duration = Duration::from_secs(30);
// Input uploads may include large media and wait for a busy ComfyUI while it
// loads nodes/models. A 60s budget was still too short after the full request
// body had been sent, so keep a bounded but practical three-minute budget
// without widening ordinary HTTP requests.
const COMFY_UPLOAD_TIMEOUT: Duration = Duration::from_secs(180);
const COMFY_UPLOAD_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_IMAGE_UPLOAD_ATTEMPTS: usize = 3;
const IMAGE_UPLOAD_RETRY_DELAYS: [Duration; 2] =
    [Duration::from_millis(200), Duration::from_millis(800)];
const MAX_IMAGE_OUTPUT_BYTES: u64 = 256 * 1024 * 1024;

pub struct ComfyHttpAdapter {
    client: Client,
    upload_client: Client,
    config: ComfyConnectionConfig,
}

impl ComfyHttpAdapter {
    pub fn new(config: ComfyConnectionConfig) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .no_proxy()
            .timeout(COMFY_HTTP_TIMEOUT)
            .build()?;
        // ComfyUI may close long-idle keep-alive sockets while reqwest still
        // considers them reusable. Generic media uploads remain non-replayable,
        // so a stale pooled socket can block until the full upload timeout
        // instead of being safely retried. Do not retain idle upload
        // connections; ordinary control and execution requests keep pooling.
        let upload_client = Client::builder()
            .no_proxy()
            .pool_max_idle_per_host(0)
            .connect_timeout(COMFY_UPLOAD_CONNECT_TIMEOUT)
            .timeout(COMFY_HTTP_TIMEOUT)
            .build()?;

        Ok(Self {
            client,
            upload_client,
            config,
        })
    }

    pub fn endpoint(&self) -> String {
        self.config.endpoint()
    }

    async fn get_json_with_timeout<T>(
        &self,
        route: &str,
        timeout: Duration,
    ) -> Result<T, ComfyAdapterError>
    where
        T: DeserializeOwned,
    {
        let endpoint = self.endpoint();
        let url = self.config.route_url(route);
        let response = self
            .client
            .get(&url)
            .timeout(timeout)
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
        let dto: SystemStatsDto = self
            .get_json_with_timeout("system_stats", COMFY_HEALTH_TIMEOUT)
            .await?;

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

    async fn upload_input_file_internal(
        &self,
        upload: ComfyInputUpload,
    ) -> Result<ComfyUploadedInput, ComfyAdapterError> {
        let url = self.config.route_url("upload/image");
        let attempt_started_at = Instant::now();
        let total_started_at = attempt_started_at;
        let expected_bytes = upload.content_length;
        let upload_filename = upload.filename.clone();
        let upload_context = upload.context.clone();
        let bytes_read = Arc::new(AtomicU64::new(0));
        let bytes_read_for_body = Arc::clone(&bytes_read);
        let stream_error = Arc::new(Mutex::new(None::<String>));
        let stream_error_for_body = Arc::clone(&stream_error);
        let (task_id, asset_id, source_bytes, width, height) =
            upload_context_fields(upload_context.as_ref());
        let source_bytes = source_bytes.or(expected_bytes);
        tracing::debug!(
            phase = "preflight",
            task_id = %task_id,
            asset_id = %asset_id,
            filename = %upload_filename,
            source_bytes = ?source_bytes,
            upload_bytes = 0u64,
            width = ?width,
            height = ?height,
            attempt = 1usize,
            attempt_elapsed_ms = 0u64,
            total_elapsed_ms = 0u64,
            http_status = Option::<u16>::None,
            error_class = "",
            "prepared ComfyUI input upload request"
        );
        let body_stream = futures_util::stream::unfold(upload.stream, move |mut stream| {
            let stream_error = Arc::clone(&stream_error_for_body);
            let bytes_read = Arc::clone(&bytes_read_for_body);
            async move {
                match stream.next_chunk().await {
                    Ok(Some(chunk)) => {
                        bytes_read.fetch_add(chunk.len() as u64, Ordering::Relaxed);
                        Some((Ok::<Vec<u8>, std::io::Error>(chunk), stream))
                    }
                    Ok(None) => None,
                    Err(error) => {
                        if let Ok(mut stored) = stream_error.lock() {
                            *stored = Some(error.clone());
                        }
                        Some((
                            Err(std::io::Error::new(std::io::ErrorKind::Other, error)),
                            stream,
                        ))
                    }
                }
            }
        });
        let body = Body::wrap_stream(body_stream);
        let part = match upload.content_length {
            Some(length) => Part::stream_with_length(body, length),
            None => Part::stream(body),
        }
        .file_name(upload.filename)
        .mime_str(&upload.content_type)
        .map_err(|error| {
            ComfyAdapterError::InputUpload(format!("invalid input MIME type: {error}"))
        })?;
        let form = Form::new()
            .part("image", part)
            .text("type", "input")
            .text("subfolder", String::new())
            .text("overwrite", "false");
        tracing::debug!(
            phase = "request_start",
            task_id = %task_id,
            asset_id = %asset_id,
            filename = %upload_filename,
            source_bytes = ?source_bytes,
            upload_bytes = bytes_read.load(Ordering::Relaxed),
            width = ?width,
            height = ?height,
            attempt = 1usize,
            attempt_elapsed_ms = attempt_started_at.elapsed().as_millis() as u64,
            total_elapsed_ms = total_started_at.elapsed().as_millis() as u64,
            http_status = Option::<u16>::None,
            error_class = "",
            "starting ComfyUI input upload request"
        );
        let response = self
            .upload_client
            .post(&url)
            .timeout(COMFY_UPLOAD_TIMEOUT)
            .multipart(form)
            .send()
            .await
            .map_err(|error| {
                let stream_message = stream_error.lock().ok().and_then(|value| value.clone());
                let mapped = match stream_message.as_deref() {
                    Some(message) => {
                        ComfyAdapterError::InputUpload(format!("stream failed: {message}"))
                    }
                    None => request_error("POST", &url, error),
                };
                tracing::warn!(
                    phase = "request_start",
                    task_id = %task_id,
                    asset_id = %asset_id,
                    filename = %upload_filename,
                    source_bytes = ?source_bytes,
                    upload_bytes = bytes_read.load(Ordering::Relaxed),
                    width = ?width,
                    height = ?height,
                    attempt = 1usize,
                    attempt_elapsed_ms = attempt_started_at.elapsed().as_millis() as u64,
                    total_elapsed_ms = total_started_at.elapsed().as_millis() as u64,
                    http_status = Option::<u16>::None,
                    error_class = mapped.kind(),
                    stream_error = ?stream_message,
                    error = %mapped,
                    "ComfyUI input upload request failed"
                );
                mapped
            })?;
        finish_upload_response(
            response,
            &url,
            &upload_filename,
            bytes_read.load(Ordering::Relaxed),
            attempt_started_at,
            total_started_at,
            upload_context.as_ref(),
            1,
        )
        .await
        .map_err(|failure| failure.error)
    }

    async fn upload_image_once(
        &self,
        upload: &ComfyImageUpload,
        attempt: usize,
        attempt_started_at: Instant,
        timeout: Duration,
        total_started_at: Instant,
    ) -> Result<ComfyUploadedInput, UploadAttemptFailure> {
        let url = self.config.route_url("upload/image");
        let upload_filename = upload.upload_name.as_str();
        let (task_id, asset_id, source_bytes, width, height) =
            upload_context_fields(upload.context.as_ref());
        let part = Part::bytes(upload.bytes.clone())
            .file_name(upload.upload_name.clone())
            .mime_str(&upload.content_type)
            .map_err(|error| {
                UploadAttemptFailure::terminal(ComfyAdapterError::InputUpload(format!(
                    "invalid input MIME type: {error}"
                )))
            })?;
        let form = Form::new()
            .part("image", part)
            .text("type", "input")
            .text("subfolder", String::new())
            .text("overwrite", "false");
        tracing::debug!(
            phase = "preflight",
            task_id = %task_id,
            asset_id = %asset_id,
            filename = %upload_filename,
            source_bytes = ?source_bytes,
            upload_bytes = upload.bytes.len() as u64,
            width = ?width,
            height = ?height,
            attempt,
            attempt_elapsed_ms = 0u64,
            total_elapsed_ms = total_started_at.elapsed().as_millis() as u64,
            http_status = Option::<u16>::None,
            error_class = "",
            "prepared ComfyUI replayable image upload request"
        );
        tracing::debug!(
            phase = "request_start",
            task_id = %task_id,
            asset_id = %asset_id,
            filename = %upload_filename,
            source_bytes = ?source_bytes,
            upload_bytes = upload.bytes.len() as u64,
            width = ?width,
            height = ?height,
            attempt,
            attempt_elapsed_ms = attempt_started_at.elapsed().as_millis() as u64,
            total_elapsed_ms = total_started_at.elapsed().as_millis() as u64,
            http_status = Option::<u16>::None,
            error_class = "",
            "starting ComfyUI replayable image upload request"
        );
        let response = self
            .upload_client
            .post(&url)
            .timeout(timeout)
            .multipart(form)
            .send()
            .await
            .map_err(|error| {
                let mapped = request_error("POST", &url, error);
                tracing::warn!(
                    phase = "request_start",
                    task_id = %task_id,
                    asset_id = %asset_id,
                    filename = %upload_filename,
                    source_bytes = ?source_bytes,
                    upload_bytes = upload.bytes.len() as u64,
                    width = ?width,
                    height = ?height,
                    attempt,
                    attempt_elapsed_ms = attempt_started_at.elapsed().as_millis() as u64,
                    total_elapsed_ms = total_started_at.elapsed().as_millis() as u64,
                    http_status = Option::<u16>::None,
                    error_class = mapped.kind(),
                    error = %mapped,
                    "ComfyUI replayable image upload request failed"
                );
                UploadAttemptFailure::request(mapped)
            })?;

        finish_upload_response(
            response,
            &url,
            upload_filename,
            upload.bytes.len() as u64,
            attempt_started_at,
            total_started_at,
            upload.context.as_ref(),
            attempt,
        )
        .await
    }

    async fn upload_image_with_retry(
        &self,
        upload: ComfyImageUpload,
    ) -> Result<ComfyUploadedInput, ComfyAdapterError> {
        self.upload_image_with_retry_timeout(upload, COMFY_UPLOAD_TIMEOUT)
            .await
    }

    async fn upload_image_with_retry_timeout(
        &self,
        upload: ComfyImageUpload,
        timeout: Duration,
    ) -> Result<ComfyUploadedInput, ComfyAdapterError> {
        let total_started_at = Instant::now();
        for attempt in 0..MAX_IMAGE_UPLOAD_ATTEMPTS {
            let attempt_started_at = Instant::now();
            match self
                .upload_image_once(
                    &upload,
                    attempt + 1,
                    attempt_started_at,
                    timeout,
                    total_started_at,
                )
                .await
            {
                Err(failure) if failure.retryable && attempt + 1 < MAX_IMAGE_UPLOAD_ATTEMPTS => {
                    let delay = failure
                        .retry_after
                        .unwrap_or(IMAGE_UPLOAD_RETRY_DELAYS[attempt]);
                    let (task_id, asset_id, source_bytes, width, height) =
                        upload_context_fields(upload.context.as_ref());
                    tracing::warn!(
                        phase = "retry",
                        task_id = %task_id,
                        asset_id = %asset_id,
                        filename = %upload.upload_name,
                        source_bytes = ?source_bytes,
                        upload_bytes = upload.bytes.len() as u64,
                        width = ?width,
                        height = ?height,
                        attempt = attempt + 1,
                        next_attempt = attempt + 2,
                        delay_ms = delay.as_millis() as u64,
                        attempt_elapsed_ms = attempt_started_at.elapsed().as_millis() as u64,
                        total_elapsed_ms = total_started_at.elapsed().as_millis() as u64,
                        http_status = ?failure.http_status,
                        error_class = failure.error.kind(),
                        error = %failure.error,
                        "retrying replayable ComfyUI image upload"
                    );
                    tokio::time::sleep(delay).await;
                }
                Ok(uploaded) => return Ok(uploaded),
                Err(failure) => return Err(failure.error),
            }
        }

        unreachable!("image upload retry loop must return on its final attempt")
    }

    async fn upload_image_internal(
        &self,
        upload: ComfyImageUpload,
    ) -> Result<ComfyUploadedInput, ComfyAdapterError> {
        // ComfyImageUpload is already buffered by the input preparer. Always
        // build a fresh Part::bytes from that replayable source so large images
        // never fall back to a one-shot stream during a retry.
        self.upload_image_with_retry(upload).await
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

    async fn cancel_prompt_internal(
        &self,
        prompt_id: &str,
    ) -> Result<CancelPromptResult, ComfyAdapterError> {
        if !is_safe_prompt_id(prompt_id) {
            return Err(ComfyAdapterError::Protocol(
                "prompt_id contains unsafe path characters".to_owned(),
            ));
        }

        let route = format!("api/jobs/{prompt_id}/cancel");
        let url = self.config.route_url(&route);
        let response = self
            .client
            .post(&url)
            .send()
            .await
            .map_err(|error| request_error("POST", &url, error))?;
        let status = response.status();

        if status == StatusCode::NOT_FOUND || status == StatusCode::METHOD_NOT_ALLOWED {
            return self.cancel_prompt_legacy(prompt_id).await;
        }
        if !status.is_success() {
            return Err(http_status_error("POST", &url, status));
        }

        let dto = response
            .json::<CancelResponseDto>()
            .await
            .map_err(|error| {
                ComfyAdapterError::Protocol(format!(
                    "POST {url} returned invalid cancel JSON: {error}"
                ))
            })?;
        match dto.cancelled {
            Some(true) => Ok(CancelPromptResult::CancellationRequested),
            Some(false) => Ok(CancelPromptResult::NotFoundOrAlreadyFinished),
            None => Err(ComfyAdapterError::Protocol(
                "POST /api/jobs/{prompt_id}/cancel response did not contain cancelled".to_owned(),
            )),
        }
    }

    async fn cancel_prompt_legacy(
        &self,
        prompt_id: &str,
    ) -> Result<CancelPromptResult, ComfyAdapterError> {
        let queue = self.get_queue_state_internal().await?;
        if queue
            .pending_prompt_ids
            .iter()
            .any(|candidate| candidate == prompt_id)
        {
            let url = self.config.route_url("queue");
            let response = self
                .client
                .post(&url)
                .json(&serde_json::json!({ "delete": [prompt_id] }))
                .send()
                .await
                .map_err(|error| request_error("POST", &url, error))?;
            if !response.status().is_success() {
                return Err(http_status_error("POST", &url, response.status()));
            }
            return Ok(CancelPromptResult::CancellationRequested);
        }

        if queue
            .running_prompt_ids
            .iter()
            .any(|candidate| candidate == prompt_id)
        {
            let url = self.config.route_url("interrupt");
            let response = self
                .client
                .post(&url)
                .json(&serde_json::json!({}))
                .send()
                .await
                .map_err(|error| request_error("POST", &url, error))?;
            if !response.status().is_success() {
                return Err(http_status_error("POST", &url, response.status()));
            }
            return Ok(CancelPromptResult::CancellationRequested);
        }

        Ok(CancelPromptResult::NotFoundOrAlreadyFinished)
    }

    async fn get_queue_state_internal(&self) -> Result<ComfyQueueState, ComfyAdapterError> {
        let body: Value = self
            .get_json_with_timeout("queue", COMFY_CONTROL_TIMEOUT)
            .await?;
        let root = body.as_object().ok_or_else(|| {
            ComfyAdapterError::Protocol("GET /queue response must be a JSON object".to_owned())
        })?;
        Ok(ComfyQueueState {
            running_prompt_ids: normalize_queue_ids(
                root.get("queue_running").ok_or_else(|| {
                    ComfyAdapterError::Protocol(
                        "GET /queue response is missing queue_running".to_owned(),
                    )
                })?,
                "queue_running",
            )?,
            pending_prompt_ids: normalize_queue_ids(
                root.get("queue_pending").ok_or_else(|| {
                    ComfyAdapterError::Protocol(
                        "GET /queue response is missing queue_pending".to_owned(),
                    )
                })?,
                "queue_pending",
            )?,
        })
    }

    async fn free_memory_internal(&self) -> Result<(), ComfyAdapterError> {
        let url = self.config.route_url("free");
        let response = self
            .client
            .post(&url)
            .timeout(COMFY_CONTROL_TIMEOUT)
            .json(&serde_json::json!({
                "unload_models": true,
                "free_memory": true
            }))
            .send()
            .await
            .map_err(|error| request_error("POST", &url, error))?;
        if !response.status().is_success() {
            return Err(http_status_error("POST", &url, response.status()));
        }
        Ok(())
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
        validate_comfy_output_file(file)?;
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

    async fn open_output_stream_internal(
        &self,
        file: &ComfyOutputFile,
    ) -> Result<Box<dyn ComfyOutputStream>, ComfyAdapterError> {
        validate_comfy_output_file(file)?;
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
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let content_length = response.content_length();
        Ok(Box::new(ComfyHttpOutputStream {
            content_type,
            content_length,
            stream: Box::pin(
                response
                    .bytes_stream()
                    .map(|item| item.map(|bytes| bytes.to_vec())),
            ),
        }))
    }
}

struct UploadAttemptFailure {
    error: ComfyAdapterError,
    retryable: bool,
    retry_after: Option<Duration>,
    http_status: Option<u16>,
}

impl UploadAttemptFailure {
    fn terminal(error: ComfyAdapterError) -> Self {
        Self {
            error,
            retryable: false,
            retry_after: None,
            http_status: None,
        }
    }

    fn request(error: ComfyAdapterError) -> Self {
        let retryable = matches!(
            error,
            ComfyAdapterError::Offline(_) | ComfyAdapterError::Timeout(_)
        );
        Self {
            error,
            retryable,
            retry_after: None,
            http_status: None,
        }
    }

    fn response(
        error: ComfyAdapterError,
        status: StatusCode,
        retry_after: Option<Duration>,
    ) -> Self {
        Self {
            error,
            retryable: matches!(
                status,
                StatusCode::REQUEST_TIMEOUT
                    | StatusCode::TOO_MANY_REQUESTS
                    | StatusCode::BAD_GATEWAY
                    | StatusCode::SERVICE_UNAVAILABLE
                    | StatusCode::GATEWAY_TIMEOUT
            ),
            retry_after,
            http_status: Some(status.as_u16()),
        }
    }
}

async fn finish_upload_response(
    response: reqwest::Response,
    url: &str,
    upload_filename: &str,
    bytes_read: u64,
    attempt_started_at: Instant,
    total_started_at: Instant,
    context: Option<&ComfyUploadContext>,
    attempt: usize,
) -> Result<ComfyUploadedInput, UploadAttemptFailure> {
    let response_status = response.status();
    let (task_id, asset_id, source_bytes, width, height) = upload_context_fields(context);
    tracing::debug!(
        phase = "response_received",
        task_id = %task_id,
        asset_id = %asset_id,
        filename = %upload_filename,
        source_bytes = ?source_bytes,
        upload_bytes = bytes_read,
        width = ?width,
        height = ?height,
        attempt,
        attempt_elapsed_ms = attempt_started_at.elapsed().as_millis() as u64,
        total_elapsed_ms = total_started_at.elapsed().as_millis() as u64,
        http_status = response_status.as_u16(),
        error_class = "",
        "received ComfyUI input upload response"
    );
    if response_status == StatusCode::PAYLOAD_TOO_LARGE {
        let error = ComfyAdapterError::InputUploadTooLarge(
            "ComfyUI rejected the multipart body with HTTP 413".to_owned(),
        );
        tracing::warn!(
            phase = "response_received",
            task_id = %task_id,
            asset_id = %asset_id,
            filename = %upload_filename,
            source_bytes = ?source_bytes,
            upload_bytes = bytes_read,
            width = ?width,
            height = ?height,
            attempt,
            attempt_elapsed_ms = attempt_started_at.elapsed().as_millis() as u64,
            total_elapsed_ms = total_started_at.elapsed().as_millis() as u64,
            http_status = response_status.as_u16(),
            error_class = error.kind(),
            "ComfyUI rejected an input upload as too large"
        );
        return Err(UploadAttemptFailure::response(error, response_status, None));
    }
    if !response_status.is_success() {
        let error =
            ComfyAdapterError::InputUpload(format!("POST {url} returned HTTP {}", response_status));
        let retry_after = (response_status == StatusCode::TOO_MANY_REQUESTS)
            .then(|| response.headers().get(RETRY_AFTER))
            .flatten()
            .and_then(parse_retry_after);
        tracing::warn!(
            phase = "response_received",
            task_id = %task_id,
            asset_id = %asset_id,
            filename = %upload_filename,
            source_bytes = ?source_bytes,
            upload_bytes = bytes_read,
            width = ?width,
            height = ?height,
            attempt,
            attempt_elapsed_ms = attempt_started_at.elapsed().as_millis() as u64,
            total_elapsed_ms = total_started_at.elapsed().as_millis() as u64,
            http_status = response_status.as_u16(),
            error_class = error.kind(),
            "ComfyUI rejected an input upload"
        );
        return Err(UploadAttemptFailure::response(
            error,
            response_status,
            retry_after,
        ));
    }
    tracing::debug!(
        phase = "parse_response",
        task_id = %task_id,
        asset_id = %asset_id,
        filename = %upload_filename,
        source_bytes = ?source_bytes,
        upload_bytes = bytes_read,
        width = ?width,
        height = ?height,
        attempt,
        attempt_elapsed_ms = attempt_started_at.elapsed().as_millis() as u64,
        total_elapsed_ms = total_started_at.elapsed().as_millis() as u64,
        http_status = response_status.as_u16(),
        error_class = "",
        "parsing ComfyUI input upload response"
    );
    let dto = response
        .json::<UploadResponseDto>()
        .await
        .map_err(|error| {
            let mapped = upload_response_error(url, error);
            tracing::warn!(
                phase = "parse_response",
                task_id = %task_id,
                asset_id = %asset_id,
                filename = %upload_filename,
                source_bytes = ?source_bytes,
                upload_bytes = bytes_read,
                width = ?width,
                height = ?height,
                attempt,
                attempt_elapsed_ms = attempt_started_at.elapsed().as_millis() as u64,
                total_elapsed_ms = total_started_at.elapsed().as_millis() as u64,
                http_status = response_status.as_u16(),
                error_class = mapped.kind(),
                error = %mapped,
                "ComfyUI input upload response could not be parsed"
            );
            UploadAttemptFailure::request(mapped)
        })?;
    let name = match dto.name.filter(|value| !value.trim().is_empty()) {
        Some(name) => name,
        None => {
            let error = ComfyAdapterError::Protocol(
                "POST /upload/image response did not contain name".to_owned(),
            );
            tracing::warn!(
                phase = "parse_response",
                task_id = %task_id,
                asset_id = %asset_id,
                filename = %upload_filename,
                source_bytes = ?source_bytes,
                upload_bytes = bytes_read,
                width = ?width,
                height = ?height,
                attempt,
                attempt_elapsed_ms = attempt_started_at.elapsed().as_millis() as u64,
                total_elapsed_ms = total_started_at.elapsed().as_millis() as u64,
                http_status = response_status.as_u16(),
                error_class = error.kind(),
                error = %error,
                "ComfyUI input upload response did not contain a name"
            );
            return Err(UploadAttemptFailure::terminal(error));
        }
    };
    let folder_type = match dto.folder_type.filter(|value| !value.trim().is_empty()) {
        Some(folder_type) => folder_type,
        None => {
            let error = ComfyAdapterError::Protocol(
                "POST /upload/image response did not contain type".to_owned(),
            );
            tracing::warn!(
                phase = "parse_response",
                task_id = %task_id,
                asset_id = %asset_id,
                filename = %upload_filename,
                source_bytes = ?source_bytes,
                upload_bytes = bytes_read,
                width = ?width,
                height = ?height,
                attempt,
                attempt_elapsed_ms = attempt_started_at.elapsed().as_millis() as u64,
                total_elapsed_ms = total_started_at.elapsed().as_millis() as u64,
                http_status = response_status.as_u16(),
                error_class = error.kind(),
                error = %error,
                "ComfyUI input upload response did not contain a folder type"
            );
            return Err(UploadAttemptFailure::terminal(error));
        }
    };
    tracing::debug!(
        phase = "parse_response",
        task_id = %task_id,
        asset_id = %asset_id,
        filename = %upload_filename,
        source_bytes = ?source_bytes,
        upload_bytes = bytes_read,
        width = ?width,
        height = ?height,
        attempt,
        attempt_elapsed_ms = attempt_started_at.elapsed().as_millis() as u64,
        total_elapsed_ms = total_started_at.elapsed().as_millis() as u64,
        http_status = response_status.as_u16(),
        error_class = "",
        "ComfyUI input upload completed"
    );
    Ok(ComfyUploadedInput {
        name,
        subfolder: dto.subfolder.unwrap_or_default(),
        folder_type,
    })
}

fn upload_context_fields(
    context: Option<&ComfyUploadContext>,
) -> (&str, &str, Option<u64>, Option<u32>, Option<u32>) {
    (
        context
            .and_then(|value| value.task_id.as_deref())
            .unwrap_or("unknown"),
        context
            .and_then(|value| value.asset_id.as_deref())
            .unwrap_or("unknown"),
        context.and_then(|value| value.source_bytes),
        context.and_then(|value| value.width),
        context.and_then(|value| value.height),
    )
}

pub struct ComfyHttpAdapterFactory;

impl ComfyAdapterFactory for ComfyHttpAdapterFactory {
    fn create(
        &self,
        config: ComfyConnectionConfig,
    ) -> Result<Arc<dyn ComfyAdapter>, ComfyAdapterError> {
        ComfyHttpAdapter::new(config)
            .map(|adapter| Arc::new(adapter) as Arc<dyn ComfyAdapter>)
            .map_err(|error| {
                ComfyAdapterError::Incompatible(format!("HTTP 客户端初始化失败：{error}"))
            })
    }
}

type HttpOutputByteStream =
    Pin<Box<dyn Stream<Item = Result<Vec<u8>, reqwest::Error>> + Send + 'static>>;

struct ComfyHttpOutputStream {
    content_type: Option<String>,
    content_length: Option<u64>,
    stream: HttpOutputByteStream,
}

#[async_trait]
impl ComfyOutputStream for ComfyHttpOutputStream {
    fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }

    fn content_length(&self) -> Option<u64> {
        self.content_length
    }

    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, ComfyAdapterError> {
        self.stream
            .next()
            .await
            .transpose()
            .map_err(|error| {
                ComfyAdapterError::OutputDownload(format!("stream read failed: {error}"))
            })
            .map(|chunk| chunk)
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
        // object_info can be large and slow while ComfyUI is loading nodes/models;
        // admission refresh must not fail with COMFY_TIMEOUT after only 5s.
        let object_info: Value = self
            .get_json_with_timeout("object_info", COMFY_CONTROL_TIMEOUT)
            .await?;

        if !object_info.is_object() {
            return Err(ComfyAdapterError::Incompatible(
                "object_info response is not a JSON object".to_owned(),
            ));
        }

        Ok(object_info)
    }

    async fn upload_input_file(
        &self,
        upload: ComfyInputUpload,
    ) -> Result<ComfyUploadedInput, ComfyAdapterError> {
        self.upload_input_file_internal(upload).await
    }

    async fn upload_image(
        &self,
        upload: ComfyImageUpload,
    ) -> Result<ComfyUploadedInput, ComfyAdapterError> {
        self.upload_image_internal(upload).await
    }

    async fn cancel_prompt(
        &self,
        prompt_id: &str,
    ) -> Result<CancelPromptResult, ComfyAdapterError> {
        self.cancel_prompt_internal(prompt_id).await
    }

    async fn get_queue_state(&self) -> Result<ComfyQueueState, ComfyAdapterError> {
        self.get_queue_state_internal().await
    }

    async fn free_memory(
        &self,
        unload_models: bool,
        free_memory: bool,
    ) -> Result<(), ComfyAdapterError> {
        if !unload_models || !free_memory {
            return Err(ComfyAdapterError::Incompatible(
                "AI Studio requires unload_models=true and free_memory=true".to_owned(),
            ));
        }
        self.free_memory_internal().await
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

    async fn open_output_stream(
        &self,
        file: &ComfyOutputFile,
    ) -> Result<Box<dyn ComfyOutputStream>, ComfyAdapterError> {
        self.open_output_stream_internal(file).await
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
        let (stream, _) = connect_async(&url)
            .await
            .map_err(|error| ComfyAdapterError::Offline(format!("connect {url}: {error}")))?;
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
    let status = history
        .get("status")
        .and_then(Value::as_object)
        .map(|status| ComfyHistoryStatus {
            status_str: status
                .get("status_str")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            completed: status.get("completed").and_then(Value::as_bool),
            messages: status.get("messages").cloned(),
        })
        .unwrap_or_default();
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
        let animated_flags = node.get("animated").and_then(Value::as_array);
        let image_results =
            normalize_saved_result_array(node_id, node, "images", animated_flags, None)?;
        let files = image_results
            .iter()
            .map(|result| result.file.clone())
            .collect();
        let mut saved_results = image_results;
        for field in ["gifs", "videos"] {
            for result in normalize_saved_result_array(node_id, node, field, None, Some(true))? {
                if !saved_results
                    .iter()
                    .any(|existing| existing.file == result.file)
                {
                    saved_results.push(result);
                }
            }
        }
        normalized.insert(
            node_id.clone(),
            ComfyNodeOutput {
                images: files,
                saved_results,
            },
        );
    }

    Ok(ComfyHistory {
        prompt_id: prompt_id.to_owned(),
        status,
        outputs: normalized,
    })
}

fn normalize_saved_result_array(
    node_id: &str,
    node: &serde_json::Map<String, Value>,
    field: &str,
    animated_flags: Option<&Vec<Value>>,
    default_animated: Option<bool>,
) -> Result<Vec<ComfySavedResult>, ComfyAdapterError> {
    let Some(value) = node.get(field) else {
        return Ok(Vec::new());
    };
    let entries = value.as_array().ok_or_else(|| {
        ComfyAdapterError::Protocol(format!(
            "history output node {node_id} {field} must be an array"
        ))
    })?;
    let mut results = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let entry = entry.as_object().ok_or_else(|| {
            ComfyAdapterError::Protocol(format!(
                "history output node {node_id} {field} item must be an object"
            ))
        })?;
        let filename = entry
            .get("filename")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                ComfyAdapterError::Protocol(format!(
                    "history output node {node_id} {field} filename is missing"
                ))
            })?;
        let file = ComfyOutputFile {
            filename: filename.to_owned(),
            subfolder: entry
                .get("subfolder")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            folder_type: entry
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("output")
                .to_owned(),
        };
        validate_comfy_output_file(&file)?;
        results.push(ComfySavedResult {
            file,
            animated: animated_flags
                .and_then(|flags| flags.get(index))
                .and_then(Value::as_bool)
                .or_else(|| entry.get("animated").and_then(Value::as_bool))
                .or(default_animated),
        });
    }
    Ok(results)
}

fn validate_comfy_output_file(file: &ComfyOutputFile) -> Result<(), ComfyAdapterError> {
    if file.filename.trim().is_empty()
        || matches!(file.filename.as_str(), "." | "..")
        || file
            .filename
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\' | ':'))
    {
        return Err(ComfyAdapterError::Protocol(
            "Comfy output filename must be one safe file name".to_owned(),
        ));
    }

    if !file.subfolder.is_empty()
        && file.subfolder.split(['/', '\\']).any(|part| {
            part.is_empty()
                || matches!(part, "." | "..")
                || part
                    .chars()
                    .any(|character| character.is_control() || character == ':')
        })
    {
        return Err(ComfyAdapterError::Protocol(
            "Comfy output subfolder must be a safe relative path".to_owned(),
        ));
    }

    if !matches!(file.folder_type.as_str(), "output" | "temp" | "input") {
        return Err(ComfyAdapterError::Protocol(format!(
            "unsupported Comfy output folder type: {}",
            file.folder_type
        )));
    }
    Ok(())
}

fn normalize_queue_ids(value: &Value, field: &str) -> Result<Vec<String>, ComfyAdapterError> {
    let items = value.as_array().ok_or_else(|| {
        ComfyAdapterError::Protocol(format!("GET /queue field {field} must be an array"))
    })?;
    let mut prompt_ids = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let tuple = item.as_array().ok_or_else(|| {
            ComfyAdapterError::Protocol(format!(
                "GET /queue field {field} item {index} must be an array"
            ))
        })?;
        let prompt_id = tuple
            .get(1)
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                ComfyAdapterError::Protocol(format!(
                    "GET /queue field {field} item {index} has no prompt_id at tuple index 1"
                ))
            })?;
        prompt_ids.push(prompt_id.to_owned());
    }
    Ok(prompt_ids)
}

fn is_safe_prompt_id(prompt_id: &str) -> bool {
    !prompt_id.trim().is_empty()
        && prompt_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
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

fn upload_response_error(url: &str, error: reqwest::Error) -> ComfyAdapterError {
    if error.is_timeout() || error.is_connect() || error.is_body() {
        request_error("POST", url, error)
    } else {
        ComfyAdapterError::Protocol(format!("POST {url} returned invalid upload JSON: {error}"))
    }
}

fn parse_retry_after(value: &reqwest::header::HeaderValue) -> Option<Duration> {
    let value = value.to_str().ok()?.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }

    let retry_at = chrono::DateTime::parse_from_rfc2822(value)
        .ok()?
        .with_timezone(&chrono::Utc);
    let wait_ms = (retry_at - chrono::Utc::now()).num_milliseconds().max(0) as u64;
    Some(Duration::from_millis(wait_ms))
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
    use super::{
        normalize_history, ComfyHttpAdapter, MAX_IMAGE_OUTPUT_BYTES, MAX_IMAGE_UPLOAD_ATTEMPTS,
    };
    use crate::application::ports::{
        CancelPromptResult, ComfyAdapter, ComfyAdapterError, ComfyConnectionConfig,
        ComfyExecutionEvent, ComfyImageUpload, ComfyInputStream, ComfyInputUpload, ComfyOutputFile,
    };
    use async_trait::async_trait;
    use futures_util::SinkExt;
    use serde_json::{json, Value};
    use std::collections::VecDeque;
    use std::net::TcpListener;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use tokio_tungstenite::{accept_async, tungstenite::Message};
    use wiremock::{
        matchers::{body_json, body_string_contains, method, path, query_param},
        Mock, MockServer, ResponseTemplate,
    };

    fn config_for(server: &MockServer) -> ComfyConnectionConfig {
        ComfyConnectionConfig::new("http", "127.0.0.1", server.address().port())
    }

    struct TestInputStream {
        chunks: VecDeque<Result<Option<Vec<u8>>, String>>,
    }

    #[async_trait]
    impl ComfyInputStream for TestInputStream {
        async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, String> {
            self.chunks.pop_front().unwrap_or(Ok(None))
        }
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
    async fn health_check_allows_slow_system_stats_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/system_stats"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({
                        "system": {"comfyui_version": "slow-health"},
                        "devices": []
                    }))
                    .set_delay(Duration::from_secs(6)),
            )
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        let stats = adapter
            .get_system_stats()
            .await
            .expect("slow health response should remain a connected ComfyUI");

        assert_eq!(stats.comfyui_version.as_deref(), Some("slow-health"));
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
    async fn free_memory_posts_exact_official_request() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/free"))
            .and(body_json(json!({
                "unload_models": true,
                "free_memory": true
            })))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        adapter
            .free_memory(true, true)
            .await
            .expect("idle memory release should succeed");
        server.verify().await;
    }

    #[tokio::test]
    async fn queue_state_allows_slow_control_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/queue"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({ "queue_running": [], "queue_pending": [] }))
                    .set_delay(Duration::from_secs(6)),
            )
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        let queue = adapter
            .get_queue_state()
            .await
            .expect("slow queue response should remain within the control timeout");

        assert!(queue.running_prompt_ids.is_empty());
        assert!(queue.pending_prompt_ids.is_empty());
    }

    #[tokio::test]
    async fn free_memory_allows_slow_control_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/free"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(6)))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        adapter
            .free_memory(true, true)
            .await
            .expect("slow memory release response should remain within the control timeout");
    }

    #[tokio::test]
    async fn free_memory_http_error_is_not_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/free"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        let error = adapter
            .free_memory(true, true)
            .await
            .expect_err("HTTP 500 must fail memory release");
        assert!(matches!(error, ComfyAdapterError::Incompatible(_)));
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
    async fn object_info_allows_slow_control_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/object_info"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"KSampler": {}}))
                    .set_delay(Duration::from_secs(6)),
            )
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        let object_info = adapter
            .get_object_info()
            .await
            .expect("slow object info response should remain within the control timeout");

        assert!(object_info.get("KSampler").is_some());
    }

    #[tokio::test]
    async fn uploads_image_as_input_and_accepts_server_identity() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/upload/image"))
            .and(body_string_contains("aistudio_task_asset.png"))
            .and(body_string_contains("input"))
            .and(body_string_contains("overwrite"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "name": "aistudio_task_asset.png",
                "subfolder": "",
                "type": "input",
                "unknown": true
            })))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        let uploaded = adapter
            .upload_image(ComfyImageUpload {
                bytes: vec![1, 2, 3],
                upload_name: "aistudio_task_asset.png".to_owned(),
                content_type: "image/png".to_owned(),
                context: None,
            })
            .await
            .expect("upload should parse");
        assert_eq!(uploaded.name, "aistudio_task_asset.png");
        assert_eq!(uploaded.folder_type, "input");
    }

    #[tokio::test]
    async fn upload_allows_slow_response_beyond_default_http_timeout() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/upload/image"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({
                        "name": "slow-input.png",
                        "subfolder": "",
                        "type": "input"
                    }))
                    .set_delay(Duration::from_secs(6)),
            )
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        let uploaded = adapter
            .upload_image(ComfyImageUpload {
                bytes: vec![1, 2, 3],
                upload_name: "slow-input.png".to_owned(),
                content_type: "image/png".to_owned(),
                context: None,
            })
            .await
            .expect("slow upload response should remain within the upload timeout");

        assert_eq!(uploaded.name, "slow-input.png");
    }

    #[tokio::test]
    async fn replayable_image_upload_retries_connection_failure_then_succeeds() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("test listener should bind");
        let port = listener
            .local_addr()
            .expect("test listener should have an address")
            .port();
        let server = tokio::spawn(async move {
            let (first, _) = listener
                .accept()
                .await
                .expect("first upload should connect");
            drop(first);

            let (mut second, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
                .await
                .expect("retry should connect")
                .expect("retry connection should be accepted");
            let request = read_upload_request(&mut second)
                .await
                .expect("retry request should contain its full body");
            assert!(String::from_utf8_lossy(&request).contains("retried.png"));
            write_upload_response(&mut second, "retried.png")
                .await
                .expect("retry response should write");
        });

        let adapter = ComfyHttpAdapter::new(ComfyConnectionConfig::new("http", "127.0.0.1", port))
            .expect("client should build");
        let uploaded = adapter
            .upload_image(ComfyImageUpload {
                bytes: vec![1, 2, 3, 4],
                upload_name: "retried.png".to_owned(),
                content_type: "image/png".to_owned(),
                context: None,
            })
            .await
            .expect("a replayable image should succeed after a connection failure");

        assert_eq!(uploaded.name, "retried.png");
        server.await.expect("retry server should not panic");
    }

    #[tokio::test]
    async fn replayable_image_upload_stops_after_three_connection_failures() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("test listener should bind");
        let port = listener
            .local_addr()
            .expect("test listener should have an address")
            .port();
        let server = tokio::spawn(async move {
            for _ in 0..MAX_IMAGE_UPLOAD_ATTEMPTS {
                let (stream, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
                    .await
                    .expect("each retry should connect")
                    .expect("retry connection should be accepted");
                drop(stream);
            }
        });

        let adapter = ComfyHttpAdapter::new(ComfyConnectionConfig::new("http", "127.0.0.1", port))
            .expect("client should build");
        let error = adapter
            .upload_image(ComfyImageUpload {
                bytes: vec![1, 2, 3, 4],
                upload_name: "exhausted.png".to_owned(),
                content_type: "image/png".to_owned(),
                context: None,
            })
            .await
            .expect_err("three connection failures should exhaust the retry budget");

        assert!(matches!(
            error,
            ComfyAdapterError::Offline(_) | ComfyAdapterError::Timeout(_)
        ));
        server.await.expect("retry server should not panic");
    }

    #[tokio::test]
    async fn replayable_image_upload_does_not_retry_http_413() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/upload/image"))
            .respond_with(ResponseTemplate::new(413))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        let error = adapter
            .upload_image(ComfyImageUpload {
                bytes: vec![1, 2, 3, 4],
                upload_name: "too-large.png".to_owned(),
                content_type: "image/png".to_owned(),
                context: None,
            })
            .await
            .expect_err("HTTP 413 should remain a terminal upload error");

        assert!(matches!(error, ComfyAdapterError::InputUploadTooLarge(_)));
        let requests = server
            .received_requests()
            .await
            .expect("wiremock should expose received requests");
        assert_eq!(requests.len(), 1, "HTTP 413 must not be retried");
    }

    #[tokio::test]
    async fn replayable_image_upload_does_not_retry_other_client_errors() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/upload/image"))
            .respond_with(ResponseTemplate::new(400))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        let error = adapter
            .upload_image(ComfyImageUpload {
                bytes: vec![1, 2, 3, 4],
                upload_name: "bad-request.png".to_owned(),
                content_type: "image/png".to_owned(),
                context: None,
            })
            .await
            .expect_err("HTTP 400 should remain a terminal upload error");

        assert!(matches!(error, ComfyAdapterError::InputUpload(_)));
        let requests = server
            .received_requests()
            .await
            .expect("wiremock should expose received requests");
        assert_eq!(requests.len(), 1, "ordinary 4xx must not be retried");
    }

    #[tokio::test]
    async fn replayable_image_upload_retries_timeout_then_succeeds() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("test listener should bind");
        let port = listener
            .local_addr()
            .expect("test listener should have an address")
            .port();
        let server = tokio::spawn(async move {
            let (mut first, _) = listener
                .accept()
                .await
                .expect("first upload should connect");
            read_upload_request(&mut first)
                .await
                .expect("first upload body should be received");
            tokio::time::sleep(Duration::from_millis(250)).await;
            drop(first);

            let (mut second, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
                .await
                .expect("retry should connect")
                .expect("retry connection should be accepted");
            read_upload_request(&mut second)
                .await
                .expect("retry request should contain its full body");
            write_upload_response(&mut second, "timeout-retry.png")
                .await
                .expect("retry response should write");
        });

        let adapter = ComfyHttpAdapter::new(ComfyConnectionConfig::new("http", "127.0.0.1", port))
            .expect("client should build");
        let uploaded = adapter
            .upload_image_with_retry_timeout(
                ComfyImageUpload {
                    bytes: vec![1, 2, 3, 4],
                    upload_name: "timeout-retry.png".to_owned(),
                    content_type: "image/png".to_owned(),
                    context: None,
                },
                Duration::from_millis(100),
            )
            .await
            .expect("a timed-out image upload should succeed on retry");

        assert_eq!(uploaded.name, "timeout-retry.png");
        server.await.expect("timeout retry server should not panic");
    }

    #[tokio::test]
    async fn large_replayable_image_upload_retries_after_disconnect() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("test listener should bind");
        let port = listener
            .local_addr()
            .expect("test listener should have an address")
            .port();
        let server = tokio::spawn(async move {
            let (mut first, _) = listener
                .accept()
                .await
                .expect("first upload should connect");
            read_upload_request(&mut first)
                .await
                .expect("first large upload body should be received");
            drop(first);

            let (mut second, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
                .await
                .expect("retry should connect")
                .expect("retry connection should be accepted");
            let request = read_upload_request(&mut second)
                .await
                .expect("retry large request should contain its full body");
            assert!(request.len() > 16 * 1024 * 1024);
            write_upload_response(&mut second, "large-retry.png")
                .await
                .expect("retry response should write");
        });

        let adapter = ComfyHttpAdapter::new(ComfyConnectionConfig::new("http", "127.0.0.1", port))
            .expect("client should build");
        let uploaded = adapter
            .upload_image(ComfyImageUpload {
                bytes: vec![7; 16 * 1024 * 1024 + 1],
                upload_name: "large-retry.png".to_owned(),
                content_type: "image/png".to_owned(),
                context: None,
            })
            .await
            .expect("a large replayable image should succeed on retry");

        assert_eq!(uploaded.name, "large-retry.png");
        server.await.expect("large retry server should not panic");
    }

    #[tokio::test]
    async fn replayable_image_upload_retries_retryable_http_statuses() {
        for (status, retry_after) in [
            (408, None),
            (429, Some("0")),
            (502, None),
            (503, None),
            (504, None),
        ] {
            let (port, server) = spawn_upload_status_sequence(status, retry_after).await;
            let adapter =
                ComfyHttpAdapter::new(ComfyConnectionConfig::new("http", "127.0.0.1", port))
                    .expect("client should build");
            let uploaded = adapter
                .upload_image(ComfyImageUpload {
                    bytes: vec![1, 2, 3],
                    upload_name: "retryable-status.png".to_owned(),
                    content_type: "image/png".to_owned(),
                    context: None,
                })
                .await
                .expect("retryable status should succeed on the next attempt");

            assert_eq!(uploaded.name, "server-after-retry.png");
            server
                .await
                .expect("status sequence server should not panic");
        }
    }

    async fn spawn_upload_status_sequence(
        status: u16,
        retry_after: Option<&'static str>,
    ) -> (u16, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("test listener should bind");
        let port = listener
            .local_addr()
            .expect("test listener should have an address")
            .port();
        let server = tokio::spawn(async move {
            for (response_status, response_retry_after) in [(status, retry_after), (200, None)] {
                let (mut stream, _) =
                    tokio::time::timeout(Duration::from_secs(5), listener.accept())
                        .await
                        .expect("upload attempt should connect")
                        .expect("upload connection should be accepted");
                let request = read_upload_request(&mut stream)
                    .await
                    .expect("status request should contain its full body");
                assert!(String::from_utf8_lossy(&request).contains("retryable-status.png"));
                write_upload_status_response(&mut stream, response_status, response_retry_after)
                    .await
                    .expect("status response should write");
            }
        });
        (port, server)
    }

    async fn write_upload_status_response(
        stream: &mut TcpStream,
        status: u16,
        retry_after: Option<&str>,
    ) -> std::io::Result<()> {
        let (reason, body) = if status == 200 {
            (
                "OK",
                r#"{"name":"server-after-retry.png","subfolder":"","type":"input"}"#,
            )
        } else {
            ("Retryable", "retryable upload status")
        };
        let retry_header = retry_after
            .map(|value| format!("Retry-After: {value}\r\n"))
            .unwrap_or_default();
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{retry_header}Connection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).await
    }

    async fn read_upload_request(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
        let mut request = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            let read = stream.read(&mut chunk).await?;
            if read == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "request ended before its body was received",
                ));
            }
            request.extend_from_slice(&chunk[..read]);
            let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n")
            else {
                continue;
            };
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    line.strip_prefix("Content-Length:")
                        .or_else(|| line.strip_prefix("content-length:"))
                })
                .and_then(|value| value.trim().parse::<usize>().ok())
                .unwrap_or_default();
            if request.len() >= header_end + 4 + content_length {
                return Ok(request);
            }
        }
    }

    async fn write_upload_response(stream: &mut TcpStream, name: &str) -> std::io::Result<()> {
        let body = format!("{{\"name\":\"{name}\",\"subfolder\":\"\",\"type\":\"input\"}}");
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(), body
        );
        stream.write_all(response.as_bytes()).await
    }

    #[tokio::test]
    async fn uploads_use_a_fresh_connection_after_a_previous_upload() {
        async fn read_request(stream: &mut TcpStream) -> std::io::Result<()> {
            let mut request = Vec::new();
            let mut chunk = [0u8; 8192];
            loop {
                let read = stream.read(&mut chunk).await?;
                if read == 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "request ended before its body was received",
                    ));
                }
                request.extend_from_slice(&chunk[..read]);
                let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n")
                else {
                    continue;
                };
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("Content-Length:")
                            .or_else(|| line.strip_prefix("content-length:"))
                    })
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .unwrap_or_default();
                if request.len() >= header_end + 4 + content_length {
                    return Ok(());
                }
            }
        }

        async fn respond(stream: &mut TcpStream) -> std::io::Result<()> {
            let body = r#"{"name":"fresh.png","subfolder":"","type":"input"}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n{}",
                body.len(), body
            );
            stream.write_all(response.as_bytes()).await
        }

        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("test listener should bind");
        let port = listener
            .local_addr()
            .expect("test listener should have an address")
            .port();
        let server = tokio::spawn(async move {
            let (mut first, _) = listener
                .accept()
                .await
                .expect("first upload should connect");
            read_request(&mut first)
                .await
                .expect("first upload request should be complete");
            respond(&mut first)
                .await
                .expect("first response should write");

            tokio::select! {
                result = listener.accept() => {
                    let (mut second, _) = result.expect("second upload should connect");
                    read_request(&mut second)
                        .await
                        .expect("second upload request should be complete");
                    respond(&mut second).await.expect("second response should write");
                    true
                }
                result = read_request(&mut first) => {
                    if result.is_ok() {
                        respond(&mut first).await.expect("reused response should write");
                        false
                    } else {
                        let (mut second, _) = tokio::time::timeout(
                            Duration::from_secs(2),
                            listener.accept(),
                        )
                        .await
                        .expect("fresh connection should arrive")
                        .expect("fresh connection should be accepted");
                        read_request(&mut second)
                            .await
                            .expect("second upload request should be complete");
                        respond(&mut second).await.expect("second response should write");
                        true
                    }
                }
            }
        });

        let adapter = ComfyHttpAdapter::new(ComfyConnectionConfig::new("http", "127.0.0.1", port))
            .expect("client should build");
        for filename in ["first.png", "second.png"] {
            adapter
                .upload_image(ComfyImageUpload {
                    bytes: vec![1, 2, 3],
                    upload_name: filename.to_owned(),
                    content_type: "image/png".to_owned(),
                    context: None,
                })
                .await
                .expect("upload should succeed");
        }

        assert!(
            tokio::time::timeout(Duration::from_secs(2), server)
                .await
                .expect("connection test should finish")
                .expect("connection test should not panic"),
            "a later upload must not reuse the previous idle connection"
        );
    }

    #[tokio::test]
    async fn uploads_video_and_audio_through_the_same_generic_input_route() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/upload/image"))
            .and(body_string_contains("type"))
            .and(body_string_contains("input"))
            .and(body_string_contains("overwrite"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "name": "server-authoritative.bin",
                "subfolder": "",
                "type": "input"
            })))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        let video = adapter
            .upload_input_file(ComfyInputUpload {
                filename: "aistudio_task_asset.mp4".to_owned(),
                content_type: "video/mp4".to_owned(),
                content_length: Some(4),
                stream: Box::new(TestInputStream {
                    chunks: VecDeque::from([Ok(Some(vec![1, 2])), Ok(Some(vec![3, 4])), Ok(None)]),
                }),
                context: None,
            })
            .await
            .expect("video upload should parse");
        assert_eq!(video.name, "server-authoritative.bin");

        let audio = adapter
            .upload_input_file(ComfyInputUpload {
                filename: "aistudio_task_asset.wav".to_owned(),
                content_type: "audio/wav".to_owned(),
                content_length: Some(2),
                stream: Box::new(TestInputStream {
                    chunks: VecDeque::from([Ok(Some(vec![5, 6])), Ok(None)]),
                }),
                context: None,
            })
            .await
            .expect("audio upload should parse");
        assert_eq!(audio.folder_type, "input");
        let requests = server
            .received_requests()
            .await
            .expect("requests should be recorded");
        assert_eq!(requests.len(), 2);
        for request in requests {
            let body = String::from_utf8_lossy(&request.body);
            assert!(body.contains("subfolder"));
            assert!(body.contains("false"));
        }
    }

    #[tokio::test]
    async fn maps_comfy_input_upload_413_to_specific_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/upload/image"))
            .respond_with(ResponseTemplate::new(413))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        let error = adapter
            .upload_input_file(ComfyInputUpload {
                filename: "large.mp4".to_owned(),
                content_type: "video/mp4".to_owned(),
                content_length: Some(1),
                stream: Box::new(TestInputStream {
                    chunks: VecDeque::from([Ok(Some(vec![1])), Ok(None)]),
                }),
                context: None,
            })
            .await
            .expect_err("413 should be rejected");
        assert!(matches!(error, ComfyAdapterError::InputUploadTooLarge(_)));
    }

    #[tokio::test]
    async fn input_stream_failure_is_not_hidden_as_a_successful_upload() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/upload/image"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "name": "should-not-be-used",
                "subfolder": "",
                "type": "input"
            })))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        let error = adapter
            .upload_input_file(ComfyInputUpload {
                filename: "broken.mp4".to_owned(),
                content_type: "video/mp4".to_owned(),
                content_length: None,
                stream: Box::new(TestInputStream {
                    chunks: VecDeque::from([Err("disk read failed".to_owned())]),
                }),
                context: None,
            })
            .await
            .expect_err("stream failure should fail upload");
        assert!(matches!(error, ComfyAdapterError::InputUpload(_)));
    }

    #[tokio::test]
    async fn modern_cancel_true_is_prompt_specific() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/jobs/prompt-1/cancel"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "cancelled": true })))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        assert_eq!(
            adapter.cancel_prompt("prompt-1").await.unwrap(),
            CancelPromptResult::CancellationRequested
        );
    }

    #[tokio::test]
    async fn modern_cancel_false_means_not_found_or_finished() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/jobs/prompt-1/cancel"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "cancelled": false })))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        assert_eq!(
            adapter.cancel_prompt("prompt-1").await.unwrap(),
            CancelPromptResult::NotFoundOrAlreadyFinished
        );
    }

    #[tokio::test]
    async fn modern_cancel_404_falls_back_to_pending_delete() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/jobs/prompt-1/cancel"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/queue"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "queue_running": [],
                "queue_pending": [[3, "prompt-1", {}, {}]]
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/queue"))
            .and(body_json(json!({ "delete": ["prompt-1"] })))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        assert_eq!(
            adapter.cancel_prompt("prompt-1").await.unwrap(),
            CancelPromptResult::CancellationRequested
        );
    }

    #[tokio::test]
    async fn modern_cancel_404_interrupts_only_the_target_running_prompt() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/jobs/prompt-1/cancel"))
            .respond_with(ResponseTemplate::new(405))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/queue"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "queue_running": [[4, "prompt-1", {}, {}]],
                "queue_pending": []
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/interrupt"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        assert_eq!(
            adapter.cancel_prompt("prompt-1").await.unwrap(),
            CancelPromptResult::CancellationRequested
        );
    }

    #[tokio::test]
    async fn legacy_fallback_never_blindly_interrupts_another_prompt() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/jobs/prompt-a/cancel"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/queue"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "queue_running": [[4, "prompt-b", {}, {}]],
                "queue_pending": []
            })))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        assert_eq!(
            adapter.cancel_prompt("prompt-a").await.unwrap(),
            CancelPromptResult::NotFoundOrAlreadyFinished
        );
    }

    #[tokio::test]
    async fn malformed_queue_is_a_protocol_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/jobs/prompt-1/cancel"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/queue"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "queue_running": {},
                "queue_pending": []
            })))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        assert!(matches!(
            adapter.cancel_prompt("prompt-1").await,
            Err(ComfyAdapterError::Protocol(_))
        ));
    }

    #[tokio::test]
    async fn malformed_upload_response_is_protocol_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/upload/image"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        let error = adapter
            .upload_image(ComfyImageUpload {
                bytes: vec![1, 2, 3],
                upload_name: "image.png".to_owned(),
                content_type: "image/png".to_owned(),
                context: None,
            })
            .await
            .expect_err("malformed upload response should fail");
        assert!(matches!(error, ComfyAdapterError::Protocol(_)));
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
                                {"filename": "a.png", "subfolder": "", "type": "output", "animated": true, "unknown": true},
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
        assert_eq!(history.outputs["9"].saved_results.len(), 2);
        assert_eq!(history.outputs["9"].saved_results[0].animated, Some(true));
        assert!(history.outputs["3"].images.is_empty());
    }

    #[test]
    fn save_video_fixture_uses_images_as_generic_saved_results() {
        let body: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/comfy_history/save_video_history.json"
        )))
        .expect("video fixture should be valid JSON");
        let history = normalize_history("prompt-video-fixture", body)
            .expect("video fixture should normalize");
        let output = &history.outputs["11"];
        assert_eq!(output.images.len(), 1);
        assert_eq!(output.saved_results.len(), 1);
        assert_eq!(output.saved_results[0].file.filename, "ComfyUI_00001.mp4");
        assert_eq!(output.saved_results[0].animated, Some(true));
    }

    #[test]
    fn vhs_gifs_are_normalized_as_saved_video_results() {
        let body = json!({
            "prompt-vhs": {
                "outputs": {
                    "62": {
                        "gifs": [{
                            "filename": "AnimateDiff_00010-audio.mp4",
                            "subfolder": "",
                            "type": "output",
                            "format": "video/h264-mp4",
                            "frame_rate": 24.0,
                            "fullpath": "D:\\ComfyUI\\output\\AnimateDiff_00010-audio.mp4"
                        }]
                    }
                },
                "status": {"status_str": "success", "completed": true}
            }
        });

        let history =
            normalize_history("prompt-vhs", body).expect("VHS history output should normalize");
        let output = &history.outputs["62"];
        assert!(output.images.is_empty());
        assert_eq!(output.saved_results.len(), 1);
        assert_eq!(
            output.saved_results[0].file.filename,
            "AnimateDiff_00010-audio.mp4"
        );
        assert_eq!(output.saved_results[0].file.folder_type, "output");
        assert_eq!(output.saved_results[0].animated, Some(true));
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
    async fn output_stream_uses_view_query_without_buffering_the_full_response_in_adapter() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/view"))
            .and(query_param("filename", "ComfyUI_00001.mp4"))
            .and(query_param("subfolder", "nested"))
            .and(query_param("type", "output"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "video/mp4")
                    .set_body_bytes(vec![0, 0, 0, 0, b'f', b't', b'y', b'p', 1, 2, 3]),
            )
            .mount(&server)
            .await;
        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        let mut stream = adapter
            .open_output_stream(&ComfyOutputFile {
                filename: "ComfyUI_00001.mp4".to_owned(),
                subfolder: "nested".to_owned(),
                folder_type: "output".to_owned(),
            })
            .await
            .expect("stream should open");
        assert_eq!(stream.content_type(), Some("video/mp4"));
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next_chunk().await.unwrap() {
            bytes.extend(chunk);
        }
        assert_eq!(bytes[4..8], *b"ftyp");
    }

    #[tokio::test]
    async fn rejects_unsafe_view_parameters_before_request() {
        let server = MockServer::start().await;
        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");

        for file in [
            ComfyOutputFile {
                filename: "../escape.png".to_owned(),
                subfolder: String::new(),
                folder_type: "output".to_owned(),
            },
            ComfyOutputFile {
                filename: "safe.png".to_owned(),
                subfolder: "../escape".to_owned(),
                folder_type: "output".to_owned(),
            },
            ComfyOutputFile {
                filename: "safe.png".to_owned(),
                subfolder: String::new(),
                folder_type: "other".to_owned(),
            },
        ] {
            assert!(matches!(
                adapter.download_output(&file).await,
                Err(ComfyAdapterError::Protocol(_))
            ));
        }

        let stream_error = match adapter
            .open_output_stream(&ComfyOutputFile {
                filename: "safe.png".to_owned(),
                subfolder: "nested//escape".to_owned(),
                folder_type: "output".to_owned(),
            })
            .await
        {
            Ok(_) => panic!("unsafe stream path should be rejected"),
            Err(error) => error,
        };
        assert!(matches!(stream_error, ComfyAdapterError::Protocol(_)));
        assert!(server
            .received_requests()
            .await
            .unwrap_or_default()
            .is_empty());
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
