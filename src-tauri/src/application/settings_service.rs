use crate::application::{
    comfy_service::{ComfyRuntime, ComfyService},
    diagnostics_service::DiagnosticsService,
    ports::{
        AppSettings, ComfyAdapter, ComfyAdapterError, ComfyAdapterFactory, ComfyConnectionConfig,
        SettingsStore,
    },
};
use crate::error::AppError;
use serde::Serialize;
use std::sync::{Arc, RwLock};

const ENDPOINT_CHANGE_BUSY_MESSAGE: &str =
    "当前仍有生成任务或生产队列正在运行，完成后才能切换 ComfyUI。";
const ENDPOINT_TEST_FAILED_MESSAGE: &str = "无法连接到该 ComfyUI 地址。";

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub schema_version: u32,
    pub endpoint: String,
    pub warning: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EndpointTestView {
    pub connected: bool,
    pub endpoint: String,
    pub version: Option<String>,
    pub gpu: Vec<String>,
    pub vram_total: Option<u64>,
    pub vram_free: Option<u64>,
    pub node_count: usize,
}

pub struct SettingsService {
    store: Arc<dyn SettingsStore>,
    runtime: Arc<ComfyRuntime>,
    comfy_service: Arc<ComfyService>,
    diagnostics_service: Arc<DiagnosticsService>,
    adapter_factory: Arc<dyn ComfyAdapterFactory>,
    settings: RwLock<AppSettings>,
    warning: RwLock<Option<String>>,
}

impl SettingsService {
    pub fn new(
        store: Arc<dyn SettingsStore>,
        loaded: crate::application::ports::LoadedSettings,
        runtime: Arc<ComfyRuntime>,
        comfy_service: Arc<ComfyService>,
        diagnostics_service: Arc<DiagnosticsService>,
        adapter_factory: Arc<dyn ComfyAdapterFactory>,
    ) -> Self {
        Self {
            store,
            runtime,
            comfy_service,
            diagnostics_service,
            adapter_factory,
            settings: RwLock::new(loaded.settings),
            warning: RwLock::new(loaded.warning),
        }
    }

    pub fn settings(&self) -> SettingsView {
        let settings = self
            .settings
            .read()
            .unwrap_or_else(|error| error.into_inner());
        SettingsView {
            schema_version: settings.schema_version,
            endpoint: settings.comfy.endpoint.clone(),
            warning: self
                .warning
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .clone(),
        }
    }

    pub async fn test_connection(&self, endpoint: &str) -> Result<EndpointTestView, AppError> {
        let config = parse_endpoint(endpoint)?;
        let adapter = self
            .adapter_factory
            .create(config.clone())
            .map_err(endpoint_test_error)?;
        test_adapter(&*adapter, config.endpoint()).await
    }

    pub async fn save_and_apply(&self, endpoint: &str) -> Result<SettingsView, AppError> {
        let config = parse_endpoint(endpoint)?;
        let activity = self.diagnostics_service.runtime_activity_status().await?;
        if activity.active_task_count > 0 || activity.production_busy {
            return Err(AppError::comfy_endpoint_change_busy(
                ENDPOINT_CHANGE_BUSY_MESSAGE,
            ));
        }

        let adapter = self
            .adapter_factory
            .create(config.clone())
            .map_err(endpoint_test_error)?;
        // Test both endpoints before persisting or swapping the shared runtime.
        test_adapter(&*adapter, config.endpoint()).await?;

        let next_settings = AppSettings {
            schema_version: 1,
            comfy: crate::application::ports::ComfySettings {
                endpoint: config.endpoint(),
            },
        };
        self.store
            .save(&next_settings)
            .await
            .map_err(|error| AppError::settings_save_failed(error.message))?;

        self.runtime.replace(config, adapter);
        self.comfy_service.invalidate_capabilities().await;
        {
            *self
                .settings
                .write()
                .unwrap_or_else(|error| error.into_inner()) = next_settings;
            *self
                .warning
                .write()
                .unwrap_or_else(|error| error.into_inner()) = None;
        }
        // A successful test already captured object_info; refreshing the shared
        // cache keeps status/capability consumers consistent after the swap.
        if let Err(error) = self.comfy_service.refresh_capabilities().await {
            tracing::warn!(
                error_code = error.code(),
                "capability refresh after endpoint switch failed"
            );
        }
        Ok(self.settings())
    }
}

fn parse_endpoint(endpoint: &str) -> Result<ComfyConnectionConfig, AppError> {
    ComfyConnectionConfig::from_endpoint(endpoint)
        .map_err(|error| AppError::comfy_endpoint_invalid(error.to_string()))
}

async fn test_adapter(
    adapter: &dyn ComfyAdapter,
    endpoint: String,
) -> Result<EndpointTestView, AppError> {
    let health = adapter.health_check().await.map_err(endpoint_test_error)?;
    let object_info = adapter
        .get_object_info()
        .await
        .map_err(endpoint_test_error)?;
    let node_count = object_info
        .as_object()
        .ok_or_else(|| AppError::comfy_endpoint_test_failed(ENDPOINT_TEST_FAILED_MESSAGE))?
        .len();
    let gpu = health
        .system
        .devices
        .iter()
        .filter_map(|device| device.name.clone())
        .collect::<Vec<_>>();
    let vram_total = sum_devices(&health.system.devices, |device| device.vram_total);
    let vram_free = sum_devices(&health.system.devices, |device| device.vram_free);
    Ok(EndpointTestView {
        connected: true,
        endpoint,
        version: health.system.comfyui_version,
        gpu,
        vram_total,
        vram_free,
        node_count,
    })
}

fn sum_devices(
    devices: &[crate::application::ports::DeviceInfo],
    get: impl Fn(&crate::application::ports::DeviceInfo) -> Option<u64>,
) -> Option<u64> {
    let values = devices.iter().filter_map(get).collect::<Vec<_>>();
    (!values.is_empty()).then(|| values.into_iter().fold(0, u64::saturating_add))
}

fn endpoint_test_error(error: ComfyAdapterError) -> AppError {
    tracing::warn!(error_type = error.kind(), "ComfyUI endpoint test failed");
    AppError::comfy_endpoint_test_failed(ENDPOINT_TEST_FAILED_MESSAGE)
}

#[cfg(test)]
mod tests {
    use super::parse_endpoint;
    use crate::application::ports::ComfyConnectionConfig;

    #[test]
    fn endpoint_validation_normalizes_and_rejects_unsafe_forms() {
        assert_eq!(
            parse_endpoint(" http://localhost:8188/ ").unwrap(),
            ComfyConnectionConfig::new("http", "localhost", 8188)
        );
        for endpoint in [
            "ws://localhost:8188",
            "file:///tmp/comfy",
            "ftp://localhost:8188",
            "http://user:pass@localhost:8188",
            "http://localhost:8188?token=secret",
            "http://localhost:8188/#fragment",
        ] {
            assert!(
                parse_endpoint(endpoint).is_err(),
                "{endpoint} must be rejected"
            );
        }
    }
}
