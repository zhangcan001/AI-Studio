use crate::application::ports::{
    ComfyAdapter, ComfyAdapterError, ComfyConnectionConfig, DeviceInfo, SystemStats,
};
use crate::error::AppError;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use std::{collections::HashSet, sync::Arc};
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
    pub node_count: usize,
    pub node_classes: HashSet<String>,
    pub raw_object_info: Value,
    pub captured_at: DateTime<Utc>,
}

impl CapabilityCache {
    fn summary(&self) -> CapabilitySummary {
        debug_assert_eq!(self.node_count, self.node_classes.len());
        debug_assert!(self.raw_object_info.is_object());

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
    adapter: Arc<dyn ComfyAdapter>,
    endpoint: String,
    capability_cache: Arc<RwLock<Option<CapabilityCache>>>,
}

impl ComfyService {
    pub fn new(adapter: Arc<dyn ComfyAdapter>, config: &ComfyConnectionConfig) -> Self {
        Self {
            adapter,
            endpoint: config.endpoint(),
            capability_cache: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn get_status(&self) -> Result<ComfyStatusView, AppError> {
        let cached_capability = self.cached_capability().await;

        match self.adapter.health_check().await {
            Ok(health) => Ok(self.connected_status(health.system, cached_capability)),
            Err(error) => {
                tracing::warn!(
                    endpoint = %self.endpoint,
                    error_type = error.kind(),
                    error = %error,
                    "ComfyUI health check failed"
                );

                Ok(ComfyStatusView {
                    status: status_for_adapter_error(&error),
                    endpoint: self.endpoint.clone(),
                    comfyui_version: None,
                    system: None,
                    devices: Vec::new(),
                    capability: cached_capability,
                })
            }
        }
    }

    pub async fn refresh_capabilities(&self) -> Result<CapabilitySummary, AppError> {
        let object_info = self.adapter.get_object_info().await.map_err(|error| {
            tracing::warn!(
                endpoint = %self.endpoint,
                error_type = error.kind(),
                error = %error,
                "ComfyUI capability refresh failed"
            );
            app_error_for_adapter_error(error)
        })?;

        let object = object_info.as_object().ok_or_else(|| {
            AppError::comfy_protocol_error("ComfyUI object_info response is not an object")
        })?;
        let node_classes = object.keys().cloned().collect::<HashSet<_>>();
        let cache = CapabilityCache {
            node_count: node_classes.len(),
            node_classes,
            raw_object_info: object_info,
            captured_at: Utc::now(),
        };
        let summary = cache.summary();

        *self.capability_cache.write().await = Some(cache);

        tracing::info!(
            endpoint = %self.endpoint,
            node_count = summary.node_count,
            "ComfyUI capability cache refreshed"
        );

        Ok(summary)
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
    ) -> ComfyStatusView {
        ComfyStatusView {
            status: ComfyConnectionStatus::Connected,
            endpoint: self.endpoint.clone(),
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
        | ComfyAdapterError::ImageUpload(_) => ComfyConnectionStatus::Incompatible,
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
        | ComfyAdapterError::ImageUpload(_) => {
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
    use serde_json::json;
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
