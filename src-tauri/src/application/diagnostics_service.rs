use crate::application::{
    comfy_service::{ComfyConnectionStatus, ComfyService, ComfyStatusView},
    ports::TaskRepository,
    production_queue_service::ProductionQueueService,
    workflow_lifecycle_service::WorkflowLifecycleService,
};
use crate::error::AppError;
use crate::infrastructure::logging::{
    read_recent_logs, LoggingStatus, DIAGNOSTIC_LOG_BYTES, DIAGNOSTIC_LOG_FILE_LIMIT,
};
use serde::Serialize;
use sqlx::SqlitePool;
use std::{
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::task;
use zip::{write::FileOptions, CompressionMethod, ZipWriter};

const MAX_DIAGNOSTICS_BUNDLE_BYTES: usize = 25 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeActivityStatusView {
    pub active_task_count: usize,
    pub production_busy: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsSummaryView {
    pub app_version: String,
    pub platform: String,
    pub architecture: String,
    pub run_mode: String,
    pub database_healthy: bool,
    pub comfy_status: String,
    pub comfy_version: Option<String>,
    pub gpu_name: Option<String>,
    pub vram_total: Option<u64>,
    pub vram_free: Option<u64>,
    pub workflow_packages: usize,
    pub valid_workflow_packages: usize,
    pub invalid_workflow_packages: usize,
    pub active_task_count: usize,
    pub production_busy: bool,
    pub logging_available: bool,
    pub log_retention_days: u32,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsExportView {
    pub file_name: String,
}

pub struct DiagnosticsService {
    database_pool: SqlitePool,
    task_repository: Arc<dyn TaskRepository>,
    comfy_service: Arc<ComfyService>,
    workflow_lifecycle_service: Arc<WorkflowLifecycleService>,
    production_queue_service: Arc<ProductionQueueService>,
    logs_dir: PathBuf,
    logging_status: LoggingStatus,
}

impl DiagnosticsService {
    pub fn new(
        database_pool: SqlitePool,
        task_repository: Arc<dyn TaskRepository>,
        comfy_service: Arc<ComfyService>,
        workflow_lifecycle_service: Arc<WorkflowLifecycleService>,
        production_queue_service: Arc<ProductionQueueService>,
        logs_dir: PathBuf,
        logging_status: LoggingStatus,
    ) -> Self {
        Self {
            database_pool,
            task_repository,
            comfy_service,
            workflow_lifecycle_service,
            production_queue_service,
            logs_dir,
            logging_status,
        }
    }

    pub async fn summary(&self) -> DiagnosticsSummaryView {
        let database_healthy = sqlx::query_scalar::<_, i64>("SELECT 1")
            .fetch_one(&self.database_pool)
            .await
            .is_ok();

        let active_task_count = match self.task_repository.list_active().await {
            Ok(tasks) => tasks.len(),
            Err(error) => {
                tracing::warn!(
                    error_type = std::any::type_name_of_val(&error),
                    "diagnostics could not read active tasks"
                );
                0
            }
        };

        let production_busy = match self.production_queue_service.admission_status().await {
            Ok(status) => status.busy,
            Err(error) => {
                tracing::warn!(
                    error_type = std::any::type_name_of_val(&error),
                    "diagnostics could not read production admission"
                );
                false
            }
        };

        let comfy = match self.comfy_service.get_status().await {
            Ok(status) => status,
            Err(error) => {
                tracing::warn!(
                    error_code = error.code(),
                    "diagnostics could not read ComfyUI status"
                );
                ComfyStatusView {
                    status: ComfyConnectionStatus::Offline,
                    endpoint: self.comfy_service.endpoint().to_owned(),
                    comfyui_version: None,
                    system: None,
                    devices: Vec::new(),
                    capability: None,
                }
            }
        };

        let (workflow_packages, valid_workflow_packages, invalid_workflow_packages) =
            match self.workflow_lifecycle_service.list_workspace().await {
                Ok(workspace) => {
                    let total = workspace.items.len();
                    let valid = workspace
                        .items
                        .iter()
                        .filter(|item| item.package_status == "VALID")
                        .count();
                    (total, valid, total.saturating_sub(valid))
                }
                Err(error) => {
                    tracing::warn!(
                        error_code = error.code(),
                        "diagnostics could not read workflow package status"
                    );
                    (0, 0, 0)
                }
            };

        let (gpu_name, vram_total, vram_free) = summarize_devices(&comfy);

        DiagnosticsSummaryView {
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            platform: std::env::consts::OS.to_owned(),
            architecture: std::env::consts::ARCH.to_owned(),
            run_mode: if cfg!(debug_assertions) {
                "开发版".to_owned()
            } else {
                "正式版".to_owned()
            },
            database_healthy,
            comfy_status: comfy_status_name(comfy.status),
            comfy_version: comfy.comfyui_version,
            gpu_name,
            vram_total,
            vram_free,
            workflow_packages,
            valid_workflow_packages,
            invalid_workflow_packages,
            active_task_count,
            production_busy,
            logging_available: self.logging_status.available,
            log_retention_days: self.logging_status.retention_days,
        }
    }

    pub async fn runtime_activity_status(&self) -> Result<RuntimeActivityStatusView, AppError> {
        let active_task_count = self
            .task_repository
            .list_active()
            .await
            .map_err(|_| AppError::internal("无法读取运行中的任务状态"))?
            .len();
        let production_busy = self
            .production_queue_service
            .admission_status()
            .await
            .map_err(|_| AppError::internal("无法读取生产队列状态"))?
            .busy;

        Ok(RuntimeActivityStatusView {
            active_task_count,
            production_busy,
        })
    }

    pub async fn export_bundle(
        &self,
        destination: PathBuf,
        summary: DiagnosticsSummaryView,
    ) -> Result<DiagnosticsExportView, AppError> {
        let logs_dir = self.logs_dir.clone();
        let generated_file_name = diagnostics_file_name();
        let bundle = task::spawn_blocking(move || build_diagnostics_bundle(&summary, &logs_dir))
            .await
            .map_err(|_| AppError::filesystem("诊断包生成失败"))?
            .map_err(|_| AppError::filesystem("诊断包生成失败"))?;

        if bundle.len() > MAX_DIAGNOSTICS_BUNDLE_BYTES {
            return Err(AppError::filesystem("诊断包超过大小限制"));
        }

        task::spawn_blocking(move || fs::write(destination, bundle))
            .await
            .map_err(|_| AppError::filesystem("诊断包保存失败"))?
            .map_err(|_| AppError::filesystem("诊断包保存失败"))?;

        Ok(DiagnosticsExportView {
            file_name: generated_file_name,
        })
    }
}

fn summarize_devices(status: &ComfyStatusView) -> (Option<String>, Option<u64>, Option<u64>) {
    let names = status
        .devices
        .iter()
        .filter_map(|device| device.name.as_deref())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let vram_total = sum_device_value(status, |device| device.vram_total);
    let vram_free = sum_device_value(status, |device| device.vram_free);
    (
        (!names.is_empty()).then(|| names.join(" · ")),
        vram_total,
        vram_free,
    )
}

fn sum_device_value(
    status: &ComfyStatusView,
    value: impl Fn(&crate::application::ports::DeviceInfo) -> Option<u64>,
) -> Option<u64> {
    let mut found = false;
    let total = status
        .devices
        .iter()
        .filter_map(|device| {
            let value = value(device);
            found |= value.is_some();
            value
        })
        .fold(0_u64, u64::saturating_add);
    found.then_some(total)
}

fn comfy_status_name(status: ComfyConnectionStatus) -> String {
    match status {
        ComfyConnectionStatus::Connected => "CONNECTED".to_owned(),
        ComfyConnectionStatus::Offline => "OFFLINE".to_owned(),
        ComfyConnectionStatus::Incompatible => "INCOMPATIBLE".to_owned(),
    }
}

fn diagnostics_file_name() -> String {
    format!(
        "AI-Studio-Diagnostics-{}.zip",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    )
}

fn build_diagnostics_bundle(
    summary: &DiagnosticsSummaryView,
    logs_dir: &Path,
) -> Result<Vec<u8>, String> {
    let diagnostics = serde_json::to_vec_pretty(summary).map_err(|_| "summary serialization")?;
    let recent_logs = read_recent_logs(logs_dir, DIAGNOSTIC_LOG_FILE_LIMIT, DIAGNOSTIC_LOG_BYTES);
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
    writer
        .start_file("diagnostics.json", options)
        .map_err(|_| "diagnostics entry")?;
    writer
        .write_all(&diagnostics)
        .map_err(|_| "diagnostics contents")?;
    writer
        .start_file("README.txt", options)
        .map_err(|_| "readme entry")?;
    writer
        .write_all(
            "AI Studio 诊断摘要\n\n此文件仅包含安全运行摘要和最近的应用日志片段。\n不会包含数据库、项目目录、资产文件、工作流原文、配方原文、Prompt 或绝对路径。\n"
                .as_bytes(),
        )
        .map_err(|_| "readme contents")?;
    for (file_name, content) in recent_logs {
        let safe_name = file_name.replace(['\\', '/'], "_");
        writer
            .start_file(format!("logs/{safe_name}"), options)
            .map_err(|_| "log entry")?;
        writer.write_all(&content).map_err(|_| "log contents")?;
    }
    Ok(writer
        .finish()
        .map(|cursor| cursor.into_inner())
        .map_err(|_| "bundle finish")?)
}

#[cfg(test)]
mod tests {
    use super::{build_diagnostics_bundle, DiagnosticsSummaryView};
    use crate::infrastructure::logging::sanitize_log_content;
    use chrono::{Duration, Utc};
    use std::{fs, io::Read};
    use tempfile::tempdir;
    use zip::ZipArchive;

    fn sample_summary() -> DiagnosticsSummaryView {
        DiagnosticsSummaryView {
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            platform: "windows".to_owned(),
            architecture: "x86_64".to_owned(),
            run_mode: "正式版".to_owned(),
            database_healthy: true,
            comfy_status: "OFFLINE".to_owned(),
            comfy_version: None,
            gpu_name: None,
            vram_total: None,
            vram_free: None,
            workflow_packages: 0,
            valid_workflow_packages: 0,
            invalid_workflow_packages: 0,
            active_task_count: 0,
            production_busy: false,
            logging_available: true,
            log_retention_days: 7,
        }
    }

    #[test]
    fn diagnostics_bundle_excludes_private_log_content_and_paths() {
        let directory = tempdir().expect("temporary directory should exist");
        let name = format!(
            "ai-studio.{}",
            (Utc::now().date_naive() - Duration::days(1)).format("%Y-%m-%d")
        );
        fs::write(
            directory.path().join(name),
            b"PRIVATE_PROMPT_SHOULD_NOT_APPEAR_123\nprompt=private\npath=C:\\Users\\private\\app.db\nnormal=kept\n",
        )
        .expect("log should write");

        let bundle = build_diagnostics_bundle(&sample_summary(), directory.path())
            .expect("diagnostics bundle should build");
        let mut archive = ZipArchive::new(std::io::Cursor::new(bundle)).expect("zip should open");
        let mut all_contents = String::new();
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).expect("zip entry should open");
            assert!(!entry.name().contains("app.db"));
            assert!(!entry.name().contains("assets"));
            assert!(!entry.name().contains("projects"));
            let mut content = String::new();
            entry.read_to_string(&mut content).ok();
            all_contents.push_str(&content);
        }
        assert!(!all_contents.contains("PRIVATE_PROMPT_SHOULD_NOT_APPEAR_123"));
        assert!(!all_contents.contains("C:\\Users\\"));
        assert!(all_contents.contains("normal=kept"));
        assert_eq!(
            sanitize_log_content(b"prompt=secret\nnormal=kept\n"),
            b"normal=kept\n"
        );
    }
}
