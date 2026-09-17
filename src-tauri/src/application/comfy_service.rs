use crate::application::ports::{
    ComfyAdapter, ComfyAdapterError, ComfyAdapterHandle, ComfyConnectionConfig, DeviceInfo,
    SystemStats,
};
use crate::error::AppError;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    sync::{Arc, RwLock as StdRwLock},
};
use tokio::sync::RwLock;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ComfyConnectionStatus {
    Connected,
    Offline,
    Incompatible,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub python_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ram_total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ram_free: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitySummary {
    pub node_count: usize,
    pub captured_at: String,
}

#[derive(Debug)]
pub struct CapabilityCache {
    pub fingerprint: String,
    pub node_count: usize,
    pub node_classes: HashSet<String>,
    pub captured_at: DateTime<Utc>,
}

impl CapabilityCache {
    fn summary(&self) -> CapabilitySummary {
        debug_assert_eq!(self.node_count, self.node_classes.len());

        CapabilitySummary {
            node_count: self.node_classes.len(),
            captured_at: self.captured_at.to_rfc3339(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComfyStatusView {
    pub status: ComfyConnectionStatus,
    pub endpoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comfyui_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<SystemSummary>,
    pub devices: Vec<DeviceInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability: Option<CapabilitySummary>,
}

pub struct ComfyService {
    runtime: Arc<ComfyRuntime>,
    capability_cache: Arc<RwLock<Option<CapabilityCache>>>,
    status_cache: Arc<RwLock<Option<ComfyStatusView>>>,
}

pub struct ComfyRuntime {
    handle: Arc<ComfyAdapterHandle>,
    config: StdRwLock<ComfyConnectionConfig>,
}

fn write_canonical_json(value: &Value, output: &mut Vec<u8>) {
    match value {
        Value::Object(object) => {
            let mut entries = object.iter().collect::<Vec<_>>();
            entries.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
            output.push(b'{');
            for (index, (key, value)) in entries.into_iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                serde_json::to_writer(&mut *output, key)
                    .expect("JSON object keys should always serialize");
                output.push(b':');
                write_canonical_json(value, output);
            }
            output.push(b'}');
        }
        Value::Array(items) => {
            output.push(b'[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                write_canonical_json(item, output);
            }
            output.push(b']');
        }
        scalar => serde_json::to_writer(output, scalar)
            .expect("JSON scalar values should always serialize"),
    }
}

fn capability_fingerprint(object_info: &Value) -> String {
    let mut canonical = Vec::new();
    write_canonical_json(object_info, &mut canonical);
    format!("{:x}", Sha256::digest(canonical))
}

impl ComfyRuntime {
    pub fn new(adapter: Arc<dyn ComfyAdapter>, config: ComfyConnectionConfig) -> Self {
        Self {
            handle: Arc::new(ComfyAdapterHandle::new(adapter)),
            config: StdRwLock::new(config),
        }
    }

    pub fn adapter(&self) -> Arc<dyn ComfyAdapter> {
        self.handle.clone()
    }

    pub fn config(&self) -> ComfyConnectionConfig {
        self.config
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub fn endpoint(&self) -> String {
        self.config().endpoint()
    }

    pub fn replace(&self, config: ComfyConnectionConfig, adapter: Arc<dyn ComfyAdapter>) {
        self.handle.replace(adapter);
        *self
            .config
            .write()
            .unwrap_or_else(|error| error.into_inner()) = config;
    }
}

impl ComfyService {
    #[cfg(test)]
    pub fn new(adapter: Arc<dyn ComfyAdapter>, config: &ComfyConnectionConfig) -> Self {
        Self::from_runtime(Arc::new(ComfyRuntime::new(adapter, config.clone())))
    }

    pub fn from_runtime(runtime: Arc<ComfyRuntime>) -> Self {
        Self {
            runtime,
            capability_cache: Arc::new(RwLock::new(None)),
            status_cache: Arc::new(RwLock::new(None)),
        }
    }

    pub fn endpoint(&self) -> String {
        self.runtime.endpoint()
    }

    pub async fn invalidate_capabilities(&self) {
        *self.capability_cache.write().await = None;
    }

    pub async fn get_status(&self) -> Result<ComfyStatusView, AppError> {
        let cached_capability = self.cached_capability().await;
        let endpoint = self.endpoint();

        match self.runtime.adapter().health_check().await {
            Ok(health) => {
                let status = self.connected_status(health.system, cached_capability, endpoint);
                *self.status_cache.write().await = Some(status.clone());
                Ok(status)
            }
            Err(error) => {
                tracing::warn!(
                    endpoint = %endpoint,
                    error_type = error.kind(),
                    "ComfyUI health check failed"
                );

                let status = ComfyStatusView {
                    status: status_for_adapter_error(&error),
                    endpoint,
                    comfyui_version: None,
                    system: None,
                    devices: Vec::new(),
                    capability: cached_capability,
                };
                *self.status_cache.write().await = Some(status.clone());
                Ok(status)
            }
        }
    }

    /// Return the last status collected by `get_status` without contacting ComfyUI.
    pub async fn cached_status(&self) -> Option<ComfyStatusView> {
        self.status_cache.read().await.clone()
    }

    pub async fn refresh_capabilities(&self) -> Result<CapabilitySummary, AppError> {
        let endpoint = self.endpoint();
        let object_info = self
            .runtime
            .adapter()
            .get_object_info()
            .await
            .map_err(|error| {
                tracing::warn!(
                    endpoint = %endpoint,
                    error_type = error.kind(),
                    "ComfyUI capability refresh failed"
                );
                app_error_for_adapter_error(error)
            })?;

        let object = object_info.as_object().ok_or_else(|| {
            AppError::comfy_protocol_error("ComfyUI object_info response is not an object")
        })?;
        let node_classes = object.keys().cloned().collect::<HashSet<_>>();
        let cache = CapabilityCache {
            fingerprint: capability_fingerprint(&object_info),
            node_count: node_classes.len(),
            node_classes,
            captured_at: Utc::now(),
        };
        let summary = cache.summary();

        *self.capability_cache.write().await = Some(cache);

        tracing::info!(
            endpoint = %endpoint,
            node_count = summary.node_count,
            "ComfyUI capability cache refreshed"
        );

        Ok(summary)
    }

    /// Observable public schema only; not proof of internal plugin compatibility.
    pub async fn capability_diagnostics(&self) -> Option<(String, bool)> {
        self.capability_cache.read().await.as_ref().map(|cache| {
            (
                cache.fingerprint.clone(),
                cache
                    .node_classes
                    .iter()
                    .any(|name| matches!(name.as_str(), "NBH3HyperStepSimple" | "NBH3HyperStep")),
            )
        })
    }

    async fn cached_capability(&self) -> Option<CapabilitySummary> {
        self.capability_cache
            .read()
            .await
            .as_ref()
            .map(CapabilityCache::summary)
    }

    fn connected_status(
        &self,
        stats: SystemStats,
        capability: Option<CapabilitySummary>,
        endpoint: String,
    ) -> ComfyStatusView {
        ComfyStatusView {
            status: ComfyConnectionStatus::Connected,
            endpoint,
            comfyui_version: stats.comfyui_version,
            system: Some(SystemSummary {
                python_version: stats.python_version,
                os: stats.os,
                ram_total: stats.ram_total,
                ram_free: stats.ram_free,
            }),
            devices: stats.devices,
            capability,
        }
    }
}

fn status_for_adapter_error(error: &ComfyAdapterError) -> ComfyConnectionStatus {
    match error {
        ComfyAdapterError::Offline(_) | ComfyAdapterError::Timeout(_) => {
            ComfyConnectionStatus::Offline
        }
        ComfyAdapterError::Incompatible(_)
        | ComfyAdapterError::Protocol(_)
        | ComfyAdapterError::HistoryNotFound(_)
        | ComfyAdapterError::OutputDownload(_)
        | ComfyAdapterError::OutputTooLarge(_)
        | ComfyAdapterError::ImageUpload(_)
        | ComfyAdapterError::InputUpload(_)
        | ComfyAdapterError::InputUploadTooLarge(_) => ComfyConnectionStatus::Incompatible,
        ComfyAdapterError::WorkflowValidation { .. } | ComfyAdapterError::StreamDisconnected(_) => {
            ComfyConnectionStatus::Incompatible
        }
    }
}

fn app_error_for_adapter_error(error: ComfyAdapterError) -> AppError {
    match error {
        ComfyAdapterError::Offline(_) => AppError::comfy_offline("无法连接到本地 ComfyUI"),
        ComfyAdapterError::Timeout(_) => AppError::comfy_timeout("ComfyUI 请求超时"),
        ComfyAdapterError::Incompatible(_)
        | ComfyAdapterError::Protocol(_)
        | ComfyAdapterError::HistoryNotFound(_)
        | ComfyAdapterError::OutputDownload(_)
        | ComfyAdapterError::OutputTooLarge(_)
        | ComfyAdapterError::ImageUpload(_)
        | ComfyAdapterError::InputUpload(_)
        | ComfyAdapterError::InputUploadTooLarge(_) => {
            AppError::comfy_protocol_error("ComfyUI 返回了不兼容的 API 响应")
        }
        ComfyAdapterError::WorkflowValidation { .. } | ComfyAdapterError::StreamDisconnected(_) => {
            AppError::comfy_protocol_error("ComfyUI 返回了不兼容的执行响应")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ComfyConnectionStatus, ComfyService};
    use crate::application::ports::{
        ComfyAdapter, ComfyAdapterError, ComfyConnectionConfig, ComfyHealth, DeviceInfo,
        SystemStats,
    };
    use async_trait::async_trait;
    use serde_json::{json, Value};
    use std::sync::Arc;

    struct FakeAdapter {
        stats: Result<SystemStats, ComfyAdapterError>,
        object_info: Result<serde_json::Value, ComfyAdapterError>,
    }

    #[async_trait]
    impl ComfyAdapter for FakeAdapter {
        async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
            self.stats
                .as_ref()
                .map(|stats| ComfyHealth {
                    system: stats.clone(),
                })
                .map_err(Clone::clone)
        }

        async fn get_system_stats(&self) -> Result<SystemStats, ComfyAdapterError> {
            self.stats.as_ref().cloned().map_err(Clone::clone)
        }

        async fn get_object_info(&self) -> Result<serde_json::Value, ComfyAdapterError> {
            self.object_info.as_ref().cloned().map_err(Clone::clone)
        }

        async fn get_history(
            &self,
            _prompt_id: &str,
        ) -> Result<crate::application::ports::ComfyHistory, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible(
                "generation is not used by ComfyService tests".to_owned(),
            ))
        }

        async fn download_output(
            &self,
            _file: &crate::application::ports::ComfyOutputFile,
        ) -> Result<crate::application::ports::ComfyOutputData, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible(
                "generation is not used by ComfyService tests".to_owned(),
            ))
        }

        async fn submit_workflow(
            &self,
            _client_id: &str,
            _prompt_id: &str,
            _workflow: serde_json::Value,
        ) -> Result<crate::application::ports::PromptSubmission, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible(
                "generation is not used by ComfyService tests".to_owned(),
            ))
        }

        async fn subscribe_events(
            &self,
            _client_id: &str,
        ) -> Result<Box<dyn crate::application::ports::ComfyEventSubscription>, ComfyAdapterError>
        {
            Err(ComfyAdapterError::Incompatible(
                "generation is not used by ComfyService tests".to_owned(),
            ))
        }
    }

    async fn fingerprint_for(object_info: Value) -> String {
        let adapter = FakeAdapter {
            stats: Err(ComfyAdapterError::Offline("not used".to_owned())),
            object_info: Ok(object_info),
        };
        let service = ComfyService::new(Arc::new(adapter), &ComfyConnectionConfig::default());
        service
            .refresh_capabilities()
            .await
            .expect("object_info capability refresh should succeed");
        service
            .capability_diagnostics()
            .await
            .expect("capability fingerprint should be cached")
            .0
    }

    #[tokio::test]
    async fn refreshes_capability_summary_without_exposing_raw_object_info() {
        let adapter = FakeAdapter {
            stats: Ok(SystemStats {
                comfyui_version: Some("test".to_owned()),
                python_version: None,
                os: None,
                ram_total: None,
                ram_free: None,
                devices: vec![DeviceInfo {
                    name: Some("Test GPU".to_owned()),
                    device_type: Some("cuda".to_owned()),
                    vram_total: Some(10),
                    vram_free: Some(5),
                }],
            }),
            object_info: Ok(json!({"KSampler": {}, "CLIPTextEncode": {}, "SaveImage": {}})),
        };
        let service = ComfyService::new(Arc::new(adapter), &ComfyConnectionConfig::default());

        let summary = service
            .refresh_capabilities()
            .await
            .expect("capability refresh should succeed");
        let status = service.get_status().await.expect("status should succeed");

        assert_eq!(summary.node_count, 3);
        assert!(status.capability.is_some());
        assert!(serde_json::to_value(&status)
            .expect("status should serialize")
            .get("rawObjectInfo")
            .is_none());
        assert!(matches!(status.status, ComfyConnectionStatus::Connected));
    }

    #[tokio::test]
    async fn capability_fingerprint_ignores_object_key_order_recursively() {
        let first: Value = serde_json::from_str(
            r#"{"KSampler":{"input":{"required":{"steps":["INT",{"default":20,"min":1}],"cfg":["FLOAT",{"default":8.0}]}}},"LoadImage":{"input":{"required":{"image":["STRING"]}}}}"#,
        )
        .unwrap();
        let reordered: Value = serde_json::from_str(
            r#"{"LoadImage":{"input":{"required":{"image":["STRING"]}}},"KSampler":{"input":{"required":{"cfg":["FLOAT",{"default":8.0}],"steps":["INT",{"min":1,"default":20}]}}}}"#,
        )
        .unwrap();

        assert_eq!(
            fingerprint_for(first).await,
            fingerprint_for(reordered).await
        );
        assert_ne!(
            fingerprint_for(json!({"EnumNode": {"choices": ["first", "second"]}})).await,
            fingerprint_for(json!({"EnumNode": {"choices": ["second", "first"]}})).await,
            "schema arrays preserve order"
        );
    }

    #[tokio::test]
    async fn capability_fingerprint_changes_when_node_classes_are_added_or_removed() {
        let base = json!({"KSampler": {}, "LoadImage": {}});
        let added = json!({"KSampler": {}, "LoadImage": {}, "SaveVideo": {}});
        let removed = json!({"KSampler": {}});

        let base_fingerprint = fingerprint_for(base).await;
        assert_ne!(base_fingerprint, fingerprint_for(added).await);
        assert_ne!(base_fingerprint, fingerprint_for(removed).await);
    }

    #[tokio::test]
    async fn capability_fingerprint_changes_when_node_schema_changes() {
        let before = json!({
            "KSampler": {"input": {"required": {"steps": ["INT", {"default": 20}]}}}
        });
        let after = json!({
            "KSampler": {"input": {"required": {"steps": ["INT", {"default": 22}]}}}
        });

        assert_ne!(fingerprint_for(before).await, fingerprint_for(after).await);
    }

    #[tokio::test]
    async fn offline_adapter_error_becomes_offline_status() {
        let adapter = FakeAdapter {
            stats: Err(ComfyAdapterError::Offline("connection refused".to_owned())),
            object_info: Ok(json!({})),
        };
        let service = ComfyService::new(Arc::new(adapter), &ComfyConnectionConfig::default());

        let status = service
            .get_status()
            .await
            .expect("offline is a normal status");

        assert!(matches!(status.status, ComfyConnectionStatus::Offline));
    }
}
