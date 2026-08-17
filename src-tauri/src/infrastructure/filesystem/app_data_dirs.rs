use crate::error::AppError;
use std::{ffi::OsString, fs, path::PathBuf};

const DATA_ROOT_ENV: &str = "AI_STUDIO_DATA_ROOT";

pub fn configured_data_root() -> Option<PathBuf> {
    resolve_data_root_from(std::env::var_os(DATA_ROOT_ENV))
}

pub fn resolve_data_root(default_root: PathBuf) -> PathBuf {
    configured_data_root().unwrap_or(default_root)
}

fn resolve_data_root_from(value: Option<OsString>) -> Option<PathBuf> {
    value
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppDataDirs {
    pub root: PathBuf,
    pub database: PathBuf,
    pub projects: PathBuf,
    pub workflow_library: PathBuf,
    pub workflow_staging: PathBuf,
    pub cache: PathBuf,
    pub logs: PathBuf,
    pub config: PathBuf,
}

impl AppDataDirs {
    pub fn initialize(root: PathBuf) -> Result<Self, AppError> {
        let directories = Self {
            database: root.join("app.db"),
            projects: root.join("projects"),
            workflow_library: root.join("workflow_library"),
            workflow_staging: root.join("workflow_staging"),
            cache: root.join("cache"),
            logs: root.join("logs"),
            config: root.join("config"),
            root,
        };

        for (name, path) in [
            ("root", directories.root.as_path()),
            ("projects", directories.projects.as_path()),
            ("workflow_library", directories.workflow_library.as_path()),
            ("workflow_staging", directories.workflow_staging.as_path()),
            ("cache", directories.cache.as_path()),
            ("config", directories.config.as_path()),
        ] {
            fs::create_dir_all(path)
                .map_err(|_| AppError::filesystem(format!("failed to create {name} directory")))?;
        }

        if let Err(error) = fs::create_dir_all(&directories.logs) {
            tracing::warn!(
                directory = "logs",
                error_type = std::any::type_name_of_val(&error),
                "persistent logging directory is unavailable"
            );
        }

        Ok(directories)
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_data_root_from, AppDataDirs};
    use std::ffi::OsString;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn app_data_dirs_construct_expected_paths() {
        let temporary_directory = tempdir().expect("temporary directory should be created");
        let root = temporary_directory.path().join("AIStudioData");

        let directories =
            AppDataDirs::initialize(root.clone()).expect("data dirs should initialize");

        assert_eq!(directories.root, root);
        assert_eq!(directories.database, root.join("app.db"));
        assert_eq!(directories.projects, root.join("projects"));
        assert_eq!(directories.workflow_library, root.join("workflow_library"));
        assert_eq!(directories.workflow_staging, root.join("workflow_staging"));
        assert_eq!(directories.cache, root.join("cache"));
        assert_eq!(directories.logs, root.join("logs"));
        assert_eq!(directories.config, root.join("config"));
    }

    #[test]
    fn app_data_dirs_create_runtime_directories() {
        let temporary_directory = tempdir().expect("temporary directory should be created");
        let directories = AppDataDirs::initialize(temporary_directory.path().join("AIStudioData"))
            .expect("data dirs should initialize");

        assert!(directories.root.is_dir());
        assert!(directories.projects.is_dir());
        assert!(directories.workflow_library.is_dir());
        assert!(directories.workflow_staging.is_dir());
        assert!(directories.cache.is_dir());
        assert!(directories.logs.is_dir());
        assert!(directories.config.is_dir());
        assert!(!directories.database.exists());
    }

    #[test]
    fn data_root_override_accepts_only_non_empty_absolute_paths() {
        let override_root = PathBuf::from("C:/isolated/AIStudioData");

        assert_eq!(
            resolve_data_root_from(Some(override_root.as_os_str().to_os_string())),
            Some(override_root.clone())
        );
        assert_eq!(resolve_data_root_from(Some(OsString::new())), None);
        assert_eq!(
            resolve_data_root_from(Some(OsString::from("relative-data-root"))),
            None
        );
        assert_eq!(resolve_data_root_from(None), None);
    }
}
