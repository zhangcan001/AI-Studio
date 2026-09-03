use crate::application::comfy_service::{ComfyConnectionStatus, ComfyService};
use crate::application::production_queue_service::{ProductionQueueError, ProductionQueueService};
use crate::application::workflow_lifecycle_service::{
    WorkflowLifecycleError, WorkflowLifecycleService, WorkflowRecipeRuntimeInspection,
};
use crate::application::workflow_onboarding_service::CapabilityIssueView;
use crate::domain::{ProductionBatchItem, ProductionBatchItemStatus};
use std::{collections::BTreeSet, error::Error, fmt, sync::Arc};

pub const RUNTIME_ADMISSION_COMFY_UNAVAILABLE: &str = "RUNTIME_ADMISSION_COMFY_UNAVAILABLE";
pub const RUNTIME_ADMISSION_COMFY_INCOMPATIBLE: &str = "RUNTIME_ADMISSION_COMFY_INCOMPATIBLE";
pub const RUNTIME_ADMISSION_CAPABILITY_REFRESH_FAILED: &str =
    "RUNTIME_ADMISSION_CAPABILITY_REFRESH_FAILED";
pub const RUNTIME_ADMISSION_WORKSPACE_DIAGNOSTICS_FAILED: &str =
    "RUNTIME_ADMISSION_WORKSPACE_DIAGNOSTICS_FAILED";
pub const RUNTIME_ADMISSION_EXACT_RECIPE_CHECK_FAILED: &str =
    "RUNTIME_ADMISSION_EXACT_RECIPE_CHECK_FAILED";
pub const RUNTIME_ADMISSION_WORKFLOW_NOT_FOUND: &str = "RUNTIME_ADMISSION_WORKFLOW_NOT_FOUND";
pub const RUNTIME_ADMISSION_RECIPE_NOT_FOUND: &str = "RUNTIME_ADMISSION_RECIPE_NOT_FOUND";
pub const RUNTIME_ADMISSION_WORKFLOW_DISABLED: &str = "RUNTIME_ADMISSION_WORKFLOW_DISABLED";
pub const RUNTIME_ADMISSION_WORKFLOW_ARCHIVED: &str = "RUNTIME_ADMISSION_WORKFLOW_ARCHIVED";
pub const RUNTIME_ADMISSION_PACKAGE_INVALID: &str = "RUNTIME_ADMISSION_PACKAGE_INVALID";
pub const RUNTIME_ADMISSION_MISSING_NODES: &str = "RUNTIME_ADMISSION_MISSING_NODES";
pub const RUNTIME_ADMISSION_CAPABILITY_INCOMPATIBLE: &str =
    "RUNTIME_ADMISSION_CAPABILITY_INCOMPATIBLE";
pub const RUNTIME_ADMISSION_CAPABILITY_NOT_CHECKED: &str =
    "RUNTIME_ADMISSION_CAPABILITY_NOT_CHECKED";
pub const RUNTIME_ADMISSION_CAPABILITY_OFFLINE: &str = "RUNTIME_ADMISSION_CAPABILITY_OFFLINE";
pub const RUNTIME_ADMISSION_CAPABILITY_UNKNOWN: &str = "RUNTIME_ADMISSION_CAPABILITY_UNKNOWN";
pub const RUNTIME_ADMISSION_DIAGNOSTICS: &str = "RUNTIME_ADMISSION_DIAGNOSTICS";
pub const RUNTIME_ADMISSION_READINESS_BLOCKED: &str = "RUNTIME_ADMISSION_READINESS_BLOCKED";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeAdmissionFailure {
    pub code: &'static str,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub reason: String,
    pub missing_nodes: Vec<String>,
}

impl fmt::Display for RuntimeAdmissionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: workflow_version_id={}, recipe_id={}, reason={}",
            self.code, self.workflow_version_id, self.recipe_id, self.reason
        )?;
        if !self.missing_nodes.is_empty() {
            write!(
                formatter,
                ", missing_nodes={}",
                self.missing_nodes.join(",")
            )?;
        }
        Ok(())
    }
}

impl Error for RuntimeAdmissionFailure {}

#[derive(Debug)]
pub enum ProductionStartAdmissionError {
    Queue(ProductionQueueError),
    Runtime(RuntimeAdmissionFailure),
}

impl From<ProductionQueueError> for ProductionStartAdmissionError {
    fn from(error: ProductionQueueError) -> Self {
        Self::Queue(error)
    }
}

impl From<RuntimeAdmissionFailure> for ProductionStartAdmissionError {
    fn from(error: RuntimeAdmissionFailure) -> Self {
        Self::Runtime(error)
    }
}

impl fmt::Display for ProductionStartAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Queue(error) => error.fmt(formatter),
            Self::Runtime(error) => error.fmt(formatter),
        }
    }
}

impl Error for ProductionStartAdmissionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Queue(error) => Some(error),
            Self::Runtime(error) => Some(error),
        }
    }
}

pub struct ProductionStartAdmissionService {
    queue: Arc<ProductionQueueService>,
    comfy: Arc<ComfyService>,
    lifecycle: Arc<WorkflowLifecycleService>,
}

impl ProductionStartAdmissionService {
    pub fn new(
        queue: Arc<ProductionQueueService>,
        comfy: Arc<ComfyService>,
        lifecycle: Arc<WorkflowLifecycleService>,
    ) -> Self {
        Self {
            queue,
            comfy,
            lifecycle,
        }
    }

    pub async fn start(
        &self,
        project_id: &str,
        batch_id: &str,
    ) -> Result<(), ProductionStartAdmissionError> {
        let _admission = self.queue.acquire_runtime_configuration_admission().await;
        let detail = self
            .queue
            .inspect_start_admitted(project_id, batch_id)
            .await?;
        let pending_pairs = pending_runtime_pairs(&detail.items);
        if !pending_pairs.is_empty() {
            let status = self.comfy.get_status().await.map_err(|error| {
                runtime_failure(
                    &pending_pairs,
                    RUNTIME_ADMISSION_COMFY_UNAVAILABLE,
                    format!("{}: {}", error.code(), error),
                    Vec::new(),
                )
            })?;
            let status_failure_code = match status.status {
                ComfyConnectionStatus::Connected => None,
                ComfyConnectionStatus::Offline => Some(RUNTIME_ADMISSION_COMFY_UNAVAILABLE),
                ComfyConnectionStatus::Incompatible => Some(RUNTIME_ADMISSION_COMFY_INCOMPATIBLE),
            };
            if let Some(code) = status_failure_code {
                return Err(ProductionStartAdmissionError::Runtime(runtime_failure(
                    &pending_pairs,
                    code,
                    format!("ComfyUI status is {:?}", status.status),
                    Vec::new(),
                )));
            }

            self.comfy.refresh_capabilities().await.map_err(|error| {
                ProductionStartAdmissionError::Runtime(runtime_failure(
                    &pending_pairs,
                    RUNTIME_ADMISSION_CAPABILITY_REFRESH_FAILED,
                    format!("{}: {}", error.code(), error),
                    Vec::new(),
                ))
            })?;

            let mut inspections = Vec::with_capacity(pending_pairs.len());
            for (workflow_version_id, recipe_id) in &pending_pairs {
                let inspection = self
                    .lifecycle
                    .inspect_recipe_runtime(workflow_version_id, recipe_id)
                    .await
                    .map_err(|error| {
                        ProductionStartAdmissionError::Runtime(
                            runtime_failure_from_lifecycle_error(
                                workflow_version_id,
                                recipe_id,
                                error,
                            ),
                        )
                    })?;
                inspections.push(inspection);
            }
            evaluate_runtime_admission(&detail.items, status.status, &inspections)?;
        }

        self.queue.commit_start_admitted(&detail).await?;
        Ok(())
    }
}

/// Return the stable, deduplicated runtime identities represented by Pending
/// items. Frozen values stay on the item and are deliberately not re-resolved.
pub(crate) fn pending_runtime_pairs(items: &[ProductionBatchItem]) -> Vec<(String, String)> {
    items
        .iter()
        .filter(|item| item.status == ProductionBatchItemStatus::Pending)
        .map(|item| (item.workflow_version_id.clone(), item.recipe_id.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Pure runtime admission decision over frozen Pending identities and one exact
/// runtime inspection per identity. It intentionally ignores all non-Pending
/// item identities and all unrelated inspections.
pub(crate) fn evaluate_runtime_admission(
    items: &[ProductionBatchItem],
    comfy_status: ComfyConnectionStatus,
    inspections: &[WorkflowRecipeRuntimeInspection],
) -> Result<Vec<(String, String)>, ProductionStartAdmissionError> {
    let pairs = pending_runtime_pairs(items);
    if pairs.is_empty() {
        return Ok(pairs);
    }
    let status_failure_code = match comfy_status {
        ComfyConnectionStatus::Connected => None,
        ComfyConnectionStatus::Offline => Some(RUNTIME_ADMISSION_COMFY_UNAVAILABLE),
        ComfyConnectionStatus::Incompatible => Some(RUNTIME_ADMISSION_COMFY_INCOMPATIBLE),
    };
    if let Some(code) = status_failure_code {
        return Err(ProductionStartAdmissionError::Runtime(runtime_failure(
            &pairs,
            code,
            format!("ComfyUI status is {:?}", comfy_status),
            Vec::new(),
        )));
    }

    for (workflow_version_id, recipe_id) in &pairs {
        let Some(inspection) = inspections.iter().find(|inspection| {
            inspection.workflow_version_id == *workflow_version_id
                && inspection.recipe_id == *recipe_id
        }) else {
            return Err(ProductionStartAdmissionError::Runtime(
                runtime_failure_for_pair(
                    workflow_version_id,
                    recipe_id,
                    RUNTIME_ADMISSION_RECIPE_NOT_FOUND,
                    "exact recipe runtime inspection is absent",
                    Vec::new(),
                ),
            ));
        };
        evaluate_recipe_runtime_inspection(inspection)
            .map_err(ProductionStartAdmissionError::Runtime)?;
    }

    Ok(pairs)
}

/// Pure admission decision for one exact recipe inspection. Readiness is a UI
/// projection, so the start gate checks its underlying facts directly.
pub(crate) fn evaluate_recipe_runtime_inspection(
    inspection: &WorkflowRecipeRuntimeInspection,
) -> Result<(), RuntimeAdmissionFailure> {
    if inspection.workflow_version_id.trim().is_empty() || inspection.recipe_id.trim().is_empty() {
        return Err(runtime_failure_for_pair(
            &inspection.workflow_version_id,
            &inspection.recipe_id,
            RUNTIME_ADMISSION_EXACT_RECIPE_CHECK_FAILED,
            "exact recipe runtime inspection has an empty identity",
            Vec::new(),
        ));
    }
    if inspection.archived {
        return Err(runtime_failure_for_pair(
            &inspection.workflow_version_id,
            &inspection.recipe_id,
            RUNTIME_ADMISSION_WORKFLOW_ARCHIVED,
            "workflow version is archived",
            Vec::new(),
        ));
    }
    if !inspection.enabled {
        return Err(runtime_failure_for_pair(
            &inspection.workflow_version_id,
            &inspection.recipe_id,
            RUNTIME_ADMISSION_WORKFLOW_DISABLED,
            "workflow version is disabled",
            Vec::new(),
        ));
    }
    if inspection.package_status != "VALID" {
        return Err(runtime_failure_for_pair(
            &inspection.workflow_version_id,
            &inspection.recipe_id,
            RUNTIME_ADMISSION_PACKAGE_INVALID,
            format!("exact package status is {}", inspection.package_status),
            Vec::new(),
        ));
    }
    if !inspection.diagnostics.is_empty() {
        return Err(runtime_failure_for_pair(
            &inspection.workflow_version_id,
            &inspection.recipe_id,
            RUNTIME_ADMISSION_DIAGNOSTICS,
            "exact recipe package diagnostics are not empty",
            Vec::new(),
        ));
    }

    let missing_nodes = missing_node_class_types(&inspection.capability_issues);
    if inspection.capability == "MISSING_NODES" || !missing_nodes.is_empty() {
        return Err(runtime_failure_for_pair(
            &inspection.workflow_version_id,
            &inspection.recipe_id,
            RUNTIME_ADMISSION_MISSING_NODES,
            "ComfyUI is missing workflow node classes",
            missing_nodes,
        ));
    }
    match inspection.capability.as_str() {
        "READY" => Ok(()),
        "INCOMPATIBLE_INPUT_VALUES" => Err(runtime_failure_for_pair(
            &inspection.workflow_version_id,
            &inspection.recipe_id,
            RUNTIME_ADMISSION_CAPABILITY_INCOMPATIBLE,
            "exact recipe inputs are incompatible with ComfyUI",
            Vec::new(),
        )),
        "NOT_CHECKED" => Err(runtime_failure_for_pair(
            &inspection.workflow_version_id,
            &inspection.recipe_id,
            RUNTIME_ADMISSION_CAPABILITY_NOT_CHECKED,
            "exact recipe runtime capability has not been checked",
            Vec::new(),
        )),
        "COMFY_OFFLINE" => Err(runtime_failure_for_pair(
            &inspection.workflow_version_id,
            &inspection.recipe_id,
            RUNTIME_ADMISSION_CAPABILITY_OFFLINE,
            "exact recipe capability reports ComfyUI offline",
            Vec::new(),
        )),
        capability => Err(runtime_failure_for_pair(
            &inspection.workflow_version_id,
            &inspection.recipe_id,
            RUNTIME_ADMISSION_CAPABILITY_UNKNOWN,
            format!("unknown exact recipe capability state {capability}"),
            Vec::new(),
        )),
    }
}

fn missing_node_class_types(issues: &[CapabilityIssueView]) -> Vec<String> {
    issues
        .iter()
        .filter(|issue| issue.code == "MISSING_NODE")
        .filter_map(|issue| issue.class_type.as_deref())
        .map(str::trim)
        .filter(|class_type| !class_type.is_empty())
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn runtime_failure_from_lifecycle_error(
    workflow_version_id: &str,
    recipe_id: &str,
    error: WorkflowLifecycleError,
) -> RuntimeAdmissionFailure {
    let code = match error.code() {
        "WORKFLOW_VERSION_NOT_FOUND" => RUNTIME_ADMISSION_WORKFLOW_NOT_FOUND,
        "RECIPE_NOT_FOUND" => RUNTIME_ADMISSION_RECIPE_NOT_FOUND,
        "RUNTIME_PACKAGE_MISSING"
        | "WORKFLOW_PACKAGE_INVALID"
        | "PACKAGE_ARCHIVE_INVALID"
        | "PACKAGE_ARCHIVE_MISSING_ENTRY"
        | "MANIFEST_INVALID"
        | "RECIPE_INVALID"
        | "WORKFLOW_NOT_API_FORMAT"
        | "WORKFLOW_INPUTS_MISSING" => RUNTIME_ADMISSION_PACKAGE_INVALID,
        "MISSING_NODES" | "MISSING_NODE" => RUNTIME_ADMISSION_MISSING_NODES,
        "INCOMPATIBLE_INPUT_VALUES" | "COMFY_PROTOCOL_ERROR" => {
            RUNTIME_ADMISSION_CAPABILITY_INCOMPATIBLE
        }
        "NOT_CHECKED" | "CAPABILITY_NOT_CHECKED" => RUNTIME_ADMISSION_CAPABILITY_NOT_CHECKED,
        "COMFY_OFFLINE" | "COMFY_UNAVAILABLE" | "COMFY_TIMEOUT" => {
            RUNTIME_ADMISSION_CAPABILITY_OFFLINE
        }
        _ => RUNTIME_ADMISSION_EXACT_RECIPE_CHECK_FAILED,
    };
    runtime_failure_for_pair(
        workflow_version_id,
        recipe_id,
        code,
        format!("{}: {}", error.code(), error),
        Vec::new(),
    )
}

fn runtime_failure(
    pairs: &[(String, String)],
    code: &'static str,
    reason: String,
    missing_nodes: Vec<String>,
) -> RuntimeAdmissionFailure {
    let (workflow_version_id, recipe_id) = &pairs[0];
    runtime_failure_for_pair(workflow_version_id, recipe_id, code, reason, missing_nodes)
}

fn runtime_failure_for_pair(
    workflow_version_id: &str,
    recipe_id: &str,
    code: &'static str,
    reason: impl Into<String>,
    missing_nodes: Vec<String>,
) -> RuntimeAdmissionFailure {
    RuntimeAdmissionFailure {
        code,
        workflow_version_id: workflow_version_id.to_owned(),
        recipe_id: recipe_id.to_owned(),
        reason: reason.into(),
        missing_nodes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::comfy_service::ComfyConnectionStatus;
    use crate::application::workflow_lifecycle_service::{
        WorkflowDiagnosticView, WorkflowRecipeRuntimeInspection,
    };
    use crate::application::workflow_onboarding_service::CapabilityIssueView;
    use crate::domain::{
        ProductionBatchId, ProductionBatchItem, ProductionBatchItemId, ProductionBatchItemStatus,
    };
    use chrono::Utc;
    use serde_json::json;

    fn item(
        workflow_version_id: &str,
        recipe_id: &str,
        status: ProductionBatchItemStatus,
    ) -> ProductionBatchItem {
        let now = Utc::now();
        ProductionBatchItem {
            id: ProductionBatchItemId::new(),
            batch_id: ProductionBatchId::new(),
            ordinal: 0,
            workflow_version_id: workflow_version_id.to_owned(),
            recipe_id: recipe_id.to_owned(),
            values_json: json!({"prompt": {"type": "string", "value": "frozen"}}),
            status,
            task_id: None,
            retry_of_item_id: None,
            error_code: None,
            error_message: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn inspection(
        workflow_version_id: &str,
        recipe_id: &str,
        capability: &str,
    ) -> WorkflowRecipeRuntimeInspection {
        WorkflowRecipeRuntimeInspection {
            workflow_id: "workflow".to_owned(),
            workflow_version_id: workflow_version_id.to_owned(),
            recipe_id: recipe_id.to_owned(),
            recipe_version: "1.0.0".to_owned(),
            enabled: true,
            archived: false,
            package_name: "runtime-package".to_owned(),
            package_status: "VALID".to_owned(),
            diagnostics: Vec::new(),
            capability: capability.to_owned(),
            capability_issues: Vec::new(),
            has_successful_run: true,
        }
    }

    fn failure(
        result: Result<Vec<(String, String)>, ProductionStartAdmissionError>,
    ) -> RuntimeAdmissionFailure {
        match result {
            Err(ProductionStartAdmissionError::Runtime(failure)) => failure,
            other => panic!("expected runtime admission failure, got {other:?}"),
        }
    }

    #[test]
    fn a1_exact_workflow_version_and_recipe_are_admitted() {
        let result = evaluate_runtime_admission(
            &[item("wv-a", "recipe-a", ProductionBatchItemStatus::Pending)],
            ComfyConnectionStatus::Connected,
            &[inspection("wv-a", "recipe-a", "READY")],
        );

        assert_eq!(
            result.unwrap(),
            vec![("wv-a".to_owned(), "recipe-a".to_owned())]
        );
    }

    #[test]
    fn a2_non_matching_recipe_is_rejected() {
        let failure = failure(evaluate_runtime_admission(
            &[item(
                "wv-a",
                "recipe-requested",
                ProductionBatchItemStatus::Pending,
            )],
            ComfyConnectionStatus::Connected,
            &[inspection("wv-a", "recipe-other", "READY")],
        ));

        assert_eq!(failure.code, RUNTIME_ADMISSION_RECIPE_NOT_FOUND);
        assert_eq!(failure.workflow_version_id, "wv-a");
        assert_eq!(failure.recipe_id, "recipe-requested");
    }

    #[test]
    fn a3_disabled_workflow_is_rejected() {
        let mut disabled = inspection("wv-a", "recipe-a", "READY");
        disabled.enabled = false;
        let failure = failure(evaluate_runtime_admission(
            &[item("wv-a", "recipe-a", ProductionBatchItemStatus::Pending)],
            ComfyConnectionStatus::Connected,
            &[disabled],
        ));

        assert_eq!(failure.code, RUNTIME_ADMISSION_WORKFLOW_DISABLED);
    }

    #[test]
    fn a4_archived_workflow_is_rejected() {
        let mut archived = inspection("wv-a", "recipe-a", "READY");
        archived.archived = true;
        let failure = failure(evaluate_runtime_admission(
            &[item("wv-a", "recipe-a", ProductionBatchItemStatus::Pending)],
            ComfyConnectionStatus::Connected,
            &[archived],
        ));

        assert_eq!(failure.code, RUNTIME_ADMISSION_WORKFLOW_ARCHIVED);
    }

    #[test]
    fn a5_invalid_package_is_rejected() {
        let mut invalid = inspection("wv-a", "recipe-a", "READY");
        invalid.package_status = "INVALID".to_owned();
        invalid.diagnostics.push(WorkflowDiagnosticView {
            code: "WORKFLOW_RUNTIME_HASH_MISMATCH".to_owned(),
            message: "hash mismatch".to_owned(),
        });
        let failure = failure(evaluate_runtime_admission(
            &[item("wv-a", "recipe-a", ProductionBatchItemStatus::Pending)],
            ComfyConnectionStatus::Connected,
            &[invalid],
        ));

        assert_eq!(failure.code, RUNTIME_ADMISSION_PACKAGE_INVALID);
    }

    #[test]
    fn a6_missing_nodes_preserve_sorted_unique_class_types() {
        let mut missing = inspection("wv-a", "recipe-a", "MISSING_NODES");
        missing.capability_issues = vec![
            CapabilityIssueView {
                code: "MISSING_NODE".to_owned(),
                class_type: Some("ZNode".to_owned()),
                node_id: Some("3".to_owned()),
                affected_node_ids: vec!["3".to_owned()],
                input_name: None,
                current_value: None,
                message: "missing ZNode".to_owned(),
            },
            CapabilityIssueView {
                code: "MISSING_NODE".to_owned(),
                class_type: Some("ANode".to_owned()),
                node_id: Some("1".to_owned()),
                affected_node_ids: vec!["1".to_owned()],
                input_name: None,
                current_value: None,
                message: "missing ANode".to_owned(),
            },
            CapabilityIssueView {
                code: "MISSING_NODE".to_owned(),
                class_type: Some("ZNode".to_owned()),
                node_id: Some("4".to_owned()),
                affected_node_ids: vec!["4".to_owned()],
                input_name: None,
                current_value: None,
                message: "missing ZNode".to_owned(),
            },
        ];
        let failure = failure(evaluate_runtime_admission(
            &[item("wv-a", "recipe-a", ProductionBatchItemStatus::Pending)],
            ComfyConnectionStatus::Connected,
            &[missing],
        ));

        assert_eq!(failure.code, RUNTIME_ADMISSION_MISSING_NODES);
        assert_eq!(failure.missing_nodes, vec!["ANode", "ZNode"]);
    }

    #[test]
    fn a7_incompatible_capability_is_rejected() {
        let failure = failure(evaluate_runtime_admission(
            &[item("wv-a", "recipe-a", ProductionBatchItemStatus::Pending)],
            ComfyConnectionStatus::Connected,
            &[inspection("wv-a", "recipe-a", "INCOMPATIBLE_INPUT_VALUES")],
        ));

        assert_eq!(failure.code, RUNTIME_ADMISSION_CAPABILITY_INCOMPATIBLE);
    }

    #[test]
    fn a8_not_checked_capability_is_rejected_fail_closed() {
        let failure = failure(evaluate_runtime_admission(
            &[item("wv-a", "recipe-a", ProductionBatchItemStatus::Pending)],
            ComfyConnectionStatus::Connected,
            &[inspection("wv-a", "recipe-a", "NOT_CHECKED")],
        ));

        assert_eq!(failure.code, RUNTIME_ADMISSION_CAPABILITY_NOT_CHECKED);
    }

    #[test]
    fn comfy_incompatible_status_is_rejected_fail_closed() {
        let failure = failure(evaluate_runtime_admission(
            &[item("wv-a", "recipe-a", ProductionBatchItemStatus::Pending)],
            ComfyConnectionStatus::Incompatible,
            &[inspection("wv-a", "recipe-a", "READY")],
        ));

        assert_eq!(failure.code, RUNTIME_ADMISSION_COMFY_INCOMPATIBLE);
    }

    #[test]
    fn a9_degraded_without_successful_history_is_allowed() {
        let mut degraded = inspection("wv-a", "recipe-a", "READY");
        degraded.has_successful_run = false;

        let result = evaluate_runtime_admission(
            &[item("wv-a", "recipe-a", ProductionBatchItemStatus::Pending)],
            ComfyConnectionStatus::Connected,
            &[degraded],
        );

        assert!(result.is_ok());
    }

    #[test]
    fn a10_unrelated_blocked_workflow_does_not_block_requested_workflow() {
        let mut unrelated = inspection("wv-unrelated", "recipe-unrelated", "MISSING_NODES");
        unrelated.package_status = "INVALID".to_owned();
        unrelated.diagnostics.push(WorkflowDiagnosticView {
            code: "WORKFLOW_PACKAGE_INVALID".to_owned(),
            message: "unrelated failure".to_owned(),
        });

        let result = evaluate_runtime_admission(
            &[item("wv-a", "recipe-a", ProductionBatchItemStatus::Pending)],
            ComfyConnectionStatus::Connected,
            &[inspection("wv-a", "recipe-a", "READY"), unrelated],
        );

        assert!(result.is_ok());
    }

    #[test]
    fn a11_only_pending_items_are_checked() {
        let result = evaluate_runtime_admission(
            &[
                item(
                    "wv-invalid-but-terminal",
                    "recipe-invalid",
                    ProductionBatchItemStatus::Succeeded,
                ),
                item(
                    "wv-current",
                    "recipe-current",
                    ProductionBatchItemStatus::Pending,
                ),
            ],
            ComfyConnectionStatus::Connected,
            &[inspection("wv-current", "recipe-current", "READY")],
        );

        assert_eq!(
            result.unwrap(),
            vec![("wv-current".to_owned(), "recipe-current".to_owned())]
        );
    }

    #[test]
    fn a12_multiple_recipes_on_one_version_match_each_exact_recipe() {
        let result = evaluate_runtime_admission(
            &[
                item("wv-a", "recipe-a", ProductionBatchItemStatus::Pending),
                item("wv-a", "recipe-b", ProductionBatchItemStatus::Pending),
            ],
            ComfyConnectionStatus::Connected,
            &[
                inspection("wv-a", "recipe-a", "READY"),
                inspection("wv-a", "recipe-b", "READY"),
            ],
        );

        assert_eq!(
            result.unwrap(),
            vec![
                ("wv-a".to_owned(), "recipe-a".to_owned()),
                ("wv-a".to_owned(), "recipe-b".to_owned()),
            ]
        );
    }

    #[test]
    fn a13_duplicate_pending_pairs_are_deduplicated_for_admission() {
        let result = evaluate_runtime_admission(
            &[
                item("wv-a", "recipe-a", ProductionBatchItemStatus::Pending),
                item("wv-a", "recipe-a", ProductionBatchItemStatus::Pending),
                item("wv-a", "recipe-a", ProductionBatchItemStatus::Pending),
                item("wv-a", "recipe-b", ProductionBatchItemStatus::Pending),
            ],
            ComfyConnectionStatus::Connected,
            &[
                inspection("wv-a", "recipe-a", "READY"),
                inspection("wv-a", "recipe-b", "READY"),
            ],
        );

        assert_eq!(result.unwrap().len(), 2);
    }

    #[test]
    fn p1_exact_recipe_ready_is_admitted_with_incompatible_sibling() {
        let result = evaluate_runtime_admission(
            &[item(
                "wv-shared",
                "recipe-a",
                ProductionBatchItemStatus::Pending,
            )],
            ComfyConnectionStatus::Connected,
            &[
                inspection("wv-shared", "recipe-a", "READY"),
                inspection("wv-shared", "recipe-b", "INCOMPATIBLE_INPUT_VALUES"),
            ],
        );

        assert_eq!(
            result.unwrap(),
            vec![("wv-shared".to_owned(), "recipe-a".to_owned())]
        );
    }

    #[test]
    fn p1_exact_recipe_incompatible_is_blocked_with_ready_sibling() {
        let failure = failure(evaluate_runtime_admission(
            &[item(
                "wv-shared",
                "recipe-b",
                ProductionBatchItemStatus::Pending,
            )],
            ComfyConnectionStatus::Connected,
            &[
                inspection("wv-shared", "recipe-a", "READY"),
                inspection("wv-shared", "recipe-b", "INCOMPATIBLE_INPUT_VALUES"),
            ],
        ));

        assert_eq!(failure.code, RUNTIME_ADMISSION_CAPABILITY_INCOMPATIBLE);
        assert_eq!(failure.recipe_id, "recipe-b");
    }
}
