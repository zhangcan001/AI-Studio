use crate::application::{
    ports::{
        Clock, RuntimeWorkflowVersionRecord, WorkflowLibrarySource, WorkflowPackageBytes,
        WorkflowPackageLoad, WorkflowPackageStore, WorkflowRuntimeRepository,
        WorkflowRuntimeStateRepository,
    },
    workflow_library_service::WorkflowLibraryService,
    workflow_manifest::WorkflowManifest,
    workflow_onboarding_service::{
        read_back_and_validate_package, CapabilityCheckView, CapabilityState,
        WorkflowOnboardingDraftView, WorkflowOnboardingService,
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
            clock,
            capability_cache: Arc::new(RwLock::new(HashMap::new())),
            workspace_cache: Arc::new(RwLock::new(HashMap::new())),
        }
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
        let enabled_by_version = states
            .into_iter()
            .map(|state| (state.workflow_version_id, state.enabled))
            .collect::<HashMap<_, _>>();
        let cached_views = self.workspace_cache.read().await.clone();
        let capabilities = self.capability_cache.read().await.clone();
        let mut items = runtime_versions
            .iter()
            .map(|version| {
                let enabled = enabled_by_version
                    .get(&version.workflow_version_id)
                    .copied()
                    .unwrap_or(true);
                fast_view_for_version(
                    version,
                    enabled,
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
            let enabled = self
                .state_repository
                .is_enabled(&version.workflow_version_id)
                .await
                .map_err(db_error)?;
            items.push(WorkflowProductionWorkspaceView {
                package_name: String::new(),
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
        let package_loads = self.load_package_loads().await?;
        let runtime_versions = self
            .runtime_repository
            .list_versions()
            .await
            .map_err(db_error)?;
        let mut packages = HashMap::new();
        for package in package_loads {
            let WorkflowPackageLoad::Loaded(files) = package else {
                continue;
            };
            let Ok(manifest) = WorkflowManifest::parse(&files.manifest_yaml) else {
                continue;
            };
            packages.insert(
                (manifest.id, manifest.workflow_version),
                files.workflow_json,
            );
        }
        let workflows = runtime_versions
            .iter()
            .filter_map(|version| {
                packages
                    .get(&(
                        version.workflow_id.clone(),
                        version.workflow_version.clone(),
                    ))
                    .map(|workflow_json| {
                        (version.workflow_version_id.clone(), workflow_json.clone())
                    })
            })
            .collect::<Vec<_>>();
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
        let package = self
            .find_package(&version.workflow_id, &version.workflow_version, None)
            .await?;
        let capability = self
            .onboarding_service
            .check_runtime_workflow(&package.workflow_json)
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
        let package = self
            .find_package(&version.workflow_id, &version.workflow_version, None)
            .await?;
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
                    return Ok(WorkflowRestoreView {
                        status: "ALREADY_INSTALLED".to_owned(),
                        package_name: String::new(),
                        workflow_id: manifest.id,
                        workflow_version: manifest.workflow_version,
                        recipe_id: Some(recipe.recipe_id.clone()),
                        enabled: self
                            .state_repository
                            .is_enabled(&existing.workflow_version_id)
                            .await
                            .map_err(db_error)?,
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
            .check_runtime_workflow(&workflow_json)
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
        let enabled = self
            .state_repository
            .is_enabled(&version.workflow_version_id)
            .await
            .map_err(db_error)?;
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
        let capability = self
            .onboarding_service
            .check_runtime_workflow(&package.workflow_json)
            .await
            .map_err(|error| WorkflowLifecycleError::new(error.code(), error.to_string()))?;
        let workflow_hash = sha256(package.workflow_json.as_bytes());
        let selected_recipe = version
            .recipes
            .iter()
            .find(|recipe| recipe.version == manifest.recipe_version);
        let recipe_hash = selected_recipe.map(|_| sha256(package.recipe_yaml.as_bytes()));
        let mut diagnostics = Vec::new();
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
        let package = self
            .find_package(&version.workflow_id, &version.workflow_version, None)
            .await?;
        Ok((version, package))
    }

    async fn find_package(
        &self,
        workflow_id: &str,
        workflow_version: &str,
        recipe_version: Option<&str>,
    ) -> Result<crate::application::ports::WorkflowPackageFiles, WorkflowLifecycleError> {
        let packages = self.load_packages().await?;
        packages
            .into_iter()
            .filter_map(|package| {
                let manifest = WorkflowManifest::parse(&package.manifest_yaml).ok()?;
                (manifest.id == workflow_id
                    && manifest.workflow_version == workflow_version
                    && recipe_version.is_none_or(|version| manifest.recipe_version == version))
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
        build_archive, diff_workflow, fast_view_for_version, parse_archive, readiness_for,
        WorkflowDiagnosticView,
    };
    use crate::application::ports::{
        RuntimeRecipeRecord, RuntimeWorkflowVersionRecord, WorkflowPackageBytes,
    };
    use crate::domain::WorkflowDocument;
    use serde_json::json;
    use std::io::{Cursor, Write};
    use zip::{write::FileOptions, ZipWriter};

    #[test]
    fn archive_round_trip_has_only_safe_runtime_files() {
        let package =
            WorkflowPackageBytes::new(b"manifest".to_vec(), b"recipe".to_vec(), b"{}".to_vec());
        let archive = build_archive(&package).unwrap();
        assert_eq!(parse_archive(&archive).unwrap(), package);
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

        let view = fast_view_for_version(&version, true, None, None);

        assert_eq!(view.workflow_version_id.as_deref(), Some("ver_1"));
        assert_eq!(view.workflow_sha256.as_deref(), Some("workflow-sha"));
        assert_eq!(view.recipes.len(), 1);
        assert_eq!(view.recipes[0].recipe_id, "recipe_1");
        assert_eq!(view.total_tasks, 3);
        assert_eq!(view.capability, "NOT_CHECKED");
    }
}
