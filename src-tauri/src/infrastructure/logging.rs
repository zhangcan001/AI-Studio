use chrono::{Duration, NaiveDate, Utc};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

pub const LOG_RETENTION_DAYS: u32 = 7;
pub const MAX_LOG_BYTES: u64 = 100 * 1024 * 1024;
pub const DIAGNOSTIC_LOG_FILE_LIMIT: usize = 7;
pub const DIAGNOSTIC_LOG_BYTES: usize = 20 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoggingStatus {
    pub available: bool,
    pub retention_days: u32,
}

static LOG_GUARD: OnceLock<Mutex<Option<WorkerGuard>>> = OnceLock::new();

pub fn initialize(logs_dir: Option<&Path>) -> LoggingStatus {
    let mut file_logging_available = false;

    if let Some(logs_dir) = logs_dir {
        if ensure_log_directory(logs_dir) {
            cleanup_log_directory(logs_dir);
            let appender = tracing_appender::rolling::daily(logs_dir, "ai-studio");
            let (writer, guard) = tracing_appender::non_blocking(appender);
            let subscriber = tracing_subscriber::fmt()
                .with_env_filter(default_filter())
                .with_ansi(false)
                .with_target(false)
                .with_writer(writer);
            if subscriber.try_init().is_ok() {
                let slot = LOG_GUARD.get_or_init(|| Mutex::new(None));
                if let Ok(mut stored_guard) = slot.lock() {
                    *stored_guard = Some(guard);
                    file_logging_available = true;
                }
            }
        }
    }

    if !file_logging_available {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(default_filter())
            .with_ansi(false)
            .with_target(false)
            .try_init();
    }

    LoggingStatus {
        available: file_logging_available,
        retention_days: LOG_RETENTION_DAYS,
    }
}

pub fn ensure_log_directory(logs_dir: &Path) -> bool {
    fs::create_dir_all(logs_dir).is_ok() && logs_dir.is_dir()
}

pub fn cleanup_log_directory(logs_dir: &Path) {
    let Some(mut entries) = read_owned_log_entries(logs_dir) else {
        return;
    };

    let cutoff = Utc::now().date_naive() - Duration::days(i64::from(LOG_RETENTION_DAYS - 1));
    entries.retain(|entry| {
        if entry.date < cutoff {
            let _ = fs::remove_file(&entry.path);
            false
        } else {
            true
        }
    });

    let mut total_bytes = entries.iter().map(|entry| entry.size).sum::<u64>();
    entries.sort_by(|left, right| {
        left.date
            .cmp(&right.date)
            .then_with(|| left.path.cmp(&right.path))
    });
    for entry in entries {
        if total_bytes <= MAX_LOG_BYTES {
            break;
        }
        if fs::remove_file(&entry.path).is_ok() {
            total_bytes = total_bytes.saturating_sub(entry.size);
        }
    }
}

pub fn read_recent_logs(
    logs_dir: &Path,
    max_files: usize,
    max_bytes: usize,
) -> Vec<(String, Vec<u8>)> {
    let Some(mut entries) = read_owned_log_entries(logs_dir) else {
        return Vec::new();
    };
    entries.sort_by(|left, right| {
        right
            .date
            .cmp(&left.date)
            .then_with(|| right.path.cmp(&left.path))
    });

    let mut selected = Vec::new();
    let mut total_bytes = 0usize;
    for entry in entries.into_iter().take(max_files) {
        let Ok(content) = fs::read(&entry.path) else {
            continue;
        };
        let sanitized = sanitize_log_content(&content);
        if sanitized.is_empty() || total_bytes >= max_bytes {
            continue;
        }
        let remaining = max_bytes - total_bytes;
        let content = truncate_utf8(sanitized, remaining);
        total_bytes += content.len();
        selected.push((entry.file_name, content));
    }
    selected
}

pub fn sanitize_log_content(content: &[u8]) -> Vec<u8> {
    String::from_utf8_lossy(content)
        .lines()
        .filter(|line| !contains_sensitive_data(line))
        .flat_map(|line| {
            line.as_bytes()
                .iter()
                .copied()
                .chain(std::iter::once(b'\n'))
        })
        .collect()
}

fn default_filter() -> EnvFilter {
    if cfg!(debug_assertions) {
        EnvFilter::new("debug,sqlx=warn,reqwest=warn,tungstenite=warn,tokio_tungstenite=warn")
    } else {
        EnvFilter::new("info,sqlx=warn,reqwest=warn,tungstenite=warn,tokio_tungstenite=warn")
    }
}

fn contains_sensitive_data(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    [
        "prompt",
        "prompt=",
        "prompt:",
        "storage_path",
        "workflow_json",
        "recipe_yaml",
        "snapshot_json",
        "database_path",
        "path=",
        "appdata\\",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
        || contains_windows_absolute_path(line)
}

fn contains_windows_absolute_path(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.windows(3).any(|window| {
        window[0].is_ascii_alphabetic() && window[1] == b':' && matches!(window[2], b'\\' | b'/')
    }) || value.contains("\\\\")
}

fn truncate_utf8(mut value: Vec<u8>, max_bytes: usize) -> Vec<u8> {
    if value.len() <= max_bytes {
        return value;
    }
    value.truncate(max_bytes);
    while std::str::from_utf8(&value).is_err() {
        value.pop();
    }
    value
}

#[derive(Clone, Debug)]
struct OwnedLogEntry {
    path: PathBuf,
    file_name: String,
    date: NaiveDate,
    size: u64,
}

fn read_owned_log_entries(logs_dir: &Path) -> Option<Vec<OwnedLogEntry>> {
    let entries = fs::read_dir(logs_dir).ok()?;
    Some(
        entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                if !path.is_file() {
                    return None;
                }
                let file_name = path.file_name()?.to_str()?.to_owned();
                let date = log_date(&file_name)?;
                let size = entry.metadata().ok()?.len();
                Some(OwnedLogEntry {
                    path,
                    file_name,
                    date,
                    size,
                })
            })
            .collect(),
    )
}

fn log_date(file_name: &str) -> Option<NaiveDate> {
    if !file_name.starts_with("ai-studio.") {
        return None;
    }
    file_name.split('.').find_map(|part| {
        if part.len() == 10 {
            NaiveDate::parse_from_str(part, "%Y-%m-%d").ok()
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{cleanup_log_directory, ensure_log_directory, read_recent_logs, MAX_LOG_BYTES};
    use chrono::{Duration, Utc};
    use std::fs;
    use tempfile::tempdir;

    fn dated_name(offset_days: i64) -> String {
        format!(
            "ai-studio.{}",
            (Utc::now().date_naive() - Duration::days(offset_days)).format("%Y-%m-%d")
        )
    }

    #[test]
    fn retention_removes_only_old_ai_studio_logs() {
        let directory = tempdir().expect("temporary log directory should exist");
        fs::write(directory.path().join(dated_name(30)), b"old").expect("old log should write");
        fs::write(directory.path().join(dated_name(1)), b"recent")
            .expect("recent log should write");
        fs::write(directory.path().join("unrelated.log"), b"keep")
            .expect("unrelated file should write");

        cleanup_log_directory(directory.path());

        assert!(!directory.path().join(dated_name(30)).exists());
        assert!(directory.path().join(dated_name(1)).exists());
        assert!(directory.path().join("unrelated.log").exists());
    }

    #[test]
    fn logging_directory_failure_is_nonfatal() {
        let directory = tempdir().expect("temporary directory should exist");
        let file_path = directory.path().join("not-a-directory");
        fs::write(&file_path, b"file").expect("file should write");
        assert!(!ensure_log_directory(&file_path.join("logs")));
    }

    #[test]
    fn diagnostic_log_selection_respects_file_and_size_limits() {
        let directory = tempdir().expect("temporary log directory should exist");
        for offset in 0..8 {
            fs::write(directory.path().join(dated_name(offset)), vec![b'x'; 8])
                .expect("log should write");
        }

        let selected = read_recent_logs(directory.path(), 7, 20);
        assert_eq!(selected.len(), 3);
        assert!(selected.iter().map(|(_, bytes)| bytes.len()).sum::<usize>() <= 20);
        assert!(MAX_LOG_BYTES > 20);
    }
}
