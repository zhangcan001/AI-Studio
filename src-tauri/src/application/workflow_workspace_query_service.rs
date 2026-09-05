//! The single read model for the technical Workflow Workspace.
//!
//! Registry rows describe logical identity and lifecycle only. Runtime rows
//! are always resolved through the exact version/recipe artifact relation;
//! RuntimeWorkflowVersionRecord::package_name is never a fallback here.

use crate::application::{
    ports::{
        Clock, RuntimeRecipeRecord, RuntimeWorkflowVersionRecord, WorkflowPackageStore,
        WorkflowRuntimeArtifactRecord, WorkflowRuntimeArtifactRepository,
        WorkflowRuntimeRepository, WorkflowRuntimeStateRepository,
    },
    workflow_lifecycle_service::{WorkflowDiagnosticView, WorkflowStagingView},
    workflow_manifest::WorkflowManifest,
    workflow_onboarding_service::{
        read_back_and_validate_package, CapabilityCheckView, CapabilityIssueView, CapabilityState,
        WorkflowOnboardingService,
    },
    workflow_registry_service::{
        WorkflowRegistryService, WorkflowRegistryServiceError, WorkflowRegistryVersionView,
        WorkflowRegistryView,
    },
};
use crate::compiler::RecipeParser;
use crate::domain::WorkflowDocument;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
    fmt,
    sync::Arc,
};
use tokio::sync::RwLock;

/// Navigation mode for the unified Workspace query.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkflowWorkspaceQueryMode {
    #[default]
    Fast,
    Refresh,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowWorkspaceQueryError {
    code: String,
    message: String,
}

impl WorkflowWorkspaceQueryError {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn code(&self) -> &str {
        &self.code
    }
}

impl fmt::Display for WorkflowWorkspaceQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl Error for WorkflowWorkspaceQueryError {}

/// Static Registry truth. Capability is intentionally absent: capability is
/// runtime evidence and belongs to WorkflowWorkspaceRuntimeView.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowWorkspaceRegistryView {
    pub workflow_id: String,
    pub name: String,
    pub source_kind: String,
    pub library_state: String,
    pub current_version_id: Option<String>,
    pub current_version: Option<WorkflowRegistryVersionView>,
    pub current_recipe:
        Option<crate::application::workflow_registry_service::WorkflowRegistryRecipeView>,
    pub versions: Vec<WorkflowRegistryVersionView>,
    pub recipes: Vec<crate::application::workflow_registry_service::WorkflowRegistryRecipeView>,
    pub project_usage_count: u64,
    pub history_count: u64,
}

/// Runtime truth for one exact version/recipe pair.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowWorkspaceRuntimeView {
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub name: String,
    pub category: String,
    pub mode: String,
    pub workflow_version: String,
    pub recipe_version: String,
    pub workflow_sha256: String,
    pub recipe_sha256: String,
    pub artifact_id: Option<String>,
    pub artifact_source_kind: Option<String>,
    pub package_name: Option<String>,
    pub package_source_path: Option<String>,
    pub artifact_status: String,
    pub package_status: String,
    pub library_state: String,
    pub enabled: bool,
    pub archived: bool,
    pub archived_at: Option<String>,
    pub capability: String,
    pub capability_issues: Vec<CapabilityIssueView>,
    pub readiness: String,
    pub readiness_reasons: Vec<String>,
    pub diagnostics: Vec<WorkflowDiagnosticView>,
    pub node_count: usize,
    pub live_verified_at: Option<String>,
    pub has_successful_run: bool,
    pub latest_success_at: Option<String>,
    pub latest_failure_at: Option<String>,
    pub active_tasks: u64,
    pub total_tasks: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowWorkspaceView {
    pub registry: WorkflowWorkspaceRegistryView,
    pub runtime: Vec<WorkflowWorkspaceRuntimeView>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowWorkspaceQueryResponse {
    pub items: Vec<WorkflowWorkspaceView>,
    pub staging: Vec<WorkflowStagingView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ExactPair {
    workflow_version_id: String,
    recipe_id: String,
}

#[derive(Clone, Debug)]
struct CachedRuntimeView {
    artifact_id: String,
    package_name: String,
    workflow_sha256: String,
    recipe_sha256: String,
    view: WorkflowWorkspaceRuntimeView,
}

/// Builds a Workspace snapshot from Registry identity plus exact runtime
/// evidence. The cache is local to this read service: a FAST query reuses the
/// last verified runtime without contacting ComfyUI or hashing package bytes.
pub struct WorkflowWorkspaceQueryService {
    registry_service: Arc<WorkflowRegistryService>,
    runtime_repository: Arc<dyn WorkflowRuntimeRepository>,
    state_repository: Arc<dyn WorkflowRuntimeStateRepository>,
    artifact_repository: Arc<dyn WorkflowRuntimeArtifactRepository>,
    package_store: Arc<dyn WorkflowPackageStore>,
    onboarding_service: Arc<WorkflowOnboardingService>,
    clock: Arc<dyn Clock>,
    cache: Arc<RwLock<HashMap<ExactPair, CachedRuntimeView>>>,
}

impl WorkflowWorkspaceQueryService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        registry_service: Arc<WorkflowRegistryService>,
        runtime_repository: Arc<dyn WorkflowRuntimeRepository>,
        state_repository: Arc<dyn WorkflowRuntimeStateRepository>,
        artifact_repository: Arc<dyn WorkflowRuntimeArtifactRepository>,
        package_store: Arc<dyn WorkflowPackageStore>,
        onboarding_service: Arc<WorkflowOnboardingService>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            registry_service,
            runtime_repository,
            state_repository,
            artifact_repository,
            package_store,
            onboarding_service,
            clock,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn fast(
        &self,
    ) -> Result<WorkflowWorkspaceQueryResponse, WorkflowWorkspaceQueryError> {
        self.query(WorkflowWorkspaceQueryMode::Fast).await
    }

    pub async fn refresh(
        &self,
    ) -> Result<WorkflowWorkspaceQueryResponse, WorkflowWorkspaceQueryError> {
        self.query(WorkflowWorkspaceQueryMode::Refresh).await
    }

    /// Store a capability result produced by an explicit recheck. The exact
    /// recipe is required so a recheck cannot update another recipe's cache.
    /// The next FAST query then returns this evidence without another ComfyUI
    /// request.
    pub async fn update_capability_cache(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
        capability: CapabilityCheckView,
    ) -> Result<(), WorkflowWorkspaceQueryError> {
        let version = self
            .runtime_repository
            .find_version(workflow_version_id)
            .await
            .map_err(repository_error)?
            .ok_or_else(|| {
                WorkflowWorkspaceQueryError::new(
                    "WORKFLOW_VERSION_NOT_FOUND",
                    format!("workflow version {workflow_version_id} was not found"),
                )
            })?;
        let recipe = version
            .recipes
            .iter()
            .find(|recipe| recipe.recipe_id == recipe_id)
            .ok_or_else(|| {
                WorkflowWorkspaceQueryError::new(
                    "RECIPE_NOT_FOUND",
                    format!("recipe {recipe_id} is not registered for the exact version"),
                )
            })?;
        let artifacts = self
            .artifact_repository
            .list()
            .await
            .map_err(repository_error)?;
        let pair = ExactPair {
            workflow_version_id: workflow_version_id.to_owned(),
            recipe_id: recipe_id.to_owned(),
        };
        let candidates = group_artifacts(artifacts).remove(&pair).unwrap_or_default();
        let artifact = match candidates.as_slice() {
            [artifact] => artifact,
            [] => {
                return Err(WorkflowWorkspaceQueryError::new(
                    "RUNTIME_PACKAGE_MISSING",
                    "the exact recipe runtime artifact is not registered",
                ))
            }
            _ => {
                return Err(WorkflowWorkspaceQueryError::new(
                    "RUNTIME_ARTIFACT_CONFLICT",
                    "more than one runtime artifact claims the exact recipe",
                ))
            }
        };
        let state = self
            .state_repository
            .find_state(workflow_version_id)
            .await
            .map_err(repository_error)?;
        let registry = match self.registry_service.get(&version.workflow_id).await {
            Ok(view) => static_registry_view(view),
            Err(WorkflowRegistryServiceError::WorkflowNotFound(_)) => {
                orphan_registry_view(&version.workflow_id, std::slice::from_ref(&version))
            }
            Err(error) => return Err(registry_error(error)),
        };
        let enabled = state.as_ref().is_none_or(|state| state.enabled);
        let archived = state.as_ref().is_some_and(|state| state.archived);
        let archived_at = state.as_ref().and_then(|state| state.archived_at);
        let mut view = self
            .cache
            .read()
            .await
            .get(&pair)
            .filter(|cached| cache_matches(cached, artifact))
            .map(|cached| cached.view.clone())
            .unwrap_or_else(|| {
                let mut view = base_runtime(
                    &version,
                    recipe,
                    Some(artifact),
                    enabled,
                    archived,
                    archived_at,
                    &registry.library_state,
                );
                view.artifact_status = "VERIFIED".to_owned();
                view.package_status = "VALID".to_owned();
                view
            });
        apply_capability(&mut view, &capability, self.clock.now().to_rfc3339());
        let package_status = view.package_status.clone();
        let artifact_status = view.artifact_status.clone();
        let diagnostics = view.diagnostics.clone();
        self.cache_runtime(
            artifact,
            finalize_runtime(view, &artifact_status, &package_status, diagnostics),
        )
        .await;
        Ok(())
    }

    /// Compatibility hook for the existing version-level recheck command.
    /// Recheck currently evaluates the newest recipe, so resolve that recipe
    /// by semantic version and persist the result under its exact pair.
    pub async fn cache_capability_for_version(
        &self,
        workflow_version_id: &str,
        capability: CapabilityCheckView,
    ) -> Result<(), WorkflowWorkspaceQueryError> {
        let version = self
            .runtime_repository
            .find_version(workflow_version_id)
            .await
            .map_err(repository_error)?
            .ok_or_else(|| {
                WorkflowWorkspaceQueryError::new(
                    "WORKFLOW_VERSION_NOT_FOUND",
                    format!("workflow version {workflow_version_id} was not found"),
                )
            })?;
        let recipe_id = version
            .recipes
            .iter()
            .max_by(|left, right| compare_versions(&left.version, &right.version))
            .map(|recipe| recipe.recipe_id.clone())
            .ok_or_else(|| {
                WorkflowWorkspaceQueryError::new(
                    "RECIPE_NOT_FOUND",
                    format!("workflow version {workflow_version_id} has no recipes"),
                )
            })?;
        self.update_capability_cache(workflow_version_id, &recipe_id, capability)
            .await
    }

    /// Explicit-recipe spelling for callers that already have the pair.
    pub async fn cache_capability_for_recipe(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
        capability: CapabilityCheckView,
    ) -> Result<(), WorkflowWorkspaceQueryError> {
        self.update_capability_cache(workflow_version_id, recipe_id, capability)
            .await
    }

    pub async fn query(
        &self,
        mode: WorkflowWorkspaceQueryMode,
    ) -> Result<WorkflowWorkspaceQueryResponse, WorkflowWorkspaceQueryError> {
        let registry = self
            .registry_service
            .list()
            .await
            .map_err(registry_error)?
            .into_iter()
            .map(static_registry_view)
            .map(|view| (view.workflow_id.clone(), view))
            .collect::<BTreeMap<_, _>>();
        let versions = self
            .runtime_repository
            .list_versions()
            .await
            .map_err(repository_error)?;
        let states = self
            .state_repository
            .list_states()
            .await
            .map_err(repository_error)?
            .into_iter()
            .map(|state| (state.workflow_version_id.clone(), state))
            .collect::<HashMap<_, _>>();
        let artifacts = self
            .artifact_repository
            .list()
            .await
            .map_err(repository_error)?;
        let artifacts_by_pair = group_artifacts(artifacts);
        let cached = self.cache.read().await.clone();
        let mut runtime_by_workflow = BTreeMap::<String, Vec<WorkflowWorkspaceRuntimeView>>::new();

        for version in &versions {
            let state = states.get(&version.workflow_version_id);
            let enabled = state.is_none_or(|state| state.enabled);
            let archived = state.is_some_and(|state| state.archived);
            let archived_at = state.and_then(|state| state.archived_at);
            let library_state = registry
                .get(&version.workflow_id)
                .map(|view| view.library_state.as_str())
                .unwrap_or("UNKNOWN");

            for recipe in &version.recipes {
                let pair = ExactPair {
                    workflow_version_id: version.workflow_version_id.clone(),
                    recipe_id: recipe.recipe_id.clone(),
                };
                let candidates = artifacts_by_pair
                    .get(&pair)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                let runtime = match mode {
                    WorkflowWorkspaceQueryMode::Fast => self.fast_runtime(
                        version,
                        recipe,
                        candidates,
                        enabled,
                        archived,
                        archived_at,
                        library_state,
                        cached.get(&pair),
                    ),
                    WorkflowWorkspaceQueryMode::Refresh => {
                        self.refresh_runtime(
                            version,
                            recipe,
                            candidates,
                            enabled,
                            archived,
                            archived_at,
                            library_state,
                        )
                        .await?
                    }
                };
                runtime_by_workflow
                    .entry(version.workflow_id.clone())
                    .or_default()
                    .push(runtime);
            }
        }

        let mut registry = registry;
        for workflow_id in runtime_by_workflow.keys() {
            registry
                .entry(workflow_id.clone())
                .or_insert_with(|| orphan_registry_view(workflow_id, &versions));
        }
        let mut items = registry
            .into_iter()
            .map(|(workflow_id, registry_view)| WorkflowWorkspaceView {
                registry: registry_view,
                runtime: runtime_by_workflow.remove(&workflow_id).unwrap_or_default(),
            })
            .collect::<Vec<_>>();
        items.sort_by(|left, right| {
            left.registry
                .name
                .cmp(&right.registry.name)
                .then(left.registry.workflow_id.cmp(&right.registry.workflow_id))
        });
        for item in &mut items {
            item.runtime.sort_by(|left, right| {
                left.workflow_version
                    .cmp(&right.workflow_version)
                    .then(left.recipe_version.cmp(&right.recipe_version))
                    .then(left.recipe_id.cmp(&right.recipe_id))
            });
        }

        let staging = self
            .package_store
            .list_staging_ids()
            .await
            .map_err(package_store_error)?
            .into_iter()
            .map(|staging_id| WorkflowStagingView {
                in_use: self.onboarding_service.is_draft_active(&staging_id),
                staging_id,
                status: "STALE_STAGING".to_owned(),
            })
            .collect();

        Ok(WorkflowWorkspaceQueryResponse { items, staging })
    }

    #[allow(clippy::too_many_arguments)]
    fn fast_runtime(
        &self,
        version: &RuntimeWorkflowVersionRecord,
        recipe: &RuntimeRecipeRecord,
        candidates: &[WorkflowRuntimeArtifactRecord],
        enabled: bool,
        archived: bool,
        archived_at: Option<chrono::DateTime<Utc>>,
        library_state: &str,
        cached: Option<&CachedRuntimeView>,
    ) -> WorkflowWorkspaceRuntimeView {
        match candidates {
            [] => finalize_runtime(
                base_runtime(
                    version,
                    recipe,
                    None,
                    enabled,
                    archived,
                    archived_at,
                    library_state,
                ),
                "MISSING",
                "MISSING",
                vec![diagnostic(
                    "RUNTIME_PACKAGE_MISSING",
                    "the exact recipe runtime artifact is not registered",
                )],
            ),
            [artifact] => {
                if let Some(cached) = cached.filter(|cached| cache_matches(cached, artifact)) {
                    let mut view = cached.view.clone();
                    refresh_runtime_metadata(
                        &mut view,
                        version,
                        recipe,
                        enabled,
                        archived,
                        archived_at,
                        library_state,
                    );
                    return view;
                }
                finalize_runtime(
                    base_runtime(
                        version,
                        recipe,
                        Some(artifact),
                        enabled,
                        archived,
                        archived_at,
                        library_state,
                    ),
                    "REGISTERED",
                    "REGISTERED",
                    vec![diagnostic(
                        "RUNTIME_STATUS_NOT_REFRESHED",
                        "exact runtime package has not been refreshed in this workspace",
                    )],
                )
            }
            _ => finalize_runtime(
                base_runtime(
                    version,
                    recipe,
                    None,
                    enabled,
                    archived,
                    archived_at,
                    library_state,
                ),
                "CONFLICT",
                "CONFLICT",
                vec![diagnostic(
                    "RUNTIME_ARTIFACT_CONFLICT",
                    "more than one runtime artifact claims the exact recipe",
                )],
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn refresh_runtime(
        &self,
        version: &RuntimeWorkflowVersionRecord,
        recipe: &RuntimeRecipeRecord,
        candidates: &[WorkflowRuntimeArtifactRecord],
        enabled: bool,
        archived: bool,
        archived_at: Option<chrono::DateTime<Utc>>,
        library_state: &str,
    ) -> Result<WorkflowWorkspaceRuntimeView, WorkflowWorkspaceQueryError> {
        if candidates.len() != 1 {
            return Ok(if candidates.is_empty() {
                finalize_runtime(
                    base_runtime(
                        version,
                        recipe,
                        None,
                        enabled,
                        archived,
                        archived_at,
                        library_state,
                    ),
                    "MISSING",
                    "MISSING",
                    vec![diagnostic(
                        "RUNTIME_PACKAGE_MISSING",
                        "the exact recipe runtime artifact is not registered",
                    )],
                )
            } else {
                finalize_runtime(
                    base_runtime(
                        version,
                        recipe,
                        None,
                        enabled,
                        archived,
                        archived_at,
                        library_state,
                    ),
                    "CONFLICT",
                    "CONFLICT",
                    vec![diagnostic(
                        "RUNTIME_ARTIFACT_CONFLICT",
                        "more than one runtime artifact claims the exact recipe",
                    )],
                )
            });
        }

        let artifact = &candidates[0];
        let mut view = base_runtime(
            version,
            recipe,
            Some(artifact),
            enabled,
            archived,
            archived_at,
            library_state,
        );
        let package = match self
            .package_store
            .read_runtime(&artifact.package_name)
            .await
        {
            Ok(package) => package,
            Err(error) => {
                view.package_status = "MISSING".to_owned();
                view.diagnostics
                    .push(diagnostic("RUNTIME_PACKAGE_MISSING", error.to_string()));
                let diagnostics = view.diagnostics.clone();
                let view = finalize_runtime(view, "PRESENT", "MISSING", diagnostics);
                self.cache_runtime(artifact, view.clone()).await;
                return Ok(view);
            }
        };

        let mut diagnostics = Vec::new();
        let manifest = match parse_manifest(&package) {
            Ok(manifest) => Some(manifest),
            Err((code, message)) => {
                diagnostics.push(diagnostic(&code, message));
                None
            }
        };
        let recipe_runtime = match parse_recipe(&package) {
            Ok(recipe) => Some(recipe),
            Err((code, message)) => {
                diagnostics.push(diagnostic(&code, message));
                None
            }
        };
        let workflow = match parse_workflow(&package) {
            Ok(workflow) => Some(workflow),
            Err((code, message)) => {
                diagnostics.push(diagnostic(&code, message));
                None
            }
        };

        if let Some(manifest) = &manifest {
            if manifest.id != version.workflow_id
                || manifest.workflow_version != version.workflow_version
                || manifest.recipe_version != recipe.version
            {
                diagnostics.push(diagnostic(
                    "WORKFLOW_PACKAGE_INVALID",
                    "runtime manifest does not match the exact Registry version and recipe",
                ));
            }
        }
        if let Some(runtime_recipe) = &recipe_runtime {
            // `recipes.id` is the database identity of the exact
            // (workflow_version_id, recipe_version) relation. Package YAML
            // carries its own stable semantic id, so it is not expected to
            // equal the generated database id. Schema and bytes are the
            // immutable checks; manifest.recipe_version above binds the
            // package to this exact Registry recipe.
            if runtime_recipe.schema_version != recipe.schema_version {
                diagnostics.push(diagnostic(
                    "RECIPE_RUNTIME_SCHEMA_MISMATCH",
                    "runtime Recipe schema does not match the exact Registry recipe",
                ));
            }
        }
        let workflow_hash = sha256(&package.workflow_api_json);
        let recipe_hash = sha256(&package.recipe_yaml);
        if workflow_hash != version.workflow_sha256 || workflow_hash != artifact.workflow_sha256 {
            diagnostics.push(diagnostic(
                "WORKFLOW_RUNTIME_HASH_MISMATCH",
                "runtime workflow bytes do not match the exact registered hash",
            ));
        }
        if recipe_hash != recipe.recipe_sha256 || recipe_hash != artifact.recipe_sha256 {
            diagnostics.push(diagnostic(
                "RECIPE_RUNTIME_HASH_MISMATCH",
                "runtime recipe bytes do not match the exact registered hash",
            ));
        }
        if let Err(error) = read_back_and_validate_package(&package) {
            diagnostics.push(diagnostic(error.code(), error.to_string()));
        }

        view.node_count = workflow
            .as_ref()
            .and_then(|workflow| workflow.value().as_object())
            .map_or(0, Map::len);
        if diagnostics.is_empty() {
            view.package_status = "VALID".to_owned();
            view.artifact_status = "VERIFIED".to_owned();
            if let Some(runtime_recipe) = recipe_runtime {
                let capability = self
                    .onboarding_service
                    .check_runtime_workflow_with_recipe(
                        &bytes_to_string(&package.workflow_api_json),
                        &runtime_recipe,
                    )
                    .await
                    .map_err(|error| {
                        WorkflowWorkspaceQueryError::new(error.code(), error.to_string())
                    })?;
                apply_capability(&mut view, &capability, self.clock.now().to_rfc3339());
            }
        } else {
            view.package_status = "INVALID".to_owned();
            view.diagnostics = diagnostics;
        }
        let artifact_status = if view.package_status == "VALID" {
            "VERIFIED"
        } else {
            "PRESENT"
        };
        let package_status = view.package_status.clone();
        let diagnostics = view.diagnostics.clone();
        let view = finalize_runtime(view, artifact_status, &package_status, diagnostics);
        self.cache_runtime(artifact, view.clone()).await;
        Ok(view)
    }

    async fn cache_runtime(
        &self,
        artifact: &WorkflowRuntimeArtifactRecord,
        view: WorkflowWorkspaceRuntimeView,
    ) {
        self.cache.write().await.insert(
            ExactPair {
                workflow_version_id: artifact.workflow_version_id.clone(),
                recipe_id: artifact.recipe_id.clone(),
            },
            CachedRuntimeView {
                artifact_id: artifact.id.clone(),
                package_name: artifact.package_name.clone(),
                workflow_sha256: artifact.workflow_sha256.clone(),
                recipe_sha256: artifact.recipe_sha256.clone(),
                view,
            },
        );
    }
}

fn static_registry_view(view: WorkflowRegistryView) -> WorkflowWorkspaceRegistryView {
    WorkflowWorkspaceRegistryView {
        workflow_id: view.workflow_id,
        name: view.name,
        source_kind: view.source_kind,
        library_state: view.library_state,
        current_version_id: view.current_version_id,
        current_version: view.current_version,
        current_recipe: view.current_recipe,
        versions: view.versions,
        recipes: view.recipes,
        project_usage_count: view.project_usage_count,
        history_count: view.history_count,
    }
}

fn orphan_registry_view(
    workflow_id: &str,
    versions: &[RuntimeWorkflowVersionRecord],
) -> WorkflowWorkspaceRegistryView {
    let records = versions
        .iter()
        .filter(|version| version.workflow_id == workflow_id)
        .collect::<Vec<_>>();
    let registry_versions = records
        .iter()
        .map(|version| WorkflowRegistryVersionView {
            workflow_version_id: version.workflow_version_id.clone(),
            workflow_id: version.workflow_id.clone(),
            version: version.workflow_version.clone(),
            workflow_sha256: version.workflow_sha256.clone(),
            is_current: version.is_current,
            enabled: true,
            archived: false,
            recipes: Vec::new(),
        })
        .collect::<Vec<_>>();
    WorkflowWorkspaceRegistryView {
        workflow_id: workflow_id.to_owned(),
        name: records
            .first()
            .map(|version| version.name.clone())
            .unwrap_or_else(|| "Unknown Workflow".to_owned()),
        source_kind: "UNKNOWN".to_owned(),
        library_state: "UNKNOWN".to_owned(),
        current_version_id: records
            .iter()
            .find(|version| version.is_current)
            .map(|version| version.workflow_version_id.clone()),
        current_version: None,
        current_recipe: None,
        versions: registry_versions,
        recipes: Vec::new(),
        project_usage_count: 0,
        history_count: 0,
    }
}

fn group_artifacts(
    artifacts: Vec<WorkflowRuntimeArtifactRecord>,
) -> HashMap<ExactPair, Vec<WorkflowRuntimeArtifactRecord>> {
    let mut grouped = HashMap::new();
    for artifact in artifacts {
        grouped
            .entry(ExactPair {
                workflow_version_id: artifact.workflow_version_id.clone(),
                recipe_id: artifact.recipe_id.clone(),
            })
            .or_insert_with(Vec::new)
            .push(artifact);
    }
    grouped
}

fn base_runtime(
    version: &RuntimeWorkflowVersionRecord,
    recipe: &RuntimeRecipeRecord,
    artifact: Option<&WorkflowRuntimeArtifactRecord>,
    enabled: bool,
    archived: bool,
    archived_at: Option<chrono::DateTime<Utc>>,
    library_state: &str,
) -> WorkflowWorkspaceRuntimeView {
    WorkflowWorkspaceRuntimeView {
        workflow_id: version.workflow_id.clone(),
        workflow_version_id: version.workflow_version_id.clone(),
        recipe_id: recipe.recipe_id.clone(),
        name: version.name.clone(),
        category: version.category.clone(),
        mode: version.mode.clone(),
        workflow_version: version.workflow_version.clone(),
        recipe_version: recipe.version.clone(),
        workflow_sha256: version.workflow_sha256.clone(),
        recipe_sha256: recipe.recipe_sha256.clone(),
        artifact_id: artifact.map(|artifact| artifact.id.clone()),
        artifact_source_kind: artifact.map(|artifact| artifact.source_kind.clone()),
        package_name: artifact.map(|artifact| artifact.package_name.clone()),
        package_source_path: artifact.and_then(|artifact| artifact.package_source_path.clone()),
        artifact_status: if artifact.is_some() {
            "REGISTERED".to_owned()
        } else {
            "MISSING".to_owned()
        },
        package_status: if artifact.is_some() {
            "REGISTERED".to_owned()
        } else {
            "MISSING".to_owned()
        },
        library_state: library_state.to_owned(),
        enabled,
        archived,
        archived_at: archived_at.map(|value| value.to_rfc3339()),
        capability: "NOT_CHECKED".to_owned(),
        capability_issues: Vec::new(),
        readiness: "DEGRADED".to_owned(),
        readiness_reasons: Vec::new(),
        diagnostics: Vec::new(),
        node_count: 0,
        live_verified_at: None,
        has_successful_run: version.has_successful_run,
        latest_success_at: version.latest_success_at.clone(),
        latest_failure_at: version.latest_failure_at.clone(),
        active_tasks: version.active_tasks,
        total_tasks: version.total_tasks,
    }
}

fn refresh_runtime_metadata(
    view: &mut WorkflowWorkspaceRuntimeView,
    version: &RuntimeWorkflowVersionRecord,
    recipe: &RuntimeRecipeRecord,
    enabled: bool,
    archived: bool,
    archived_at: Option<chrono::DateTime<Utc>>,
    library_state: &str,
) {
    view.workflow_id = version.workflow_id.clone();
    view.workflow_version_id = version.workflow_version_id.clone();
    view.recipe_id = recipe.recipe_id.clone();
    view.name = version.name.clone();
    view.category = version.category.clone();
    view.mode = version.mode.clone();
    view.workflow_version = version.workflow_version.clone();
    view.recipe_version = recipe.version.clone();
    view.workflow_sha256 = version.workflow_sha256.clone();
    view.recipe_sha256 = recipe.recipe_sha256.clone();
    view.enabled = enabled;
    view.archived = archived;
    view.archived_at = archived_at.map(|value| value.to_rfc3339());
    view.library_state = library_state.to_owned();
    view.has_successful_run = version.has_successful_run;
    view.latest_success_at = version.latest_success_at.clone();
    view.latest_failure_at = version.latest_failure_at.clone();
    view.active_tasks = version.active_tasks;
    view.total_tasks = version.total_tasks;
    let updated = finalize_runtime(
        view.clone(),
        &view.artifact_status,
        &view.package_status,
        view.diagnostics.clone(),
    );
    *view = updated;
}

fn finalize_runtime(
    mut view: WorkflowWorkspaceRuntimeView,
    artifact_status: &str,
    package_status: &str,
    diagnostics: Vec<WorkflowDiagnosticView>,
) -> WorkflowWorkspaceRuntimeView {
    view.artifact_status = artifact_status.to_owned();
    view.package_status = package_status.to_owned();
    view.diagnostics = diagnostics;
    let mut reasons = Vec::new();
    if view.archived {
        reasons.push("workflow version is archived".to_owned());
    }
    if view.library_state != "ACTIVE" {
        reasons.push("workflow is not active in the Registry".to_owned());
    }
    if !view.enabled {
        reasons.push("runtime artifact is disabled".to_owned());
    }
    if view.package_status == "MISSING" {
        reasons.push("exact runtime package is missing".to_owned());
    } else if view.package_status == "CONFLICT" {
        reasons.push("exact runtime artifact is ambiguous".to_owned());
    } else if view.package_status == "INVALID" {
        reasons.push("exact runtime package audit failed".to_owned());
    } else if view.package_status == "REGISTERED" {
        reasons.push("exact runtime package has not been refreshed".to_owned());
    }
    match view.capability.as_str() {
        "MISSING_NODES" => reasons.push("ComfyUI is missing workflow nodes".to_owned()),
        "INCOMPATIBLE_INPUT_VALUES" => {
            reasons.push("workflow inputs are incompatible with ComfyUI".to_owned())
        }
        "COMFY_OFFLINE" => reasons.push("ComfyUI is offline".to_owned()),
        "NOT_CHECKED" => reasons.push("runtime capability has not been checked".to_owned()),
        _ => {}
    }
    if !view.diagnostics.is_empty() {
        reasons.push(format!(
            "{} runtime diagnostics reported",
            view.diagnostics.len()
        ));
    }
    if !view.has_successful_run && reasons.is_empty() {
        reasons.push("no successful generation has been recorded".to_owned());
    }
    let blocked = view.archived
        || view.library_state != "ACTIVE"
        || !view.enabled
        || matches!(
            view.package_status.as_str(),
            "MISSING" | "CONFLICT" | "INVALID"
        )
        || !view.diagnostics.is_empty()
        || matches!(
            view.capability.as_str(),
            "MISSING_NODES" | "INCOMPATIBLE_INPUT_VALUES" | "COMFY_OFFLINE"
        );
    view.readiness = if blocked {
        "BLOCKED".to_owned()
    } else if view.capability == "NOT_CHECKED"
        || view.package_status == "REGISTERED"
        || !view.has_successful_run
    {
        "DEGRADED".to_owned()
    } else {
        "READY".to_owned()
    };
    view.readiness_reasons = reasons;
    view
}

fn cache_matches(cached: &CachedRuntimeView, artifact: &WorkflowRuntimeArtifactRecord) -> bool {
    cached.artifact_id == artifact.id
        && cached.package_name == artifact.package_name
        && cached.workflow_sha256 == artifact.workflow_sha256
        && cached.recipe_sha256 == artifact.recipe_sha256
}

fn parse_manifest(
    package: &crate::application::ports::WorkflowPackageBytes,
) -> Result<WorkflowManifest, (String, String)> {
    let text = String::from_utf8(package.manifest_yaml.clone())
        .map_err(|error| ("WORKFLOW_PACKAGE_INVALID".to_owned(), error.to_string()))?;
    let manifest = WorkflowManifest::parse(&text)
        .map_err(|error| ("WORKFLOW_PACKAGE_INVALID".to_owned(), error))?;
    manifest
        .validate()
        .map_err(|error| ("WORKFLOW_PACKAGE_INVALID".to_owned(), error))?;
    Ok(manifest)
}

fn parse_recipe(
    package: &crate::application::ports::WorkflowPackageBytes,
) -> Result<crate::domain::Recipe, (String, String)> {
    let text = String::from_utf8(package.recipe_yaml.clone())
        .map_err(|error| ("RECIPE_INVALID".to_owned(), error.to_string()))?;
    RecipeParser::parse(&text).map_err(|error| ("RECIPE_INVALID".to_owned(), error.to_string()))
}

fn parse_workflow(
    package: &crate::application::ports::WorkflowPackageBytes,
) -> Result<WorkflowDocument, (String, String)> {
    let text = String::from_utf8(package.workflow_api_json.clone())
        .map_err(|error| ("WORKFLOW_PACKAGE_INVALID".to_owned(), error.to_string()))?;
    let value = serde_json::from_str::<Value>(&text)
        .map_err(|error| ("WORKFLOW_PACKAGE_INVALID".to_owned(), error.to_string()))?;
    WorkflowDocument::parse(value)
        .map_err(|error| ("WORKFLOW_PACKAGE_INVALID".to_owned(), error.to_string()))
}

fn bytes_to_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn diagnostic(code: impl Into<String>, message: impl Into<String>) -> WorkflowDiagnosticView {
    WorkflowDiagnosticView {
        code: code.into(),
        message: message.into(),
    }
}

fn apply_capability(
    view: &mut WorkflowWorkspaceRuntimeView,
    capability: &CapabilityCheckView,
    fallback_timestamp: String,
) {
    view.capability = capability_state_name(capability.state).to_owned();
    view.capability_issues = capability.issues.clone();
    view.live_verified_at = Some(capability.checked_at.clone().unwrap_or(fallback_timestamp));
}

fn capability_state_name(state: CapabilityState) -> &'static str {
    match state {
        CapabilityState::NotChecked => "NOT_CHECKED",
        CapabilityState::Ready => "READY",
        CapabilityState::MissingNodes => "MISSING_NODES",
        CapabilityState::IncompatibleInputValues => "INCOMPATIBLE_INPUT_VALUES",
        CapabilityState::ComfyOffline => "COMFY_OFFLINE",
    }
}

fn repository_error(
    error: crate::application::ports::RepositoryError,
) -> WorkflowWorkspaceQueryError {
    WorkflowWorkspaceQueryError::new("WORKFLOW_WORKSPACE_REPOSITORY_ERROR", error.to_string())
}

fn registry_error(error: WorkflowRegistryServiceError) -> WorkflowWorkspaceQueryError {
    WorkflowWorkspaceQueryError::new(error.code(), error.to_string())
}

fn package_store_error(
    error: crate::application::ports::WorkflowPackageStoreError,
) -> WorkflowWorkspaceQueryError {
    WorkflowWorkspaceQueryError::new("WORKFLOW_PACKAGE_STORE_ERROR", error.to_string())
}

fn compare_versions(left: &str, right: &str) -> std::cmp::Ordering {
    let left_parts = left
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect::<Vec<_>>();
    let right_parts = right
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect::<Vec<_>>();
    (0..3)
        .map(|index| {
            left_parts
                .get(index)
                .copied()
                .unwrap_or(0)
                .cmp(&right_parts.get(index).copied().unwrap_or(0))
        })
        .find(|ordering| *ordering != std::cmp::Ordering::Equal)
        .unwrap_or_else(|| left.cmp(right))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version() -> RuntimeWorkflowVersionRecord {
        RuntimeWorkflowVersionRecord {
            workflow_version_id: "wfv_test".to_owned(),
            workflow_id: "wfl_test".to_owned(),
            name: "Test".to_owned(),
            category: "video".to_owned(),
            mode: "text_to_video".to_owned(),
            workflow_version: "1.0.0".to_owned(),
            workflow_sha256: "workflow".to_owned(),
            api_workflow_json: "{}".to_owned(),
            package_name: Some("legacy_should_not_be_used".to_owned()),
            is_current: true,
            recipes: vec![RuntimeRecipeRecord {
                recipe_id: "rcp_test".to_owned(),
                version: "1.0.0".to_owned(),
                schema_version: 1,
                recipe_yaml: String::new(),
                recipe_sha256: "recipe".to_owned(),
            }],
            active_tasks: 0,
            total_tasks: 0,
            has_successful_run: false,
            latest_success_at: None,
            latest_failure_at: None,
        }
    }

    #[test]
    fn fast_missing_artifact_never_uses_legacy_package_name() {
        let version = version();
        let recipe = &version.recipes[0];
        let view = finalize_runtime(
            base_runtime(&version, recipe, None, true, false, None, "ACTIVE"),
            "MISSING",
            "MISSING",
            vec![diagnostic(
                "RUNTIME_PACKAGE_MISSING",
                "missing exact artifact",
            )],
        );
        assert_eq!(view.package_name, None);
        assert_eq!(view.package_status, "MISSING");
        assert_eq!(view.readiness, "BLOCKED");
    }

    #[test]
    fn conflict_is_blocked_without_selecting_an_artifact() {
        let version = version();
        let recipe = &version.recipes[0];
        let view = finalize_runtime(
            base_runtime(&version, recipe, None, true, false, None, "ACTIVE"),
            "CONFLICT",
            "CONFLICT",
            vec![diagnostic("RUNTIME_ARTIFACT_CONFLICT", "two artifacts")],
        );
        assert_eq!(view.artifact_id, None);
        assert_eq!(view.readiness, "BLOCKED");
        assert_eq!(view.diagnostics[0].code, "RUNTIME_ARTIFACT_CONFLICT");
    }

    #[test]
    fn static_registry_view_has_no_runtime_capability_field() {
        let registry = WorkflowRegistryView {
            workflow_id: "wfl_test".to_owned(),
            name: "Test".to_owned(),
            source_kind: "USER".to_owned(),
            library_state: "ACTIVE".to_owned(),
            current_version_id: None,
            current_version: None,
            current_recipe: None,
            versions: Vec::new(),
            recipes: Vec::new(),
            project_usage_count: 0,
            history_count: 0,
        };
        let static_view = static_registry_view(registry);
        let json = serde_json::to_value(static_view).expect("static view serializes");
        assert!(json.get("capability").is_none());
    }
}
