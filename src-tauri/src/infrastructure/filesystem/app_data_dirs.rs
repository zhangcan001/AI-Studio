use crate::error::AppError;
use std::{fs, path::PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppDataDirs {
    pub root: PathBuf,
    pub database: PathBuf,
    pub projects: PathBuf,
    pub workflow_library: PathBuf,
    pub cache: PathBuf,
    pub logs: PathBuf,
}

impl AppDataDirs {
    pub fn initialize(root: PathBuf) -> Result<Self, AppError> {
        let directories = Self {
            database: root.join("app.db"),
            projects: root.join("projects"),
            workflow_library: root.join("workflow_library"),
            cache: root.join("cache"),
            logs: root.join("logs"),
            root,
        };

        for (name, path) in [
            ("root", directories.root.as_path()),
            ("projects", directories.projects.as_path()),
            ("workflow_library", directories.workflow_library.as_path()),
            ("cache", directories.cache.as_path()),
            ("logs", directories.logs.as_path()),
        ] {
            fs::create_dir_all(path).map_err(|error| {
                AppError::filesystem(format!(
                    "failed to create {name} directory at {}: {error}",
                    path.display()
                ))
            })?;
        }

        Ok(directories)
    }
}

#[cfg(test)]
mod tests {
    use super::AppDataDirs;
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
        assert_eq!(directories.cache, root.join("cache"));
        assert_eq!(directories.logs, root.join("logs"));
    }

    #[test]
    fn app_data_dirs_create_runtime_directories() {
        let temporary_directory = tempdir().expect("temporary directory should be created");
        let directories = AppDataDirs::initialize(temporary_directory.path().join("AIStudioData"))
            .expect("data dirs should initialize");

        assert!(directories.root.is_dir());
        assert!(directories.projects.is_dir());
        assert!(directories.workflow_library.is_dir());
        assert!(directories.cache.is_dir());
        assert!(directories.logs.is_dir());
        assert!(!directories.database.exists());
    }
}
