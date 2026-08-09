use crate::application::ports::{AppSettings, LoadedSettings, SettingsStore};
use crate::error::AppError;
use async_trait::async_trait;
use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
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
                tracing::warn!(error = %error, path = %self.path.display(), "settings file could not be read");
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
            Ok(_) => LoadedSettings {
                settings: AppSettings::default(),
                warning: Some(SETTINGS_READ_WARNING.to_owned()),
            },
            Err(error) => {
                tracing::warn!(error = %error, path = %self.path.display(), "settings JSON is invalid");
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

        if let Err(error) = fs::rename(&temp_path, &self.path) {
            // Windows cannot rename over an existing file. Remove the old
            // file only after the replacement has been fully written.
            if self.path.exists() {
                fs::remove_file(&self.path)
                    .and_then(|_| fs::rename(&temp_path, &self.path))
                    .map_err(|replace_error| {
                        AppError::filesystem(format!("设置文件替换失败：{replace_error}"))
                    })?;
            } else {
                let _ = fs::remove_file(&temp_path);
                return Err(AppError::filesystem(error.to_string()));
            }
        }
        if let Ok(directory) = File::open(parent) {
            let _ = directory.sync_all();
        }
        Ok(())
    }
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
    use crate::application::ports::{AppSettings, ComfySettings, SettingsStore};
    use tempfile::tempdir;

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
        };
        store.save(&settings).await.unwrap();
        assert!(directory.path().join("settings.json").is_file());
        assert_eq!(store.load().await.settings, settings);
        assert!(!directory.path().join("settings.tmp").exists());
    }
}
