use crate::application::{
    comfy_service::{ComfyConnectionStatus, ComfyService, ComfyStatusView},
    diagnostics_service::{DiagnosticsService, RuntimeActivityStatusView},
    workflow_lifecycle_service::{
        WorkflowLifecycleService, WorkflowProductionWorkspaceResponse,
        WorkflowProductionWorkspaceView,
    },
    workflow_onboarding_service::CapabilityIssueView,
};
use crate::error::AppError;
use chrono::Utc;
use serde::Serialize;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ComfyPreflightStatus {
    Ready,
    Warning,
    Blocked,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ComfyPreflightIssueSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ComfyPreflightIssue {
    pub severity: ComfyPreflightIssueSeverity,
    pub code: String,
    pub title: String,
    pub detail: String,
    pub workflow_id: Option<String>,
    pub workflow_version_id: Option<String>,
    pub missing_nodes: Option<Vec<String>>,
    pub suggested_action: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ComfyPreflightWorkflow {
    pub workflow_id: Option<String>,
    pub workflow_version_id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub status: String,
    pub missing_nodes: Vec<String>,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ComfyPreflightWorkflowSummary {
    pub workflow_total: usize,
    pub workflow_ready: usize,
    pub workflow_blocked: usize,
    pub items: Vec<ComfyPreflightWorkflow>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComfyPreflightReport {
    pub endpoint: String,
    pub status: ComfyPreflightStatus,
    pub checked_at: String,
    pub connection: ComfyConnectionStatus,
    pub comfyui_version: Option<String>,
    pub python_version: Option<String>,
    pub gpu: Option<String>,
    pub vram_total: Option<u64>,
    pub vram_free: Option<u64>,
    pub node_count: Option<usize>,
    pub runtime_busy: bool,
    pub active_task_count: usize,
    pub production_busy: bool,
    pub workflow_summary: ComfyPreflightWorkflowSummary,
    pub issues: Vec<ComfyPreflightIssue>,
}

pub struct ComfyPreflightService {
    comfy_service: Arc<ComfyService>,
    diagnostics_service: Arc<DiagnosticsService>,
    workflow_lifecycle_service: Arc<WorkflowLifecycleService>,
}

impl ComfyPreflightService {
    pub fn new(
        comfy_service: Arc<ComfyService>,
        diagnostics_service: Arc<DiagnosticsService>,
        workflow_lifecycle_service: Arc<WorkflowLifecycleService>,
    ) -> Self {
        Self {
            comfy_service,
            diagnostics_service,
            workflow_lifecycle_service,
        }
    }

    /// Read the current endpoint only. It never applies settings or starts generation.
    pub async fn current(&self) -> Result<ComfyPreflightReport, AppError> {
        let comfy_status = self.comfy_service.get_status().await?;
        let activity = self.diagnostics_service.runtime_activity_status().await?;
        let (node_count, capability_error) = match self.comfy_service.refresh_capabilities().await {
            Ok(summary) => (Some(summary.node_count), None),
            Err(error) => (None, Some((error.code().to_owned(), error.to_string()))),
        };
        let (workspace, workspace_error) = match self
            .workflow_lifecycle_service
            .list_workspace_diagnostics()
            .await
        {
            Ok(response) => (Some(response), None),
            Err(error) => (None, Some((error.code().to_owned(), error.to_string()))),
        };

        Ok(compose_report(
            comfy_status,
            activity,
            node_count,
            capability_error,
            workspace,
            workspace_error,
        ))
    }
}

fn compose_report(
    comfy_status: ComfyStatusView,
    activity: RuntimeActivityStatusView,
    node_count: Option<usize>,
    capability_error: Option<(String, String)>,
    workspace: Option<WorkflowProductionWorkspaceResponse>,
    workspace_error: Option<(String, String)>,
) -> ComfyPreflightReport {
    let mut issues = Vec::new();
    push_connection_issue(&mut issues, comfy_status.status, &comfy_status.endpoint);
    if let Some((code, detail)) = capability_error {
        issues.push(ComfyPreflightIssue {
            severity: ComfyPreflightIssueSeverity::Error,
            code,
            title: "ComfyUI 能力信息不可用".to_owned(),
            detail,
            workflow_id: None,
            workflow_version_id: None,
            missing_nodes: None,
            suggested_action: Some("检查 ComfyUI 的 /object_info API。".to_owned()),
        });
    }
    if let Some((code, detail)) = workspace_error {
        issues.push(ComfyPreflightIssue {
            severity: ComfyPreflightIssueSeverity::Error,
            code,
            title: "工作流预检不可用".to_owned(),
            detail,
            workflow_id: None,
            workflow_version_id: None,
            missing_nodes: None,
            suggested_action: Some("检查工作流运行包和本地工作流库。".to_owned()),
        });
    }

    let items = workspace.map_or_else(Vec::new, |response| response.items);
    let workflow_summary = summarize_workflows(&items, &mut issues);
    let has_enabled_workflow = items
        .iter()
        .any(|item| item.enabled && item.workflow_version_id.is_some());
    if workflow_summary.workflow_total == 0 || !has_enabled_workflow {
        issues.push(ComfyPreflightIssue {
            severity: ComfyPreflightIssueSeverity::Error,
            code: "NO_PRODUCTION_WORKFLOW".to_owned(),
            title: "没有可用生产工作流".to_owned(),
            detail: "当前没有可用于生产的已启用工作流运行包。".to_owned(),
            workflow_id: None,
            workflow_version_id: None,
            missing_nodes: None,
            suggested_action: Some("导入或启用至少一个生产工作流。".to_owned()),
        });
    }

    let status = preflight_status(
        comfy_status.status,
        node_count.is_some(),
        &workflow_summary,
        has_enabled_workflow,
    );

    ComfyPreflightReport {
        endpoint: comfy_status.endpoint.clone(),
        status,
        checked_at: Utc::now().to_rfc3339(),
        connection: comfy_status.status,
        comfyui_version: comfy_status.comfyui_version.clone(),
        python_version: comfy_status
            .system
            .as_ref()
            .and_then(|system| system.python_version.clone()),
        gpu: gpu_name(&comfy_status),
        vram_total: sum_device_value(&comfy_status, |device| device.vram_total),
        vram_free: sum_device_value(&comfy_status, |device| device.vram_free),
        node_count,
        runtime_busy: activity.active_task_count > 0 || activity.production_busy,
        active_task_count: activity.active_task_count,
        production_busy: activity.production_busy,
        workflow_summary,
        issues,
    }
}

fn summarize_workflows(
    items: &[WorkflowProductionWorkspaceView],
    issues: &mut Vec<ComfyPreflightIssue>,
) -> ComfyPreflightWorkflowSummary {
    let mut workflow_total = 0;
    let mut workflow_ready = 0;
    let mut workflow_blocked = 0;
    let mut summaries = Vec::with_capacity(items.len());

    for item in items {
        if item.workflow_version_id.is_some() {
            workflow_total += 1;
        }
        let missing_nodes = item
            .capability_issues
            .iter()
            .filter(|issue| issue.code == "MISSING_NODE")
            .filter_map(|issue| issue.class_type.clone())
            .collect::<Vec<_>>();
        summaries.push(ComfyPreflightWorkflow {
            workflow_id: item.workflow_id.clone(),
            workflow_version_id: item.workflow_version_id.clone(),
            name: item.name.clone(),
            version: item.workflow_version.clone(),
            status: if item.enabled {
                item.readiness.clone()
            } else {
                "DISABLED".to_owned()
            },
            missing_nodes: missing_nodes.clone(),
            reason: item
                .readiness_reasons
                .first()
                .cloned()
                .or_else(|| item.error_message.clone()),
        });

        for diagnostic in &item.diagnostics {
            issues.push(workflow_issue(
                ComfyPreflightIssueSeverity::Error,
                &diagnostic.code,
                "工作流运行包存在问题",
                &diagnostic.message,
                item,
                None,
                Some("修复或重新安装该工作流运行包。"),
            ));
        }
        if !item.enabled {
            if item.workflow_version_id.is_some() {
                issues.push(workflow_issue(
                    ComfyPreflightIssueSeverity::Info,
                    "WORKFLOW_DISABLED",
                    "工作流已停用",
                    "该工作流不会参与当前生产能力判断。",
                    item,
                    None,
                    Some("如需生产，请在工作流管理中启用它。"),
                ));
            }
            continue;
        }

        match item.readiness.as_str() {
            "READY" => workflow_ready += 1,
            "BLOCKED" => {
                workflow_blocked += 1;
                if item.capability_issues.is_empty() && item.diagnostics.is_empty() {
                    issues.push(workflow_issue(
                        ComfyPreflightIssueSeverity::Error,
                        "WORKFLOW_BLOCKED",
                        "工作流不可用",
                        &item.readiness_reasons.join(" "),
                        item,
                        None,
                        Some("检查工作流运行包、配方和当前 ComfyUI 能力。"),
                    ));
                }
            }
            _ => issues.push(workflow_issue(
                ComfyPreflightIssueSeverity::Warning,
                "WORKFLOW_DEGRADED",
                "工作流尚未完全就绪",
                &item.readiness_reasons.join(" "),
                item,
                None,
                Some("完成工作流的当前环境检查或真实验证。"),
            )),
        }
        for capability_issue in &item.capability_issues {
            issues.push(capability_issue_to_preflight(item, capability_issue));
        }
    }

    ComfyPreflightWorkflowSummary {
        workflow_total,
        workflow_ready,
        workflow_blocked,
        items: summaries,
    }
}

fn preflight_status(
    connection: ComfyConnectionStatus,
    object_info_ready: bool,
    workflows: &ComfyPreflightWorkflowSummary,
    has_enabled_workflow: bool,
) -> ComfyPreflightStatus {
    if !matches!(connection, ComfyConnectionStatus::Connected)
        || !object_info_ready
        || workflows.workflow_total == 0
        || !has_enabled_workflow
        || (workflows.workflow_blocked > 0 && workflows.workflow_ready == 0)
    {
        return ComfyPreflightStatus::Blocked;
    }
    if workflows.workflow_blocked > 0 || workflows.workflow_ready < workflows.workflow_total {
        ComfyPreflightStatus::Warning
    } else {
        ComfyPreflightStatus::Ready
    }
}

fn push_connection_issue(
    issues: &mut Vec<ComfyPreflightIssue>,
    connection: ComfyConnectionStatus,
    endpoint: &str,
) {
    let (code, title, detail, suggested_action) = match connection {
        ComfyConnectionStatus::Connected => return,
        ComfyConnectionStatus::Offline => (
            "COMFY_OFFLINE",
            "无法连接 ComfyUI",
            format!("当前 endpoint {endpoint} 无法连接。"),
            "启动 ComfyUI 或检查当前环境地址。",
        ),
        ComfyConnectionStatus::Incompatible => (
            "COMFY_INCOMPATIBLE",
            "ComfyUI API 不兼容",
            format!("当前 endpoint {endpoint} 返回了不兼容的 API 响应。"),
            "确认 ComfyUI 提供 /system_stats 和 /object_info。",
        ),
    };
    issues.push(ComfyPreflightIssue {
        severity: ComfyPreflightIssueSeverity::Error,
        code: code.to_owned(),
        title: title.to_owned(),
        detail,
        workflow_id: None,
        workflow_version_id: None,
        missing_nodes: None,
        suggested_action: Some(suggested_action.to_owned()),
    });
}

fn capability_issue_to_preflight(
    item: &WorkflowProductionWorkspaceView,
    issue: &CapabilityIssueView,
) -> ComfyPreflightIssue {
    let missing_nodes = (issue.code == "MISSING_NODE").then(|| {
        issue
            .class_type
            .clone()
            .into_iter()
            .chain(issue.affected_node_ids.iter().cloned())
            .collect()
    });
    let missing = issue.code == "MISSING_NODE";
    ComfyPreflightIssue {
        severity: if missing {
            ComfyPreflightIssueSeverity::Error
        } else {
            ComfyPreflightIssueSeverity::Warning
        },
        code: issue.code.clone(),
        title: if missing {
            "缺少 ComfyUI 节点".to_owned()
        } else {
            "工作流能力不匹配".to_owned()
        },
        detail: issue.message.clone(),
        workflow_id: item.workflow_id.clone(),
        workflow_version_id: item.workflow_version_id.clone(),
        missing_nodes,
        suggested_action: Some(if missing {
            "安装对应的 ComfyUI 节点后重新预检。".to_owned()
        } else {
            "检查工作流输入与当前 ComfyUI 能力。".to_owned()
        }),
    }
}

fn workflow_issue(
    severity: ComfyPreflightIssueSeverity,
    code: &str,
    title: &str,
    detail: &str,
    item: &WorkflowProductionWorkspaceView,
    missing_nodes: Option<Vec<String>>,
    suggested_action: Option<&str>,
) -> ComfyPreflightIssue {
    ComfyPreflightIssue {
        severity,
        code: code.to_owned(),
        title: title.to_owned(),
        detail: detail.to_owned(),
        workflow_id: item.workflow_id.clone(),
        workflow_version_id: item.workflow_version_id.clone(),
        missing_nodes,
        suggested_action: suggested_action.map(str::to_owned),
    }
}

fn gpu_name(status: &ComfyStatusView) -> Option<String> {
    let names = status
        .devices
        .iter()
        .filter_map(|device| device.name.as_deref())
        .collect::<Vec<_>>();
    (!names.is_empty()).then(|| names.join(" · "))
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

#[cfg(test)]
mod tests {
    use super::{compose_report, ComfyPreflightIssueSeverity, ComfyPreflightStatus};
    use crate::application::{
        comfy_service::{ComfyConnectionStatus, ComfyStatusView, SystemSummary},
        diagnostics_service::RuntimeActivityStatusView,
        ports::DeviceInfo,
        workflow_lifecycle_service::{
            WorkflowProductionWorkspaceResponse, WorkflowProductionWorkspaceView,
        },
        workflow_onboarding_service::CapabilityIssueView,
    };

    fn status(connection: ComfyConnectionStatus) -> ComfyStatusView {
        ComfyStatusView {
            status: connection,
            endpoint: "http://127.0.0.1:8188".to_owned(),
            comfyui_version: Some("0.33.0".to_owned()),
            system: Some(SystemSummary {
                python_version: Some("3.12.10".to_owned()),
                os: None,
                ram_total: None,
                ram_free: None,
            }),
            devices: vec![DeviceInfo {
                name: Some("Test GPU".to_owned()),
                device_type: Some("cuda".to_owned()),
                vram_total: Some(16),
                vram_free: Some(8),
            }],
            capability: None,
        }
    }

    fn workflow(enabled: bool, readiness: &str) -> WorkflowProductionWorkspaceView {
        WorkflowProductionWorkspaceView {
            package_name: "test".to_owned(),
            builtin: true,
            archived: false,
            archived_at: None,
            package_status: "VALID".to_owned(),
            error_code: None,
            error_message: None,
            workflow_id: Some("wfl_test".to_owned()),
            workflow_version_id: Some("wfv_test".to_owned()),
            name: Some("Test".to_owned()),
            category: Some("image".to_owned()),
            mode: Some("t2i".to_owned()),
            workflow_version: Some("1.0.0".to_owned()),
            workflow_sha256: None,
            recipe_sha256: None,
            enabled,
            capability: if readiness == "READY" {
                "READY".to_owned()
            } else {
                "MISSING_NODES".to_owned()
            },
            readiness: readiness.to_owned(),
            readiness_reasons: if readiness == "READY" {
                Vec::new()
            } else {
                vec!["缺少 ComfyUI 工作流节点。".to_owned()]
            },
            capability_issues: if readiness == "READY" {
                Vec::new()
            } else {
                vec![CapabilityIssueView {
                    code: "MISSING_NODE".to_owned(),
                    class_type: Some("MissingNode".to_owned()),
                    node_id: None,
                    affected_node_ids: Vec::new(),
                    input_name: None,
                    current_value: None,
                    message: "Missing ComfyUI node class MissingNode".to_owned(),
                }]
            },
            node_count: 1,
            recipes: Vec::new(),
            active_tasks: 0,
            total_tasks: 0,
            has_successful_run: true,
            latest_success_at: None,
            latest_failure_at: None,
            live_verified_at: None,
            diagnostics: Vec::new(),
        }
    }

    fn response(
        items: Vec<WorkflowProductionWorkspaceView>,
    ) -> WorkflowProductionWorkspaceResponse {
        WorkflowProductionWorkspaceResponse {
            items,
            staging: Vec::new(),
        }
    }

    #[test]
    fn ready_report_aggregates_runtime_and_workflow_data() {
        let report = compose_report(
            status(ComfyConnectionStatus::Connected),
            RuntimeActivityStatusView {
                active_task_count: 2,
                production_busy: true,
            },
            Some(4516),
            None,
            Some(response(vec![workflow(true, "READY")])),
            None,
        );

        assert_eq!(report.status, ComfyPreflightStatus::Ready);
        assert_eq!(report.comfyui_version.as_deref(), Some("0.33.0"));
        assert_eq!(report.python_version.as_deref(), Some("3.12.10"));
        assert_eq!(report.gpu.as_deref(), Some("Test GPU"));
        assert_eq!(report.vram_total, Some(16));
        assert_eq!(report.node_count, Some(4516));
        assert!(report.runtime_busy);
        assert_eq!(report.workflow_summary.workflow_ready, 1);
    }

    #[test]
    fn missing_node_is_warning_when_another_workflow_is_ready() {
        let report = compose_report(
            status(ComfyConnectionStatus::Connected),
            RuntimeActivityStatusView {
                active_task_count: 0,
                production_busy: false,
            },
            Some(3),
            None,
            Some(response(vec![
                workflow(true, "READY"),
                workflow(true, "BLOCKED"),
            ])),
            None,
        );

        assert_eq!(report.status, ComfyPreflightStatus::Warning);
        assert_eq!(report.workflow_summary.workflow_blocked, 1);
        let issue = report
            .issues
            .iter()
            .find(|issue| issue.code == "MISSING_NODE")
            .expect("missing node issue");
        assert_eq!(issue.severity, ComfyPreflightIssueSeverity::Error);
        assert_eq!(
            issue.missing_nodes.as_ref().expect("missing nodes"),
            &vec!["MissingNode".to_owned()]
        );
    }

    #[test]
    fn offline_or_invalid_object_info_blocks_preflight() {
        let report = compose_report(
            status(ComfyConnectionStatus::Offline),
            RuntimeActivityStatusView {
                active_task_count: 0,
                production_busy: false,
            },
            None,
            Some(("COMFY_OFFLINE".to_owned(), "connection refused".to_owned())),
            Some(response(Vec::new())),
            None,
        );

        assert_eq!(report.status, ComfyPreflightStatus::Blocked);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "COMFY_OFFLINE"));
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "NO_PRODUCTION_WORKFLOW"));
    }

    #[test]
    fn disabled_workflow_is_info_and_not_counted_as_blocked() {
        let report = compose_report(
            status(ComfyConnectionStatus::Connected),
            RuntimeActivityStatusView {
                active_task_count: 0,
                production_busy: false,
            },
            Some(3),
            None,
            Some(response(vec![workflow(false, "BLOCKED")])),
            None,
        );

        assert_eq!(report.workflow_summary.workflow_blocked, 0);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "WORKFLOW_DISABLED"
                && issue.severity == ComfyPreflightIssueSeverity::Info));
    }
}
