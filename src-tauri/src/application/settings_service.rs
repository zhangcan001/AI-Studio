use crate::application::{
    comfy_service::{ComfyRuntime, ComfyService},
    diagnostics_service::{DiagnosticsService, RuntimeActivityStatusView},
    ports::{
        AppSettings, ComfyAdapter, ComfyAdapterError, ComfyAdapterFactory, ComfyConnectionConfig,
        RuntimeParameterProfile, SettingsStore,
    },
    production_queue_service::ProductionQueueService,
};
use crate::error::AppError;
use async_trait::async_trait;
use serde::Serialize;
use std::sync::{Arc, RwLock};
use tokio::sync::OwnedMutexGuard;

const ENDPOINT_CHANGE_BUSY_MESSAGE: &str =
    "当前仍有生成任务或生产队列正在运行，完成后才能切换 ComfyUI。";
const ENDPOINT_TEST_FAILED_MESSAGE: &str = "无法连接到该 ComfyUI 地址。";

#[async_trait]
pub trait RuntimeActivityProvider: Send + Sync {
    async fn runtime_activity_status(&self) -> Result<RuntimeActivityStatusView, AppError>;
}

#[async_trait]
impl RuntimeActivityProvider for DiagnosticsService {
    async fn runtime_activity_status(&self) -> Result<RuntimeActivityStatusView, AppError> {
        DiagnosticsService::runtime_activity_status(self).await
    }
}

#[async_trait]
pub trait RuntimeConfigurationAdmission: Send + Sync {
    async fn acquire(&self) -> OwnedMutexGuard<()>;
}

#[async_trait]
impl RuntimeConfigurationAdmission for ProductionQueueService {
    async fn acquire(&self) -> OwnedMutexGuard<()> {
        self.acquire_runtime_configuration_admission().await
    }
}

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
    activity_provider: Arc<dyn RuntimeActivityProvider>,
    configuration_admission: Arc<dyn RuntimeConfigurationAdmission>,
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
        activity_provider: Arc<dyn RuntimeActivityProvider>,
        configuration_admission: Arc<dyn RuntimeConfigurationAdmission>,
        adapter_factory: Arc<dyn ComfyAdapterFactory>,
    ) -> Self {
        Self {
            store,
            runtime,
            comfy_service,
            activity_provider,
            configuration_admission,
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

    pub fn preferred_preset(
        &self,
        project_id: &str,
        workflow_version_id: &str,
        recipe_id: &str,
    ) -> Option<String> {
        let settings = self
            .settings
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let key = crate::application::ports::settings_store::preferred_preset_key(
            project_id,
            workflow_version_id,
            recipe_id,
        );
        settings.preferred_presets.get(&key).cloned()
    }

    pub fn runtime_profiles(&self) -> Vec<RuntimeParameterProfile> {
        self.settings
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .runtime_profiles
            .clone()
    }

    pub fn production_queue_name_presets(&self) -> Vec<String> {
        self.settings
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .production_queue_name_presets
            .clone()
    }

    pub async fn save_production_queue_name_preset(
        &self,
        name: &str,
    ) -> Result<Vec<String>, AppError> {
        let name = validate_queue_name_preset(name)?;
        let mut next_settings = self
            .settings
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        next_settings
            .production_queue_name_presets
            .retain(|current| current.to_lowercase() != name.to_lowercase());
        next_settings.production_queue_name_presets.insert(0, name);
        next_settings.production_queue_name_presets.truncate(20);
        self.store
            .save(&next_settings)
            .await
            .map_err(|error| AppError::settings_save_failed(error.message))?;
        let presets = next_settings.production_queue_name_presets.clone();
        *self
            .settings
            .write()
            .unwrap_or_else(|error| error.into_inner()) = next_settings;
        Ok(presets)
    }

    pub async fn delete_production_queue_name_preset(&self, name: &str) -> Result<(), AppError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::invalid_input("队列名称模板不能为空。"));
        }
        let mut next_settings = self
            .settings
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        next_settings
            .production_queue_name_presets
            .retain(|current| current != name);
        self.store
            .save(&next_settings)
            .await
            .map_err(|error| AppError::settings_save_failed(error.message))?;
        *self
            .settings
            .write()
            .unwrap_or_else(|error| error.into_inner()) = next_settings;
        Ok(())
    }

    pub async fn save_runtime_profile(
        &self,
        mut profile: RuntimeParameterProfile,
    ) -> Result<RuntimeParameterProfile, AppError> {
        validate_runtime_profile(&profile)?;
        profile.id = profile.id.trim().to_owned();
        profile.workflow_version_id = profile.workflow_version_id.trim().to_owned();
        profile.recipe_id = profile.recipe_id.trim().to_owned();
        profile.name = profile.name.trim().to_owned();
        profile.updated_at = profile.updated_at.trim().to_owned();
        profile.values.retain(|key, _| !key.trim().is_empty());

        let mut next_settings = self
            .settings
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        next_settings
            .runtime_profiles
            .retain(|current| current.id != profile.id);
        next_settings.runtime_profiles.insert(0, profile.clone());
        self.store
            .save(&next_settings)
            .await
            .map_err(|error| AppError::settings_save_failed(error.message))?;
        *self
            .settings
            .write()
            .unwrap_or_else(|error| error.into_inner()) = next_settings;
        Ok(profile)
    }

    pub async fn delete_runtime_profile(&self, profile_id: &str) -> Result<(), AppError> {
        let profile_id = profile_id.trim();
        if profile_id.is_empty() {
            return Err(AppError::invalid_input("参数档案 ID 不能为空。"));
        }
        let mut next_settings = self
            .settings
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        next_settings
            .runtime_profiles
            .retain(|profile| profile.id != profile_id);
        self.store
            .save(&next_settings)
            .await
            .map_err(|error| AppError::settings_save_failed(error.message))?;
        *self
            .settings
            .write()
            .unwrap_or_else(|error| error.into_inner()) = next_settings;
        Ok(())
    }

    pub async fn set_preferred_preset(
        &self,
        project_id: &str,
        workflow_version_id: &str,
        recipe_id: &str,
        preset_id: Option<&str>,
    ) -> Result<(), AppError> {
        let key = crate::application::ports::settings_store::preferred_preset_key(
            project_id,
            workflow_version_id,
            recipe_id,
        );
        let mut next_settings = self
            .settings
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        match preset_id {
            Some(preset_id) => {
                next_settings
                    .preferred_presets
                    .insert(key, preset_id.to_owned());
            }
            None => {
                next_settings.preferred_presets.remove(&key);
            }
        }
        self.store
            .save(&next_settings)
            .await
            .map_err(|error| AppError::settings_save_failed(error.message))?;
        *self
            .settings
            .write()
            .unwrap_or_else(|error| error.into_inner()) = next_settings;
        Ok(())
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
        let adapter = self
            .adapter_factory
            .create(config.clone())
            .map_err(endpoint_test_error)?;
        // Test the candidate before acquiring the global gate so a slow remote
        // endpoint does not block generation or queue dispatch.
        test_adapter(&*adapter, config.endpoint()).await?;

        // The final activity check and every state-changing operation below
        // are serialized with generation_create, generation_create_batch,
        // production queue start, and recovery through the same admission gate.
        let _configuration_admission = self.configuration_admission.acquire().await;
        let activity = self.activity_provider.runtime_activity_status().await?;
        if activity.active_task_count > 0 || activity.production_busy {
            return Err(AppError::comfy_endpoint_change_busy(
                ENDPOINT_CHANGE_BUSY_MESSAGE,
            ));
        }

        // Recheck the candidate while the gate is held, immediately before
        // persisting and swapping the shared runtime adapter.
        adapter.health_check().await.map_err(endpoint_test_error)?;

        let preferred_presets = self
            .settings
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .preferred_presets
            .clone();
        let runtime_profiles = self
            .settings
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .runtime_profiles
            .clone();
        let production_queue_name_presets = self
            .settings
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .production_queue_name_presets
            .clone();
        let next_settings = AppSettings {
            schema_version: 1,
            comfy: crate::application::ports::ComfySettings {
                endpoint: config.endpoint(),
            },
            preferred_presets,
            runtime_profiles,
            production_queue_name_presets,
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

fn validate_queue_name_preset(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() || value.contains(['\r', '\n']) {
        return Err(AppError::invalid_input("队列名称模板必须是单行非空文本。"));
    }
    if value.chars().count() > 120 {
        return Err(AppError::invalid_input("队列名称模板最多 120 个字符。"));
    }
    Ok(value.to_owned())
}

fn validate_runtime_profile(profile: &RuntimeParameterProfile) -> Result<(), AppError> {
    if profile.id.trim().is_empty() {
        return Err(AppError::invalid_input("参数档案 ID 不能为空。"));
    }
    if profile.workflow_version_id.trim().is_empty() {
        return Err(AppError::invalid_input("参数档案必须关联工作流版本。"));
    }
    if profile.recipe_id.trim().is_empty() {
        return Err(AppError::invalid_input("参数档案必须关联 Recipe。"));
    }
    if profile.name.trim().is_empty() {
        return Err(AppError::invalid_input("参数档案名称不能为空。"));
    }
    if profile.name.chars().count() > 80 {
        return Err(AppError::invalid_input("参数档案名称最多 80 个字符。"));
    }
    if profile.updated_at.trim().is_empty() {
        return Err(AppError::invalid_input("参数档案更新时间不能为空。"));
    }
    if profile.values.keys().any(|key| key.trim().is_empty()) {
        return Err(AppError::invalid_input("参数档案包含空的字段键。"));
    }
    Ok(())
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
    use super::*;
    use crate::application::diagnostics_service::RuntimeActivityStatusView;
    use crate::application::ports::{
        AppSettings, ComfyConnectionConfig, ComfyEventSubscription, ComfyHealth, ComfyHistory,
        ComfyOutputData, ComfyOutputFile, LoadedSettings, PromptSubmission,
        RuntimeParameterProfile, SettingsStore, SystemStats,
    };
    use async_trait::async_trait;
    use serde_json::Value;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    };
    use tokio::sync::{Mutex as AsyncMutex, Notify};

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

    #[derive(Default)]
    struct MemorySettingsStore {
        saved: Mutex<Vec<AppSettings>>,
    }

    #[async_trait]
    impl SettingsStore for MemorySettingsStore {
        async fn load(&self) -> LoadedSettings {
            LoadedSettings {
                settings: AppSettings::default(),
                warning: None,
            }
        }

        async fn save(&self, settings: &AppSettings) -> Result<(), AppError> {
            self.saved.lock().unwrap().push(settings.clone());
            Ok(())
        }
    }

    struct TestActivityProvider {
        status: Mutex<RuntimeActivityStatusView>,
    }

    #[async_trait]
    impl RuntimeActivityProvider for TestActivityProvider {
        async fn runtime_activity_status(&self) -> Result<RuntimeActivityStatusView, AppError> {
            Ok(self.status.lock().unwrap().clone())
        }
    }

    struct TestAdmission {
        gate: Arc<AsyncMutex<()>>,
    }

    #[async_trait]
    impl RuntimeConfigurationAdmission for TestAdmission {
        async fn acquire(&self) -> OwnedMutexGuard<()> {
            Arc::clone(&self.gate).lock_owned().await
        }
    }

    #[derive(Clone)]
    struct BlockingAdapter {
        health_calls: Arc<AtomicUsize>,
        health_started: Arc<Notify>,
        allow_first_health: Arc<Notify>,
    }

    impl BlockingAdapter {
        fn new() -> Self {
            Self {
                health_calls: Arc::new(AtomicUsize::new(0)),
                health_started: Arc::new(Notify::new()),
                allow_first_health: Arc::new(Notify::new()),
            }
        }

        fn health(&self) -> ComfyHealth {
            ComfyHealth {
                system: SystemStats {
                    comfyui_version: Some("test".to_owned()),
                    python_version: None,
                    os: None,
                    ram_total: None,
                    ram_free: None,
                    devices: Vec::new(),
                },
            }
        }
    }

    #[async_trait]
    impl ComfyAdapter for BlockingAdapter {
        async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
            if self.health_calls.fetch_add(1, Ordering::SeqCst) == 0 {
                self.health_started.notify_one();
                self.allow_first_health.notified().await;
            }
            Ok(self.health())
        }

        async fn get_system_stats(&self) -> Result<SystemStats, ComfyAdapterError> {
            Ok(self.health().system)
        }

        async fn get_object_info(&self) -> Result<Value, ComfyAdapterError> {
            Ok(serde_json::json!({"TestNode": {}}))
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

    struct TestFactory {
        adapter: BlockingAdapter,
    }

    impl ComfyAdapterFactory for TestFactory {
        fn create(
            &self,
            _config: ComfyConnectionConfig,
        ) -> Result<Arc<dyn ComfyAdapter>, ComfyAdapterError> {
            Ok(Arc::new(self.adapter.clone()))
        }
    }

    fn test_settings_service(
        activity: Arc<TestActivityProvider>,
        admission: Arc<TestAdmission>,
        store: Arc<MemorySettingsStore>,
        adapter: BlockingAdapter,
    ) -> Arc<SettingsService> {
        let config = ComfyConnectionConfig::default();
        let runtime = Arc::new(ComfyRuntime::new(Arc::new(adapter.clone()), config));
        let comfy_service = Arc::new(ComfyService::from_runtime(runtime.clone()));
        Arc::new(SettingsService::new(
            store,
            LoadedSettings {
                settings: AppSettings::default(),
                warning: None,
            },
            runtime,
            comfy_service,
            activity,
            admission,
            Arc::new(TestFactory { adapter }),
        ))
    }

    async fn assert_busy_change_is_rejected(status: RuntimeActivityStatusView) {
        let adapter = BlockingAdapter::new();
        let activity = Arc::new(TestActivityProvider {
            status: Mutex::new(RuntimeActivityStatusView {
                active_task_count: 0,
                production_busy: false,
            }),
        });
        let admission = Arc::new(TestAdmission {
            gate: Arc::new(AsyncMutex::new(())),
        });
        let store = Arc::new(MemorySettingsStore::default());
        let service = test_settings_service(
            activity.clone(),
            admission.clone(),
            store.clone(),
            adapter.clone(),
        );

        let running = {
            let service = service.clone();
            tokio::spawn(async move { service.save_and_apply("http://localhost:8188").await })
        };
        adapter.health_started.notified().await;

        // Simulate a competing generation/queue operation winning the same
        // global admission gate while the candidate endpoint is being tested.
        let competing_gate = admission.acquire().await;
        *activity.status.lock().unwrap() = status;
        drop(competing_gate);
        adapter.allow_first_health.notify_one();

        let error = running.await.unwrap().unwrap_err();
        assert_eq!(error.code(), "COMFY_ENDPOINT_CHANGE_BUSY");
        assert_eq!(service.settings().endpoint, "http://127.0.0.1:8188");
        assert_eq!(store.saved.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn active_task_cannot_race_endpoint_apply_after_candidate_test() {
        assert_busy_change_is_rejected(RuntimeActivityStatusView {
            active_task_count: 1,
            production_busy: false,
        })
        .await;
    }

    #[tokio::test]
    async fn production_queue_cannot_race_endpoint_apply_after_candidate_test() {
        assert_busy_change_is_rejected(RuntimeActivityStatusView {
            active_task_count: 0,
            production_busy: true,
        })
        .await;
    }

    #[tokio::test]
    async fn runtime_profiles_are_saved_listed_and_deleted_with_settings() {
        let adapter = BlockingAdapter::new();
        let activity = Arc::new(TestActivityProvider {
            status: Mutex::new(RuntimeActivityStatusView {
                active_task_count: 0,
                production_busy: false,
            }),
        });
        let admission = Arc::new(TestAdmission {
            gate: Arc::new(AsyncMutex::new(())),
        });
        let store = Arc::new(MemorySettingsStore::default());
        let service = test_settings_service(activity, admission, store.clone(), adapter.clone());
        let profile = RuntimeParameterProfile {
            id: "profile-1".to_owned(),
            workflow_version_id: "wfv-1".to_owned(),
            recipe_id: "rcp-1".to_owned(),
            name: "预览".to_owned(),
            values: std::collections::BTreeMap::from([
                ("steps".to_owned(), 8),
                ("width".to_owned(), 512),
            ]),
            updated_at: "2026-08-10T00:00:00Z".to_owned(),
        };

        assert_eq!(
            service.save_runtime_profile(profile.clone()).await.unwrap(),
            profile
        );
        assert_eq!(service.runtime_profiles(), vec![profile]);
        assert_eq!(
            store
                .saved
                .lock()
                .unwrap()
                .last()
                .unwrap()
                .runtime_profiles
                .len(),
            1
        );

        service.delete_runtime_profile("profile-1").await.unwrap();
        assert!(service.runtime_profiles().is_empty());
        assert!(store
            .saved
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .runtime_profiles
            .is_empty());
    }

    #[tokio::test]
    async fn production_queue_name_presets_round_trip_without_task_data() {
        let adapter = BlockingAdapter::new();
        let activity = Arc::new(TestActivityProvider {
            status: Mutex::new(RuntimeActivityStatusView {
                active_task_count: 0,
                production_busy: false,
            }),
        });
        let admission = Arc::new(TestAdmission {
            gate: Arc::new(AsyncMutex::new(())),
        });
        let store = Arc::new(MemorySettingsStore::default());
        let service = test_settings_service(activity, admission, store.clone(), adapter);

        let saved = service
            .save_production_queue_name_preset(" 第02集 图片 ")
            .await
            .unwrap();
        assert_eq!(saved[0], "第02集 图片");
        assert!(service
            .production_queue_name_presets()
            .contains(&"第02集 图片".to_owned()));
        assert_eq!(
            store
                .saved
                .lock()
                .unwrap()
                .last()
                .unwrap()
                .runtime_profiles
                .len(),
            0
        );

        service
            .delete_production_queue_name_preset("第02集 图片")
            .await
            .unwrap();
        assert!(!service
            .production_queue_name_presets()
            .contains(&"第02集 图片".to_owned()));
    }
}
