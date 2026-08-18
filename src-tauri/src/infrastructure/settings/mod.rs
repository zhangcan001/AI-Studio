use crate::application::ports::{AppSettings, LoadedSettings, SettingsStore};
use crate::error::AppError;
use async_trait::async_trait;
use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

const SETTINGS_READ_WARNING: &str = "设置文件无法读取，当前已使用默认配置。";

pub struct JsonSettingsStore {
    path: PathBuf,
}

impl JsonSettingsStore {
    pub fn new(config_dir: PathBuf) -> Self {
        Self {
            path: config_dir.join("settings.json"),
        }
    }

    fn load_sync(&self) -> LoadedSettings {
        let contents = match fs::read_to_string(&self.path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return LoadedSettings {
                    settings: AppSettings::default(),
                    warning: None,
                };
            }
            Err(error) => {
                tracing::warn!(error_kind = ?error.kind(), "settings file could not be read");
                return LoadedSettings {
                    settings: AppSettings::default(),
                    warning: Some(SETTINGS_READ_WARNING.to_owned()),
                };
            }
        };

        match serde_json::from_str::<AppSettings>(&contents) {
            Ok(settings) if settings.schema_version == 1 => LoadedSettings {
                settings,
                warning: None,
            },
            Ok(_) => {
                tracing::warn!(
                    error_kind = "unsupported_schema",
                    "settings JSON is unsupported"
                );
                LoadedSettings {
                    settings: AppSettings::default(),
                    warning: Some(SETTINGS_READ_WARNING.to_owned()),
                }
            }
            Err(_error) => {
                tracing::warn!(error_kind = "invalid_json", "settings JSON is invalid");
                LoadedSettings {
                    settings: AppSettings::default(),
                    warning: Some(SETTINGS_READ_WARNING.to_owned()),
                }
            }
        }
    }

    fn save_sync(&self, settings: &AppSettings) -> Result<(), AppError> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| AppError::filesystem("设置目录不可用"))?;
        fs::create_dir_all(parent).map_err(|error| AppError::filesystem(error.to_string()))?;
        let temp_path = parent.join(format!("settings.{}.tmp", Uuid::new_v4()));
        let bytes = serde_json::to_vec_pretty(settings)
            .map_err(|error| AppError::internal(format!("设置序列化失败：{error}")))?;
        let write_result = (|| -> Result<(), AppError> {
            let mut file = File::create(&temp_path)
                .map_err(|error| AppError::filesystem(error.to_string()))?;
            file.write_all(&bytes)
                .map_err(|error| AppError::filesystem(error.to_string()))?;
            file.sync_all()
                .map_err(|error| AppError::filesystem(error.to_string()))?;
            Ok(())
        })();
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }

        if let Err(error) = replace_settings_file(&temp_path, &self.path) {
            let _ = fs::remove_file(&temp_path);
            return Err(AppError::filesystem(format!("设置文件替换失败：{error}")));
        }
        if let Ok(directory) = File::open(parent) {
            let _ = directory.sync_all();
        }
        Ok(())
    }
}

#[cfg(windows)]
fn replace_settings_file(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_settings_file(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)
}

#[async_trait]
impl SettingsStore for JsonSettingsStore {
    async fn load(&self) -> LoadedSettings {
        self.load_sync()
    }

    async fn save(&self, settings: &AppSettings) -> Result<(), AppError> {
        self.save_sync(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::JsonSettingsStore;
    use crate::application::ports::{AppSettings, ComfySettings, SettingsStore, WorkspaceResume};
    use std::{
        io::Write,
        sync::{Arc, Mutex},
    };
    use tempfile::tempdir;

    #[derive(Clone)]
    struct LogBuffer(Arc<Mutex<Vec<u8>>>);

    impl Write for LogBuffer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn missing_settings_use_default_without_warning() {
        let directory = tempdir().unwrap();
        let store = JsonSettingsStore::new(directory.path().to_owned());
        let loaded = store.load().await;
        assert_eq!(loaded.settings, AppSettings::default());
        assert!(loaded.warning.is_none());
    }

    #[tokio::test]
    async fn valid_settings_and_unknown_fields_are_tolerated() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{"schemaVersion":1,"comfy":{"endpoint":"http://localhost:8188","future":true},"future":"ignored"}"#,
        )
        .unwrap();
        let loaded = JsonSettingsStore::new(directory.path().to_owned())
            .load()
            .await;
        assert_eq!(loaded.settings.comfy.endpoint, "http://localhost:8188");
        assert!(loaded.settings.comfy_environment_profiles.is_empty());
        assert!(loaded.warning.is_none());
    }

    #[tokio::test]
    async fn legacy_settings_default_workspace_resume_without_schema_bump() {
        let directory = tempdir().unwrap();
        std::fs::write(
            directory.path().join("settings.json"),
            r#"{"schemaVersion":1,"comfy":{"endpoint":"http://localhost:8188"}}"#,
        )
        .unwrap();

        let loaded = JsonSettingsStore::new(directory.path().to_owned())
            .load()
            .await;
        assert_eq!(loaded.settings.workspace_resume, WorkspaceResume::default());
        assert!(loaded.warning.is_none());
    }

    #[tokio::test]
    async fn invalid_json_uses_default_and_does_not_overwrite_file() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("settings.json");
        std::fs::write(&path, "{not-json").unwrap();
        let store = JsonSettingsStore::new(directory.path().to_owned());
        let loaded = store.load().await;
        assert_eq!(loaded.settings, AppSettings::default());
        assert!(loaded.warning.is_some());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "{not-json");
    }

    #[tokio::test]
    async fn save_uses_json_and_round_trips() {
        let directory = tempdir().unwrap();
        let store = JsonSettingsStore::new(directory.path().to_owned());
        let settings = AppSettings {
            schema_version: 1,
            comfy: ComfySettings {
                endpoint: "https://lan-host:9443".to_owned(),
            },
            workspace_resume: WorkspaceResume::default(),
            preferred_presets: std::collections::BTreeMap::new(),
            runtime_profiles: Vec::new(),
            production_queue_name_presets: Vec::new(),
            comfy_environment_profiles: Vec::new(),
        };
        store.save(&settings).await.unwrap();
        assert!(directory.path().join("settings.json").is_file());
        assert_eq!(store.load().await.settings, settings);
        assert!(!directory.path().join("settings.tmp").exists());
    }

    #[tokio::test]
    async fn environment_profiles_round_trip_without_schema_bump() {
        let directory = tempdir().unwrap();
        let store = JsonSettingsStore::new(directory.path().to_owned());
        let profile = crate::application::ports::ComfyEnvironmentProfile {
            id: "env-1".to_owned(),
            name: "WorkFisher".to_owned(),
            endpoint: "http://127.0.0.1:8188".to_owned(),
            created_at: "2026-08-17T00:00:00Z".to_owned(),
            updated_at: "2026-08-17T00:00:00Z".to_owned(),
        };
        let mut settings = AppSettings::default();
        settings.comfy_environment_profiles = vec![profile.clone()];

        store.save(&settings).await.unwrap();

        let loaded = store.load().await;
        assert_eq!(loaded.settings.schema_version, 1);
        assert_eq!(loaded.settings.comfy_environment_profiles, vec![profile]);
    }

    #[tokio::test]
    async fn workspace_resume_round_trips_without_schema_bump() {
        let directory = tempdir().unwrap();
        let store = JsonSettingsStore::new(directory.path().to_owned());
        let mut settings = AppSettings::default();
        settings.workspace_resume = WorkspaceResume {
            last_project_id: Some("project-1".to_owned()),
            last_workspace: Some("shots".to_owned()),
            last_shot_id: Some("shot-2".to_owned()),
        };

        store.save(&settings).await.unwrap();

        assert_eq!(store.load().await.settings, settings);
    }

    #[tokio::test]
    async fn successful_replacement_keeps_only_complete_new_settings() {
        let directory = tempdir().unwrap();
        let store = JsonSettingsStore::new(directory.path().to_owned());
        let old = AppSettings::default();
        let new = AppSettings {
            schema_version: 1,
            comfy: ComfySettings {
                endpoint: "http://localhost:8188".to_owned(),
            },
            workspace_resume: WorkspaceResume::default(),
            preferred_presets: std::collections::BTreeMap::new(),
            runtime_profiles: Vec::new(),
            production_queue_name_presets: Vec::new(),
            comfy_environment_profiles: Vec::new(),
        };
        store.save(&old).await.unwrap();
        store.save(&new).await.unwrap();
        assert_eq!(store.load().await.settings, new);
    }

    #[tokio::test]
    async fn replacement_failure_keeps_existing_target_entry() {
        let directory = tempdir().unwrap();
        let target = directory.path().join("settings.json");
        std::fs::write(&target, br#"{"schemaVersion":1}"#).unwrap();
        let source = directory.path().join("settings-source.tmp");
        let _error = super::replace_settings_file(&source, &target).unwrap_err();
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            r#"{"schemaVersion":1}"#
        );
    }

    #[test]
    fn invalid_settings_warning_does_not_write_absolute_path_to_raw_log() {
        let directory = tempdir().unwrap();
        let private_directory = directory.path().join("PRIVATE_USER");
        std::fs::create_dir_all(&private_directory).unwrap();
        std::fs::write(private_directory.join("settings.json"), "{not-json").unwrap();
        let store = JsonSettingsStore::new(private_directory);
        let output = Arc::new(Mutex::new(Vec::new()));
        let output_for_writer = output.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_target(false)
            .with_writer(move || LogBuffer(output_for_writer.clone()))
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            let loaded = store.load_sync();
            assert!(loaded.warning.is_some());
        });

        let raw_log = String::from_utf8(output.lock().unwrap().clone()).unwrap();
        assert!(!raw_log.contains("PRIVATE_USER"));
        assert!(!raw_log.contains("C:\\Users\\"));
        assert!(!raw_log.contains("settings.json"));
    }
}
