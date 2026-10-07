use crate::application::{
    comfy_service::{ComfyConnectionStatus, ComfyService, ComfyStatusView},
    ports::{DatabaseHealthProbe, TaskRepository},
    production_queue_service::ProductionQueueService,
    workflow_lifecycle_service::WorkflowLifecycleService,
};
use crate::error::AppError;
use crate::infrastructure::logging::{
    read_recent_logs, LoggingStatus, DIAGNOSTIC_LOG_BYTES, DIAGNOSTIC_LOG_FILE_LIMIT,
};
use serde::Serialize;
use std::{
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::task;
use zip::{write::FileOptions, CompressionMethod, ZipWriter};

const MAX_DIAGNOSTICS_BUNDLE_BYTES: usize = 25 * 1024 * 1024;
const DIAGNOSTIC_PACKAGE_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

// Diagnostics must not await every live package/runtime probe while offline.
// Cancellation drops only this read future; execution/readiness authority stays unchanged.
async fn bounded_diagnostic_read<T, E>(
    budget: std::time::Duration,
    read: impl std::future::Future<Output = Result<T, E>>,
) -> Option<T> {
    tokio::time::timeout(budget, read).await.ok()?.ok()
}

pub mod execution;

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
    pub workflow_packages: Option<usize>,
    pub valid_workflow_packages: Option<usize>,
    pub invalid_workflow_packages: Option<usize>,
    pub active_task_count: Option<usize>,
    pub production_busy: Option<bool>,
    pub logging_available: bool,
    pub log_retention_days: u32,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsExportView {
    pub file_name: String,
}

pub struct DiagnosticsService {
    database_health_probe: Arc<dyn DatabaseHealthProbe>,
    task_repository: Arc<dyn TaskRepository>,
    comfy_service: Arc<ComfyService>,
    workflow_lifecycle_service: Arc<WorkflowLifecycleService>,
    production_queue_service: Arc<ProductionQueueService>,
    logs_dir: PathBuf,
    logging_status: LoggingStatus,
}

impl DiagnosticsService {
    /// Explicit project scope: never label a bounded project window as global history.
    pub async fn execution_health(
        &self,
        project_id: &str,
    ) -> Result<execution::ExecutionHealth, AppError> {
        let tasks = self.recent_diagnostic_tasks(project_id).await?;
        Ok(execution::ExecutionHealth::from_recent(project_id, &tasks))
    }

    pub async fn recent_failures(
        &self,
        project_id: &str,
    ) -> Result<Vec<execution::RecentFailure>, AppError> {
        let tasks = self.recent_diagnostic_tasks(project_id).await?;
        Ok(execution::recent_failures(project_id, &tasks))
    }

    async fn recent_diagnostic_tasks(
        &self,
        project_id: &str,
    ) -> Result<Vec<crate::application::ports::TaskDiagnosticFacts>, AppError> {
        if project_id.trim().is_empty() {
            return Err(AppError::invalid_input("诊断需要明确的项目范围"));
        }
        self.task_repository
            .list_recent_diagnostic_facts(project_id, execution::RECENT_TASK_LIMIT)
            .await
            .map_err(|_| AppError::database("无法读取最近任务诊断"))
    }

    pub async fn task_timeline(
        &self,
        project_id: &str,
        task_id: &str,
    ) -> Result<execution::TaskTimeline, AppError> {
        let id = crate::domain::TaskId::parse(task_id)
            .map_err(|_| AppError::invalid_input("任务标识无效"))?;
        let task = self
            .task_repository
            .find_diagnostic_facts(project_id, &id)
            .await
            .map_err(|_| AppError::database("无法读取任务诊断"))?
            .filter(|task| task.project_id == project_id)
            .ok_or_else(|| AppError::invalid_input("当前项目中找不到该任务"))?;
        Ok(execution::TaskTimeline::from_facts(&task))
    }

    pub fn new(
        database_health_probe: Arc<dyn DatabaseHealthProbe>,
        task_repository: Arc<dyn TaskRepository>,
        comfy_service: Arc<ComfyService>,
        workflow_lifecycle_service: Arc<WorkflowLifecycleService>,
        production_queue_service: Arc<ProductionQueueService>,
        logs_dir: PathBuf,
        logging_status: LoggingStatus,
    ) -> Self {
        Self {
            database_health_probe,
            task_repository,
            comfy_service,
            workflow_lifecycle_service,
            production_queue_service,
            logs_dir,
            logging_status,
        }
    }

    pub async fn summary(&self) -> DiagnosticsSummaryView {
        let database_healthy = self.database_health_probe.is_healthy().await;

        let active_task_count = match self.task_repository.list_active().await {
            Ok(tasks) => Some(tasks.len()),
            Err(error) => {
                tracing::warn!(
                    error_type = std::any::type_name_of_val(&error),
                    "diagnostics could not read active tasks"
                );
                None
            }
        };

        let production_busy = match self.production_queue_service.admission_status().await {
            Ok(status) => Some(status.busy),
            Err(error) => {
                tracing::warn!(
                    error_type = std::any::type_name_of_val(&error),
                    "diagnostics could not read production admission"
                );
                None
            }
        };

        let mut comfy_read_unknown = false;
        let comfy = match self.comfy_service.get_status().await {
            Ok(status) => status,
            Err(error) => {
                comfy_read_unknown = true;
                tracing::warn!(
                    error_code = error.code(),
                    "diagnostics could not read ComfyUI status"
                );
                ComfyStatusView {
                    status: ComfyConnectionStatus::Offline,
                    endpoint: self.comfy_service.endpoint().to_owned(),
                    runtime_generation: self.comfy_service.runtime_generation(),
                    comfyui_version: None,
                    system: None,
                    devices: Vec::new(),
                    capability: None,
                }
            }
        };

        let (workflow_packages, valid_workflow_packages, invalid_workflow_packages) =
            match bounded_diagnostic_read(
                DIAGNOSTIC_PACKAGE_READ_TIMEOUT,
                self.workflow_lifecycle_service.list_workspace_diagnostics(),
            )
            .await
            {
                Some(workspace) => {
                    let total = workspace.items.len();
                    let valid = workspace
                        .items
                        .iter()
                        .filter(|item| item.package_status == "VALID")
                        .count();
                    (Some(total), Some(valid), Some(total.saturating_sub(valid)))
                }
                None => {
                    tracing::warn!(
                        error_code = "DIAGNOSTIC_PACKAGE_READ_UNAVAILABLE",
                        "diagnostics could not read workflow package status"
                    );
                    (None, None, None)
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
            comfy_status: diagnostic_comfy_status(comfy.status, comfy_read_unknown),
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

fn diagnostic_comfy_status(status: ComfyConnectionStatus, read_unknown: bool) -> String {
    if read_unknown {
        "UNKNOWN".to_owned()
    } else {
        comfy_status_name(status)
    }
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
    // Runtime-supplied device/version text is not trusted merely because the
    // field lives in a typed summary. The bundle's path/secret policy applies
    // to every string, not only the separate log entries.
    let mut safe_summary = summary.clone();
    for value in [
        &mut safe_summary.app_version,
        &mut safe_summary.platform,
        &mut safe_summary.architecture,
        &mut safe_summary.run_mode,
        &mut safe_summary.comfy_status,
    ] {
        *value = safe_export_text(value).unwrap_or_else(|| "UNAVAILABLE".to_owned());
    }
    safe_summary.comfy_version = safe_summary
        .comfy_version
        .as_deref()
        .and_then(safe_export_text);
    safe_summary.gpu_name = safe_summary.gpu_name.as_deref().and_then(safe_export_text);
    let diagnostics =
        serde_json::to_vec_pretty(&safe_summary).map_err(|_| "summary serialization")?;
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

fn safe_export_text(value: &str) -> Option<String> {
    if value.len() > 256 || value.contains(['\r', '\n']) {
        return None;
    }
    let mut expected = value.as_bytes().to_vec();
    expected.push(b'\n');
    (crate::infrastructure::logging::sanitize_log_content(value.as_bytes()) == expected)
        .then(|| value.to_owned())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn phase13_package_probe_deadline_and_error_are_unknown_not_zero() {
        use super::*;
        let budget = std::time::Duration::from_millis(1);
        assert_eq!(
            bounded_diagnostic_read(budget, async { Ok::<_, ()>(7) }).await,
            Some(7)
        );
        assert_eq!(
            bounded_diagnostic_read(budget, async { Err::<usize, _>(()) }).await,
            None
        );
        assert_eq!(
            bounded_diagnostic_read(budget, std::future::pending::<Result<usize, ()>>()).await,
            None
        );
    }
    #[test]
    fn phase13_failed_comfy_probe_is_unknown_not_offline() {
        use super::*;
        assert_eq!(
            diagnostic_comfy_status(ComfyConnectionStatus::Offline, true),
            "UNKNOWN"
        );
        assert_eq!(
            diagnostic_comfy_status(ComfyConnectionStatus::Offline, false),
            "OFFLINE"
        );
        assert_eq!(
            diagnostic_comfy_status(ComfyConnectionStatus::Connected, false),
            "CONNECTED"
        );
    }
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
            workflow_packages: Some(0),
            valid_workflow_packages: Some(0),
            invalid_workflow_packages: Some(0),
            active_task_count: Some(0),
            production_busy: Some(false),
            logging_available: true,
            log_retention_days: 7,
        }
    }

    #[test]
    fn phase13_case6_bundle_privacy_entries_and_runtime_text() {
        let directory = tempdir().unwrap();
        fs::write(directory.path().join("app.db"), b"PRIVATE_DATABASE").unwrap();
        fs::write(directory.path().join("image.png"), b"PRIVATE_ASSET").unwrap();
        fs::write(directory.path().join("workflow.json"), b"PRIVATE_WORKFLOW").unwrap();
        fs::write(directory.path().join("recipe.yaml"), b"PRIVATE_RECIPE").unwrap();
        fs::write(directory.path().join(format!("ai-studio.{}", Utc::now().date_naive())),
            b"prompt=PRIVATE_PROMPT\nsnapshot_json=PRIVATE_SNAPSHOT\nAuthorization: Bearer PRIVATE_TOKEN\nsource=/home/private/project\nerror_code=COMFY_OFFLINE\n").unwrap();
        let mut summary = sample_summary();
        summary.gpu_name = Some(r"C:\Users\private\GPU".to_owned());
        summary.comfy_version = Some("https://private:password@example.com".to_owned());
        summary.active_task_count = None;
        summary.production_busy = None;
        let bytes = build_diagnostics_bundle(&summary, directory.path()).unwrap();
        assert!(bytes.len() <= super::MAX_DIAGNOSTICS_BUNDLE_BYTES);
        let mut archive = ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut all = String::new();
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).unwrap();
            assert!(
                entry.name() == "diagnostics.json"
                    || entry.name() == "README.txt"
                    || entry.name().starts_with("logs/ai-studio.")
            );
            entry.read_to_string(&mut all).unwrap();
        }
        assert!(!all.contains("PRIVATE_"));
        assert!(!all.contains(r"C:\Users"));
        assert!(!all.contains("/home/private"));
        assert!(!all.contains("example.com"));
        assert!(all.contains("COMFY_OFFLINE"));
        assert!(all.contains("\"activeTaskCount\": null"));
        assert!(all.contains("\"productionBusy\": null"));
        assert_eq!(
            fs::read(directory.path().join("app.db")).unwrap(),
            b"PRIVATE_DATABASE"
        );
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
