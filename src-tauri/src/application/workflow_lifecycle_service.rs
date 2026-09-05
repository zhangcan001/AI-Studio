use crate::application::{
    builtin_runtime_packages::{audit_installed, is_builtin_package_name},
    ports::{
        Clock, ProjectWorkflowBindingRecord, ProjectWorkflowBindingRepository, RuntimeRecipeRecord,
        RuntimeWorkflowVersionRecord, WorkflowLibrarySource, WorkflowPackageBytes,
        WorkflowPackageLoad, WorkflowPackageStore, WorkflowRuntimeArtifactRecord,
        WorkflowRuntimeArtifactRepository, WorkflowRuntimeRepository, WorkflowRuntimeState,
        WorkflowRuntimeStateRepository,
    },
    workflow_library_service::WorkflowLibraryService,
    workflow_manifest::WorkflowManifest,
    workflow_onboarding_service::{
        read_back_and_validate_package, CapabilityCheckView, CapabilityIssueView, CapabilityState,
        RuntimeWorkflowCapabilityInput, WorkflowOnboardingDraftView, WorkflowOnboardingService,
    },
};
use crate::compiler::RecipeParser;
use crate::domain::{Recipe, WorkflowDocument};
use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    error::Error,
    fmt,
    io::{Cursor, Read, Write},
    path::Path,
    sync::Arc,
    time::Instant,
};
use tokio::sync::RwLock;
use uuid::Uuid;
use zip::{write::FileOptions, CompressionMethod, ZipArchive, ZipWriter};

pub const MAX_WORKFLOW_ARCHIVE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_WORKFLOW_ARCHIVE_UNCOMPRESSED_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_WORKFLOW_ARCHIVE_FILES: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowLifecycleError {
    code: &'static str,
    message: String,
}

impl WorkflowLifecycleError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }
}

impl fmt::Display for WorkflowLifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl Error for WorkflowLifecycleError {}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRecipeSummaryView {
    pub recipe_id: String,
    pub version: String,
    pub input_count: usize,
    pub output_count: usize,
    pub preset_count: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct WorkflowRecipeRuntimeInspection {
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub recipe_version: String,
    pub enabled: bool,
    pub archived: bool,
    pub package_name: String,
    pub package_status: String,
    pub diagnostics: Vec<WorkflowDiagnosticView>,
    pub capability: String,
    pub capability_issues: Vec<CapabilityIssueView>,
    pub has_successful_run: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowDiagnosticView {
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowStagingView {
    pub staging_id: String,
    pub status: String,
    pub in_use: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowProductionWorkspaceView {
    pub package_name: String,
    pub builtin: bool,
    pub archived: bool,
    pub archived_at: Option<String>,
    pub package_status: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub workflow_id: Option<String>,
    pub workflow_version_id: Option<String>,
    pub name: Option<String>,
    pub category: Option<String>,
    pub mode: Option<String>,
    pub workflow_version: Option<String>,
    pub workflow_sha256: Option<String>,
    pub recipe_sha256: Option<String>,
    pub enabled: bool,
    pub capability: String,
    pub readiness: String,
    pub readiness_reasons: Vec<String>,
    pub capability_issues:
        Vec<crate::application::workflow_onboarding_service::CapabilityIssueView>,
    pub node_count: usize,
    pub recipes: Vec<WorkflowRecipeSummaryView>,
    pub active_tasks: u64,
    pub total_tasks: u64,
    pub has_successful_run: bool,
    pub latest_success_at: Option<String>,
    pub latest_failure_at: Option<String>,
    pub live_verified_at: Option<String>,
    pub diagnostics: Vec<WorkflowDiagnosticView>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowProductionWorkspaceResponse {
    pub items: Vec<WorkflowProductionWorkspaceView>,
    pub staging: Vec<WorkflowStagingView>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowDeletionInspection {
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub name: String,
    pub builtin: bool,
    pub enabled: bool,
    pub archived: bool,
    pub archived_at: Option<String>,
    pub active_task_count: u64,
    pub active_queue_item_count: u64,
    pub historical_task_count: u64,
    pub production_batch_item_count: u64,
    pub benchmark_reference_count: u64,
    #[serde(rename = "deleteAction")]
    pub delete_action: String,
    pub project_binding_count: u64,
    pub project_binding_scopes: Vec<String>,
    pub can_hard_delete: bool,
    pub requires_archive: bool,
    pub blocking_reasons: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowDeletionResult {
    pub action: String,
    #[serde(rename = "deleteAction")]
    pub delete_action: String,
    pub project_binding_count: u64,
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub archived: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowCapabilityBatchView {
    pub workflow_version_id: String,
    pub capability: CapabilityCheckView,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRestoreView {
    pub status: String,
    pub package_name: String,
    pub workflow_id: String,
    pub workflow_version: String,
    pub recipe_id: Option<String>,
    pub enabled: bool,
    pub capability: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRestoreResult {
    pub workflow_version_id: String,
    pub archived: bool,
    pub enabled: bool,
    pub capability: String,
    pub readiness: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowExportView {
    pub file_name: String,
    #[serde(skip)]
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowChangedClassTypeView {
    pub node_id: String,
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowValueChangeView {
    pub node_id: String,
    pub input: String,
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowLinkChangeView {
    pub node_id: String,
    pub input: String,
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowVersionDiffView {
    pub workflow_id: String,
    pub version_a: String,
    pub version_b: String,
    pub node_count_a: usize,
    pub node_count_b: usize,
    pub added_nodes: Vec<String>,
    pub removed_nodes: Vec<String>,
    pub changed_class_types: Vec<WorkflowChangedClassTypeView>,
    pub changed_literal_inputs: Vec<WorkflowValueChangeView>,
    pub changed_links: Vec<WorkflowLinkChangeView>,
    pub recipe_input_changes: Vec<String>,
    pub binding_changes: Vec<String>,
    pub output_changes: Vec<String>,
}

pub struct WorkflowLifecycleService {
    source: Arc<dyn WorkflowLibrarySource>,
    library_service: Arc<WorkflowLibraryService>,
    onboarding_service: Arc<WorkflowOnboardingService>,
    runtime_repository: Arc<dyn WorkflowRuntimeRepository>,
    state_repository: Arc<dyn WorkflowRuntimeStateRepository>,
    package_store: Arc<dyn WorkflowPackageStore>,
    project_workflow_binding_repository: Option<Arc<dyn ProjectWorkflowBindingRepository>>,
    runtime_artifact_repository: Option<Arc<dyn WorkflowRuntimeArtifactRepository>>,
    clock: Arc<dyn Clock>,
    capability_cache: Arc<RwLock<HashMap<String, CapabilityCheckView>>>,
    workspace_cache: Arc<RwLock<HashMap<String, WorkflowProductionWorkspaceView>>>,
}

impl WorkflowLifecycleService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source: Arc<dyn WorkflowLibrarySource>,
        library_service: Arc<WorkflowLibraryService>,
        onboarding_service: Arc<WorkflowOnboardingService>,
        runtime_repository: Arc<dyn WorkflowRuntimeRepository>,
        state_repository: Arc<dyn WorkflowRuntimeStateRepository>,
        package_store: Arc<dyn WorkflowPackageStore>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            source,
            library_service,
            onboarding_service,
            runtime_repository,
            state_repository,
            package_store,
            project_workflow_binding_repository: None,
            runtime_artifact_repository: None,
            clock,
            capability_cache: Arc::new(RwLock::new(HashMap::new())),
            workspace_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn with_project_workflow_binding_repository(
        mut self,
        repository: Arc<dyn ProjectWorkflowBindingRepository>,
    ) -> Self {
        self.project_workflow_binding_repository = Some(repository);
        self
    }

    pub fn with_runtime_artifact_repository(
        mut self,
        repository: Arc<dyn WorkflowRuntimeArtifactRepository>,
    ) -> Self {
        self.runtime_artifact_repository = Some(repository);
        self
    }

    /// Fast navigation path. This deliberately reads only registered runtime
    /// metadata, enabled state, and the in-memory capability/workspace cache.
    /// Package parsing, hashing, and live ComfyUI checks belong to the
    /// explicit diagnostics/refresh path below.
    pub async fn list_workspace(
        &self,
    ) -> Result<WorkflowProductionWorkspaceResponse, WorkflowLifecycleError> {
        let started = Instant::now();
        let runtime_versions = self
            .runtime_repository
            .list_versions()
            .await
            .map_err(db_error)?;
        let states = self
            .state_repository
            .list_states()
            .await
            .map_err(db_error)?;
        let state_by_version = states
            .into_iter()
            .map(|state| {
                (
                    state.workflow_version_id,
                    (state.enabled, state.archived, state.archived_at),
                )
            })
            .collect::<HashMap<_, _>>();
        let cached_views = self.workspace_cache.read().await.clone();
        let capabilities = self.capability_cache.read().await.clone();
        let mut items = runtime_versions
            .iter()
            .map(|version| {
                let (enabled, archived, archived_at) = state_by_version
                    .get(&version.workflow_version_id)
                    .copied()
                    .unwrap_or((true, false, None));
                fast_view_for_version(
                    version,
                    enabled,
                    archived,
                    archived_at.map(|value| value.to_rfc3339()),
                    cached_views.get(&version.workflow_version_id),
                    capabilities.get(&version.workflow_version_id),
                )
            })
            .collect::<Vec<_>>();
        let staging = self
            .package_store
            .list_staging_ids()
            .await
            .map_err(store_error)?
            .into_iter()
            .map(|staging_id| WorkflowStagingView {
                in_use: self.onboarding_service.is_draft_active(&staging_id),
                staging_id,
                status: "STALE_STAGING".to_owned(),
            })
            .collect();
        items.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then(left.workflow_version.cmp(&right.workflow_version))
        });
        tracing::info!(
            workflow_workspace_list_fast_ms = started.elapsed().as_millis() as u64,
            items = items.len(),
            "workflow workspace fast list completed"
        );
        Ok(WorkflowProductionWorkspaceResponse { items, staging })
    }

    /// Explicit refresh/diagnostics path. This is intentionally the old,
    /// complete package audit and may parse/hash packages and query ComfyUI.
    pub async fn list_workspace_diagnostics(
        &self,
    ) -> Result<WorkflowProductionWorkspaceResponse, WorkflowLifecycleError> {
        let package_loads = self.load_package_loads().await?;
        let packages = package_loads
            .iter()
            .filter_map(|package| match package {
                WorkflowPackageLoad::Loaded(files) => Some(files),
                WorkflowPackageLoad::Invalid { .. } => None,
            })
            .collect::<Vec<_>>();
        let runtime_versions = self
            .runtime_repository
            .list_versions()
            .await
            .map_err(db_error)?;
        let mut by_key = HashMap::new();
        for version in &runtime_versions {
            by_key.insert(
                (
                    version.workflow_id.clone(),
                    version.workflow_version.clone(),
                ),
                version,
            );
        }
        let mut items = Vec::new();
        let mut seen_keys = HashSet::new();

        for package in &package_loads {
            if let WorkflowPackageLoad::Invalid {
                package_name,
                message,
            } = package
            {
                items.push(invalid_package_view(
                    package_name,
                    "WORKFLOW_PACKAGE_INVALID",
                    message,
                ));
            }
        }

        for package in &packages {
            let manifest = match WorkflowManifest::parse(&package.manifest_yaml) {
                Ok(manifest) => manifest,
                Err(error) => {
                    items.push(invalid_package_view(
                        &package.package_name,
                        "WORKFLOW_PACKAGE_INVALID",
                        error,
                    ));
                    continue;
                }
            };
            if let Err(error) = manifest.validate() {
                items.push(invalid_package_view(
                    &package.package_name,
                    "WORKFLOW_PACKAGE_INVALID",
                    error,
                ));
                continue;
            }
            let key = (manifest.id.clone(), manifest.workflow_version.clone());
            let Some(version) = by_key.get(&key).copied() else {
                items.push(invalid_package_view(
                    &package.package_name,
                    "DATABASE_REGISTRATION_MISSING",
                    "runtime package is valid but not registered in the database",
                ));
                continue;
            };
            if !seen_keys.insert(key.clone()) {
                continue;
            }
            items.push(self.view_for_package(package, &manifest, version).await?);
        }

        for version in &runtime_versions {
            let key = (
                version.workflow_id.clone(),
                version.workflow_version.clone(),
            );
            if seen_keys.contains(&key) {
                continue;
            }
            let state = self
                .state_repository
                .find_state(&version.workflow_version_id)
                .await
                .map_err(db_error)?;
            let enabled = state.as_ref().map_or(true, |state| state.enabled);
            let archived = state.as_ref().is_some_and(|state| state.archived);
            let archived_at = state
                .as_ref()
                .and_then(|state| state.archived_at)
                .map(|value| value.to_rfc3339());
            items.push(WorkflowProductionWorkspaceView {
                package_name: String::new(),
                builtin: version
                    .package_name
                    .as_deref()
                    .is_some_and(is_builtin_package_name),
                archived,
                archived_at,
                package_status: "MISSING".to_owned(),
                error_code: Some("RUNTIME_PACKAGE_MISSING".to_owned()),
                error_message: Some("database registration has no runtime package".to_owned()),
                workflow_id: Some(version.workflow_id.clone()),
                workflow_version_id: Some(version.workflow_version_id.clone()),
                name: Some(version.name.clone()),
                category: Some(version.category.clone()),
                mode: Some(version.mode.clone()),
                workflow_version: Some(version.workflow_version.clone()),
                workflow_sha256: Some(version.workflow_sha256.clone()),
                recipe_sha256: version
                    .recipes
                    .last()
                    .map(|recipe| recipe.recipe_sha256.clone()),
                enabled,
                capability: "NOT_CHECKED".to_owned(),
                readiness: "BLOCKED".to_owned(),
                readiness_reasons: vec!["运行包缺失，无法进入生产就绪状态。".to_owned()],
                capability_issues: Vec::new(),
                node_count: 0,
                recipes: recipe_summaries(version),
                active_tasks: version.active_tasks,
                total_tasks: version.total_tasks,
                has_successful_run: version.has_successful_run,
                latest_success_at: version.latest_success_at.clone(),
                latest_failure_at: version.latest_failure_at.clone(),
                live_verified_at: version.latest_success_at.clone(),
                diagnostics: vec![WorkflowDiagnosticView {
                    code: "RUNTIME_PACKAGE_MISSING".to_owned(),
                    message: "database registration has no runtime package".to_owned(),
                }],
            });
        }

        let staging = self
            .package_store
            .list_staging_ids()
            .await
            .map_err(store_error)?
            .into_iter()
            .map(|staging_id| WorkflowStagingView {
                in_use: self.onboarding_service.is_draft_active(&staging_id),
                staging_id,
                status: "STALE_STAGING".to_owned(),
            })
            .collect();
        items.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then(left.workflow_version.cmp(&right.workflow_version))
        });
        {
            let mut workspace_cache = self.workspace_cache.write().await;
            let mut capability_cache = self.capability_cache.write().await;
            for item in &items {
                if let Some(workflow_version_id) = &item.workflow_version_id {
                    workspace_cache.insert(workflow_version_id.clone(), item.clone());
                    capability_cache.insert(
                        workflow_version_id.clone(),
                        CapabilityCheckView {
                            state: capability_state_enum(&item.capability),
                            checked_at: None,
                            issues: item.capability_issues.clone(),
                        },
                    );
                }
            }
        }
        Ok(WorkflowProductionWorkspaceResponse { items, staging })
    }

    pub async fn refresh_workspace(
        &self,
    ) -> Result<WorkflowProductionWorkspaceResponse, WorkflowLifecycleError> {
        self.library_service.sync().await.map_err(|error| {
            WorkflowLifecycleError::new("WORKFLOW_LIBRARY_ERROR", error.to_string())
        })?;
        self.list_workspace_diagnostics().await
    }

    pub async fn recheck_all_capabilities(
        &self,
    ) -> Result<Vec<WorkflowCapabilityBatchView>, WorkflowLifecycleError> {
        let runtime_versions = self
            .runtime_repository
            .list_versions()
            .await
            .map_err(db_error)?;
        let mut workflows = Vec::new();
        for version in &runtime_versions {
            let Some(recipe) = version
                .recipes
                .iter()
                .max_by(|left, right| compare_versions(&left.version, &right.version))
            else {
                continue;
            };
            let package = match self.find_exact_package(version, recipe).await {
                Ok(package) => package,
                Err(error)
                    if matches!(
                        error.code(),
                        "RUNTIME_PACKAGE_MISSING"
                            | "RUNTIME_ARTIFACT_HASH_MISMATCH"
                            | "WORKFLOW_PACKAGE_INVALID"
                            | "WORKFLOW_RUNTIME_HASH_MISMATCH"
                            | "RECIPE_RUNTIME_HASH_MISMATCH"
                    ) =>
                {
                    // The unified Workspace query owns the user-visible
                    // missing/invalid diagnostics. Do not scan a legacy
                    // package directory or substitute another recipe here.
                    tracing::warn!(
                        workflow_version_id = %version.workflow_version_id,
                        recipe_id = %recipe.recipe_id,
                        error = %error,
                        "skipping capability recheck for an invalid exact runtime artifact"
                    );
                    continue;
                }
                Err(error) => return Err(error),
            };
            let recipe = RecipeParser::parse(&package.recipe_yaml)
                .map_err(|error| invalid_error(error.to_string()))?;
            workflows.push(RuntimeWorkflowCapabilityInput {
                workflow_version_id: version.workflow_version_id.clone(),
                workflow_json: package.workflow_json,
                recipe,
            });
        }
        let checked = self
            .onboarding_service
            .check_runtime_workflows(&workflows)
            .await
            .map_err(|error| WorkflowLifecycleError::new(error.code(), error.to_string()))?;
        let mut workspace_cache = self.workspace_cache.write().await;
        let mut capability_cache = self.capability_cache.write().await;
        let mut result = Vec::with_capacity(checked.len());
        for (workflow_version_id, capability) in checked {
            let capability_state = capability_state(&capability);
            if let Some(view) = workspace_cache.get_mut(&workflow_version_id) {
                view.capability = capability_state;
                view.capability_issues = capability.issues.clone();
                let (readiness, reasons) = readiness_for(
                    view.enabled,
                    &view.package_status,
                    &view.capability,
                    &view.diagnostics,
                    view.recipes.len(),
                    view.has_successful_run,
                );
                view.readiness = readiness;
                view.readiness_reasons = reasons;
            }
            capability_cache.insert(workflow_version_id.clone(), capability.clone());
            result.push(WorkflowCapabilityBatchView {
                workflow_version_id,
                capability,
            });
        }
        Ok(result)
    }

    pub async fn set_enabled(
        &self,
        workflow_version_id: &str,
        enabled: bool,
    ) -> Result<(), WorkflowLifecycleError> {
        let state = self
            .state_repository
            .find_state(workflow_version_id)
            .await
            .map_err(db_error)?;
        if state.as_ref().is_some_and(|state| state.archived) {
            return Err(WorkflowLifecycleError::new(
                "WORKFLOW_ARCHIVED",
                "archived workflow versions must be restored before they can be enabled",
            ));
        }
        self.runtime_repository
            .find_version(workflow_version_id)
            .await
            .map_err(db_error)?
            .ok_or_else(|| {
                WorkflowLifecycleError::new(
                    "WORKFLOW_VERSION_NOT_FOUND",
                    "workflow version was not found",
                )
            })?;
        self.state_repository
            .set_enabled(workflow_version_id, enabled, self.clock.now())
            .await
            .map_err(db_error)?;
        if let Some(view) = self
            .workspace_cache
            .write()
            .await
            .get_mut(workflow_version_id)
        {
            view.enabled = enabled;
            let (readiness, reasons) = readiness_for(
                view.enabled,
                &view.package_status,
                &view.capability,
                &view.diagnostics,
                view.recipes.len(),
                view.has_successful_run,
            );
            view.readiness = readiness;
            view.readiness_reasons = reasons;
        }
        Ok(())
    }

    pub async fn inspect_deletion(
        &self,
        workflow_version_id: &str,
    ) -> Result<WorkflowDeletionInspection, WorkflowLifecycleError> {
        let version = self
            .runtime_repository
            .find_version(workflow_version_id)
            .await
            .map_err(db_error)?
            .ok_or_else(|| {
                WorkflowLifecycleError::new(
                    "WORKFLOW_VERSION_NOT_FOUND",
                    "workflow version was not found",
                )
            })?;
        let counts = self
            .runtime_repository
            .inspect_deletion(workflow_version_id)
            .await
            .map_err(db_error)?
            .unwrap_or_default();
        let packages = self
            .find_packages(&version.workflow_id, &version.workflow_version)
            .await?;
        let builtin = version
            .package_name
            .as_deref()
            .is_some_and(is_builtin_package_name)
            || packages
                .iter()
                .any(|package| is_builtin_package_name(&package.package_name));
        let state = self
            .state_repository
            .find_state(workflow_version_id)
            .await
            .map_err(db_error)?;
        let enabled = state.as_ref().map_or(true, |state| state.enabled);
        let archived = state.as_ref().is_some_and(|state| state.archived);
        let archived_at = state
            .as_ref()
            .and_then(|state| state.archived_at)
            .map(|value| value.to_rfc3339());
        let mut blocking_reasons = Vec::new();
        if archived {
            blocking_reasons.push("该工作流版本已从生产库移除，请先恢复后再管理。".to_owned());
        }
        if counts.active_task_count > 0 {
            blocking_reasons.push(format!(
                "仍有 {} 个活动任务或队列项，完成或取消后才能处理。",
                counts.active_task_count
            ));
        }
        if counts.active_queue_item_count > 0 {
            blocking_reasons.push(format!(
                "仍有 {} 个生产队列项处于待开始、运行或暂停状态，完成或取消后才能处理。",
                counts.active_queue_item_count
            ));
        }
        if counts.historical_task_count > 0 {
            blocking_reasons.push(format!(
                "存在 {} 条历史任务记录，将保留运行包并归档。",
                counts.historical_task_count
            ));
        }
        if counts.production_batch_item_count > 0 {
            blocking_reasons.push(format!(
                "存在 {} 条生产批次引用，将保留历史真相并归档。",
                counts.production_batch_item_count
            ));
        }
        if counts.other_reference_count > 0 {
            blocking_reasons.push(format!(
                "存在 {} 条预设、模板或 Shot 配置引用，不能永久删除。",
                counts.other_reference_count
            ));
        }
        if counts.benchmark_reference_count > 0 {
            blocking_reasons.push(format!(
                "存在 {} 条 Benchmark 历史引用，将保留历史真相并归档。",
                counts.benchmark_reference_count
            ));
        }
        let project_bindings = self.list_project_bindings(workflow_version_id).await?;
        let mut project_binding_scopes = project_bindings
            .iter()
            .map(|binding| format!("{} {}", binding.stage, binding.mode))
            .collect::<Vec<_>>();
        project_binding_scopes.sort();
        project_binding_scopes.dedup();
        let project_binding_count = project_bindings.len() as u64;
        let has_active_work = counts.active_task_count > 0 || counts.active_queue_item_count > 0;
        let has_history = counts.historical_task_count > 0
            || counts.production_batch_item_count > 0
            || counts.other_reference_count > 0
            || counts.benchmark_reference_count > 0
            || project_binding_count > 0;
        let delete_action = deletion_action(builtin, archived, has_active_work, has_history);
        Ok(WorkflowDeletionInspection {
            workflow_id: version.workflow_id,
            workflow_version_id: version.workflow_version_id,
            name: version.name,
            builtin,
            enabled,
            archived,
            archived_at,
            active_task_count: counts.active_task_count,
            active_queue_item_count: counts.active_queue_item_count,
            historical_task_count: counts.historical_task_count,
            production_batch_item_count: counts.production_batch_item_count,
            benchmark_reference_count: counts.benchmark_reference_count,
            delete_action: delete_action.to_owned(),
            project_binding_count,
            project_binding_scopes,
            can_hard_delete: delete_action == "HARD_DELETE",
            requires_archive: delete_action == "REMOVE",
            blocking_reasons,
        })
    }

    pub async fn delete_version(
        &self,
        workflow_version_id: &str,
    ) -> Result<WorkflowDeletionResult, WorkflowLifecycleError> {
        let inspection = self.inspect_deletion(workflow_version_id).await?;
        if inspection.archived {
            return Err(WorkflowLifecycleError::new(
                "WORKFLOW_ARCHIVED_DELETE_BLOCKED",
                "archived workflow versions must be restored before they can be managed",
            ));
        }
        if inspection.active_task_count > 0 || inspection.active_queue_item_count > 0 {
            return Err(WorkflowLifecycleError::new(
                "WORKFLOW_DELETE_BLOCKED_ACTIVE_TASKS",
                inspection.blocking_reasons.join(" "),
            ));
        }

        let project_bindings = if inspection.delete_action == "REMOVE" {
            self.list_project_bindings(workflow_version_id).await?
        } else {
            Vec::new()
        };
        if inspection.delete_action == "REMOVE" {
            return self.remove_version(&inspection, &project_bindings).await;
        }

        let version = self
            .runtime_repository
            .find_version(workflow_version_id)
            .await
            .map_err(db_error)?
            .ok_or_else(|| {
                WorkflowLifecycleError::new(
                    "WORKFLOW_VERSION_NOT_FOUND",
                    "workflow version was not found",
                )
            })?;
        let packages = self
            .find_packages(&version.workflow_id, &version.workflow_version)
            .await?;
        // Recheck immediately before any package/runtime deletion. A binding
        // can arrive after inspect_deletion selected HARD_DELETE; in that case
        // the safe outcome is the normal reversible REMOVE path.
        let late_project_bindings = self.list_project_bindings(workflow_version_id).await?;
        if !late_project_bindings.is_empty() {
            return self
                .remove_version(&inspection, &late_project_bindings)
                .await;
        }
        let mut package_names = packages
            .iter()
            .map(|package| package.package_name.clone())
            .collect::<Vec<_>>();
        if let Some(package_name) = &version.package_name {
            if !package_names.iter().any(|name| name == package_name) {
                package_names.push(package_name.clone());
            }
        }
        let mut removed = Vec::new();
        for package_name in package_names {
            let bytes = match self.package_store.read_runtime(&package_name).await {
                Ok(bytes) => bytes,
                Err(error) => {
                    self.restore_removed_packages(&removed).await;
                    return Err(store_error(error));
                }
            };
            if let Err(error) = self.package_store.remove_published(&package_name).await {
                self.restore_removed_packages(&removed).await;
                return Err(store_error(error));
            }
            removed.push((package_name, bytes));
        }
        if let Err(error) = self
            .runtime_repository
            .delete_version(
                &version.workflow_version_id,
                &version.workflow_id,
                self.clock.now(),
            )
            .await
        {
            self.restore_removed_packages(&removed).await;
            return Err(db_error(error));
        }
        self.capability_cache
            .write()
            .await
            .remove(workflow_version_id);
        self.workspace_cache
            .write()
            .await
            .remove(workflow_version_id);
        Ok(WorkflowDeletionResult {
            action: "HARD_DELETE".to_owned(),
            delete_action: "HARD_DELETE".to_owned(),
            project_binding_count: 0,
            workflow_id: version.workflow_id,
            workflow_version_id: version.workflow_version_id,
            archived: false,
        })
    }

    async fn list_project_bindings(
        &self,
        workflow_version_id: &str,
    ) -> Result<Vec<ProjectWorkflowBindingRecord>, WorkflowLifecycleError> {
        match &self.project_workflow_binding_repository {
            Some(repository) => repository
                .list_for_workflow_version(workflow_version_id)
                .await
                .map_err(db_error),
            None => Ok(Vec::new()),
        }
    }

    async fn remove_version(
        &self,
        inspection: &WorkflowDeletionInspection,
        project_bindings: &[ProjectWorkflowBindingRecord],
    ) -> Result<WorkflowDeletionResult, WorkflowLifecycleError> {
        let previous_state = self
            .state_repository
            .find_state(&inspection.workflow_version_id)
            .await
            .map_err(db_error)?;
        let now = self.clock.now();
        self.state_repository
            .set_archived(&inspection.workflow_version_id, true, false, Some(now), now)
            .await
            .map_err(db_error)?;

        let cleared_count = if let Some(repository) = &self.project_workflow_binding_repository {
            match repository
                .clear_by_workflow_version(&inspection.workflow_version_id)
                .await
            {
                Ok(count) => count,
                Err(error) => {
                    let cleanup_message = error.to_string();
                    let compensation = self
                        .restore_runtime_state(
                            &inspection.workflow_version_id,
                            previous_state.as_ref(),
                        )
                        .await;
                    match compensation {
                        Ok(()) => {
                            let (previous_archived, previous_enabled) = previous_state
                                .as_ref()
                                .map(|state| (state.archived, state.enabled))
                                .unwrap_or((false, true));
                            self.update_cached_archive_state(
                                &inspection.workflow_version_id,
                                previous_archived,
                                previous_enabled,
                            )
                            .await;
                            return Err(WorkflowLifecycleError::new(
                                "WORKFLOW_DELETE_BINDING_CLEANUP_FAILED",
                                format!(
                                    "clearing {} exact project binding(s) failed: {cleanup_message}",
                                    project_bindings.len()
                                ),
                            ));
                        }
                        Err(compensation_error) => {
                            return Err(WorkflowLifecycleError::new(
                                "WORKFLOW_DELETE_COMPENSATION_FAILED",
                                format!(
                                    "clearing {} exact project binding(s) failed: {cleanup_message}; \
                                     restoring runtime state also failed: {compensation_error}",
                                    project_bindings.len()
                                ),
                            ));
                        }
                    }
                }
            }
        } else {
            0
        };

        self.update_cached_archive_state(&inspection.workflow_version_id, true, false)
            .await;
        self.capability_cache
            .write()
            .await
            .remove(&inspection.workflow_version_id);
        Ok(WorkflowDeletionResult {
            action: "REMOVE".to_owned(),
            delete_action: "REMOVE".to_owned(),
            project_binding_count: cleared_count,
            workflow_id: inspection.workflow_id.clone(),
            workflow_version_id: inspection.workflow_version_id.clone(),
            archived: true,
        })
    }

    async fn restore_runtime_state(
        &self,
        workflow_version_id: &str,
        previous_state: Option<&WorkflowRuntimeState>,
    ) -> Result<(), WorkflowLifecycleError> {
        let (archived, enabled, archived_at) = previous_state
            .map(|state| (state.archived, state.enabled, state.archived_at))
            .unwrap_or((false, true, None));
        self.state_repository
            .set_archived(
                workflow_version_id,
                archived,
                enabled,
                archived_at,
                self.clock.now(),
            )
            .await
            .map_err(db_error)
    }

    pub async fn delete_workflow(
        &self,
        workflow_id: &str,
    ) -> Result<Vec<WorkflowDeletionResult>, WorkflowLifecycleError> {
        let versions = self
            .runtime_repository
            .list_versions()
            .await
            .map_err(db_error)?
            .into_iter()
            .filter(|version| version.workflow_id == workflow_id)
            .map(|version| version.workflow_version_id)
            .collect::<Vec<_>>();
        if versions.is_empty() {
            return Err(WorkflowLifecycleError::new(
                "WORKFLOW_NOT_FOUND",
                "workflow was not found",
            ));
        }
        for version_id in &versions {
            let inspection = self.inspect_deletion(version_id).await?;
            if inspection.active_task_count > 0
                || inspection.active_queue_item_count > 0
                || inspection.archived
            {
                return Err(WorkflowLifecycleError::new(
                    if inspection.active_task_count > 0 || inspection.active_queue_item_count > 0 {
                        "WORKFLOW_DELETE_BLOCKED_ACTIVE_TASKS"
                    } else {
                        "WORKFLOW_ARCHIVED_DELETE_BLOCKED"
                    },
                    inspection.blocking_reasons.join(" "),
                ));
            }
        }
        let mut result = Vec::new();
        for version_id in versions {
            match self.delete_version(&version_id).await {
                Ok(deletion) => result.push(deletion),
                Err(error) if result.is_empty() => return Err(error),
                Err(error) => {
                    return Err(WorkflowLifecycleError::new(
                        "WORKFLOW_DELETE_WORKFLOW_PARTIAL_FAILURE",
                        format!(
                            "workflow version {version_id} failed after {} version(s) were processed: {error}",
                            result.len()
                        ),
                    ));
                }
            }
        }
        Ok(result)
    }

    pub async fn restore_version(
        &self,
        workflow_version_id: &str,
    ) -> Result<WorkflowRestoreResult, WorkflowLifecycleError> {
        self.runtime_repository
            .find_version(workflow_version_id)
            .await
            .map_err(db_error)?
            .ok_or_else(|| {
                WorkflowLifecycleError::new(
                    "WORKFLOW_VERSION_NOT_FOUND",
                    "workflow version was not found",
                )
            })?;
        let state = self
            .state_repository
            .find_state(workflow_version_id)
            .await
            .map_err(db_error)?;
        if !state.as_ref().is_some_and(|state| state.archived) {
            return Err(WorkflowLifecycleError::new(
                "WORKFLOW_NOT_ARCHIVED",
                "workflow version is not archived",
            ));
        }
        let now = self.clock.now();
        self.state_repository
            .set_archived(workflow_version_id, false, false, None, now)
            .await
            .map_err(db_error)?;
        self.workspace_cache
            .write()
            .await
            .remove(workflow_version_id);
        self.capability_cache
            .write()
            .await
            .remove(workflow_version_id);
        let capability = match self.recheck_capability(workflow_version_id).await {
            Ok(capability) => capability,
            Err(error) => {
                let capability = capability_for_restore_error(&error, self.clock.now());
                self.capability_cache
                    .write()
                    .await
                    .insert(workflow_version_id.to_owned(), capability.clone());
                tracing::warn!(
                    workflow_version_id,
                    error = %error,
                    "workflow restored without a successful capability recheck"
                );
                capability
            }
        };
        let capability_name = capability_state(&capability);
        let enabled = if capability.state == CapabilityState::Ready {
            self.set_enabled(workflow_version_id, true).await?;
            true
        } else {
            false
        };
        Ok(WorkflowRestoreResult {
            workflow_version_id: workflow_version_id.to_owned(),
            archived: false,
            enabled,
            readiness: restore_readiness(&capability_name, enabled),
            capability: capability_name,
        })
    }

    async fn update_cached_archive_state(
        &self,
        workflow_version_id: &str,
        archived: bool,
        enabled: bool,
    ) {
        if let Some(view) = self
            .workspace_cache
            .write()
            .await
            .get_mut(workflow_version_id)
        {
            view.archived = archived;
            view.archived_at = if archived {
                Some(self.clock.now().to_rfc3339())
            } else {
                None
            };
            view.enabled = enabled;
            let (readiness, reasons) = readiness_for(
                enabled,
                &view.package_status,
                &view.capability,
                &view.diagnostics,
                view.recipes.len(),
                view.has_successful_run,
            );
            view.readiness = readiness;
            view.readiness_reasons = reasons;
        }
    }

    pub async fn inspect_recipe_runtime(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
    ) -> Result<WorkflowRecipeRuntimeInspection, WorkflowLifecycleError> {
        let version = self
            .runtime_repository
            .find_version(workflow_version_id)
            .await
            .map_err(db_error)?
            .ok_or_else(|| {
                WorkflowLifecycleError::new(
                    "WORKFLOW_VERSION_NOT_FOUND",
                    "workflow version was not found",
                )
            })?;
        let recipe = version
            .recipes
            .iter()
            .find(|recipe| recipe.recipe_id == recipe_id)
            .ok_or_else(|| {
                WorkflowLifecycleError::new(
                    "RECIPE_NOT_FOUND",
                    "recipe was not found for the workflow version",
                )
            })?;
        let state = self
            .state_repository
            .find_state(workflow_version_id)
            .await
            .map_err(db_error)?;
        let enabled = state.as_ref().map_or(true, |state| state.enabled);
        let archived = state.as_ref().is_some_and(|state| state.archived);
        let invalid = |package_name: String, diagnostics: Vec<WorkflowDiagnosticView>| {
            WorkflowRecipeRuntimeInspection {
                workflow_id: version.workflow_id.clone(),
                workflow_version_id: version.workflow_version_id.clone(),
                recipe_id: recipe.recipe_id.clone(),
                recipe_version: recipe.version.clone(),
                enabled,
                archived,
                package_name,
                package_status: "INVALID".to_owned(),
                diagnostics,
                capability: "NOT_CHECKED".to_owned(),
                capability_issues: Vec::new(),
                has_successful_run: version.has_successful_run,
            }
        };

        let package = match self.find_exact_package(&version, recipe).await {
            Ok(package) => package,
            Err(error) if error.code() == "RUNTIME_PACKAGE_MISSING" => {
                return Ok(invalid(
                    version.package_name.clone().unwrap_or_default(),
                    vec![WorkflowDiagnosticView {
                        code: "RUNTIME_PACKAGE_MISSING".to_owned(),
                        message: "the exact recipe runtime package was not found".to_owned(),
                    }],
                ));
            }
            Err(error) => return Err(error),
        };
        let manifest = match WorkflowManifest::parse(&package.manifest_yaml) {
            Ok(manifest) => manifest,
            Err(error) => {
                return Ok(invalid(
                    package.package_name,
                    vec![WorkflowDiagnosticView {
                        code: "WORKFLOW_PACKAGE_INVALID".to_owned(),
                        message: error,
                    }],
                ));
            }
        };
        let mut diagnostics = Vec::new();
        if let Err(error) = manifest.validate() {
            diagnostics.push(WorkflowDiagnosticView {
                code: "WORKFLOW_PACKAGE_INVALID".to_owned(),
                message: error,
            });
        }
        if manifest.id != version.workflow_id {
            diagnostics.push(WorkflowDiagnosticView {
                code: "WORKFLOW_PACKAGE_INVALID".to_owned(),
                message: "runtime package workflow id does not match the registered workflow"
                    .to_owned(),
            });
        }
        if manifest.workflow_version != version.workflow_version {
            diagnostics.push(WorkflowDiagnosticView {
                code: "WORKFLOW_PACKAGE_INVALID".to_owned(),
                message: "runtime package workflow version does not match the registered version"
                    .to_owned(),
            });
        }
        if manifest.recipe_version != recipe.version {
            diagnostics.push(WorkflowDiagnosticView {
                code: "WORKFLOW_PACKAGE_INVALID".to_owned(),
                message: "runtime package recipe version does not match the exact recipe"
                    .to_owned(),
            });
        }
        if sha256(package.workflow_json.as_bytes()) != version.workflow_sha256 {
            diagnostics.push(WorkflowDiagnosticView {
                code: "WORKFLOW_RUNTIME_HASH_MISMATCH".to_owned(),
                message: "runtime workflow bytes do not match the registered hash".to_owned(),
            });
        }
        if sha256(package.recipe_yaml.as_bytes()) != recipe.recipe_sha256 {
            diagnostics.push(WorkflowDiagnosticView {
                code: "RECIPE_RUNTIME_HASH_MISMATCH".to_owned(),
                message: "exact runtime recipe bytes do not match the registered hash".to_owned(),
            });
        }
        let exact_recipe = match RecipeParser::parse(&package.recipe_yaml) {
            Ok(recipe) => recipe,
            Err(error) => {
                diagnostics.push(WorkflowDiagnosticView {
                    code: "RECIPE_INVALID".to_owned(),
                    message: error.to_string(),
                });
                return Ok(invalid(package.package_name, diagnostics));
            }
        };
        if let Err(error) = parse_workflow(&package.workflow_json) {
            diagnostics.push(WorkflowDiagnosticView {
                code: "WORKFLOW_PACKAGE_INVALID".to_owned(),
                message: error.to_string(),
            });
            return Ok(invalid(package.package_name, diagnostics));
        }
        if !diagnostics.is_empty() {
            return Ok(invalid(package.package_name, diagnostics));
        }
        let capability = self
            .onboarding_service
            .check_runtime_workflow_with_recipe(&package.workflow_json, &exact_recipe)
            .await
            .map_err(|error| WorkflowLifecycleError::new(error.code(), error.to_string()))?;
        Ok(WorkflowRecipeRuntimeInspection {
            workflow_id: version.workflow_id,
            workflow_version_id: version.workflow_version_id,
            recipe_id: recipe.recipe_id.clone(),
            recipe_version: recipe.version.clone(),
            enabled,
            archived,
            package_name: package.package_name,
            package_status: "VALID".to_owned(),
            diagnostics,
            capability: capability_state(&capability),
            capability_issues: capability.issues,
            has_successful_run: version.has_successful_run,
        })
    }

    async fn find_exact_package(
        &self,
        version: &RuntimeWorkflowVersionRecord,
        recipe: &crate::application::ports::RuntimeRecipeRecord,
    ) -> Result<crate::application::ports::WorkflowPackageFiles, WorkflowLifecycleError> {
        if let Some(repository) = &self.runtime_artifact_repository {
            let artifacts = repository
                .list_for_recipe(&version.workflow_version_id, &recipe.recipe_id)
                .await
                .map_err(db_error)?;
            let artifact = match artifacts.as_slice() {
                [] => {
                    return Err(WorkflowLifecycleError::new(
                        "RUNTIME_PACKAGE_MISSING",
                        "the exact recipe runtime artifact was not registered",
                    ))
                }
                [artifact] => artifact,
                _ => {
                    return Err(WorkflowLifecycleError::new(
                        "RUNTIME_PACKAGE_AMBIGUOUS",
                        "more than one runtime artifact is registered for the exact recipe",
                    ))
                }
            };
            let bytes = self
                .package_store
                .read_runtime(&artifact.package_name)
                .await
                .map_err(|error| {
                    WorkflowLifecycleError::new("RUNTIME_PACKAGE_MISSING", error.to_string())
                })?;
            let (manifest_yaml, recipe_yaml, workflow_json) =
                validate_exact_runtime_package(&bytes, version, recipe, artifact)?;
            return Ok(crate::application::ports::WorkflowPackageFiles {
                package_name: artifact.package_name.clone(),
                package_source_path: artifact.package_source_path.clone(),
                manifest_yaml,
                recipe_yaml,
                workflow_json,
            });
        }
        self.find_package(
            &version.workflow_id,
            &version.workflow_version,
            Some(&recipe.version),
        )
        .await
    }

    async fn restore_removed_packages(&self, packages: &[(String, WorkflowPackageBytes)]) {
        for (package_name, bytes) in packages {
            let staging_id = format!("onb_{}", Uuid::new_v4());
            if self.package_store.stage(&staging_id, bytes).await.is_ok() {
                if self
                    .package_store
                    .publish_atomic(&staging_id, package_name)
                    .await
                    .is_err()
                {
                    let _ = self.package_store.remove_staging(&staging_id).await;
                }
            }
        }
    }

    pub async fn recheck_capability(
        &self,
        workflow_version_id: &str,
    ) -> Result<CapabilityCheckView, WorkflowLifecycleError> {
        let version = self
            .runtime_repository
            .find_version(workflow_version_id)
            .await
            .map_err(db_error)?
            .ok_or_else(|| {
                WorkflowLifecycleError::new(
                    "WORKFLOW_VERSION_NOT_FOUND",
                    "workflow version was not found",
                )
            })?;
        let package = self.find_default_recipe_package(&version).await?;
        let recipe = RecipeParser::parse(&package.recipe_yaml)
            .map_err(|error| invalid_error(error.to_string()))?;
        let capability = self
            .onboarding_service
            .check_runtime_workflow_with_recipe(&package.workflow_json, &recipe)
            .await
            .map_err(|error| WorkflowLifecycleError::new(error.code(), error.to_string()))?;
        self.capability_cache
            .write()
            .await
            .insert(workflow_version_id.to_owned(), capability.clone());
        if let Some(view) = self
            .workspace_cache
            .write()
            .await
            .get_mut(workflow_version_id)
        {
            view.capability = capability_state(&capability);
            view.capability_issues = capability.issues.clone();
            let (readiness, reasons) = readiness_for(
                view.enabled,
                &view.package_status,
                &view.capability,
                &view.diagnostics,
                view.recipes.len(),
                view.has_successful_run,
            );
            view.readiness = readiness;
            view.readiness_reasons = reasons;
        }
        Ok(capability)
    }

    pub async fn duplicate_recipe(
        &self,
        workflow_version_id: &str,
        recipe_id: Option<String>,
        recipe_version: Option<String>,
    ) -> Result<WorkflowOnboardingDraftView, WorkflowLifecycleError> {
        let version = self
            .runtime_repository
            .find_version(workflow_version_id)
            .await
            .map_err(db_error)?
            .ok_or_else(|| {
                WorkflowLifecycleError::new(
                    "WORKFLOW_VERSION_NOT_FOUND",
                    "workflow version was not found",
                )
            })?;
        let source_recipe_version = recipe_id
            .as_deref()
            .and_then(|id| version.recipes.iter().find(|recipe| recipe.recipe_id == id))
            .map(|recipe| recipe.version.as_str());
        self.onboarding_service
            .duplicate_recipe_draft(
                &version.workflow_id,
                &version.workflow_version,
                source_recipe_version,
                recipe_version,
            )
            .await
            .map_err(|error| WorkflowLifecycleError::new(error.code(), error.to_string()))
    }

    pub async fn compare_versions(
        &self,
        version_a_id: &str,
        version_b_id: &str,
    ) -> Result<WorkflowVersionDiffView, WorkflowLifecycleError> {
        let a = self.version_with_package(version_a_id).await?;
        let b = self.version_with_package(version_b_id).await?;
        if a.0.workflow_id != b.0.workflow_id {
            return Err(WorkflowLifecycleError::new(
                "WORKFLOW_VERSION_COMPARE_INVALID",
                "versions must belong to the same workflow",
            ));
        }
        let workflow_a = parse_workflow(&a.1.workflow_json)?;
        let workflow_b = parse_workflow(&b.1.workflow_json)?;
        let (
            added_nodes,
            removed_nodes,
            changed_class_types,
            changed_literal_inputs,
            changed_links,
        ) = diff_workflow(&workflow_a, &workflow_b);
        let recipe_a = RecipeParser::parse(&a.1.recipe_yaml)
            .map_err(|error| invalid_error(error.to_string()))?;
        let recipe_b = RecipeParser::parse(&b.1.recipe_yaml)
            .map_err(|error| invalid_error(error.to_string()))?;
        let (recipe_input_changes, binding_changes, output_changes) =
            diff_recipe(&recipe_a, &recipe_b);
        Ok(WorkflowVersionDiffView {
            workflow_id: a.0.workflow_id.clone(),
            version_a: a.0.workflow_version.clone(),
            version_b: b.0.workflow_version.clone(),
            node_count_a: workflow_a.value().as_object().map_or(0, Map::len),
            node_count_b: workflow_b.value().as_object().map_or(0, Map::len),
            added_nodes,
            removed_nodes,
            changed_class_types,
            changed_literal_inputs,
            changed_links,
            recipe_input_changes,
            binding_changes,
            output_changes,
        })
    }

    pub async fn export_package(
        &self,
        workflow_version_id: &str,
    ) -> Result<WorkflowExportView, WorkflowLifecycleError> {
        let version = self
            .runtime_repository
            .find_version(workflow_version_id)
            .await
            .map_err(db_error)?
            .ok_or_else(|| {
                WorkflowLifecycleError::new(
                    "WORKFLOW_VERSION_NOT_FOUND",
                    "workflow version was not found",
                )
            })?;
        let package = self.find_default_recipe_package(&version).await?;
        let bytes = self
            .package_store
            .read_runtime(&package.package_name)
            .await
            .map_err(store_error)?;
        read_back_and_validate_package(&bytes)
            .map_err(|error| WorkflowLifecycleError::new(error.code(), error.to_string()))?;
        let archive = build_archive(&bytes)?;
        Ok(WorkflowExportView {
            file_name: format!(
                "{}-{}.aistudio-workflow.zip",
                safe_file_stem(&version.name),
                version.workflow_version
            ),
            bytes: archive,
        })
    }

    pub async fn restore_package(
        &self,
        archive_bytes: Vec<u8>,
    ) -> Result<WorkflowRestoreView, WorkflowLifecycleError> {
        let package = parse_archive(&archive_bytes)?;
        read_back_and_validate_package(&package)
            .map_err(|error| WorkflowLifecycleError::new(error.code(), error.to_string()))?;
        let manifest = parse_manifest_bytes(&package.manifest_yaml)?;
        let workflow_json = String::from_utf8(package.workflow_api_json.clone())
            .map_err(|error| invalid_error(error.to_string()))?;
        let recipe_yaml = String::from_utf8(package.recipe_yaml.clone())
            .map_err(|error| invalid_error(error.to_string()))?;
        let recipe =
            RecipeParser::parse(&recipe_yaml).map_err(|error| invalid_error(error.to_string()))?;
        let workflow_sha = sha256(&package.workflow_api_json);
        let recipe_sha = sha256(&package.recipe_yaml);

        let existing_versions = self
            .runtime_repository
            .list_versions()
            .await
            .map_err(db_error)?;
        if let Some(existing) = existing_versions.iter().find(|version| {
            version.workflow_id == manifest.id
                && version.workflow_version == manifest.workflow_version
        }) {
            if existing.workflow_sha256 != workflow_sha {
                return Err(WorkflowLifecycleError::new(
                    "WORKFLOW_VERSION_CONFLICT",
                    "workflow version already exists with different content",
                ));
            }
            if let Some(recipe) = existing
                .recipes
                .iter()
                .find(|recipe| recipe.version == manifest.recipe_version)
            {
                if recipe.recipe_sha256 == recipe_sha {
                    let state = self
                        .state_repository
                        .find_state(&existing.workflow_version_id)
                        .await
                        .map_err(db_error)?;
                    return Ok(WorkflowRestoreView {
                        status: "ALREADY_INSTALLED".to_owned(),
                        package_name: String::new(),
                        workflow_id: manifest.id,
                        workflow_version: manifest.workflow_version,
                        recipe_id: Some(recipe.recipe_id.clone()),
                        enabled: state.as_ref().map_or(true, |value| value.enabled),
                        capability: "READY".to_owned(),
                    });
                }
                return Err(WorkflowLifecycleError::new(
                    "RECIPE_VERSION_CONFLICT",
                    "recipe version already exists with different content",
                ));
            }
        }

        let capability = self
            .onboarding_service
            .check_runtime_workflow_with_recipe(&workflow_json, &recipe)
            .await
            .map_err(|error| WorkflowLifecycleError::new(error.code(), error.to_string()))?;
        let enabled = capability.state
            == crate::application::workflow_onboarding_service::CapabilityState::Ready;
        let package_name = format!(
            "{}_{}_{}_{}_{}",
            safe_identifier(&manifest.id),
            manifest.workflow_version.replace('.', "_"),
            safe_identifier(&manifest.recipe_version),
            &workflow_sha[..8],
            &recipe_sha[..8],
        );
        let staging_id = format!("onb_{}", Uuid::new_v4());
        self.package_store
            .stage(&staging_id, &package)
            .await
            .map_err(store_error)?;
        let staged = match self.package_store.read_staging(&staging_id).await {
            Ok(value) => value,
            Err(error) => {
                let _ = self.package_store.remove_staging(&staging_id).await;
                return Err(store_error(error));
            }
        };
        if let Err(error) = read_back_and_validate_package(&staged) {
            let _ = self.package_store.remove_staging(&staging_id).await;
            return Err(WorkflowLifecycleError::new(error.code(), error.to_string()));
        }
        if let Err(error) = self
            .package_store
            .publish_atomic(&staging_id, &package_name)
            .await
        {
            let _ = self.package_store.remove_staging(&staging_id).await;
            return Err(store_error(error));
        }
        let sync = self.library_service.sync().await;
        if let Err(error) = sync {
            let _ = self.package_store.remove_published(&package_name).await;
            return Err(WorkflowLifecycleError::new(
                "WORKFLOW_PACKAGE_INVALID",
                error.to_string(),
            ));
        }
        let registered = self
            .runtime_repository
            .list_versions()
            .await
            .map_err(db_error)?;
        let record = registered
            .iter()
            .find(|version| {
                version.workflow_id == manifest.id
                    && version.workflow_version == manifest.workflow_version
            })
            .ok_or_else(|| {
                WorkflowLifecycleError::new(
                    "DATABASE_REGISTRATION_MISSING",
                    "restored package was not registered",
                )
            })?;
        self.state_repository
            .set_enabled(&record.workflow_version_id, enabled, self.clock.now())
            .await
            .map_err(db_error)?;
        let recipe_id = record
            .recipes
            .iter()
            .find(|recipe| recipe.version == manifest.recipe_version)
            .map(|recipe| recipe.recipe_id.clone());
        Ok(WorkflowRestoreView {
            status: "RESTORED".to_owned(),
            package_name,
            workflow_id: manifest.id,
            workflow_version: manifest.workflow_version,
            recipe_id,
            enabled,
            capability: capability_state(&capability),
        })
    }

    pub async fn cleanup_staging(&self, staging_id: &str) -> Result<(), WorkflowLifecycleError> {
        if self.onboarding_service.is_draft_active(staging_id) {
            return Err(WorkflowLifecycleError::new(
                "STAGING_IN_USE",
                "staging is still used by an onboarding draft",
            ));
        }
        self.package_store
            .remove_staging(staging_id)
            .await
            .map_err(store_error)
    }

    async fn view_for_package(
        &self,
        package: &crate::application::ports::WorkflowPackageFiles,
        manifest: &WorkflowManifest,
        version: &RuntimeWorkflowVersionRecord,
    ) -> Result<WorkflowProductionWorkspaceView, WorkflowLifecycleError> {
        let state = self
            .state_repository
            .find_state(&version.workflow_version_id)
            .await
            .map_err(db_error)?;
        let enabled = state.as_ref().map_or(true, |state| state.enabled);
        let archived = state.as_ref().is_some_and(|state| state.archived);
        let archived_at = state
            .as_ref()
            .and_then(|state| state.archived_at)
            .map(|value| value.to_rfc3339());
        let workflow = match parse_workflow(&package.workflow_json) {
            Ok(workflow) => workflow,
            Err(error) => {
                return Ok(invalid_package_view(
                    &package.package_name,
                    "WORKFLOW_PACKAGE_INVALID",
                    error.to_string(),
                ));
            }
        };
        let recipe = RecipeParser::parse(&package.recipe_yaml)
            .map_err(|error| WorkflowLifecycleError::new("RECIPE_INVALID", error.to_string()))?;
        let capability = self
            .onboarding_service
            .check_runtime_workflow_with_recipe(&package.workflow_json, &recipe)
            .await
            .map_err(|error| WorkflowLifecycleError::new(error.code(), error.to_string()))?;
        let workflow_hash = sha256(package.workflow_json.as_bytes());
        let selected_recipe = version
            .recipes
            .iter()
            .find(|recipe| recipe.version == manifest.recipe_version);
        let recipe_hash = selected_recipe.map(|_| sha256(package.recipe_yaml.as_bytes()));
        let mut diagnostics = Vec::new();
        if is_builtin_package_name(&package.package_name) {
            if let Some(source_path) = package.package_source_path.as_deref() {
                let root = Path::new(source_path)
                    .parent()
                    .unwrap_or_else(|| Path::new(source_path));
                if let Some(mismatch) = audit_installed(root)
                    .into_iter()
                    .find(|mismatch| mismatch.package_name == package.package_name)
                {
                    diagnostics.push(WorkflowDiagnosticView {
                        code: mismatch.code,
                        message: format!(
                            "内置 Runtime Package 与程序内 immutable 内容不一致（expected {}, actual {}）；请执行修复动作。",
                            mismatch.expected_sha256, mismatch.actual_sha256
                        ),
                    });
                }
            }
        }
        if workflow_hash != version.workflow_sha256 {
            diagnostics.push(WorkflowDiagnosticView {
                code: "WORKFLOW_RUNTIME_HASH_MISMATCH".to_owned(),
                message: "runtime workflow bytes do not match the registered hash".to_owned(),
            });
        }
        if selected_recipe.is_none() {
            diagnostics.push(WorkflowDiagnosticView {
                code: "RECIPE_RUNTIME_HASH_MISMATCH".to_owned(),
                message: "runtime recipe is not registered for this package".to_owned(),
            });
        } else if recipe_hash.as_deref()
            != selected_recipe.map(|recipe| recipe.recipe_sha256.as_str())
        {
            diagnostics.push(WorkflowDiagnosticView {
                code: "RECIPE_RUNTIME_HASH_MISMATCH".to_owned(),
                message: "runtime recipe bytes do not match the registered hash".to_owned(),
            });
        }
        let package_status = if diagnostics.is_empty() {
            "VALID"
        } else {
            "INVALID"
        };
        let capability_state = capability_state(&capability);
        let (readiness, readiness_reasons) = readiness_for(
            enabled,
            package_status,
            &capability_state,
            &diagnostics,
            version.recipes.len(),
            version.has_successful_run,
        );
        Ok(WorkflowProductionWorkspaceView {
            package_name: package.package_name.clone(),
            builtin: is_builtin_package_name(&package.package_name),
            archived,
            archived_at,
            package_status: package_status.to_owned(),
            error_code: diagnostics
                .first()
                .map(|diagnostic| diagnostic.code.clone()),
            error_message: diagnostics
                .first()
                .map(|diagnostic| diagnostic.message.clone()),
            workflow_id: Some(version.workflow_id.clone()),
            workflow_version_id: Some(version.workflow_version_id.clone()),
            name: Some(version.name.clone()),
            category: Some(version.category.clone()),
            mode: Some(version.mode.clone()),
            workflow_version: Some(version.workflow_version.clone()),
            workflow_sha256: Some(version.workflow_sha256.clone()),
            recipe_sha256: selected_recipe.map(|recipe| recipe.recipe_sha256.clone()),
            enabled,
            capability: capability_state,
            readiness,
            readiness_reasons,
            capability_issues: capability.issues,
            node_count: workflow.value().as_object().map_or(0, Map::len),
            recipes: recipe_summaries(version),
            active_tasks: version.active_tasks,
            total_tasks: version.total_tasks,
            has_successful_run: version.has_successful_run,
            latest_success_at: version.latest_success_at.clone(),
            latest_failure_at: version.latest_failure_at.clone(),
            live_verified_at: version.latest_success_at.clone(),
            diagnostics,
        })
    }

    async fn version_with_package(
        &self,
        workflow_version_id: &str,
    ) -> Result<
        (
            RuntimeWorkflowVersionRecord,
            crate::application::ports::WorkflowPackageFiles,
        ),
        WorkflowLifecycleError,
    > {
        let version = self
            .runtime_repository
            .find_version(workflow_version_id)
            .await
            .map_err(db_error)?
            .ok_or_else(|| {
                WorkflowLifecycleError::new(
                    "WORKFLOW_VERSION_NOT_FOUND",
                    "workflow version was not found",
                )
            })?;
        let package = self.find_default_recipe_package(&version).await?;
        Ok((version, package))
    }

    async fn find_default_recipe_package(
        &self,
        version: &RuntimeWorkflowVersionRecord,
    ) -> Result<crate::application::ports::WorkflowPackageFiles, WorkflowLifecycleError> {
        let recipe = version
            .recipes
            .iter()
            .max_by(|left, right| compare_versions(&left.version, &right.version))
            .ok_or_else(|| {
                WorkflowLifecycleError::new(
                    "RUNTIME_PACKAGE_MISSING",
                    "the workflow version has no registered recipe",
                )
            })?;
        self.find_exact_package(version, recipe).await
    }

    async fn find_package(
        &self,
        workflow_id: &str,
        workflow_version: &str,
        recipe_version: Option<&str>,
    ) -> Result<crate::application::ports::WorkflowPackageFiles, WorkflowLifecycleError> {
        let packages = self.find_packages(workflow_id, workflow_version).await?;
        packages
            .into_iter()
            .filter_map(|package| {
                let manifest = WorkflowManifest::parse(&package.manifest_yaml).ok()?;
                recipe_version
                    .is_none_or(|version| manifest.recipe_version == version)
                    .then_some((manifest, package))
            })
            .max_by(|left, right| compare_versions(&left.0.recipe_version, &right.0.recipe_version))
            .map(|(_, package)| package)
            .ok_or_else(|| {
                WorkflowLifecycleError::new(
                    "RUNTIME_PACKAGE_MISSING",
                    "the runtime package was not found",
                )
            })
    }

    async fn find_packages(
        &self,
        workflow_id: &str,
        workflow_version: &str,
    ) -> Result<Vec<crate::application::ports::WorkflowPackageFiles>, WorkflowLifecycleError> {
        Ok(self
            .load_packages()
            .await?
            .into_iter()
            .filter(|package| {
                WorkflowManifest::parse(&package.manifest_yaml).is_ok_and(|manifest| {
                    manifest.id == workflow_id && manifest.workflow_version == workflow_version
                })
            })
            .collect())
    }

    async fn load_packages(
        &self,
    ) -> Result<Vec<crate::application::ports::WorkflowPackageFiles>, WorkflowLifecycleError> {
        Ok(self
            .load_package_loads()
            .await?
            .into_iter()
            .filter_map(|package| match package {
                WorkflowPackageLoad::Loaded(files) => Some(files),
                WorkflowPackageLoad::Invalid { .. } => None,
            })
            .collect())
    }

    async fn load_package_loads(&self) -> Result<Vec<WorkflowPackageLoad>, WorkflowLifecycleError> {
        self.source.load_packages().await.map_err(|error| {
            WorkflowLifecycleError::new("WORKFLOW_LIBRARY_ERROR", error.to_string())
        })
    }
}

fn invalid_package_view(
    package_name: &str,
    code: &str,
    message: impl Into<String>,
) -> WorkflowProductionWorkspaceView {
    let message = message.into();
    WorkflowProductionWorkspaceView {
        package_name: package_name.to_owned(),
        builtin: is_builtin_package_name(package_name),
        archived: false,
        archived_at: None,
        package_status: "INVALID".to_owned(),
        error_code: Some(code.to_owned()),
        error_message: Some(message.clone()),
        workflow_id: None,
        workflow_version_id: None,
        name: None,
        category: None,
        mode: None,
        workflow_version: None,
        workflow_sha256: None,
        recipe_sha256: None,
        enabled: false,
        capability: "NOT_CHECKED".to_owned(),
        readiness: "BLOCKED".to_owned(),
        readiness_reasons: vec![message.clone()],
        capability_issues: Vec::new(),
        node_count: 0,
        recipes: Vec::new(),
        active_tasks: 0,
        total_tasks: 0,
        has_successful_run: false,
        latest_success_at: None,
        latest_failure_at: None,
        live_verified_at: None,
        diagnostics: vec![WorkflowDiagnosticView {
            code: code.to_owned(),
            message,
        }],
    }
}

fn fast_view_for_version(
    version: &RuntimeWorkflowVersionRecord,
    enabled: bool,
    archived: bool,
    archived_at: Option<String>,
    cached: Option<&WorkflowProductionWorkspaceView>,
    capability: Option<&CapabilityCheckView>,
) -> WorkflowProductionWorkspaceView {
    let cached_capability = cached.map(|view| CapabilityCheckView {
        state: capability_state_enum(&view.capability),
        checked_at: None,
        issues: view.capability_issues.clone(),
    });
    let capability = capability.or(cached_capability.as_ref());
    let capability_name = capability
        .map(capability_state)
        .unwrap_or_else(|| "NOT_CHECKED".to_owned());
    let mut view = cached
        .cloned()
        .unwrap_or_else(|| WorkflowProductionWorkspaceView {
            package_name: version.workflow_id.clone(),
            builtin: version
                .package_name
                .as_deref()
                .is_some_and(is_builtin_package_name),
            archived,
            archived_at: archived_at.clone(),
            package_status: "VALID".to_owned(),
            error_code: None,
            error_message: None,
            workflow_id: Some(version.workflow_id.clone()),
            workflow_version_id: Some(version.workflow_version_id.clone()),
            name: Some(version.name.clone()),
            category: Some(version.category.clone()),
            mode: Some(version.mode.clone()),
            workflow_version: Some(version.workflow_version.clone()),
            workflow_sha256: Some(version.workflow_sha256.clone()),
            recipe_sha256: version
                .recipes
                .last()
                .map(|recipe| recipe.recipe_sha256.clone()),
            enabled,
            capability: capability_name.clone(),
            readiness: "DEGRADED".to_owned(),
            readiness_reasons: Vec::new(),
            capability_issues: capability
                .map(|value| value.issues.clone())
                .unwrap_or_default(),
            node_count: 0,
            recipes: fast_recipe_summaries(version),
            active_tasks: version.active_tasks,
            total_tasks: version.total_tasks,
            has_successful_run: version.has_successful_run,
            latest_success_at: version.latest_success_at.clone(),
            latest_failure_at: version.latest_failure_at.clone(),
            live_verified_at: version.latest_success_at.clone(),
            diagnostics: Vec::new(),
        });
    view.enabled = enabled;
    view.package_name = version
        .package_name
        .clone()
        .unwrap_or_else(|| view.package_name.clone());
    view.builtin = version
        .package_name
        .as_deref()
        .map_or(view.builtin, is_builtin_package_name);
    view.archived = archived;
    view.archived_at = archived_at;
    view.workflow_id = Some(version.workflow_id.clone());
    view.workflow_version_id = Some(version.workflow_version_id.clone());
    view.name = Some(version.name.clone());
    view.category = Some(version.category.clone());
    view.mode = Some(version.mode.clone());
    view.workflow_version = Some(version.workflow_version.clone());
    view.workflow_sha256 = Some(version.workflow_sha256.clone());
    view.recipe_sha256 = version
        .recipes
        .last()
        .map(|recipe| recipe.recipe_sha256.clone());
    view.active_tasks = version.active_tasks;
    view.total_tasks = version.total_tasks;
    view.has_successful_run = version.has_successful_run;
    view.latest_success_at = version.latest_success_at.clone();
    view.latest_failure_at = version.latest_failure_at.clone();
    if view.recipes.is_empty() {
        view.recipes = fast_recipe_summaries(version);
    }
    if let Some(capability) = capability {
        view.capability = capability_name;
        view.capability_issues = capability.issues.clone();
    }
    let (readiness, reasons) = readiness_for(
        enabled,
        &view.package_status,
        &view.capability,
        &view.diagnostics,
        view.recipes.len(),
        view.has_successful_run,
    );
    view.readiness = readiness;
    view.readiness_reasons = reasons;
    view
}

fn fast_recipe_summaries(version: &RuntimeWorkflowVersionRecord) -> Vec<WorkflowRecipeSummaryView> {
    version
        .recipes
        .iter()
        .map(|recipe| WorkflowRecipeSummaryView {
            recipe_id: recipe.recipe_id.clone(),
            version: recipe.version.clone(),
            input_count: 0,
            output_count: 0,
            preset_count: None,
        })
        .collect()
}

fn capability_state_enum(value: &str) -> CapabilityState {
    match value {
        "READY" => CapabilityState::Ready,
        "MISSING_NODES" => CapabilityState::MissingNodes,
        "INCOMPATIBLE_INPUT_VALUES" => CapabilityState::IncompatibleInputValues,
        "COMFY_OFFLINE" => CapabilityState::ComfyOffline,
        _ => CapabilityState::NotChecked,
    }
}

fn readiness_for(
    enabled: bool,
    package_status: &str,
    capability: &str,
    diagnostics: &[WorkflowDiagnosticView],
    recipe_count: usize,
    has_successful_run: bool,
) -> (String, Vec<String>) {
    let mut reasons = Vec::new();
    if !enabled {
        reasons.push("运行包已停用。".to_owned());
    }
    if package_status != "VALID" {
        reasons.push("运行包校验未通过。".to_owned());
    }
    if recipe_count == 0 {
        reasons.push("没有可用配方。".to_owned());
    }
    match capability {
        "MISSING_NODES" => reasons.push("ComfyUI 缺少工作流节点。".to_owned()),
        "INCOMPATIBLE_INPUT_VALUES" => {
            reasons.push("工作流输入与当前 ComfyUI 能力不兼容。".to_owned())
        }
        "COMFY_OFFLINE" => reasons.push("ComfyUI 当前离线。".to_owned()),
        "NOT_CHECKED" => reasons.push("尚未完成当前运行环境检查。".to_owned()),
        _ => {}
    }
    if !diagnostics.is_empty() {
        reasons.push(format!("存在 {} 条运行包诊断。", diagnostics.len()));
    }
    if !has_successful_run && reasons.is_empty() {
        reasons.push("尚未完成真实生成验证。".to_owned());
    }
    let readiness = if !reasons.is_empty()
        && (reasons.iter().any(|reason| {
            reason.contains("停用")
                || reason.contains("校验")
                || reason.contains("缺少")
                || reason.contains("不兼容")
                || reason.contains("离线")
                || reason.contains("没有可用")
                || reason.contains("诊断")
        })) {
        "BLOCKED"
    } else if !has_successful_run || capability == "NOT_CHECKED" {
        "DEGRADED"
    } else {
        "READY"
    };
    (readiness.to_owned(), reasons)
}

fn recipe_summaries(version: &RuntimeWorkflowVersionRecord) -> Vec<WorkflowRecipeSummaryView> {
    version
        .recipes
        .iter()
        .map(|recipe| {
            let (input_count, output_count) = RecipeParser::parse(&recipe.recipe_yaml)
                .map(|parsed| (parsed.inputs.len(), parsed.outputs.len()))
                .unwrap_or((0, 0));
            WorkflowRecipeSummaryView {
                recipe_id: recipe.recipe_id.clone(),
                version: recipe.version.clone(),
                input_count,
                output_count,
                preset_count: None,
            }
        })
        .collect()
}

fn parse_manifest_bytes(bytes: &[u8]) -> Result<WorkflowManifest, WorkflowLifecycleError> {
    let text =
        String::from_utf8(bytes.to_vec()).map_err(|error| invalid_error(error.to_string()))?;
    WorkflowManifest::parse(&text)
        .map_err(|error| invalid_error(error))?
        .validate()
        .map_err(|error| invalid_error(error))?;
    WorkflowManifest::parse(&text).map_err(|error| invalid_error(error))
}

fn parse_workflow(value: &str) -> Result<WorkflowDocument, WorkflowLifecycleError> {
    let parsed: Value =
        serde_json::from_str(value).map_err(|error| invalid_error(error.to_string()))?;
    WorkflowDocument::parse(parsed).map_err(|error| invalid_error(error.to_string()))
}

fn validate_exact_runtime_package(
    package: &WorkflowPackageBytes,
    version: &RuntimeWorkflowVersionRecord,
    recipe: &RuntimeRecipeRecord,
    artifact: &WorkflowRuntimeArtifactRecord,
) -> Result<(String, String, String), WorkflowLifecycleError> {
    if artifact.workflow_sha256 != version.workflow_sha256
        || artifact.recipe_sha256 != recipe.recipe_sha256
    {
        return Err(WorkflowLifecycleError::new(
            "RUNTIME_ARTIFACT_HASH_MISMATCH",
            "runtime artifact hashes do not match the registered workflow version and recipe",
        ));
    }

    read_back_and_validate_package(package)
        .map_err(|error| WorkflowLifecycleError::new(error.code(), error.to_string()))?;

    let manifest_yaml = String::from_utf8(package.manifest_yaml.clone()).map_err(|error| {
        WorkflowLifecycleError::new("WORKFLOW_PACKAGE_INVALID", error.to_string())
    })?;
    let manifest = WorkflowManifest::parse(&manifest_yaml)
        .map_err(|error| WorkflowLifecycleError::new("WORKFLOW_PACKAGE_INVALID", error))?;
    if manifest.id != version.workflow_id
        || manifest.workflow_version != version.workflow_version
        || manifest.recipe_version != recipe.version
    {
        return Err(WorkflowLifecycleError::new(
            "WORKFLOW_PACKAGE_INVALID",
            "runtime manifest does not match the exact workflow version and recipe",
        ));
    }

    if sha256(&package.workflow_api_json) != version.workflow_sha256
        || sha256(&package.workflow_api_json) != artifact.workflow_sha256
    {
        return Err(WorkflowLifecycleError::new(
            "WORKFLOW_RUNTIME_HASH_MISMATCH",
            "runtime workflow bytes do not match the exact registered artifact hash",
        ));
    }
    if sha256(&package.recipe_yaml) != recipe.recipe_sha256
        || sha256(&package.recipe_yaml) != artifact.recipe_sha256
    {
        return Err(WorkflowLifecycleError::new(
            "RECIPE_RUNTIME_HASH_MISMATCH",
            "runtime recipe bytes do not match the exact registered artifact hash",
        ));
    }

    let recipe_yaml = String::from_utf8(package.recipe_yaml.clone()).map_err(|error| {
        WorkflowLifecycleError::new("WORKFLOW_PACKAGE_INVALID", error.to_string())
    })?;
    let workflow_json = String::from_utf8(package.workflow_api_json.clone()).map_err(|error| {
        WorkflowLifecycleError::new("WORKFLOW_PACKAGE_INVALID", error.to_string())
    })?;
    Ok((manifest_yaml, recipe_yaml, workflow_json))
}

fn build_archive(package: &WorkflowPackageBytes) -> Result<Vec<u8>, WorkflowLifecycleError> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, bytes) in [
        ("manifest.yaml", &package.manifest_yaml),
        ("recipe.yaml", &package.recipe_yaml),
        ("workflow_api.json", &package.workflow_api_json),
    ] {
        writer
            .start_file(name, options)
            .map_err(|error| archive_error(error.to_string()))?;
        writer
            .write_all(bytes)
            .map_err(|error| archive_error(error.to_string()))?;
    }
    writer
        .finish()
        .map_err(|error| archive_error(error.to_string()))
        .map(|cursor| cursor.into_inner())
}

fn parse_archive(bytes: &[u8]) -> Result<WorkflowPackageBytes, WorkflowLifecycleError> {
    if bytes.len() > MAX_WORKFLOW_ARCHIVE_BYTES {
        return Err(WorkflowLifecycleError::new(
            "PACKAGE_ARCHIVE_TOO_LARGE",
            "archive exceeds the 64 MiB compressed limit",
        ));
    }
    let mut archive =
        ZipArchive::new(Cursor::new(bytes)).map_err(|error| archive_error(error.to_string()))?;
    if archive.len() > MAX_WORKFLOW_ARCHIVE_FILES {
        return Err(WorkflowLifecycleError::new(
            "PACKAGE_ARCHIVE_TOO_MANY_FILES",
            "archive contains too many files",
        ));
    }
    let allowed = [
        "manifest.yaml",
        "recipe.yaml",
        "workflow_api.json",
        "package_info.json",
    ];
    let mut entries = BTreeMap::new();
    let mut compressed_total = 0u64;
    let mut uncompressed_total = 0u64;
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|error| archive_error(error.to_string()))?;
        let name = file.name().to_owned();
        if !allowed.contains(&name.as_str())
            || file.is_dir()
            || file.enclosed_name().is_none()
            || file.enclosed_name().unwrap().components().count() != 1
        {
            return Err(WorkflowLifecycleError::new(
                "PACKAGE_ARCHIVE_UNEXPECTED_ENTRY",
                "archive contains an unexpected or unsafe entry",
            ));
        }
        if file
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(WorkflowLifecycleError::new(
                "PACKAGE_ARCHIVE_UNEXPECTED_ENTRY",
                "archive symlinks are not allowed",
            ));
        }
        if entries.contains_key(&name) {
            return Err(WorkflowLifecycleError::new(
                "PACKAGE_ARCHIVE_UNEXPECTED_ENTRY",
                "archive contains duplicate entries",
            ));
        }
        compressed_total = compressed_total.saturating_add(file.compressed_size());
        uncompressed_total = uncompressed_total.saturating_add(file.size());
        if compressed_total > MAX_WORKFLOW_ARCHIVE_BYTES as u64
            || uncompressed_total > MAX_WORKFLOW_ARCHIVE_UNCOMPRESSED_BYTES
        {
            return Err(WorkflowLifecycleError::new(
                "PACKAGE_ARCHIVE_TOO_LARGE",
                "archive exceeds the 64 MiB size limit",
            ));
        }
        let mut content = Vec::new();
        file.read_to_end(&mut content)
            .map_err(|error| archive_error(error.to_string()))?;
        entries.insert(name, content);
    }
    let manifest = entries.remove("manifest.yaml").ok_or_else(|| {
        WorkflowLifecycleError::new("PACKAGE_ARCHIVE_MISSING_ENTRY", "manifest.yaml is required")
    })?;
    let recipe = entries.remove("recipe.yaml").ok_or_else(|| {
        WorkflowLifecycleError::new("PACKAGE_ARCHIVE_MISSING_ENTRY", "recipe.yaml is required")
    })?;
    let workflow = entries.remove("workflow_api.json").ok_or_else(|| {
        WorkflowLifecycleError::new(
            "PACKAGE_ARCHIVE_MISSING_ENTRY",
            "workflow_api.json is required",
        )
    })?;
    Ok(WorkflowPackageBytes::new(manifest, recipe, workflow))
}

fn diff_workflow(
    left: &WorkflowDocument,
    right: &WorkflowDocument,
) -> (
    Vec<String>,
    Vec<String>,
    Vec<WorkflowChangedClassTypeView>,
    Vec<WorkflowValueChangeView>,
    Vec<WorkflowLinkChangeView>,
) {
    let left_nodes = left.value().as_object().cloned().unwrap_or_default();
    let right_nodes = right.value().as_object().cloned().unwrap_or_default();
    let left_ids = left_nodes.keys().cloned().collect::<BTreeSet<_>>();
    let right_ids = right_nodes.keys().cloned().collect::<BTreeSet<_>>();
    let added_nodes = right_ids.difference(&left_ids).cloned().collect();
    let removed_nodes = left_ids.difference(&right_ids).cloned().collect();
    let mut classes = Vec::new();
    let mut literals = Vec::new();
    let mut links = Vec::new();
    for node_id in left_ids.intersection(&right_ids) {
        let left_node = left_nodes.get(node_id).and_then(Value::as_object);
        let right_node = right_nodes.get(node_id).and_then(Value::as_object);
        let left_class = left_node
            .and_then(|node| node.get("class_type"))
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let right_class = right_node
            .and_then(|node| node.get("class_type"))
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if left_class != right_class {
            classes.push(WorkflowChangedClassTypeView {
                node_id: node_id.clone(),
                from: truncate(left_class),
                to: truncate(right_class),
            });
        }
        let left_inputs = left_node
            .and_then(|node| node.get("inputs"))
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let right_inputs = right_node
            .and_then(|node| node.get("inputs"))
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let names = left_inputs
            .keys()
            .chain(right_inputs.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        for input in names {
            let left_value = left_inputs.get(&input);
            let right_value = right_inputs.get(&input);
            if left_value == right_value {
                continue;
            }
            if is_link_value(left_value) || is_link_value(right_value) {
                links.push(WorkflowLinkChangeView {
                    node_id: node_id.clone(),
                    input,
                    from: value_summary(left_value),
                    to: value_summary(right_value),
                });
            } else {
                literals.push(WorkflowValueChangeView {
                    node_id: node_id.clone(),
                    input,
                    from: value_summary(left_value),
                    to: value_summary(right_value),
                });
            }
        }
    }
    (added_nodes, removed_nodes, classes, literals, links)
}

fn diff_recipe(left: &Recipe, right: &Recipe) -> (Vec<String>, Vec<String>, Vec<String>) {
    let left_inputs = left.inputs.keys().cloned().collect::<BTreeSet<_>>();
    let right_inputs = right.inputs.keys().cloned().collect::<BTreeSet<_>>();
    let mut input_changes = Vec::new();
    for key in left_inputs.difference(&right_inputs) {
        input_changes.push(format!("removed input {key}"));
    }
    for key in right_inputs.difference(&left_inputs) {
        input_changes.push(format!("added input {key}"));
    }
    for key in left_inputs.intersection(&right_inputs) {
        let a = left.inputs.get(key).unwrap();
        let b = right.inputs.get(key).unwrap();
        if a != b {
            input_changes.push(format!("changed input {key}: {} → {}", a.kind(), b.kind()));
        }
    }
    let left_bindings = left
        .bindings
        .iter()
        .map(|binding| {
            format!(
                "{}:{:?}->{}:{}",
                binding.source, binding.item_index, binding.target.node, binding.target.input
            )
        })
        .collect::<BTreeSet<_>>();
    let right_bindings = right
        .bindings
        .iter()
        .map(|binding| {
            format!(
                "{}:{:?}->{}:{}",
                binding.source, binding.item_index, binding.target.node, binding.target.input
            )
        })
        .collect::<BTreeSet<_>>();
    let binding_changes = left_bindings
        .symmetric_difference(&right_bindings)
        .cloned()
        .collect();
    let left_outputs = left
        .outputs
        .iter()
        .map(|output| format!("{}:{}:{}", output.id, output.node, output.required))
        .collect::<BTreeSet<_>>();
    let right_outputs = right
        .outputs
        .iter()
        .map(|output| format!("{}:{}:{}", output.id, output.node, output.required))
        .collect::<BTreeSet<_>>();
    let output_changes = left_outputs
        .symmetric_difference(&right_outputs)
        .cloned()
        .collect();
    (input_changes, binding_changes, output_changes)
}

fn is_link_value(value: Option<&Value>) -> bool {
    value.and_then(Value::as_array).is_some_and(|values| {
        values.len() == 2 && values[0].as_str().is_some() && values[1].as_u64().is_some()
    })
}

fn value_summary(value: Option<&Value>) -> String {
    let Some(value) = value else {
        return "<missing>".to_owned();
    };
    let text = match value {
        Value::String(value) => value.rsplit(['/', '\\']).next().unwrap_or(value).to_owned(),
        Value::Array(values) if is_link_value(Some(value)) => "Node connection".to_owned(),
        _ => value.to_string(),
    };
    truncate(&text)
}

fn truncate(value: &str) -> String {
    value.chars().take(120).collect()
}

fn capability_state(capability: &CapabilityCheckView) -> String {
    serde_json::to_value(capability)
        .ok()
        .and_then(|value| {
            value
                .get("state")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "NOT_CHECKED".to_owned())
}

fn capability_for_restore_error(
    error: &WorkflowLifecycleError,
    checked_at: chrono::DateTime<chrono::Utc>,
) -> CapabilityCheckView {
    let state = match error.code() {
        "COMFY_OFFLINE" => CapabilityState::ComfyOffline,
        "MISSING_NODES" | "MISSING_NODE" => CapabilityState::MissingNodes,
        "INCOMPATIBLE_INPUT_VALUES" | "COMFY_PROTOCOL_ERROR" => {
            CapabilityState::IncompatibleInputValues
        }
        _ => CapabilityState::NotChecked,
    };
    CapabilityCheckView {
        state,
        checked_at: Some(checked_at.to_rfc3339()),
        issues: vec![CapabilityIssueView {
            code: error.code().to_owned(),
            class_type: None,
            node_id: None,
            affected_node_ids: Vec::new(),
            input_name: None,
            current_value: None,
            message: error.to_string(),
        }],
    }
}

fn restore_readiness(capability: &str, enabled: bool) -> String {
    if enabled && capability == "READY" {
        "ACTIVE".to_owned()
    } else {
        "RESTORED_NEEDS_ATTENTION".to_owned()
    }
}

fn compare_versions(left: &str, right: &str) -> std::cmp::Ordering {
    let parse = |value: &str| {
        value
            .split('.')
            .map(|part| part.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    parse(left).cmp(&parse(right))
}

fn safe_identifier(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_owned()
}

fn safe_file_stem(value: &str) -> String {
    let stem = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if stem.is_empty() {
        "workflow".to_owned()
    } else {
        stem
    }
}

fn sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn deletion_action(
    builtin: bool,
    archived: bool,
    has_active_work: bool,
    has_history: bool,
) -> &'static str {
    if archived || has_active_work {
        "BLOCKED"
    } else if builtin || has_history {
        "REMOVE"
    } else {
        "HARD_DELETE"
    }
}

fn db_error(error: crate::application::ports::RepositoryError) -> WorkflowLifecycleError {
    WorkflowLifecycleError::new("DATABASE_ERROR", error.to_string())
}

fn store_error(error: impl fmt::Display) -> WorkflowLifecycleError {
    WorkflowLifecycleError::new("WORKFLOW_PACKAGE_STORE_ERROR", error.to_string())
}

fn invalid_error(message: impl Into<String>) -> WorkflowLifecycleError {
    WorkflowLifecycleError::new("WORKFLOW_PACKAGE_INVALID", message)
}

fn archive_error(message: impl Into<String>) -> WorkflowLifecycleError {
    WorkflowLifecycleError::new("PACKAGE_ARCHIVE_INVALID", message)
}

#[cfg(test)]
mod tests {
    use super::{
        build_archive, deletion_action, diff_workflow, fast_view_for_version, parse_archive,
        readiness_for, sha256, WorkflowDiagnosticView, WorkflowLifecycleService,
    };
    use crate::application::{
        ports::{
            Clock, ComfyAdapter, ComfyAdapterError, ComfyEventSubscription, ComfyHealth,
            ComfyHistory, ComfyOutputData, ComfyOutputFile, ProjectWorkflowBindingRecord,
            ProjectWorkflowBindingRepository, PromptSubmission, RepositoryError,
            RuntimeRecipeRecord, RuntimeWorkflowVersionRecord, SystemStats,
            WorkflowLibraryRepository, WorkflowLibrarySource, WorkflowLibrarySourceError,
            WorkflowPackageBytes, WorkflowPackageFiles, WorkflowPackageLoad,
            WorkflowPackageQuarantineResult, WorkflowPackageRecord, WorkflowPackageRegistration,
            WorkflowPackageStore, WorkflowPackageStoreError, WorkflowRunRepository,
            WorkflowRuntimeRepository, WorkflowRuntimeState, WorkflowRuntimeStateRepository,
        },
        workflow_library_service::WorkflowLibraryService,
        workflow_onboarding_service::WorkflowOnboardingService,
    };
    use crate::domain::WorkflowDocument;
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use serde_json::{json, Value};
    use std::{
        io::{Cursor, Write},
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
    };
    use zip::{write::FileOptions, ZipWriter};

    const EXACT_WORKFLOW_JSON: &str = r#"{
  "1": {"class_type": "TestNode", "inputs": {"mode": "bad"}}
}"#;
    const EXACT_RECIPE_A_YAML: &str = r#"schema_version: 1
id: rcp_exact_a
name: Exact Recipe A
workflow:
  file: workflow_api.json
inputs:
  mode:
    type: textarea
    label: Mode
    required: true
    default: bad
bindings:
  - source: mode
    target:
      node: "1"
      input: mode
outputs: []
"#;
    const EXACT_RECIPE_B_YAML: &str = r#"schema_version: 1
id: rcp_exact_b
name: Exact Recipe B
workflow:
  file: workflow_api.json
inputs:
  prompt:
    type: textarea
    label: Prompt
    required: false
    default: ""
bindings: []
outputs: []
"#;

    #[derive(Clone)]
    struct ExactRecipeSource {
        packages: Vec<WorkflowPackageFiles>,
        load_calls: Arc<AtomicUsize>,
        empty: Arc<std::sync::atomic::AtomicBool>,
    }

    #[async_trait]
    impl WorkflowLibrarySource for ExactRecipeSource {
        async fn load_packages(
            &self,
        ) -> Result<Vec<WorkflowPackageLoad>, WorkflowLibrarySourceError> {
            self.load_calls.fetch_add(1, Ordering::SeqCst);
            if self.empty.load(Ordering::SeqCst) {
                return Ok(Vec::new());
            }
            Ok(self
                .packages
                .iter()
                .cloned()
                .map(WorkflowPackageLoad::Loaded)
                .collect())
        }
    }

    struct ExactLibraryRepository;

    #[async_trait]
    impl WorkflowLibraryRepository for ExactLibraryRepository {
        async fn register_package(
            &self,
            _package: &WorkflowPackageRecord,
        ) -> Result<WorkflowPackageRegistration, RepositoryError> {
            Ok(WorkflowPackageRegistration::Inserted)
        }
    }

    struct ExactRunRepository;

    #[async_trait]
    impl WorkflowRunRepository for ExactRunRepository {
        async fn has_successful_run(
            &self,
            _workflow_id: &str,
            _workflow_version: &str,
        ) -> Result<bool, RepositoryError> {
            Ok(false)
        }
    }

    struct ExactPackageStore;

    fn package_store_error() -> WorkflowPackageStoreError {
        WorkflowPackageStoreError {
            message: "test package store does not provide this operation".to_owned(),
        }
    }

    #[async_trait]
    impl WorkflowPackageStore for ExactPackageStore {
        async fn stage(
            &self,
            _staging_id: &str,
            _package: &WorkflowPackageBytes,
        ) -> Result<(), WorkflowPackageStoreError> {
            Ok(())
        }

        async fn read_staging(
            &self,
            _staging_id: &str,
        ) -> Result<WorkflowPackageBytes, WorkflowPackageStoreError> {
            Err(package_store_error())
        }

        async fn publish_atomic(
            &self,
            _staging_id: &str,
            _package_name: &str,
        ) -> Result<(), WorkflowPackageStoreError> {
            Ok(())
        }

        async fn remove_staging(&self, _staging_id: &str) -> Result<(), WorkflowPackageStoreError> {
            Ok(())
        }

        async fn remove_published(
            &self,
            _package_name: &str,
        ) -> Result<(), WorkflowPackageStoreError> {
            Ok(())
        }

        async fn quarantine_published(
            &self,
            _operation_id: &str,
            _package_name: &str,
        ) -> Result<WorkflowPackageQuarantineResult, WorkflowPackageStoreError> {
            Err(package_store_error())
        }

        async fn restore_quarantined(
            &self,
            _operation_id: &str,
            _package_name: &str,
        ) -> Result<(), WorkflowPackageStoreError> {
            Err(package_store_error())
        }

        async fn remove_quarantine(
            &self,
            _operation_id: &str,
        ) -> Result<(), WorkflowPackageStoreError> {
            Err(package_store_error())
        }

        async fn list_published(&self) -> Result<Vec<String>, WorkflowPackageStoreError> {
            Ok(Vec::new())
        }

        async fn read_runtime(
            &self,
            _package_name: &str,
        ) -> Result<WorkflowPackageBytes, WorkflowPackageStoreError> {
            Err(package_store_error())
        }

        async fn list_staging_ids(&self) -> Result<Vec<String>, WorkflowPackageStoreError> {
            Ok(Vec::new())
        }
    }

    struct ExactRuntimeRepository {
        version: RuntimeWorkflowVersionRecord,
    }

    #[async_trait]
    impl WorkflowRuntimeRepository for ExactRuntimeRepository {
        async fn list_versions(
            &self,
        ) -> Result<Vec<RuntimeWorkflowVersionRecord>, RepositoryError> {
            Ok(vec![self.version.clone()])
        }

        async fn find_version(
            &self,
            workflow_version_id: &str,
        ) -> Result<Option<RuntimeWorkflowVersionRecord>, RepositoryError> {
            Ok((self.version.workflow_version_id == workflow_version_id)
                .then(|| self.version.clone()))
        }

        async fn inspect_deletion(
            &self,
            _workflow_version_id: &str,
        ) -> Result<Option<crate::application::ports::WorkflowDeletionCounts>, RepositoryError>
        {
            Ok(None)
        }

        async fn delete_version(
            &self,
            _workflow_version_id: &str,
            _workflow_id: &str,
            _updated_at: DateTime<Utc>,
        ) -> Result<(), RepositoryError> {
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct ExactStateRepository {
        state: Arc<Mutex<Option<WorkflowRuntimeState>>>,
    }

    impl ExactStateRepository {
        fn with_state(state: WorkflowRuntimeState) -> Self {
            Self {
                state: Arc::new(Mutex::new(Some(state))),
            }
        }

        fn snapshot(&self) -> Option<WorkflowRuntimeState> {
            self.state.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl WorkflowRuntimeStateRepository for ExactStateRepository {
        async fn is_enabled(&self, workflow_version_id: &str) -> Result<bool, RepositoryError> {
            Ok(self
                .state
                .lock()
                .unwrap()
                .as_ref()
                .filter(|state| state.workflow_version_id == workflow_version_id)
                .map_or(true, |state| state.enabled))
        }

        async fn set_enabled(
            &self,
            workflow_version_id: &str,
            enabled: bool,
            updated_at: DateTime<Utc>,
        ) -> Result<(), RepositoryError> {
            let mut state = self.state.lock().unwrap();
            let current = state.clone().unwrap_or(WorkflowRuntimeState {
                workflow_version_id: workflow_version_id.to_owned(),
                enabled: true,
                archived: false,
                archived_at: None,
                updated_at,
            });
            *state = Some(WorkflowRuntimeState {
                enabled,
                updated_at,
                ..current
            });
            Ok(())
        }

        async fn find_state(
            &self,
            workflow_version_id: &str,
        ) -> Result<Option<WorkflowRuntimeState>, RepositoryError> {
            Ok(self
                .state
                .lock()
                .unwrap()
                .clone()
                .filter(|state| state.workflow_version_id == workflow_version_id))
        }

        async fn set_archived(
            &self,
            workflow_version_id: &str,
            archived: bool,
            enabled: bool,
            archived_at: Option<DateTime<Utc>>,
            updated_at: DateTime<Utc>,
        ) -> Result<(), RepositoryError> {
            *self.state.lock().unwrap() = Some(WorkflowRuntimeState {
                workflow_version_id: workflow_version_id.to_owned(),
                enabled,
                archived,
                archived_at,
                updated_at,
            });
            Ok(())
        }

        async fn list_states(&self) -> Result<Vec<WorkflowRuntimeState>, RepositoryError> {
            Ok(self.state.lock().unwrap().clone().into_iter().collect())
        }
    }

    #[derive(Clone)]
    struct ExactBindingRepository {
        bindings: Arc<Mutex<Vec<ProjectWorkflowBindingRecord>>>,
        fail_clear: bool,
        late_binding: Option<ProjectWorkflowBindingRecord>,
        list_calls: Arc<AtomicUsize>,
    }

    impl ExactBindingRepository {
        fn one(workflow_version_id: &str, fail_clear: bool) -> Self {
            Self {
                bindings: Arc::new(Mutex::new(vec![binding_record(workflow_version_id)])),
                fail_clear,
                late_binding: None,
                list_calls: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn late(workflow_version_id: &str) -> Self {
            Self {
                bindings: Arc::new(Mutex::new(Vec::new())),
                fail_clear: false,
                late_binding: Some(binding_record(workflow_version_id)),
                list_calls: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn len(&self) -> usize {
            self.bindings.lock().unwrap().len()
        }
    }

    fn binding_record(workflow_version_id: &str) -> ProjectWorkflowBindingRecord {
        let now = Utc::now();
        ProjectWorkflowBindingRecord {
            project_id: "project-exact".to_owned(),
            stage: "VIDEO".to_owned(),
            mode: "DEFAULT".to_owned(),
            workflow_version_id: workflow_version_id.to_owned(),
            recipe_id: "recipe-a".to_owned(),
            created_at: now,
            updated_at: now,
        }
    }

    #[async_trait]
    impl ProjectWorkflowBindingRepository for ExactBindingRepository {
        async fn list_for_project(
            &self,
            project_id: &str,
        ) -> Result<Vec<ProjectWorkflowBindingRecord>, RepositoryError> {
            Ok(self
                .bindings
                .lock()
                .unwrap()
                .iter()
                .filter(|binding| binding.project_id == project_id)
                .cloned()
                .collect())
        }

        async fn replace_for_project(
            &self,
            _project_id: &str,
            _bindings: &[ProjectWorkflowBindingRecord],
        ) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn list_for_workflow_version(
            &self,
            workflow_version_id: &str,
        ) -> Result<Vec<ProjectWorkflowBindingRecord>, RepositoryError> {
            if let Some(binding) = &self.late_binding {
                if self.list_calls.fetch_add(1, Ordering::SeqCst) >= 1 {
                    let mut bindings = self.bindings.lock().unwrap();
                    if bindings.is_empty() {
                        bindings.push(binding.clone());
                    }
                }
            }
            Ok(self
                .bindings
                .lock()
                .unwrap()
                .iter()
                .filter(|binding| binding.workflow_version_id == workflow_version_id)
                .cloned()
                .collect())
        }

        async fn clear_by_workflow_version(
            &self,
            workflow_version_id: &str,
        ) -> Result<u64, RepositoryError> {
            if self.fail_clear {
                return Err(RepositoryError::database("forced binding cleanup failure"));
            }
            let mut bindings = self.bindings.lock().unwrap();
            let before = bindings.len();
            bindings.retain(|binding| binding.workflow_version_id != workflow_version_id);
            Ok((before - bindings.len()) as u64)
        }
    }

    struct ExactClock;

    impl Clock for ExactClock {
        fn now(&self) -> DateTime<Utc> {
            Utc::now()
        }
    }

    struct ExactComfyAdapter {
        object_info: Value,
    }

    #[async_trait]
    impl ComfyAdapter for ExactComfyAdapter {
        async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible(
                "not used by exact inspection test".to_owned(),
            ))
        }

        async fn get_system_stats(&self) -> Result<SystemStats, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible(
                "not used by exact inspection test".to_owned(),
            ))
        }

        async fn get_object_info(&self) -> Result<Value, ComfyAdapterError> {
            Ok(self.object_info.clone())
        }

        async fn get_history(&self, _prompt_id: &str) -> Result<ComfyHistory, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible(
                "not used by exact inspection test".to_owned(),
            ))
        }

        async fn download_output(
            &self,
            _file: &ComfyOutputFile,
        ) -> Result<ComfyOutputData, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible(
                "not used by exact inspection test".to_owned(),
            ))
        }

        async fn submit_workflow(
            &self,
            _client_id: &str,
            _prompt_id: &str,
            _workflow: Value,
        ) -> Result<PromptSubmission, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible(
                "not used by exact inspection test".to_owned(),
            ))
        }

        async fn subscribe_events(
            &self,
            _client_id: &str,
        ) -> Result<Box<dyn ComfyEventSubscription>, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible(
                "not used by exact inspection test".to_owned(),
            ))
        }
    }

    fn exact_package(
        package_name: &str,
        recipe_version: &str,
        recipe_yaml: &str,
    ) -> WorkflowPackageFiles {
        WorkflowPackageFiles {
            package_name: package_name.to_owned(),
            package_source_path: None,
            manifest_yaml: format!(
                "schema_version: 1\nid: wfl_exact_shared\nname: Exact Shared\nworkflow_version: 1.0.0\nrecipe_version: {recipe_version}\ncategory: image\nmode: text_to_image\n"
            ),
            recipe_yaml: recipe_yaml.to_owned(),
            workflow_json: EXACT_WORKFLOW_JSON.to_owned(),
        }
    }

    fn exact_service() -> (WorkflowLifecycleService, Arc<ExactRecipeSource>) {
        exact_service_with_recipe_hash_mismatch(false)
    }

    fn exact_service_with_recipe_hash_mismatch(
        recipe_hash_mismatch: bool,
    ) -> (WorkflowLifecycleService, Arc<ExactRecipeSource>) {
        let (service, source, _) = exact_service_with_options(
            recipe_hash_mismatch,
            None,
            json!({
                "TestNode": {"input": {"required": {"mode": [["good"]]}}}
            }),
            None,
        );
        (service, source)
    }

    fn exact_service_with_options(
        recipe_hash_mismatch: bool,
        initial_state: Option<WorkflowRuntimeState>,
        object_info: Value,
        binding_repository: Option<Arc<dyn ProjectWorkflowBindingRepository>>,
    ) -> (
        WorkflowLifecycleService,
        Arc<ExactRecipeSource>,
        Arc<ExactStateRepository>,
    ) {
        let mut package_a = exact_package("pkg-exact-a", "1.0.0", EXACT_RECIPE_A_YAML);
        if recipe_hash_mismatch {
            package_a
                .recipe_yaml
                .push_str("\n# registered hash intentionally differs\n");
        }
        let package_b = exact_package("pkg-exact-b", "2.0.0", EXACT_RECIPE_B_YAML);
        let source = Arc::new(ExactRecipeSource {
            packages: vec![package_a, package_b],
            load_calls: Arc::new(AtomicUsize::new(0)),
            empty: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        });
        let version = RuntimeWorkflowVersionRecord {
            workflow_version_id: "wv-exact-shared".to_owned(),
            workflow_id: "wfl_exact_shared".to_owned(),
            name: "Exact Shared".to_owned(),
            category: "image".to_owned(),
            mode: "text_to_image".to_owned(),
            workflow_version: "1.0.0".to_owned(),
            workflow_sha256: sha256(EXACT_WORKFLOW_JSON.as_bytes()),
            api_workflow_json: EXACT_WORKFLOW_JSON.to_owned(),
            package_name: Some("pkg-exact-a".to_owned()),
            is_current: true,
            recipes: vec![
                RuntimeRecipeRecord {
                    recipe_id: "recipe-a".to_owned(),
                    version: "1.0.0".to_owned(),
                    schema_version: 1,
                    recipe_yaml: EXACT_RECIPE_A_YAML.to_owned(),
                    recipe_sha256: sha256(EXACT_RECIPE_A_YAML.as_bytes()),
                },
                RuntimeRecipeRecord {
                    recipe_id: "recipe-b".to_owned(),
                    version: "2.0.0".to_owned(),
                    schema_version: 1,
                    recipe_yaml: EXACT_RECIPE_B_YAML.to_owned(),
                    recipe_sha256: sha256(EXACT_RECIPE_B_YAML.as_bytes()),
                },
            ],
            active_tasks: 0,
            total_tasks: 0,
            has_successful_run: false,
            latest_success_at: None,
            latest_failure_at: None,
        };
        let runtime: Arc<dyn WorkflowRuntimeRepository> =
            Arc::new(ExactRuntimeRepository { version });
        let state_repository = Arc::new(
            initial_state
                .map(ExactStateRepository::with_state)
                .unwrap_or_default(),
        );
        let state: Arc<dyn WorkflowRuntimeStateRepository> = state_repository.clone();
        let clock: Arc<dyn Clock> = Arc::new(ExactClock);
        let library = Arc::new(WorkflowLibraryService::new(
            source.clone(),
            Arc::new(ExactLibraryRepository),
            clock.clone(),
        ));
        let package_store: Arc<dyn WorkflowPackageStore> = Arc::new(ExactPackageStore);
        let onboarding = Arc::new(
            WorkflowOnboardingService::new(
                source.clone(),
                Arc::new(ExactComfyAdapter { object_info }),
                library.clone(),
                Arc::new(ExactRunRepository),
                package_store.clone(),
                clock.clone(),
            )
            .with_runtime_state(runtime.clone(), state.clone()),
        );
        let service = WorkflowLifecycleService::new(
            source.clone(),
            library,
            onboarding,
            runtime,
            state,
            package_store,
            clock,
        );
        let service = match binding_repository {
            Some(repository) => service.with_project_workflow_binding_repository(repository),
            None => service,
        };
        (service, source, state_repository)
    }

    #[tokio::test]
    async fn exact_recipe_inspection_is_recipe_scoped_and_does_not_pollute_workspace_cache() {
        let (service, source) = exact_service();

        let recipe_a = service
            .inspect_recipe_runtime("wv-exact-shared", "recipe-a")
            .await
            .unwrap();
        assert_eq!(recipe_a.recipe_id, "recipe-a");
        assert_eq!(recipe_a.recipe_version, "1.0.0");
        assert_eq!(recipe_a.package_name, "pkg-exact-a");
        assert_eq!(recipe_a.package_status, "VALID");
        assert_eq!(recipe_a.capability, "READY");

        let recipe_b = service
            .inspect_recipe_runtime("wv-exact-shared", "recipe-b")
            .await
            .unwrap();
        assert_eq!(recipe_b.recipe_id, "recipe-b");
        assert_eq!(recipe_b.recipe_version, "2.0.0");
        assert_eq!(recipe_b.package_name, "pkg-exact-b");
        assert_eq!(recipe_b.capability, "INCOMPATIBLE_INPUT_VALUES");

        let workspace = service.list_workspace().await.unwrap();
        assert_eq!(workspace.items[0].capability, "NOT_CHECKED");
        assert_eq!(source.load_calls.load(Ordering::SeqCst), 2);

        let recipe_a_again = service
            .inspect_recipe_runtime("wv-exact-shared", "recipe-a")
            .await
            .unwrap();
        assert_eq!(recipe_a_again.capability, "READY");
        assert_eq!(source.load_calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn exact_recipe_hash_diagnostics_return_invalid_before_capability() {
        let (service, _source) = exact_service_with_recipe_hash_mismatch(true);

        let inspection = service
            .inspect_recipe_runtime("wv-exact-shared", "recipe-a")
            .await
            .unwrap();

        assert_eq!(inspection.package_status, "INVALID");
        assert_eq!(inspection.capability, "NOT_CHECKED");
        assert!(inspection
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "RECIPE_RUNTIME_HASH_MISMATCH"));
    }

    #[tokio::test]
    async fn restoring_a_ready_workflow_reenables_it_and_returns_the_result() {
        let now = Utc::now();
        let (service, _source, state) = exact_service_with_options(
            false,
            Some(WorkflowRuntimeState {
                workflow_version_id: "wv-exact-shared".to_owned(),
                enabled: false,
                archived: true,
                archived_at: Some(now),
                updated_at: now,
            }),
            json!({
                "TestNode": {"input": {"required": {"mode": [["bad"]]}}}
            }),
            None,
        );

        let result = service.restore_version("wv-exact-shared").await.unwrap();

        assert_eq!(result.workflow_version_id, "wv-exact-shared");
        assert!(!result.archived);
        assert!(result.enabled);
        assert_eq!(result.capability, "READY");
        assert_eq!(result.readiness, "ACTIVE");
        let state = state.snapshot().unwrap();
        assert!(!state.archived);
        assert!(state.enabled);
        assert!(service
            .capability_cache
            .read()
            .await
            .contains_key("wv-exact-shared"));
    }

    #[tokio::test]
    async fn restoring_a_blocked_workflow_keeps_it_disabled_with_explicit_readiness() {
        let now = Utc::now();
        let (service, _source, state) = exact_service_with_options(
            false,
            Some(WorkflowRuntimeState {
                workflow_version_id: "wv-exact-shared".to_owned(),
                enabled: false,
                archived: true,
                archived_at: Some(now),
                updated_at: now,
            }),
            json!({
                "TestNode": {"input": {"required": {"mode": [["good"]]}}}
            }),
            None,
        );

        let result = service.restore_version("wv-exact-shared").await.unwrap();

        assert!(!result.archived);
        assert!(!result.enabled);
        assert_eq!(result.capability, "INCOMPATIBLE_INPUT_VALUES");
        assert_eq!(result.readiness, "RESTORED_NEEDS_ATTENTION");
        let state = state.snapshot().unwrap();
        assert!(!state.archived);
        assert!(!state.enabled);
    }

    #[tokio::test]
    async fn restore_succeeds_when_capability_recheck_fails() {
        let now = Utc::now();
        let (service, source, state) = exact_service_with_options(
            false,
            Some(WorkflowRuntimeState {
                workflow_version_id: "wv-exact-shared".to_owned(),
                enabled: false,
                archived: true,
                archived_at: Some(now),
                updated_at: now,
            }),
            json!({
                "TestNode": {"input": {"required": {"mode": [["bad"]]}}}
            }),
            None,
        );
        source.empty.store(true, Ordering::SeqCst);

        let result = service.restore_version("wv-exact-shared").await.unwrap();

        assert!(!result.archived);
        assert!(!result.enabled);
        assert_eq!(result.capability, "NOT_CHECKED");
        assert_eq!(result.readiness, "RESTORED_NEEDS_ATTENTION");
        let state = state.snapshot().unwrap();
        assert!(!state.archived);
        assert!(!state.enabled);
    }

    #[tokio::test]
    async fn removing_a_workflow_clears_exact_bindings_and_returns_the_cleared_count() {
        let now = Utc::now();
        let binding = Arc::new(ExactBindingRepository::one("wv-exact-shared", false));
        let (service, _source, state) = exact_service_with_options(
            false,
            Some(WorkflowRuntimeState {
                workflow_version_id: "wv-exact-shared".to_owned(),
                enabled: true,
                archived: false,
                archived_at: None,
                updated_at: now,
            }),
            json!({
                "TestNode": {"input": {"required": {"mode": [["good"]]}}}
            }),
            Some(binding.clone()),
        );

        let result = service.delete_version("wv-exact-shared").await.unwrap();

        assert_eq!(result.action, "REMOVE");
        assert_eq!(result.project_binding_count, 1);
        assert!(result.archived);
        let state = state.snapshot().unwrap();
        assert!(state.archived);
        assert!(!state.enabled);
        assert_eq!(binding.len(), 0);
    }

    #[tokio::test]
    async fn binding_cleanup_failure_compensates_the_previous_runtime_state() {
        let now = Utc::now();
        let binding = Arc::new(ExactBindingRepository::one("wv-exact-shared", true));
        let (service, _source, state) = exact_service_with_options(
            false,
            Some(WorkflowRuntimeState {
                workflow_version_id: "wv-exact-shared".to_owned(),
                enabled: true,
                archived: false,
                archived_at: None,
                updated_at: now,
            }),
            json!({
                "TestNode": {"input": {"required": {"mode": [["good"]]}}}
            }),
            Some(binding.clone()),
        );

        let error = service
            .delete_version("wv-exact-shared")
            .await
            .expect_err("binding cleanup failure must fail closed");

        assert_eq!(error.code(), "WORKFLOW_DELETE_BINDING_CLEANUP_FAILED");
        let state = state.snapshot().unwrap();
        assert!(!state.archived);
        assert!(state.enabled);
        assert_eq!(binding.len(), 1);
    }

    #[tokio::test]
    async fn late_binding_downgrades_hard_delete_to_remove() {
        let now = Utc::now();
        let binding = Arc::new(ExactBindingRepository::late("wv-exact-shared"));
        let (service, _source, state) = exact_service_with_options(
            false,
            Some(WorkflowRuntimeState {
                workflow_version_id: "wv-exact-shared".to_owned(),
                enabled: true,
                archived: false,
                archived_at: None,
                updated_at: now,
            }),
            json!({
                "TestNode": {"input": {"required": {"mode": [["good"]]}}}
            }),
            Some(binding.clone()),
        );

        let result = service.delete_version("wv-exact-shared").await.unwrap();

        assert_eq!(result.action, "REMOVE");
        assert_eq!(result.project_binding_count, 1);
        assert!(state.snapshot().unwrap().archived);
        assert_eq!(binding.len(), 0);
    }

    #[test]
    fn archive_round_trip_has_only_safe_runtime_files() {
        let package =
            WorkflowPackageBytes::new(b"manifest".to_vec(), b"recipe".to_vec(), b"{}".to_vec());
        let archive = build_archive(&package).unwrap();
        assert_eq!(parse_archive(&archive).unwrap(), package);
    }

    #[test]
    fn deletion_policy_removes_products_and_historical_user_workflows_without_hard_delete() {
        assert_eq!(deletion_action(true, false, false, false), "REMOVE");
        assert_eq!(deletion_action(false, false, false, true), "REMOVE");
        assert_eq!(deletion_action(false, false, false, false), "HARD_DELETE");
        assert_eq!(deletion_action(true, false, true, false), "BLOCKED");
        assert_eq!(deletion_action(false, true, false, false), "BLOCKED");
    }

    #[test]
    fn version_diff_reports_nodes_literals_classes_and_links() {
        let left = WorkflowDocument::parse(json!({
            "1": {"class_type":"A","inputs":{"value":1,"link":["2",0]}},
            "2": {"class_type":"B","inputs":{}}
        }))
        .unwrap();
        let right = WorkflowDocument::parse(json!({
            "1": {"class_type":"C","inputs":{"value":2,"link":["3",0]}},
            "3": {"class_type":"D","inputs":{}}
        }))
        .unwrap();
        let (added, removed, classes, literals, links) = diff_workflow(&left, &right);
        assert_eq!(added, vec!["3"]);
        assert_eq!(removed, vec!["2"]);
        assert_eq!(classes.len(), 1);
        assert_eq!(literals.len(), 1);
        assert_eq!(links.len(), 1);
    }

    #[test]
    fn unsafe_archive_entry_is_rejected() {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("../outside", FileOptions::default())
            .unwrap();
        writer.write_all(b"escape").unwrap();
        let archive = writer.finish().unwrap().into_inner();
        let error = parse_archive(&archive).unwrap_err();
        assert_eq!(error.code(), "PACKAGE_ARCHIVE_UNEXPECTED_ENTRY");
    }

    #[test]
    fn readiness_distinguishes_live_evidence_from_blocking_diagnostics() {
        let (ready, reasons) = readiness_for(true, "VALID", "READY", &[], 1, true);
        assert_eq!(ready, "READY");
        assert!(reasons.is_empty());

        let (degraded, reasons) = readiness_for(true, "VALID", "NOT_CHECKED", &[], 1, false);
        assert_eq!(degraded, "DEGRADED");
        assert!(!reasons.is_empty());

        let diagnostics = vec![WorkflowDiagnosticView {
            code: "WORKFLOW_RUNTIME_HASH_MISMATCH".to_owned(),
            message: "hash mismatch".to_owned(),
        }];
        let (blocked, _) = readiness_for(true, "INVALID", "READY", &diagnostics, 1, true);
        assert_eq!(blocked, "BLOCKED");
    }

    #[test]
    fn fast_workspace_view_uses_registered_metadata_without_package_parsing() {
        let version = RuntimeWorkflowVersionRecord {
            workflow_version_id: "ver_1".to_owned(),
            workflow_id: "wfl_demo".to_owned(),
            name: "Demo".to_owned(),
            category: "video".to_owned(),
            mode: "image_to_video".to_owned(),
            workflow_version: "1.0.0".to_owned(),
            workflow_sha256: "workflow-sha".to_owned(),
            api_workflow_json: "{}".to_owned(),
            package_name: None,
            is_current: true,
            recipes: vec![RuntimeRecipeRecord {
                recipe_id: "recipe_1".to_owned(),
                version: "1.0.0".to_owned(),
                schema_version: 1,
                recipe_yaml: "not parsed on the fast path".to_owned(),
                recipe_sha256: "recipe-sha".to_owned(),
            }],
            active_tasks: 0,
            total_tasks: 3,
            has_successful_run: false,
            latest_success_at: None,
            latest_failure_at: None,
        };

        let view = fast_view_for_version(&version, true, false, None, None, None);

        assert_eq!(view.workflow_version_id.as_deref(), Some("ver_1"));
        assert_eq!(view.workflow_sha256.as_deref(), Some("workflow-sha"));
        assert_eq!(view.recipes.len(), 1);
        assert_eq!(view.recipes[0].recipe_id, "recipe_1");
        assert_eq!(view.total_tasks, 3);
        assert_eq!(view.capability, "NOT_CHECKED");
    }
}
