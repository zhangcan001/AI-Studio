use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

const DATA_ROOT_ENV: &str = "AI_STUDIO_DATA_ROOT";
const DATA_ROOT_POINTER_FILE: &str = "data_root.json";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DataRootPointer {
    data_root: String,
}

pub fn pointer_file_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_DATA_HOME"))
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .map(|base| base.join("AIStudio").join(DATA_ROOT_POINTER_FILE))
}

pub fn read_persisted_data_root() -> Option<PathBuf> {
    let path = pointer_file_path()?;
    let contents = fs::read_to_string(path).ok()?;
    let parsed: DataRootPointer = serde_json::from_str(&contents).ok()?;
    let candidate = PathBuf::from(parsed.data_root.trim());
    candidate.is_absolute().then_some(candidate)
}

pub fn write_persisted_data_root(target: &Path) -> Result<(), AppError> {
    if !target.is_absolute() {
        return Err(AppError::invalid_input("数据目录必须是绝对路径。"));
    }
    fs::create_dir_all(target)
        .map_err(|error| AppError::filesystem(format!("无法创建目标数据目录：{error}")))?;
    let pointer =
        pointer_file_path().ok_or_else(|| AppError::filesystem("无法定位本地数据指针目录。"))?;
    atomic_json(
        &pointer,
        &DataRootPointer {
            data_root: target.to_string_lossy().into_owned(),
        },
    )
}

fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), AppError> {
    use std::io::Write;
    let parent = path
        .parent()
        .ok_or_else(|| AppError::filesystem("配置目录不可用"))?;
    fs::create_dir_all(parent).map_err(|e| AppError::filesystem(e.to_string()))?;
    let temporary = parent.join(format!(".data-root-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|e| AppError::filesystem(e.to_string()))?;
        let bytes = serde_json::to_vec(value).map_err(|e| AppError::internal(e.to_string()))?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| AppError::filesystem(e.to_string()))?;
        drop(file);
        crate::infrastructure::settings::replace_settings_file(&temporary, path)
            .map_err(|e| AppError::filesystem(e.to_string()))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[derive(Serialize, Deserialize)]
struct PendingMigration {
    source: PathBuf,
    target: PathBuf,
}
fn migration_pointer() -> Option<PathBuf> {
    pointer_file_path().map(|p| p.with_file_name("data_root_migration.json"))
}

pub fn pending_data_root() -> Option<PathBuf> {
    let request: PendingMigration =
        serde_json::from_slice(&fs::read(migration_pointer()?).ok()?).ok()?;
    Some(request.target)
}

fn validate_migration_target(source: &Path, target: &Path) -> Result<(), AppError> {
    if !target.is_absolute() || !source.is_absolute() {
        return Err(AppError::invalid_input("迁移目录必须为绝对路径"));
    }
    if target.exists() {
        return Err(AppError::invalid_input(
            "目标 AIStudioData 已存在，请选择没有同名数据目录的位置。",
        ));
    }
    let parent = target
        .parent()
        .and_then(|p| p.canonicalize().ok())
        .ok_or_else(|| AppError::invalid_input("迁移目标目录不可用"))?;
    let source = source
        .canonicalize()
        .map_err(|_| AppError::filesystem("当前数据目录不可用"))?;
    if parent.starts_with(&source) {
        return Err(AppError::invalid_input("不能迁移到当前数据目录内部"));
    }
    Ok(())
}

pub fn schedule_data_root_migration(source: &Path, target: &Path) -> Result<(), AppError> {
    if std::env::var_os(DATA_ROOT_ENV).is_some_and(|v| !v.is_empty()) {
        return Err(AppError::invalid_input(
            "环境变量指定数据目录时不能安排迁移",
        ));
    }
    validate_migration_target(source, target)?;
    let pointer = migration_pointer().ok_or_else(|| AppError::filesystem("无法定位迁移配置"))?;
    atomic_json(
        &pointer,
        &PendingMigration {
            source: source.to_owned(),
            target: target.to_owned(),
        },
    )
}

/// Called only during startup, before the application opens its database or services.
pub async fn complete_pending_data_root_migration(current: &Path) -> Result<PathBuf, AppError> {
    if std::env::var_os(DATA_ROOT_ENV).is_some_and(|v| !v.is_empty()) {
        return Ok(current.to_owned());
    }
    let Some(pointer) = migration_pointer() else {
        return Ok(current.to_owned());
    };
    complete_migration_at(current, &pointer).await
}

struct OwnedMigrationStaging(PathBuf);
impl Drop for OwnedMigrationStaging {
    fn drop(&mut self) {
        let (Ok(meta), Some(parent)) = (fs::symlink_metadata(&self.0), self.0.parent()) else {
            return;
        };
        let (Ok(owned), Ok(parent)) = (self.0.canonicalize(), parent.canonicalize()) else {
            return;
        };
        if meta.is_dir()
            && !meta.file_type().is_symlink()
            && owned.parent() == Some(parent.as_path())
            && owned
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with(".ai-studio-migration-"))
        {
            let _ = fs::remove_dir_all(owned);
        }
    }
}

async fn complete_migration_at(current: &Path, pointer: &Path) -> Result<PathBuf, AppError> {
    if !pointer.exists() {
        return Ok(current.to_owned());
    }
    let request: PendingMigration = serde_json::from_slice(
        &fs::read(pointer).map_err(|e| AppError::filesystem(e.to_string()))?,
    )
    .map_err(|_| AppError::invalid_input("迁移配置损坏，请重新选择迁移目录"))?;
    if request.source.canonicalize().ok() != current.canonicalize().ok() {
        return Err(AppError::invalid_input("数据目录已改变，请重新安排迁移"));
    }
    validate_migration_target(current, &request.target)?;
    if let Some((free, _)) = disk_space_bytes(&request.target) {
        if free < directory_size_bytes(current).saturating_add(256 * 1024 * 1024) {
            return Err(AppError::filesystem("目标磁盘空间不足，继续使用原数据目录"));
        }
    }
    // A new owned staging directory prevents partial copies or existing destination overwrites.
    let staging = request
        .target
        .parent()
        .unwrap()
        .join(format!(".ai-studio-migration-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&staging).map_err(|e| AppError::filesystem(e.to_string()))?;
    let _owned_staging = OwnedMigrationStaging(staging.clone());
    for name in ["projects", "workflow_library", "config"] {
        let source = current.join(name);
        if source.exists() {
            copy_migration_directory(&source, &staging.join(name))?;
        }
    }
    if current.join("app.db").exists() {
        crate::infrastructure::database::snapshot_database_file(
            &current.join("app.db"),
            &staging.join("app.db"),
        )
        .await?;
        crate::infrastructure::database::rebase_migration_paths(
            &staging.join("app.db"),
            current,
            &request.target,
        )
        .await?;
    }
    // Publish the complete owned directory in one same-volume rename.
    if request.target.exists() {
        return Err(AppError::filesystem(
            "目标目录已出现，已停止迁移以保护现有数据",
        ));
    }
    fs::rename(&staging, &request.target).map_err(|e| AppError::filesystem(e.to_string()))?;
    let active_pointer = pointer.with_file_name(DATA_ROOT_POINTER_FILE);
    atomic_json(
        &active_pointer,
        &DataRootPointer {
            data_root: request.target.to_string_lossy().into_owned(),
        },
    )?;
    let _ = fs::remove_file(pointer);
    Ok(request.target)
}

fn copy_migration_directory(source: &Path, target: &Path) -> Result<(), AppError> {
    fs::create_dir(target).map_err(|e| AppError::filesystem(e.to_string()))?;
    for entry in fs::read_dir(source).map_err(|e| AppError::filesystem(e.to_string()))? {
        let entry = entry.map_err(|e| AppError::filesystem(e.to_string()))?;
        let kind = entry
            .file_type()
            .map_err(|e| AppError::filesystem(e.to_string()))?;
        if kind.is_symlink() {
            return Err(AppError::filesystem(
                "数据目录包含链接，已停止迁移以避免遗漏素材",
            ));
        }
        if kind.is_dir() {
            copy_migration_directory(&entry.path(), &target.join(entry.file_name()))?;
        } else if kind.is_file() {
            fs::copy(entry.path(), target.join(entry.file_name()))
                .map_err(|e| AppError::filesystem(e.to_string()))?;
        }
    }
    Ok(())
}

pub fn clear_persisted_data_root() -> Result<(), AppError> {
    if let Some(pending) = migration_pointer() {
        if pending.exists() {
            fs::remove_file(pending).map_err(|e| AppError::filesystem(e.to_string()))?;
        }
    }
    if let Some(pointer) = pointer_file_path() {
        if pointer.exists() {
            fs::remove_file(&pointer).map_err(|error| {
                AppError::filesystem(format!("重置默认数据目录配置失败：{error}"))
            })?;
        }
    }
    Ok(())
}

pub fn configured_data_root() -> Option<PathBuf> {
    resolve_data_root_from(std::env::var_os(DATA_ROOT_ENV)).or_else(read_persisted_data_root)
}

pub fn resolve_data_root(default_root: PathBuf) -> PathBuf {
    configured_data_root().unwrap_or(default_root)
}

/// Query free and total bytes on the filesystem containing `path`.
#[cfg(windows)]
pub fn disk_space_bytes(path: &Path) -> Option<(u64, u64)> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let mut existing = path;
    while !existing.exists() {
        existing = existing.parent()?;
    }
    let wide: Vec<u16> = existing
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut free_bytes_available = 0u64;
    let mut total_bytes = 0u64;
    let mut total_free_bytes = 0u64;
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free_bytes_available,
            &mut total_bytes,
            &mut total_free_bytes,
        )
    };
    if ok != 0 {
        Some((free_bytes_available, total_bytes))
    } else {
        None
    }
}

#[cfg(not(windows))]
pub fn disk_space_bytes(_path: &Path) -> Option<(u64, u64)> {
    None
}

pub fn directory_size_bytes(path: &Path) -> u64 {
    fn walk(dir: &Path, depth: usize) -> u64 {
        if depth > 24 {
            return 0;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            return 0;
        };
        let mut total = 0u64;
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.is_file() {
                total = total.saturating_add(meta.len());
            } else if meta.is_dir() && !meta.file_type().is_symlink() {
                total = total.saturating_add(walk(&entry.path(), depth + 1));
            }
        }
        total
    }
    if path.is_file() {
        return fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    }
    walk(path, 0)
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
        #[cfg(windows)]
        let override_root = PathBuf::from(r"C:\isolated\AIStudioData");
        #[cfg(not(windows))]
        let override_root = PathBuf::from("/isolated/AIStudioData");

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
    #[tokio::test]
    async fn pending_migration_copies_latest_wal_and_config_only_at_startup() {
        let temp = tempdir().unwrap();
        let source = temp.path().join("old");
        let target = temp.path().join("new");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(source.join("config")).unwrap();
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(source.join("app.db"))
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_with(options)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE evidence (value TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE projects (root_path TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE assets (storage_path TEXT, thumbnail_path TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        std::fs::create_dir_all(source.join("projects/p/assets")).unwrap();
        std::fs::write(
            source.join("projects/p/assets/video.mp4"),
            b"video evidence",
        )
        .unwrap();
        sqlx::query("INSERT INTO projects VALUES (?)")
            .bind(source.join("projects/p").to_string_lossy().as_ref())
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO assets VALUES (?,NULL)")
            .bind(
                source
                    .join("projects/p/assets/video.mp4")
                    .to_string_lossy()
                    .as_ref(),
            )
            .execute(&pool)
            .await
            .unwrap();
        let pointer = temp.path().join("data_root_migration.json");
        super::atomic_json(
            &pointer,
            &super::PendingMigration {
                source: source.clone(),
                target: target.clone(),
            },
        )
        .unwrap();
        assert!(!target.exists());
        sqlx::query("INSERT INTO evidence VALUES ('edited-after-scheduling')")
            .execute(&pool)
            .await
            .unwrap();
        std::fs::write(source.join("config/settings.json"), b"latest config").unwrap();
        let migrated = super::complete_migration_at(&source, &pointer)
            .await
            .unwrap();
        assert_eq!(migrated, target);
        assert!(!pointer.exists());
        let copied = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(target.join("app.db")))
            .await
            .unwrap();
        let value: String = sqlx::query_scalar("SELECT value FROM evidence")
            .fetch_one(&copied)
            .await
            .unwrap();
        assert_eq!(value, "edited-after-scheduling");
        let asset_path: String = sqlx::query_scalar("SELECT storage_path FROM assets")
            .fetch_one(&copied)
            .await
            .unwrap();
        assert_eq!(
            std::path::PathBuf::from(asset_path),
            target.join("projects/p/assets/video.mp4")
        );
        assert_eq!(
            std::fs::read(target.join("projects/p/assets/video.mp4")).unwrap(),
            b"video evidence"
        );
        let original: String = sqlx::query_scalar("SELECT root_path FROM projects")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            std::path::PathBuf::from(original),
            source.join("projects/p")
        );

        assert_eq!(
            std::fs::read(target.join("config/settings.json")).unwrap(),
            b"latest config"
        );
        assert!(source.join("app.db").exists());
        copied.close().await;
        pool.close().await;
    }

    #[test]
    fn migration_refuses_existing_or_nested_destinations_without_touching_them() {
        let temp = tempdir().unwrap();
        let source = temp.path().join("old");
        let target = temp.path().join("existing");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("app.db"), b"unrelated existing data").unwrap();
        assert!(super::validate_migration_target(&source, &target).is_err());
        assert!(super::validate_migration_target(&source, &source.join("nested")).is_err());
        assert_eq!(
            std::fs::read(target.join("app.db")).unwrap(),
            b"unrelated existing data"
        );
    }
    #[tokio::test]
    async fn failed_migration_keeps_original_pointer_and_cleans_only_owned_staging() {
        let temp = tempdir().unwrap();
        let source = temp.path().join("old");
        let target = temp.path().join("new");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("projects"), b"invalid directory").unwrap();
        let pointer = temp.path().join("data_root_migration.json");
        super::atomic_json(
            &pointer,
            &super::PendingMigration {
                source: source.clone(),
                target: target.clone(),
            },
        )
        .unwrap();
        assert!(super::complete_migration_at(&source, &pointer)
            .await
            .is_err());
        assert!(!target.exists());
        assert!(pointer.exists());
        assert!(source.join("projects").is_file());
        assert!(std::fs::read_dir(temp.path()).unwrap().all(|e| !e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".ai-studio-migration-")));
    }
}
