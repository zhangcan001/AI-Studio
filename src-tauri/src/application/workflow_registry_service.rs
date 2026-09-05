use crate::application::{
    builtin_runtime_packages::is_builtin_package_name,
    ports::{
        Clock, ProjectWorkflowBindingRecord, ProjectWorkflowBindingRepository, RepositoryError,
        RuntimeRecipeRecord, RuntimeWorkflowVersionRecord, WorkflowDeletionCounts,
        WorkflowPackageQuarantineResult, WorkflowPackageStore, WorkflowPurgeOperationEntry,
        WorkflowPurgeOperationRecord, WorkflowRegistryRepository,
        WorkflowRuntimeArtifactRepository, WorkflowRuntimeRepository, WorkflowRuntimeState,
        WorkflowRuntimeStateRepository,
    },
    workflow_manifest::WorkflowManifest,
};
use crate::compiler::RecipeParser;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    error::Error,
    fmt,
    sync::Arc,
};
use tokio::sync::Mutex;
use uuid::Uuid;

pub const WORKFLOW_SOURCE_PRODUCT: &str = "PRODUCT";
pub const WORKFLOW_SOURCE_USER: &str = "USER";
pub const WORKFLOW_LIBRARY_ACTIVE: &str = "ACTIVE";
pub const WORKFLOW_LIBRARY_REMOVED: &str = "REMOVED";

/// The persisted Registry V2 values. The legacy runtime repository does not
/// expose the new columns yet, so the service keeps these values at the
/// application boundary until the Registry repository is available.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowSourceKind {
    Product,
    User,
}

impl WorkflowSourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Product => WORKFLOW_SOURCE_PRODUCT,
            Self::User => WORKFLOW_SOURCE_USER,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowLibraryState {
    Active,
    Removed,
}

impl WorkflowLibraryState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => WORKFLOW_LIBRARY_ACTIVE,
            Self::Removed => WORKFLOW_LIBRARY_REMOVED,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRegistryRecipeView {
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub version: String,
    pub schema_version: u32,
    #[serde(skip_serializing)]
    pub recipe_yaml: String,
    pub recipe_sha256: String,
    pub package_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRegistryVersionView {
    pub workflow_version_id: String,
    pub workflow_id: String,
    pub version: String,
    pub workflow_sha256: String,
    pub is_current: bool,
    pub enabled: bool,
    pub archived: bool,
    pub recipes: Vec<WorkflowRegistryRecipeView>,
}

/// One row per logical Workflow. Versions and recipes are nested deliberately;
/// callers must not turn this read model back into one list row per version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRegistryView {
    pub workflow_id: String,
    pub name: String,
    pub source_kind: String,
    pub library_state: String,
    pub current_version_id: Option<String>,
    pub current_version: Option<WorkflowRegistryVersionView>,
    pub current_recipe: Option<WorkflowRegistryRecipeView>,
    pub versions: Vec<WorkflowRegistryVersionView>,
    pub recipes: Vec<WorkflowRegistryRecipeView>,
    pub project_usage_count: u64,
    pub history_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRegistryMutationResult {
    pub workflow_id: String,
    pub library_state: String,
    pub cleared_binding_count: u64,
    pub version_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRegistryRestoreResult {
    pub workflow_id: String,
    pub library_state: String,
    pub current_version_id: Option<String>,
    pub enabled: bool,
    pub readiness: String,
    pub capability: String,
    pub project_binding_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRegistryPurgeResult {
    pub workflow_id: String,
    pub version_count: u64,
    pub recipe_count: u64,
    pub committed: bool,
    pub cleanup_pending: bool,
    pub warning: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowPurgeInspection {
    pub workflow_id: String,
    pub name: String,
    pub source_kind: String,
    pub library_state: String,
    pub task_count: u64,
    pub batch_item_count: u64,
    pub preset_count: u64,
    pub template_count: u64,
    pub shot_config_count: u64,
    pub benchmark_count: u64,
    pub binding_count: u64,
    pub stage_count: u64,
    pub run_template_count: u64,
    pub package_count: u64,
    pub can_purge: bool,
    pub blocking_reasons: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkflowPurgeRecoveryReport {
    pub reconciled_operations: u64,
}

/// DB-backed identity candidate used by import/recognition.  The candidate is
/// built from the immutable version/recipe rows and the exact artifact table;
/// the package directory is deliberately not consulted to decide whether a
/// logical Workflow already exists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowRegistryIdentityCandidate {
    pub workflow_id: String,
    pub workflow_name: String,
    pub category: String,
    pub mode: String,
    pub source_kind: String,
    pub library_state: String,
    pub workflow_version_id: String,
    pub workflow_version: String,
    pub workflow_sha256: String,
    pub api_workflow_json: String,
    pub recipe_id: String,
    pub recipe_version: String,
    pub recipe_schema_version: u32,
    pub recipe_yaml: String,
    pub recipe_sha256: String,
    pub package_name: Option<String>,
    pub package_source_path: Option<String>,
}

/// Minimal Registry application service using the ports available before
/// Migration 028. The new repository port can replace this adapter without
/// changing the domain decisions below.
pub struct WorkflowRegistryService {
    runtime_repository: Arc<dyn WorkflowRuntimeRepository>,
    state_repository: Arc<dyn WorkflowRuntimeStateRepository>,
    binding_repository: Arc<dyn ProjectWorkflowBindingRepository>,
    clock: Arc<dyn Clock>,
    registry_repository: Option<Arc<dyn WorkflowRegistryRepository>>,
    runtime_artifact_repository: Option<Arc<dyn WorkflowRuntimeArtifactRepository>>,
    package_store: Option<Arc<dyn WorkflowPackageStore>>,
    lifecycle_gate: Arc<Mutex<()>>,
}

impl WorkflowRegistryService {
    pub fn new(
        runtime_repository: Arc<dyn WorkflowRuntimeRepository>,
        state_repository: Arc<dyn WorkflowRuntimeStateRepository>,
        binding_repository: Arc<dyn ProjectWorkflowBindingRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            runtime_repository,
            state_repository,
            binding_repository,
            clock,
            registry_repository: None,
            runtime_artifact_repository: None,
            package_store: None,
            lifecycle_gate: Arc::new(Mutex::new(())),
        }
    }

    /// Attach the first-class logical Workflow repository. The legacy
    /// runtime-only projection remains available as a compatibility fallback
    /// for tests and old callers during the strangler cutover.
    pub fn with_registry_repository(
        mut self,
        repository: Arc<dyn WorkflowRegistryRepository>,
    ) -> Self {
        self.registry_repository = Some(repository);
        self
    }

    pub fn with_runtime_artifact_repository(
        mut self,
        repository: Arc<dyn WorkflowRuntimeArtifactRepository>,
    ) -> Self {
        self.runtime_artifact_repository = Some(repository);
        self
    }

    pub fn with_package_store(mut self, package_store: Arc<dyn WorkflowPackageStore>) -> Self {
        self.package_store = Some(package_store);
        self
    }

    /// Reconcile purge journals before the runtime library is scanned. A
    /// present Registry row means the database purge did not commit and the
    /// quarantined packages must be restored; an absent row means cleanup is
    /// committed and the quarantine must not be restored.
    pub async fn recover_pending_purges(
        &self,
    ) -> Result<WorkflowPurgeRecoveryReport, WorkflowRegistryServiceError> {
        let _guard = self.lifecycle_gate.lock().await;
        self.recover_pending_purges_inner().await
    }

    async fn recover_pending_purges_inner(
        &self,
    ) -> Result<WorkflowPurgeRecoveryReport, WorkflowRegistryServiceError> {
        let Some(package_store) = &self.package_store else {
            return Ok(WorkflowPurgeRecoveryReport::default());
        };
        let operations = package_store
            .list_purge_operations()
            .await
            .map_err(|error| purge_recovery_blocked("<purge-root>", error.to_string()))?;
        if operations.is_empty() {
            return Ok(WorkflowPurgeRecoveryReport::default());
        }
        let Some(repository) = &self.registry_repository else {
            return Err(purge_recovery_blocked(
                "<registry>",
                "workflow registry repository is not configured",
            ));
        };

        let mut report = WorkflowPurgeRecoveryReport::default();
        let mut first_error = None;
        for operation in operations {
            let operation_id = operation.operation_id().to_owned();
            let result = match operation {
                WorkflowPurgeOperationEntry::Journal(record) => {
                    self.recover_journal_operation(package_store, repository, &record)
                        .await
                }
                WorkflowPurgeOperationEntry::Legacy { operation_id } => {
                    self.recover_legacy_operation(package_store, repository, &operation_id)
                        .await
                }
                WorkflowPurgeOperationEntry::Malformed {
                    operation_id,
                    message,
                } => Err(purge_recovery_blocked(&operation_id, message)),
            };
            match result {
                Ok(()) => report.reconciled_operations += 1,
                Err(error) => {
                    tracing::error!(
                        error_type = "WORKFLOW_PURGE_RECOVERY_BLOCKED",
                        operation_id = %operation_id,
                        error = %error,
                        "workflow purge recovery left operation untouched"
                    );
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }
        first_error.map_or(Ok(report), Err)
    }

    async fn recover_journal_operation(
        &self,
        package_store: &Arc<dyn WorkflowPackageStore>,
        repository: &Arc<dyn WorkflowRegistryRepository>,
        operation: &WorkflowPurgeOperationRecord,
    ) -> Result<(), WorkflowRegistryServiceError> {
        let quarantined = package_store
            .list_quarantined_packages(&operation.operation_id)
            .await
            .map_err(|error| purge_recovery_blocked(&operation.operation_id, error.to_string()))?;
        if quarantined
            .iter()
            .any(|package_name| !operation.package_names.contains(package_name))
        {
            return Err(purge_recovery_blocked(
                &operation.operation_id,
                "quarantine contains a package not listed in the purge journal",
            ));
        }
        let workflow = repository
            .get(&operation.workflow_id)
            .await
            .map_err(|error| purge_recovery_blocked(&operation.operation_id, error.to_string()))?;
        if let Some(workflow) = workflow {
            if workflow.source_kind != WORKFLOW_SOURCE_USER {
                return Err(purge_recovery_blocked(
                    &operation.operation_id,
                    "purge journal points to a PRODUCT workflow",
                ));
            }
            self.restore_quarantined_packages(package_store, &operation.operation_id, &quarantined)
                .await
                .map_err(|error| purge_recovery_blocked(&operation.operation_id, error))?;
        }
        package_store
            .remove_quarantine(&operation.operation_id)
            .await
            .map_err(|error| purge_recovery_blocked(&operation.operation_id, error.to_string()))
    }

    async fn recover_legacy_operation(
        &self,
        package_store: &Arc<dyn WorkflowPackageStore>,
        repository: &Arc<dyn WorkflowRegistryRepository>,
        operation_id: &str,
    ) -> Result<(), WorkflowRegistryServiceError> {
        let package_names = package_store
            .list_quarantined_packages(operation_id)
            .await
            .map_err(|error| purge_recovery_blocked(operation_id, error.to_string()))?;
        let Some(workflow_id) = self
            .legacy_quarantine_workflow_id(package_store, operation_id, &package_names)
            .await?
        else {
            return Err(purge_recovery_blocked(
                operation_id,
                "legacy quarantine has no unambiguous USER workflow identity",
            ));
        };
        let workflow = repository
            .get(&workflow_id)
            .await
            .map_err(|error| purge_recovery_blocked(operation_id, error.to_string()))?;
        if let Some(workflow) = workflow {
            if workflow.source_kind != WORKFLOW_SOURCE_USER {
                return Err(purge_recovery_blocked(
                    operation_id,
                    "legacy quarantine points to a PRODUCT workflow",
                ));
            }
            self.restore_quarantined_packages(package_store, operation_id, &package_names)
                .await
                .map_err(|error| purge_recovery_blocked(operation_id, error))?;
        }
        package_store
            .remove_quarantine(operation_id)
            .await
            .map_err(|error| purge_recovery_blocked(operation_id, error.to_string()))
    }

    async fn legacy_quarantine_workflow_id(
        &self,
        package_store: &Arc<dyn WorkflowPackageStore>,
        operation_id: &str,
        package_names: &[String],
    ) -> Result<Option<String>, WorkflowRegistryServiceError> {
        let mut workflow_id = None;
        for package_name in package_names {
            if is_builtin_package_name(package_name) {
                return Err(purge_recovery_blocked(
                    operation_id,
                    "legacy quarantine contains a PRODUCT package",
                ));
            }
            let package = package_store
                .read_quarantined(operation_id, package_name)
                .await
                .map_err(|error| purge_recovery_blocked(operation_id, error.to_string()))?;
            let manifest_yaml = String::from_utf8(package.manifest_yaml).map_err(|error| {
                purge_recovery_blocked(
                    operation_id,
                    format!("invalid quarantined manifest: {error}"),
                )
            })?;
            let manifest = WorkflowManifest::parse(&manifest_yaml)
                .map_err(|error| purge_recovery_blocked(operation_id, error))?;
            manifest
                .validate()
                .map_err(|error| purge_recovery_blocked(operation_id, error))?;
            if let Some(existing) = &workflow_id {
                if existing != &manifest.id {
                    return Err(purge_recovery_blocked(
                        operation_id,
                        "legacy quarantine packages identify different workflows",
                    ));
                }
            } else {
                workflow_id = Some(manifest.id);
            }
        }
        Ok(workflow_id)
    }

    /// Reads the Registry read model and groups every version into one logical
    /// Workflow row.
    pub async fn list(&self) -> Result<Vec<WorkflowRegistryView>, WorkflowRegistryServiceError> {
        let (versions, states) = self.load_runtime().await?;
        let mut grouped = BTreeMap::<String, Vec<RuntimeWorkflowVersionRecord>>::new();
        for version in versions {
            grouped
                .entry(version.workflow_id.clone())
                .or_default()
                .push(version);
        }

        let mut views = Vec::with_capacity(grouped.len());
        if let Some(repository) = &self.registry_repository {
            for record in repository.list().await? {
                let workflow_id = record.id.clone();
                let workflow_versions = grouped.remove(&workflow_id).unwrap_or_default();
                if workflow_versions.is_empty() {
                    continue;
                }
                views.push(
                    self.build_view(&workflow_id, workflow_versions, &states, Some(&record))
                        .await?,
                );
            }
        }
        for (workflow_id, versions) in grouped {
            views.push(
                self.build_view(&workflow_id, versions, &states, None)
                    .await?,
            );
        }
        views.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then(left.workflow_id.cmp(&right.workflow_id))
        });
        Ok(views)
    }

    pub async fn get(
        &self,
        workflow_id: &str,
    ) -> Result<WorkflowRegistryView, WorkflowRegistryServiceError> {
        self.list()
            .await?
            .into_iter()
            .find(|workflow| workflow.workflow_id == workflow_id)
            .ok_or_else(|| WorkflowRegistryServiceError::WorkflowNotFound(workflow_id.to_owned()))
    }

    pub async fn inspect_purge(
        &self,
        workflow_id: &str,
    ) -> Result<WorkflowPurgeInspection, WorkflowRegistryServiceError> {
        let Some(repository) = &self.registry_repository else {
            return Err(WorkflowRegistryServiceError::Repository(
                RepositoryError::database("workflow registry repository is not configured"),
            ));
        };
        let record = repository.get(workflow_id).await?.ok_or_else(|| {
            WorkflowRegistryServiceError::WorkflowNotFound(workflow_id.to_owned())
        })?;
        let (all_versions, states) = self.load_runtime().await?;
        let versions = all_versions
            .into_iter()
            .filter(|version| version.workflow_id == workflow_id)
            .collect::<Vec<_>>();
        if versions.is_empty() {
            return Err(WorkflowRegistryServiceError::WorkflowNotFound(
                workflow_id.to_owned(),
            ));
        }

        let references = repository
            .inspect_purge(workflow_id)
            .await?
            .ok_or_else(|| {
                WorkflowRegistryServiceError::WorkflowNotFound(workflow_id.to_owned())
            })?;
        let package_count = self.runtime_artifact_package_count(&versions).await?;
        let mut blocking_reasons = Vec::new();
        if record.source_kind != WORKFLOW_SOURCE_USER {
            blocking_reasons.push("PRODUCT workflows can never be purged".to_owned());
        }
        if record.library_state != WORKFLOW_LIBRARY_REMOVED {
            blocking_reasons.push("workflow must be REMOVED before purge".to_owned());
        }
        if versions
            .iter()
            .any(|version| !state_for(&states, &version.workflow_version_id).archived)
        {
            blocking_reasons
                .push("every workflow version must be archived before purge".to_owned());
        }
        append_purge_reference_reasons(&references, &mut blocking_reasons);
        if self.runtime_artifact_repository.is_none() {
            blocking_reasons.push("runtime artifact repository is not configured".to_owned());
        }
        if self.package_store.is_none() {
            blocking_reasons.push("runtime package store is not configured".to_owned());
        }

        Ok(WorkflowPurgeInspection {
            workflow_id: record.id,
            name: record.name,
            source_kind: record.source_kind,
            library_state: record.library_state,
            task_count: references.task_count,
            batch_item_count: references.batch_item_count,
            preset_count: references.preset_count,
            template_count: references.template_count,
            shot_config_count: references.shot_config_count,
            benchmark_count: references.benchmark_count,
            binding_count: references.binding_count,
            stage_count: references.stage_count,
            run_template_count: references.run_template_count,
            package_count,
            can_purge: blocking_reasons.is_empty(),
            blocking_reasons,
        })
    }

    /// Return immutable identity candidates from the database Registry.  A
    /// removed Workflow remains in this result so an exact import can offer a
    /// restore action instead of creating a duplicate.
    pub async fn identity_candidates(
        &self,
    ) -> Result<Vec<WorkflowRegistryIdentityCandidate>, WorkflowRegistryServiceError> {
        let versions = self.runtime_repository.list_versions().await?;
        let registry_records = if let Some(repository) = &self.registry_repository {
            repository
                .list()
                .await?
                .into_iter()
                .map(|record| (record.id.clone(), record))
                .collect::<BTreeMap<_, _>>()
        } else {
            BTreeMap::new()
        };
        let mut candidates = Vec::new();
        for version in versions {
            let Some(record) = registry_records.get(&version.workflow_id) else {
                // In compatibility mode the old package scanner remains the
                // fallback.  Once Migration 028 is active, a missing logical
                // record is not a valid Registry identity candidate.
                if self.registry_repository.is_some() {
                    continue;
                }
                let fallback_source = source_kind_for(std::slice::from_ref(&version))
                    .as_str()
                    .to_owned();
                let fallback_state = WORKFLOW_LIBRARY_ACTIVE.to_owned();
                for recipe in version.recipes {
                    candidates.push(WorkflowRegistryIdentityCandidate {
                        workflow_id: version.workflow_id.clone(),
                        workflow_name: version.name.clone(),
                        category: version.category.clone(),
                        mode: version.mode.clone(),
                        source_kind: fallback_source.clone(),
                        library_state: fallback_state.clone(),
                        workflow_version_id: version.workflow_version_id.clone(),
                        workflow_version: version.workflow_version.clone(),
                        workflow_sha256: version.workflow_sha256.clone(),
                        api_workflow_json: version.api_workflow_json.clone(),
                        recipe_id: recipe.recipe_id,
                        recipe_version: recipe.version,
                        recipe_schema_version: recipe.schema_version,
                        recipe_yaml: recipe.recipe_yaml,
                        recipe_sha256: recipe.recipe_sha256,
                        package_name: version.package_name.clone(),
                        package_source_path: None,
                    });
                }
                continue;
            };
            for recipe in version.recipes {
                let artifacts = match &self.runtime_artifact_repository {
                    Some(repository) => {
                        repository
                            .list_for_recipe(&version.workflow_version_id, &recipe.recipe_id)
                            .await?
                    }
                    None => Vec::new(),
                };
                if artifacts.is_empty() {
                    candidates.push(WorkflowRegistryIdentityCandidate {
                        workflow_id: record.id.clone(),
                        workflow_name: record.name.clone(),
                        category: version.category.clone(),
                        mode: version.mode.clone(),
                        source_kind: record.source_kind.clone(),
                        library_state: record.library_state.clone(),
                        workflow_version_id: version.workflow_version_id.clone(),
                        workflow_version: version.workflow_version.clone(),
                        workflow_sha256: version.workflow_sha256.clone(),
                        api_workflow_json: version.api_workflow_json.clone(),
                        recipe_id: recipe.recipe_id.clone(),
                        recipe_version: recipe.version.clone(),
                        recipe_schema_version: recipe.schema_version,
                        recipe_yaml: recipe.recipe_yaml.clone(),
                        recipe_sha256: recipe.recipe_sha256.clone(),
                        package_name: None,
                        package_source_path: None,
                    });
                } else {
                    for artifact in artifacts {
                        candidates.push(WorkflowRegistryIdentityCandidate {
                            workflow_id: record.id.clone(),
                            workflow_name: record.name.clone(),
                            category: version.category.clone(),
                            mode: version.mode.clone(),
                            source_kind: record.source_kind.clone(),
                            library_state: record.library_state.clone(),
                            workflow_version_id: version.workflow_version_id.clone(),
                            workflow_version: version.workflow_version.clone(),
                            workflow_sha256: version.workflow_sha256.clone(),
                            api_workflow_json: version.api_workflow_json.clone(),
                            recipe_id: recipe.recipe_id.clone(),
                            recipe_version: recipe.version.clone(),
                            recipe_schema_version: recipe.schema_version,
                            recipe_yaml: recipe.recipe_yaml.clone(),
                            recipe_sha256: recipe.recipe_sha256.clone(),
                            package_name: Some(artifact.package_name),
                            package_source_path: artifact.package_source_path,
                        });
                    }
                }
            }
        }
        Ok(candidates)
    }

    pub async fn rename(
        &self,
        workflow_id: &str,
        name: &str,
    ) -> Result<WorkflowRegistryView, WorkflowRegistryServiceError> {
        let Some(repository) = &self.registry_repository else {
            return Err(WorkflowRegistryServiceError::Repository(
                RepositoryError::database("workflow registry repository is not configured"),
            ));
        };
        repository
            .rename(workflow_id, name, self.clock.now())
            .await?
            .ok_or_else(|| {
                WorkflowRegistryServiceError::WorkflowNotFound(workflow_id.to_owned())
            })?;
        self.get(workflow_id).await
    }

    pub async fn set_current_version(
        &self,
        workflow_id: &str,
        workflow_version_id: &str,
    ) -> Result<WorkflowRegistryView, WorkflowRegistryServiceError> {
        let Some(repository) = &self.registry_repository else {
            return Err(WorkflowRegistryServiceError::Repository(
                RepositoryError::database("workflow registry repository is not configured"),
            ));
        };
        repository
            .set_current_version(workflow_id, workflow_version_id, self.clock.now())
            .await?;
        self.get(workflow_id).await
    }

    /// Resolve availability from the exact frozen pair. `current_version` is
    /// intentionally not part of this predicate.
    pub async fn is_available(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
    ) -> Result<bool, WorkflowRegistryServiceError> {
        let (versions, states) = self.load_runtime().await?;
        let Some(version) = versions
            .iter()
            .find(|version| version.workflow_version_id == workflow_version_id)
        else {
            return Ok(false);
        };
        if !version
            .recipes
            .iter()
            .any(|recipe| recipe.recipe_id == recipe_id)
        {
            return Ok(false);
        }

        if let Some(repository) = &self.registry_repository {
            let Some(workflow) = repository.get(&version.workflow_id).await? else {
                return Ok(false);
            };
            if workflow.library_state != WORKFLOW_LIBRARY_ACTIVE {
                return Ok(false);
            }
        }

        let version_state = state_for(&states, workflow_version_id);
        if !version_state.enabled || version_state.archived {
            return Ok(false);
        }

        let Some(artifact_repository) = &self.runtime_artifact_repository else {
            // New production admission is never allowed to infer a runtime
            // package from workflow_versions.package_name.
            return Ok(false);
        };
        let Some(recipe) = version
            .recipes
            .iter()
            .find(|recipe| recipe.recipe_id == recipe_id)
        else {
            return Ok(false);
        };
        let artifacts = artifact_repository
            .list_for_recipe(workflow_version_id, recipe_id)
            .await?;
        let [artifact] = artifacts.as_slice() else {
            return Ok(false);
        };
        if artifact.workflow_sha256 != version.workflow_sha256
            || artifact.recipe_sha256 != recipe.recipe_sha256
        {
            return Ok(false);
        }

        // Production admission must also prove that the canonical artifact
        // still points at a readable, identity-matching package. The optional
        // store is retained for isolated historical fixtures; the production
        // composition root always supplies it.
        if let Some(package_store) = &self.package_store {
            let Ok(package) = package_store.read_runtime(&artifact.package_name).await else {
                return Ok(false);
            };
            let Ok(manifest_yaml) = String::from_utf8(package.manifest_yaml.clone()) else {
                return Ok(false);
            };
            let Ok(manifest) = WorkflowManifest::parse(&manifest_yaml) else {
                return Ok(false);
            };
            if manifest.validate().is_err()
                || manifest.id != version.workflow_id
                || manifest.workflow_version != version.workflow_version
                || manifest.recipe_version != recipe.version
            {
                return Ok(false);
            }
            let Ok(recipe_yaml) = String::from_utf8(package.recipe_yaml.clone()) else {
                return Ok(false);
            };
            let Ok(runtime_recipe) = RecipeParser::parse(&recipe_yaml) else {
                return Ok(false);
            };
            if runtime_recipe.schema_version != recipe.schema_version
                || sha256(package.workflow_api_json.as_slice()) != version.workflow_sha256
                || sha256(package.recipe_yaml.as_slice()) != recipe.recipe_sha256
            {
                return Ok(false);
            }
        }

        let workflow_active = versions
            .iter()
            .filter(|candidate| candidate.workflow_id == version.workflow_id)
            .any(|candidate| !state_for(&states, &candidate.workflow_version_id).archived);
        Ok(workflow_active)
    }

    pub async fn resolve(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
    ) -> Result<Option<WorkflowRegistryRecipeView>, WorkflowRegistryServiceError> {
        if !self.is_available(workflow_version_id, recipe_id).await? {
            return Ok(None);
        }
        let version = self
            .runtime_repository
            .find_version(workflow_version_id)
            .await?
            .ok_or_else(|| WorkflowRegistryServiceError::VersionNotFound {
                workflow_version_id: workflow_version_id.to_owned(),
            })?;
        let Some(recipe) = version
            .recipes
            .iter()
            .find(|recipe| recipe.recipe_id == recipe_id)
        else {
            return Ok(None);
        };
        let mut view = recipe_view(workflow_version_id, recipe);
        if let Some(repository) = &self.runtime_artifact_repository {
            let artifacts = repository
                .list_for_recipe(workflow_version_id, recipe_id)
                .await?;
            match artifacts.as_slice() {
                [artifact] => view.package_name = Some(artifact.package_name.clone()),
                [] => return Ok(None),
                _ => {
                    return Err(WorkflowRegistryServiceError::Repository(
                        RepositoryError::database(
                            "multiple runtime artifacts are registered for the exact recipe",
                        ),
                    ))
                }
            }
        }
        Ok(Some(view))
    }

    /// Normal delete is always logical. It archives the whole logical
    /// Workflow, clears every exact binding, and retains versions/recipes,
    /// runtime records, tasks, batches, and history.
    pub async fn remove_workflow(
        &self,
        workflow_id: &str,
    ) -> Result<WorkflowRegistryMutationResult, WorkflowRegistryServiceError> {
        let _guard = self.lifecycle_gate.lock().await;
        self.remove_workflow_inner(workflow_id).await
    }

    async fn remove_workflow_inner(
        &self,
        workflow_id: &str,
    ) -> Result<WorkflowRegistryMutationResult, WorkflowRegistryServiceError> {
        let (versions, states) = self.load_runtime().await?;
        let versions = versions
            .into_iter()
            .filter(|version| version.workflow_id == workflow_id)
            .collect::<Vec<_>>();
        if versions.is_empty() {
            return Err(WorkflowRegistryServiceError::WorkflowNotFound(
                workflow_id.to_owned(),
            ));
        }

        if let Some(repository) = &self.registry_repository {
            let mut cleared_binding_count = 0;
            for version in &versions {
                cleared_binding_count += self
                    .binding_repository
                    .list_for_workflow_version(&version.workflow_version_id)
                    .await?
                    .len() as u64;
            }
            let record = repository.remove(workflow_id, self.clock.now()).await?;
            let Some(record) = record else {
                return Err(WorkflowRegistryServiceError::WorkflowNotFound(
                    workflow_id.to_owned(),
                ));
            };
            return Ok(WorkflowRegistryMutationResult {
                workflow_id: record.id,
                library_state: record.library_state,
                cleared_binding_count,
                version_count: versions.len() as u64,
            });
        }

        let version_ids = versions
            .iter()
            .map(|version| version.workflow_version_id.clone())
            .collect::<Vec<_>>();
        let counts = self.deletion_counts(&version_ids).await?;
        if counts
            .iter()
            .any(|count| count.active_task_count > 0 || count.active_queue_item_count > 0)
        {
            return Err(WorkflowRegistryServiceError::Blocked(
                "active task or production queue references exist".to_owned(),
            ));
        }

        let state_snapshot = version_ids
            .iter()
            .map(|version_id| (version_id.clone(), states.get(version_id).cloned()))
            .collect::<Vec<_>>();
        let binding_snapshot = self.snapshot_bindings(&version_ids).await?;
        let now = self.clock.now();

        for version_id in &version_ids {
            if let Err(error) = self
                .state_repository
                .set_archived(version_id, true, false, Some(now), now)
                .await
            {
                let compensation = self.compensate(&state_snapshot, &binding_snapshot).await;
                return Err(compensation_or_repository(
                    "workflow state update",
                    error,
                    compensation,
                ));
            }
        }

        let mut cleared_binding_count = 0;
        for version_id in &version_ids {
            match self
                .binding_repository
                .clear_by_workflow_version(version_id)
                .await
            {
                Ok(count) => cleared_binding_count += count,
                Err(error) => {
                    let compensation = self.compensate(&state_snapshot, &binding_snapshot).await;
                    return Err(compensation_or_repository(
                        "project binding cleanup",
                        error,
                        compensation,
                    ));
                }
            }
        }

        Ok(WorkflowRegistryMutationResult {
            workflow_id: workflow_id.to_owned(),
            library_state: WORKFLOW_LIBRARY_REMOVED.to_owned(),
            cleared_binding_count,
            version_count: version_ids.len() as u64,
        })
    }

    pub async fn remove(
        &self,
        workflow_id: &str,
    ) -> Result<WorkflowRegistryMutationResult, WorkflowRegistryServiceError> {
        self.remove_workflow(workflow_id).await
    }

    /// Restores only the logical library state. Bindings are deliberately not
    /// recreated; the user must explicitly bind the exact pair again.
    pub async fn restore_workflow(
        &self,
        workflow_id: &str,
    ) -> Result<WorkflowRegistryRestoreResult, WorkflowRegistryServiceError> {
        let _guard = self.lifecycle_gate.lock().await;
        self.restore_workflow_inner(workflow_id).await
    }

    async fn restore_workflow_inner(
        &self,
        workflow_id: &str,
    ) -> Result<WorkflowRegistryRestoreResult, WorkflowRegistryServiceError> {
        let (all_versions, states) = self.load_runtime().await?;
        let versions = all_versions
            .into_iter()
            .filter(|version| version.workflow_id == workflow_id)
            .collect::<Vec<_>>();
        if versions.is_empty() {
            return Err(WorkflowRegistryServiceError::WorkflowNotFound(
                workflow_id.to_owned(),
            ));
        }

        if let Some(repository) = &self.registry_repository {
            let Some(existing) = repository.get(workflow_id).await? else {
                return Err(WorkflowRegistryServiceError::WorkflowNotFound(
                    workflow_id.to_owned(),
                ));
            };
            if existing.library_state != WORKFLOW_LIBRARY_REMOVED {
                return Err(WorkflowRegistryServiceError::NotRemoved(
                    workflow_id.to_owned(),
                ));
            }
            let restored = repository
                .restore(workflow_id, self.clock.now())
                .await?
                .ok_or_else(|| {
                    WorkflowRegistryServiceError::WorkflowNotFound(workflow_id.to_owned())
                })?;
            let current = restored
                .current_version_id
                .as_deref()
                .and_then(|version_id| {
                    versions
                        .iter()
                        .find(|version| version.workflow_version_id == version_id)
                });
            let enabled = current
                .map(|version| state_for(&states, &version.workflow_version_id).enabled)
                .unwrap_or(true);
            return Ok(WorkflowRegistryRestoreResult {
                workflow_id: restored.id,
                library_state: restored.library_state,
                current_version_id: restored.current_version_id,
                enabled,
                readiness: if enabled {
                    "ACTIVE"
                } else {
                    "RESTORED_NEEDS_ATTENTION"
                }
                .to_owned(),
                capability: "NOT_CHECKED".to_owned(),
                project_binding_count: 0,
            });
        }
        if versions
            .iter()
            .any(|version| !state_for(&states, &version.workflow_version_id).archived)
        {
            return Err(WorkflowRegistryServiceError::NotRemoved(
                workflow_id.to_owned(),
            ));
        }

        let version_ids = versions
            .iter()
            .map(|version| version.workflow_version_id.clone())
            .collect::<Vec<_>>();
        let state_snapshot = version_ids
            .iter()
            .map(|version_id| (version_id.clone(), states.get(version_id).cloned()))
            .collect::<Vec<_>>();
        let now = self.clock.now();
        for version in &versions {
            let previous = state_for(&states, &version.workflow_version_id);
            if let Err(error) = self
                .state_repository
                .set_archived(
                    &version.workflow_version_id,
                    false,
                    previous.enabled,
                    None,
                    now,
                )
                .await
            {
                let empty_bindings: BindingSnapshot = Vec::new();
                let compensation = self.compensate(&state_snapshot, &empty_bindings).await;
                return Err(compensation_or_repository(
                    "workflow restore",
                    error,
                    compensation,
                ));
            }
        }

        let current = versions.iter().find(|version| version.is_current);
        let current_state = current
            .map(|version| state_for(&states, &version.workflow_version_id))
            .unwrap_or_default();
        Ok(WorkflowRegistryRestoreResult {
            workflow_id: workflow_id.to_owned(),
            library_state: WORKFLOW_LIBRARY_ACTIVE.to_owned(),
            current_version_id: current.map(|version| version.workflow_version_id.clone()),
            enabled: current_state.enabled,
            readiness: if current_state.enabled {
                "ACTIVE".to_owned()
            } else {
                "RESTORED_NEEDS_ATTENTION".to_owned()
            },
            capability: "NOT_CHECKED".to_owned(),
            project_binding_count: 0,
        })
    }

    pub async fn restore(
        &self,
        workflow_id: &str,
    ) -> Result<WorkflowRegistryRestoreResult, WorkflowRegistryServiceError> {
        self.restore_workflow(workflow_id).await
    }

    /// Permanent purge is the only hard-delete path. Runtime package
    /// directories are first moved into an operation-scoped quarantine. The
    /// registry transaction is only attempted after the complete reference
    /// preflight succeeds, and every failure restores the moved directories.
    pub async fn purge_workflow(
        &self,
        workflow_id: &str,
    ) -> Result<WorkflowRegistryPurgeResult, WorkflowRegistryServiceError> {
        let _guard = self.lifecycle_gate.lock().await;
        self.purge_workflow_inner(workflow_id).await
    }

    async fn purge_workflow_inner(
        &self,
        workflow_id: &str,
    ) -> Result<WorkflowRegistryPurgeResult, WorkflowRegistryServiceError> {
        let (all_versions, states) = self.load_runtime().await?;
        let versions = all_versions
            .into_iter()
            .filter(|version| version.workflow_id == workflow_id)
            .collect::<Vec<_>>();
        if versions.is_empty() {
            return Err(WorkflowRegistryServiceError::WorkflowNotFound(
                workflow_id.to_owned(),
            ));
        }

        if let Some(repository) = &self.registry_repository {
            let record = repository.get(workflow_id).await?.ok_or_else(|| {
                WorkflowRegistryServiceError::WorkflowNotFound(workflow_id.to_owned())
            })?;
            if record.source_kind != WORKFLOW_SOURCE_USER {
                return Err(WorkflowRegistryServiceError::PurgeBlocked(
                    "PRODUCT workflows can never be purged".to_owned(),
                ));
            }
            if record.library_state != WORKFLOW_LIBRARY_REMOVED {
                return Err(WorkflowRegistryServiceError::PurgeBlocked(
                    "workflow must be REMOVED before purge".to_owned(),
                ));
            }
            if versions
                .iter()
                .any(|version| !state_for(&states, &version.workflow_version_id).archived)
            {
                return Err(WorkflowRegistryServiceError::PurgeBlocked(
                    "every workflow version must be archived before purge".to_owned(),
                ));
            }
            let references = repository
                .inspect_purge(workflow_id)
                .await?
                .ok_or_else(|| {
                    WorkflowRegistryServiceError::WorkflowNotFound(workflow_id.to_owned())
                })?;
            if references.total() > 0 {
                return Err(WorkflowRegistryServiceError::PurgeBlocked(format!(
                    "workflow has references: tasks={}, batches={}, presets={}, templates={}, shots={}, benchmarks={}, bindings={}, stages={}, run_templates={}",
                    references.task_count,
                    references.batch_item_count,
                    references.preset_count,
                    references.template_count,
                    references.shot_config_count,
                    references.benchmark_count,
                    references.binding_count,
                    references.stage_count,
                    references.run_template_count,
                )));
            }
            let version_count = versions.len() as u64;
            let recipe_count = versions
                .iter()
                .map(|version| version.recipes.len() as u64)
                .sum();
            let package_store = self.package_store.as_ref().ok_or_else(|| {
                WorkflowRegistryServiceError::PurgeBlocked(
                    "runtime package store is not configured; purge was not executed".to_owned(),
                )
            })?;
            let package_names = self.runtime_package_names(&versions).await?;
            let operation = WorkflowPurgeOperationRecord {
                schema_version: 1,
                operation_id: format!("purge_{}", Uuid::new_v4()),
                workflow_id: workflow_id.to_owned(),
                package_names: package_names.iter().cloned().collect(),
                created_at: self.clock.now().to_rfc3339(),
            };
            let operation_id = operation.operation_id.clone();
            package_store
                .prepare_purge_operation(&operation)
                .await
                .map_err(|error| WorkflowRegistryServiceError::PurgePackage(error.to_string()))?;
            let mut quarantined = Vec::with_capacity(package_names.len());
            for package_name in &package_names {
                let quarantine = package_store
                    .quarantine_published(&operation_id, package_name)
                    .await;
                match quarantine {
                    Ok(WorkflowPackageQuarantineResult::Quarantined) => {
                        quarantined.push(package_name.clone());
                    }
                    Ok(WorkflowPackageQuarantineResult::AlreadyMissing) => {}
                    Err(error) => {
                        let compensation = self
                            .rollback_purge_operation(package_store, &operation_id, &quarantined)
                            .await;
                        if let Err(compensation) = compensation {
                            return Err(WorkflowRegistryServiceError::PurgeCompensationFailed {
                                operation: operation_id.clone(),
                                cause: error.to_string(),
                                compensation,
                            });
                        }
                        return Err(WorkflowRegistryServiceError::PurgePackage(
                            error.to_string(),
                        ));
                    }
                }
            }

            match repository.purge(workflow_id).await {
                Ok(true) => {}
                Ok(false) => {
                    let compensation = self
                        .rollback_purge_operation(package_store, &operation_id, &quarantined)
                        .await;
                    if let Err(compensation) = compensation {
                        return Err(WorkflowRegistryServiceError::PurgeCompensationFailed {
                            operation: operation_id,
                            cause: "workflow purge did not find the workflow".to_owned(),
                            compensation,
                        });
                    }
                    return Err(WorkflowRegistryServiceError::WorkflowNotFound(
                        workflow_id.to_owned(),
                    ));
                }
                Err(error) => match repository.get(workflow_id).await {
                    Ok(Some(_)) => {
                        let compensation = self
                            .rollback_purge_operation(package_store, &operation_id, &quarantined)
                            .await;
                        if let Err(compensation) = compensation {
                            return Err(WorkflowRegistryServiceError::PurgeCompensationFailed {
                                operation: operation_id,
                                cause: error.to_string(),
                                compensation,
                            });
                        }
                        return Err(error.into());
                    }
                    Ok(None) => {
                        tracing::warn!(
                            workflow_id,
                            operation_id = %operation_id,
                            error = %error,
                            "workflow purge database outcome reported an error after commit"
                        );
                    }
                    Err(recheck_error) => {
                        return Err(WorkflowRegistryServiceError::PurgeCompensationFailed {
                            operation: operation_id,
                            cause: format!(
                                "{error}; database outcome recheck failed: {recheck_error}"
                            ),
                            compensation: "purge journal preserved for startup recovery".to_owned(),
                        });
                    }
                },
            }
            let (cleanup_pending, warning) =
                match package_store.remove_quarantine(&operation_id).await {
                    Ok(()) => (false, None),
                    Err(error) => {
                        tracing::error!(
                            workflow_id,
                            operation_id = %operation_id,
                            error = %error,
                            "workflow purge committed but quarantine cleanup failed"
                        );
                        (
                            true,
                            Some("工作流已永久删除，但临时隔离文件清理未完成。".to_owned()),
                        )
                    }
                };
            return Ok(WorkflowRegistryPurgeResult {
                workflow_id: workflow_id.to_owned(),
                version_count,
                recipe_count,
                committed: true,
                cleanup_pending,
                warning,
            });
        }
        let source_kind = source_kind_for(&versions).as_str();
        if source_kind != WORKFLOW_SOURCE_USER {
            return Err(WorkflowRegistryServiceError::PurgeBlocked(
                "PRODUCT workflows can never be purged".to_owned(),
            ));
        }
        if versions
            .iter()
            .any(|version| !state_for(&states, &version.workflow_version_id).archived)
        {
            return Err(WorkflowRegistryServiceError::PurgeBlocked(
                "workflow must be REMOVED before purge".to_owned(),
            ));
        }

        let version_ids = versions
            .iter()
            .map(|version| version.workflow_version_id.clone())
            .collect::<Vec<_>>();
        let counts = self.deletion_counts(&version_ids).await?;
        let binding_count = self.binding_count(&version_ids).await?;
        if binding_count > 0 || counts.iter().any(has_purge_references) {
            return Err(WorkflowRegistryServiceError::PurgeBlocked(format!(
                "workflow has references: project_bindings={binding_count}"
            )));
        }

        let recipe_count = versions
            .iter()
            .map(|version| version.recipes.len() as u64)
            .sum();
        for version in &versions {
            self.runtime_repository
                .delete_version(
                    &version.workflow_version_id,
                    &version.workflow_id,
                    self.clock.now(),
                )
                .await?;
        }
        Ok(WorkflowRegistryPurgeResult {
            workflow_id: workflow_id.to_owned(),
            version_count: versions.len() as u64,
            recipe_count,
            committed: true,
            cleanup_pending: false,
            warning: None,
        })
    }

    pub async fn purge(
        &self,
        workflow_id: &str,
    ) -> Result<WorkflowRegistryPurgeResult, WorkflowRegistryServiceError> {
        self.purge_workflow(workflow_id).await
    }

    async fn runtime_package_names(
        &self,
        versions: &[RuntimeWorkflowVersionRecord],
    ) -> Result<BTreeSet<String>, WorkflowRegistryServiceError> {
        let Some(repository) = &self.runtime_artifact_repository else {
            return Err(WorkflowRegistryServiceError::PurgeBlocked(
                "runtime artifact repository is not configured; purge was not executed".to_owned(),
            ));
        };
        let Some(package_store) = self.package_store.as_ref() else {
            return Err(WorkflowRegistryServiceError::PurgeBlocked(
                "runtime package store is not configured; purge was not executed".to_owned(),
            ));
        };
        let mut package_names = BTreeSet::new();
        for version in versions {
            for artifact in repository
                .list_for_workflow_version(&version.workflow_version_id)
                .await?
            {
                package_names.insert(artifact.package_name);
            }
            // Keep the compatibility column in the purge set as a safety net
            // for databases upgraded from 028 where the provisional artifact
            // was removed before the first real package sync.
            if let Some(package_name) = version
                .package_name
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty())
            {
                package_names.insert(package_name.to_owned());
            }
        }

        // A package can predate the artifact row (or have been left behind by
        // an interrupted upgrade). Read manifests to discover every package
        // belonging to this logical Workflow before deleting its Registry
        // rows, so the next startup sync cannot resurrect it.
        for package_name in package_store
            .list_published()
            .await
            .map_err(|error| WorkflowRegistryServiceError::PurgePackage(error.to_string()))?
        {
            if package_names.contains(&package_name) {
                continue;
            }
            let Ok(package) = package_store.read_runtime(&package_name).await else {
                continue;
            };
            let Ok(manifest_yaml) = String::from_utf8(package.manifest_yaml) else {
                continue;
            };
            let Ok(manifest) = WorkflowManifest::parse(&manifest_yaml) else {
                continue;
            };
            if versions.iter().any(|version| {
                manifest.id == version.workflow_id
                    && manifest.workflow_version == version.workflow_version
            }) {
                package_names.insert(package_name);
            }
        }
        Ok(package_names)
    }

    async fn runtime_artifact_package_count(
        &self,
        versions: &[RuntimeWorkflowVersionRecord],
    ) -> Result<u64, WorkflowRegistryServiceError> {
        let Some(repository) = &self.runtime_artifact_repository else {
            return Ok(0);
        };
        let mut package_names = BTreeSet::new();
        for version in versions {
            for artifact in repository
                .list_for_workflow_version(&version.workflow_version_id)
                .await?
            {
                package_names.insert(artifact.package_name);
            }
        }
        Ok(package_names.len() as u64)
    }

    async fn restore_quarantined_packages(
        &self,
        package_store: &Arc<dyn WorkflowPackageStore>,
        operation_id: &str,
        package_names: &[String],
    ) -> Result<(), String> {
        let mut failures = Vec::new();
        for package_name in package_names.iter().rev() {
            if let Err(error) = package_store
                .restore_quarantined(operation_id, package_name)
                .await
            {
                failures.push(format!("{package_name}: {error}"));
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("; "))
        }
    }

    async fn rollback_purge_operation(
        &self,
        package_store: &Arc<dyn WorkflowPackageStore>,
        operation_id: &str,
        package_names: &[String],
    ) -> Result<(), String> {
        self.restore_quarantined_packages(package_store, operation_id, package_names)
            .await?;
        package_store
            .remove_quarantine(operation_id)
            .await
            .map_err(|error| error.to_string())
    }

    async fn load_runtime(
        &self,
    ) -> Result<
        (
            Vec<RuntimeWorkflowVersionRecord>,
            HashMap<String, WorkflowRuntimeState>,
        ),
        WorkflowRegistryServiceError,
    > {
        let versions = self.runtime_repository.list_versions().await?;
        let states = self
            .state_repository
            .list_states()
            .await?
            .into_iter()
            .map(|state| (state.workflow_version_id.clone(), state))
            .collect();
        Ok((versions, states))
    }

    async fn build_view(
        &self,
        workflow_id: &str,
        mut versions: Vec<RuntimeWorkflowVersionRecord>,
        states: &HashMap<String, WorkflowRuntimeState>,
        record: Option<&crate::application::ports::WorkflowRegistryRecord>,
    ) -> Result<WorkflowRegistryView, WorkflowRegistryServiceError> {
        versions.sort_by(|left, right| {
            compare_versions(&left.workflow_version, &right.workflow_version)
                .then(left.workflow_version_id.cmp(&right.workflow_version_id))
        });
        let mut version_views = versions
            .iter()
            .map(|version| version_view(version, states))
            .collect::<Vec<_>>();
        if let Some(repository) = &self.runtime_artifact_repository {
            for version in &mut version_views {
                let artifacts = repository
                    .list_for_workflow_version(&version.workflow_version_id)
                    .await?;
                for recipe in &mut version.recipes {
                    let matching = artifacts
                        .iter()
                        .filter(|artifact| artifact.recipe_id == recipe.recipe_id)
                        .collect::<Vec<_>>();
                    recipe.package_name =
                        (matching.len() == 1).then(|| matching[0].package_name.clone());
                }
            }
        }
        let recipes = version_views
            .iter()
            .flat_map(|version| version.recipes.iter().cloned())
            .collect::<Vec<_>>();
        let current_version = record
            .and_then(|record| record.current_version_id.as_deref())
            .and_then(|version_id| {
                version_views
                    .iter()
                    .find(|version| version.workflow_version_id == version_id)
            })
            .or_else(|| version_views.iter().find(|version| version.is_current))
            .cloned();
        let current_recipe = current_version.as_ref().and_then(|version| {
            version
                .recipes
                .iter()
                .max_by(|left, right| compare_versions(&left.version, &right.version))
                .cloned()
        });
        let library_state = record
            .map(|record| {
                if record.library_state == WORKFLOW_LIBRARY_REMOVED {
                    WorkflowLibraryState::Removed
                } else {
                    WorkflowLibraryState::Active
                }
            })
            .unwrap_or_else(|| {
                if versions
                    .iter()
                    .all(|version| state_for(states, &version.workflow_version_id).archived)
                {
                    WorkflowLibraryState::Removed
                } else {
                    WorkflowLibraryState::Active
                }
            });
        let mut project_ids = BTreeSet::new();
        let mut history_count = 0;
        for version in &versions {
            for binding in self
                .binding_repository
                .list_for_workflow_version(&version.workflow_version_id)
                .await?
            {
                project_ids.insert(binding.project_id);
            }
            if let Some(counts) = self
                .runtime_repository
                .inspect_deletion(&version.workflow_version_id)
                .await?
            {
                history_count += counts.historical_task_count
                    + counts.production_batch_item_count
                    + counts.other_reference_count
                    + counts.benchmark_reference_count;
            }
        }
        let first = versions.first().ok_or_else(|| {
            WorkflowRegistryServiceError::WorkflowNotFound(workflow_id.to_owned())
        })?;
        Ok(WorkflowRegistryView {
            workflow_id: workflow_id.to_owned(),
            name: record
                .map(|record| record.name.clone())
                .unwrap_or_else(|| first.name.clone()),
            source_kind: record
                .map(|record| record.source_kind.clone())
                .unwrap_or_else(|| source_kind_for(&versions).as_str().to_owned()),
            library_state: library_state.as_str().to_owned(),
            current_version_id: current_version
                .as_ref()
                .map(|version| version.workflow_version_id.clone()),
            current_version,
            current_recipe,
            versions: version_views,
            recipes,
            project_usage_count: project_ids.len() as u64,
            history_count,
        })
    }

    async fn deletion_counts(
        &self,
        version_ids: &[String],
    ) -> Result<Vec<WorkflowDeletionCounts>, WorkflowRegistryServiceError> {
        let mut counts = Vec::with_capacity(version_ids.len());
        for version_id in version_ids {
            counts.push(
                self.runtime_repository
                    .inspect_deletion(version_id)
                    .await?
                    .unwrap_or_default(),
            );
        }
        Ok(counts)
    }

    async fn binding_count(
        &self,
        version_ids: &[String],
    ) -> Result<u64, WorkflowRegistryServiceError> {
        let mut count = 0;
        for version_id in version_ids {
            count += self
                .binding_repository
                .list_for_workflow_version(version_id)
                .await?
                .len() as u64;
        }
        Ok(count)
    }

    async fn snapshot_bindings(
        &self,
        version_ids: &[String],
    ) -> Result<BindingSnapshot, WorkflowRegistryServiceError> {
        let mut project_ids = BTreeSet::new();
        for version_id in version_ids {
            for binding in self
                .binding_repository
                .list_for_workflow_version(version_id)
                .await?
            {
                project_ids.insert(binding.project_id);
            }
        }
        let mut snapshots = Vec::with_capacity(project_ids.len());
        for project_id in project_ids {
            snapshots.push((
                project_id.clone(),
                self.binding_repository
                    .list_for_project(&project_id)
                    .await?,
            ));
        }
        Ok(snapshots)
    }

    async fn compensate(
        &self,
        states: &[(String, Option<WorkflowRuntimeState>)],
        bindings: &BindingSnapshot,
    ) -> Result<(), String> {
        let mut failures = Vec::new();
        for (version_id, previous) in states {
            let (archived, enabled, archived_at) = previous
                .as_ref()
                .map(|state| (state.archived, state.enabled, state.archived_at))
                .unwrap_or((false, true, None));
            if let Err(error) = self
                .state_repository
                .set_archived(version_id, archived, enabled, archived_at, self.clock.now())
                .await
            {
                failures.push(format!("state {version_id}: {error}"));
            }
        }
        for (project_id, previous) in bindings {
            if let Err(error) = self
                .binding_repository
                .replace_for_project(project_id, previous)
                .await
            {
                failures.push(format!("project {project_id}: {error}"));
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("; "))
        }
    }
}

type BindingSnapshot = Vec<(String, Vec<ProjectWorkflowBindingRecord>)>;

fn compensation_or_repository(
    operation: &str,
    error: RepositoryError,
    compensation: Result<(), String>,
) -> WorkflowRegistryServiceError {
    match compensation {
        Ok(()) => WorkflowRegistryServiceError::Repository(error),
        Err(compensation) => WorkflowRegistryServiceError::CompensationFailed {
            operation: operation.to_owned(),
            cause: error.to_string(),
            compensation,
        },
    }
}

fn purge_recovery_blocked(
    operation: &str,
    reason: impl Into<String>,
) -> WorkflowRegistryServiceError {
    WorkflowRegistryServiceError::PurgeRecoveryBlocked {
        operation: operation.to_owned(),
        reason: reason.into(),
    }
}

fn state_for(
    states: &HashMap<String, WorkflowRuntimeState>,
    workflow_version_id: &str,
) -> VersionState {
    states
        .get(workflow_version_id)
        .map(|state| VersionState {
            enabled: state.enabled,
            archived: state.archived,
        })
        .unwrap_or_default()
}

#[derive(Clone, Copy, Debug)]
struct VersionState {
    enabled: bool,
    archived: bool,
}

impl Default for VersionState {
    fn default() -> Self {
        Self {
            enabled: true,
            archived: false,
        }
    }
}

fn version_view(
    version: &RuntimeWorkflowVersionRecord,
    states: &HashMap<String, WorkflowRuntimeState>,
) -> WorkflowRegistryVersionView {
    let state = state_for(states, &version.workflow_version_id);
    WorkflowRegistryVersionView {
        workflow_version_id: version.workflow_version_id.clone(),
        workflow_id: version.workflow_id.clone(),
        version: version.workflow_version.clone(),
        workflow_sha256: version.workflow_sha256.clone(),
        is_current: version.is_current,
        enabled: state.enabled,
        archived: state.archived,
        recipes: version
            .recipes
            .iter()
            .map(|recipe| recipe_view(&version.workflow_version_id, recipe))
            .collect(),
    }
}

fn recipe_view(
    workflow_version_id: &str,
    recipe: &RuntimeRecipeRecord,
) -> WorkflowRegistryRecipeView {
    WorkflowRegistryRecipeView {
        workflow_version_id: workflow_version_id.to_owned(),
        recipe_id: recipe.recipe_id.clone(),
        version: recipe.version.clone(),
        schema_version: recipe.schema_version,
        recipe_yaml: recipe.recipe_yaml.clone(),
        recipe_sha256: recipe.recipe_sha256.clone(),
        package_name: None,
    }
}

fn source_kind_for(versions: &[RuntimeWorkflowVersionRecord]) -> WorkflowSourceKind {
    // Compatibility-only fallback. Migration 028's source_kind column must
    // replace this package-name check once the Registry repository lands.
    if versions.iter().any(|version| {
        version
            .package_name
            .as_deref()
            .is_some_and(is_builtin_package_name)
    }) {
        WorkflowSourceKind::Product
    } else {
        WorkflowSourceKind::User
    }
}

fn has_purge_references(counts: &WorkflowDeletionCounts) -> bool {
    counts.active_task_count > 0
        || counts.active_queue_item_count > 0
        || counts.historical_task_count > 0
        || counts.production_batch_item_count > 0
        || counts.other_reference_count > 0
        || counts.benchmark_reference_count > 0
}

fn append_purge_reference_reasons(
    references: &crate::application::ports::WorkflowPurgeReferenceCounts,
    reasons: &mut Vec<String>,
) {
    let reference_labels = [
        (references.task_count, "任务"),
        (references.batch_item_count, "生产批次条目"),
        (references.preset_count, "预设"),
        (references.template_count, "项目模板"),
        (references.shot_config_count, "镜头阶段配置"),
        (references.benchmark_count, "基准候选"),
        (references.binding_count, "项目工作流绑定"),
        (references.stage_count, "生产阶段"),
        (references.run_template_count, "生产运行模板"),
    ];
    for (count, label) in reference_labels {
        if count > 0 {
            reasons.push(format!("仍有 {count} 个{label}引用"));
        }
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
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

#[derive(Debug)]
pub enum WorkflowRegistryServiceError {
    Repository(RepositoryError),
    WorkflowNotFound(String),
    VersionNotFound {
        workflow_version_id: String,
    },
    Blocked(String),
    NotRemoved(String),
    PurgeBlocked(String),
    PurgePackage(String),
    PurgeCompensationFailed {
        operation: String,
        cause: String,
        compensation: String,
    },
    PurgeRecoveryBlocked {
        operation: String,
        reason: String,
    },
    CompensationFailed {
        operation: String,
        cause: String,
        compensation: String,
    },
}

impl WorkflowRegistryServiceError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Repository(_) => "WORKFLOW_REGISTRY_REPOSITORY_ERROR",
            Self::WorkflowNotFound(_) => "WORKFLOW_NOT_FOUND",
            Self::VersionNotFound { .. } => "WORKFLOW_VERSION_NOT_FOUND",
            Self::Blocked(_) => "WORKFLOW_DELETE_BLOCKED_ACTIVE_TASKS",
            Self::NotRemoved(_) => "WORKFLOW_NOT_REMOVED",
            Self::PurgeBlocked(_) => "WORKFLOW_PURGE_BLOCKED",
            Self::PurgePackage(_) => "WORKFLOW_PURGE_PACKAGE_ERROR",
            Self::PurgeCompensationFailed { .. } => "WORKFLOW_PURGE_COMPENSATION_FAILED",
            Self::PurgeRecoveryBlocked { .. } => "WORKFLOW_PURGE_RECOVERY_BLOCKED",
            Self::CompensationFailed { .. } => "WORKFLOW_REGISTRY_COMPENSATION_FAILED",
        }
    }
}

impl fmt::Display for WorkflowRegistryServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Repository(error) => error.fmt(formatter),
            Self::WorkflowNotFound(workflow_id) => {
                write!(formatter, "WORKFLOW_NOT_FOUND: workflow {workflow_id} was not found")
            }
            Self::VersionNotFound {
                workflow_version_id,
            } => write!(
                formatter,
                "WORKFLOW_VERSION_NOT_FOUND: workflow version {workflow_version_id} was not found"
            ),
            Self::Blocked(message) => write!(formatter, "WORKFLOW_DELETE_BLOCKED: {message}"),
            Self::NotRemoved(workflow_id) => write!(
                formatter,
                "WORKFLOW_NOT_REMOVED: workflow {workflow_id} is already active"
            ),
            Self::PurgeBlocked(message) => write!(formatter, "WORKFLOW_PURGE_BLOCKED: {message}"),
            Self::PurgePackage(message) => {
                write!(formatter, "WORKFLOW_PURGE_PACKAGE_ERROR: {message}")
            }
            Self::PurgeCompensationFailed {
                operation,
                cause,
                compensation,
            } => write!(
                formatter,
                "WORKFLOW_PURGE_COMPENSATION_FAILED: {operation} failed ({cause}); compensation failed ({compensation})"
            ),
            Self::PurgeRecoveryBlocked { operation, reason } => write!(
                formatter,
                "WORKFLOW_PURGE_RECOVERY_BLOCKED: {operation} is unresolved ({reason})"
            ),
            Self::CompensationFailed {
                operation,
                cause,
                compensation,
            } => write!(
                formatter,
                "WORKFLOW_REGISTRY_COMPENSATION_FAILED: {operation} failed ({cause}); compensation failed ({compensation})"
            ),
        }
    }
}

impl Error for WorkflowRegistryServiceError {}

impl From<RepositoryError> for WorkflowRegistryServiceError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        compare_versions, WorkflowLibraryState, WorkflowSourceKind, WORKFLOW_LIBRARY_ACTIVE,
        WORKFLOW_LIBRARY_REMOVED, WORKFLOW_SOURCE_PRODUCT, WORKFLOW_SOURCE_USER,
    };

    #[test]
    fn registry_constants_and_version_order_are_stable() {
        assert_eq!(
            WorkflowSourceKind::Product.as_str(),
            WORKFLOW_SOURCE_PRODUCT
        );
        assert_eq!(WorkflowSourceKind::User.as_str(), WORKFLOW_SOURCE_USER);
        assert_eq!(
            WorkflowLibraryState::Active.as_str(),
            WORKFLOW_LIBRARY_ACTIVE
        );
        assert_eq!(
            WorkflowLibraryState::Removed.as_str(),
            WORKFLOW_LIBRARY_REMOVED
        );
        assert!(compare_versions("1.10.0", "1.9.0").is_gt());
    }
}
