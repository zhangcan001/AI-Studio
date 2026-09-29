use crate::application::ports::{
    Clock, ProjectRepository, ProjectWorkflowBindingRecord, ProjectWorkflowBindingRepository,
    RepositoryError, WorkflowRuntimeRepository, WorkflowRuntimeStateRepository,
};
use crate::application::workflow_registry_service::WorkflowRegistryService;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, error::Error, fmt, sync::Arc};
use uuid::Uuid;

pub const IMAGE_STAGE: &str = "IMAGE";
pub const VIDEO_STAGE: &str = "VIDEO";
pub const DEFAULT_MODE: &str = "DEFAULT";
pub const PROJECT_WORKFLOW_BINDING_REVISION_CONFLICT: &str =
    "PROJECT_WORKFLOW_BINDING_REVISION_CONFLICT";

pub const VIDEO_MODES: [&str; 7] = [
    "FL2VA_TEXT_TO_VIDEO",
    "FL2VA_IMAGE_TO_VIDEO",
    "FL2VA_FIRST_LAST",
    "REF2VA_IMAGE",
    "REF2VA_AUDIO",
    "REF2VA_IMAGE_AUDIO",
    "REF2VA_VIDEO_IMAGE",
];

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkflowBindingInput {
    pub stage: String,
    pub mode: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkflowBindingView {
    pub stage: String,
    pub mode: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub binding_instance_id: String,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub available: bool,
    pub availability_reasons: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkflowBindingUpsertRequest {
    pub stage: String,
    pub mode: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    #[serde(default)]
    pub expected_binding_instance_id: Option<String>,
    #[serde(default)]
    pub expected_revision: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkflowBindingRemoveRequest {
    pub stage: String,
    pub mode: String,
    pub expected_binding_instance_id: Option<String>,
    pub expected_revision: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkflowConfigUpdateRequest {
    #[serde(default)]
    pub bindings: Vec<ProjectWorkflowBindingInput>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkflowConfigView {
    pub project_id: String,
    pub image_default: Option<ProjectWorkflowBindingView>,
    pub video_default: Option<ProjectWorkflowBindingView>,
    pub video_mode_overrides: Vec<ProjectWorkflowBindingView>,
}

pub struct ProjectWorkflowBindingService {
    binding_repository: Arc<dyn ProjectWorkflowBindingRepository>,
    project_repository: Arc<dyn ProjectRepository>,
    runtime_repository: Arc<dyn WorkflowRuntimeRepository>,
    runtime_state_repository: Arc<dyn WorkflowRuntimeStateRepository>,
    clock: Arc<dyn Clock>,
    registry: Option<Arc<WorkflowRegistryService>>,
}

impl ProjectWorkflowBindingService {
    pub fn new(
        binding_repository: Arc<dyn ProjectWorkflowBindingRepository>,
        project_repository: Arc<dyn ProjectRepository>,
        runtime_repository: Arc<dyn WorkflowRuntimeRepository>,
        runtime_state_repository: Arc<dyn WorkflowRuntimeStateRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            binding_repository,
            project_repository,
            runtime_repository,
            runtime_state_repository,
            clock,
            registry: None,
        }
    }

    /// Lets the V2 Registry own logical Workflow state while keeping the
    /// legacy constructor source-compatible during the strangler cutover.
    pub fn with_registry(mut self, registry: Arc<WorkflowRegistryService>) -> Self {
        self.registry = Some(registry);
        self
    }

    pub async fn get(
        &self,
        project_id: &str,
    ) -> Result<ProjectWorkflowConfigView, ProjectWorkflowBindingServiceError> {
        self.ensure_project(project_id).await?;
        let bindings = self.binding_repository.list_for_project(project_id).await?;
        let mut views = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let availability = self.inspect_availability(&binding).await?;
            views.push(ProjectWorkflowBindingView {
                stage: binding.stage,
                mode: binding.mode,
                workflow_version_id: binding.workflow_version_id,
                recipe_id: binding.recipe_id,
                binding_instance_id: binding.binding_instance_id,
                revision: binding.revision,
                created_at: binding.created_at,
                updated_at: binding.updated_at,
                available: availability.available,
                availability_reasons: availability.reasons,
            });
        }
        Ok(config_from_bindings(project_id, views))
    }

    pub async fn upsert(
        &self,
        project_id: &str,
        request: ProjectWorkflowBindingUpsertRequest,
    ) -> Result<ProjectWorkflowConfigView, ProjectWorkflowBindingServiceError> {
        self.ensure_project(project_id).await?;
        let mut keys = HashSet::new();
        let (stage, mode, workflow_version_id, recipe_id) = self
            .validate_binding_input(
                project_id,
                request.stage,
                request.mode,
                request.workflow_version_id,
                request.recipe_id,
                &mut keys,
            )
            .await?;
        let expected = match (
            request.expected_binding_instance_id,
            request.expected_revision,
        ) {
            (None, None) => None,
            (Some(instance_id), Some(revision)) if !instance_id.trim().is_empty() && revision > 0 => {
                Some((instance_id, revision))
            }
            _ => {
                return Err(ProjectWorkflowBindingServiceError::Invalid(
                    "PROJECT_WORKFLOW_BINDING_EXPECTED_TOKEN: expectedBindingInstanceId and expectedRevision must be provided together"
                        .to_owned(),
                ))
            }
        };
        let now = self.clock.now();
        match expected {
            None => {
                if self
                    .binding_repository
                    .find_slot(project_id, &stage, &mode)
                    .await?
                    .is_some()
                {
                    return Err(self
                        .revision_conflict(project_id, &stage, &mode, None, None)
                        .await?);
                }
                let binding = ProjectWorkflowBindingRecord {
                    project_id: project_id.to_owned(),
                    stage,
                    mode,
                    workflow_version_id,
                    recipe_id,
                    binding_instance_id: new_binding_instance_id(),
                    revision: 1,
                    created_at: now,
                    updated_at: now,
                };
                match self.binding_repository.insert_slot(&binding).await {
                    Ok(()) => {}
                    Err(error) if is_unique_constraint(&error) => {
                        return Err(self
                            .revision_conflict(
                                project_id,
                                &binding.stage,
                                &binding.mode,
                                None,
                                None,
                            )
                            .await?);
                    }
                    Err(error) => {
                        return Err(ProjectWorkflowBindingServiceError::Repository(error))
                    }
                }
            }
            Some((instance_id, revision)) => {
                let affected = self
                    .binding_repository
                    .update_slot(
                        project_id,
                        &stage,
                        &mode,
                        &instance_id,
                        revision,
                        &workflow_version_id,
                        &recipe_id,
                        now,
                    )
                    .await?;
                if affected != 1 {
                    return Err(self
                        .revision_conflict(
                            project_id,
                            &stage,
                            &mode,
                            Some(instance_id),
                            Some(revision),
                        )
                        .await?);
                }
            }
        }
        self.get(project_id).await
    }

    pub async fn remove(
        &self,
        project_id: &str,
        request: ProjectWorkflowBindingRemoveRequest,
    ) -> Result<ProjectWorkflowConfigView, ProjectWorkflowBindingServiceError> {
        self.ensure_project(project_id).await?;
        let stage = request.stage.trim().to_owned();
        let mode = request.mode.trim().to_owned();
        let mut keys = HashSet::new();
        validate_binding_shape(&stage, &mode, "placeholder", "placeholder", &mut keys)?;
        let (Some(instance_id), Some(revision)) = (
            request.expected_binding_instance_id,
            request.expected_revision,
        ) else {
            return Err(ProjectWorkflowBindingServiceError::Invalid(
                "PROJECT_WORKFLOW_BINDING_EXPECTED_TOKEN: remove requires expectedBindingInstanceId and expectedRevision"
                    .to_owned(),
            ));
        };
        if instance_id.trim().is_empty() || revision < 1 {
            return Err(ProjectWorkflowBindingServiceError::Invalid(
                "PROJECT_WORKFLOW_BINDING_EXPECTED_TOKEN: remove token is invalid".to_owned(),
            ));
        }
        let affected = self
            .binding_repository
            .delete_slot(project_id, &stage, &mode, &instance_id, revision)
            .await?;
        if affected != 1 {
            return Err(self
                .revision_conflict(project_id, &stage, &mode, Some(instance_id), Some(revision))
                .await?);
        }
        self.get(project_id).await
    }

    pub async fn replace(
        &self,
        project_id: &str,
        request: ProjectWorkflowConfigUpdateRequest,
    ) -> Result<ProjectWorkflowConfigView, ProjectWorkflowBindingServiceError> {
        self.ensure_project(project_id).await?;
        let mut keys = HashSet::new();
        let now = self.clock.now();
        let mut records = Vec::with_capacity(request.bindings.len());

        for input in request.bindings {
            let stage = input.stage.trim().to_owned();
            let mode = input.mode.trim().to_owned();
            let workflow_version_id = input.workflow_version_id.trim().to_owned();
            let recipe_id = input.recipe_id.trim().to_owned();
            validate_binding_shape(&stage, &mode, &workflow_version_id, &recipe_id, &mut keys)?;

            let version = self
                .runtime_repository
                .find_version(&workflow_version_id)
                .await?
                .ok_or_else(|| {
                    ProjectWorkflowBindingServiceError::Invalid(format!(
                        "PROJECT_WORKFLOW_WORKFLOW_NOT_FOUND: workflow version {workflow_version_id} was not found"
                    ))
                })?;
            if !version
                .recipes
                .iter()
                .any(|recipe| recipe.recipe_id == recipe_id)
            {
                let recipe_exists_elsewhere = self
                    .runtime_repository
                    .list_versions()
                    .await?
                    .into_iter()
                    .any(|candidate| {
                        candidate
                            .recipes
                            .iter()
                            .any(|recipe| recipe.recipe_id == recipe_id)
                    });
                let code = if recipe_exists_elsewhere {
                    "PROJECT_WORKFLOW_RECIPE_MISMATCH"
                } else {
                    "PROJECT_WORKFLOW_RECIPE_NOT_FOUND"
                };
                return Err(ProjectWorkflowBindingServiceError::Invalid(format!(
                    "{code}: recipe {recipe_id} is not available in workflow version {workflow_version_id}"
                )));
            }

            if !self
                .is_workflow_available_for_recipe(&workflow_version_id, &recipe_id)
                .await?
            {
                return Err(ProjectWorkflowBindingServiceError::Invalid(format!(
                    "PROJECT_WORKFLOW_WORKFLOW_UNAVAILABLE: workflow version {workflow_version_id} is unavailable"
                )));
            }

            records.push(ProjectWorkflowBindingRecord {
                project_id: project_id.to_owned(),
                stage,
                mode,
                workflow_version_id,
                recipe_id,
                binding_instance_id: new_binding_instance_id(),
                revision: 1,
                created_at: now,
                updated_at: now,
            });
        }

        self.binding_repository
            .replace_for_project(project_id, &records)
            .await?;
        self.get(project_id).await
    }

    pub async fn workflow_version_binding_summary(
        &self,
        workflow_version_id: &str,
    ) -> Result<(u64, Vec<String>), ProjectWorkflowBindingServiceError> {
        let bindings = self
            .binding_repository
            .list_for_workflow_version(workflow_version_id)
            .await?;
        let mut scopes = bindings
            .iter()
            .map(|binding| format!("{} {}", binding.stage, binding.mode))
            .collect::<Vec<_>>();
        scopes.sort();
        scopes.dedup();
        Ok((bindings.len() as u64, scopes))
    }

    pub async fn clear_by_workflow_version(
        &self,
        workflow_version_id: &str,
    ) -> Result<u64, ProjectWorkflowBindingServiceError> {
        Ok(self
            .binding_repository
            .clear_by_workflow_version(workflow_version_id)
            .await?)
    }

    pub async fn is_workflow_available_for_recipe(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
    ) -> Result<bool, ProjectWorkflowBindingServiceError> {
        Ok(self
            .inspect_workflow_availability(workflow_version_id, recipe_id)
            .await?
            .available)
    }

    async fn inspect_workflow_availability(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
    ) -> Result<BindingAvailabilityInspection, ProjectWorkflowBindingServiceError> {
        if let Some(registry) = &self.registry {
            let inspection = registry
                .inspect_availability(workflow_version_id, recipe_id)
                .await
                .map_err(|error| ProjectWorkflowBindingServiceError::Registry(error.to_string()))?;
            return Ok(BindingAvailabilityInspection {
                available: inspection.available,
                reasons: inspection.reasons,
            });
        }

        let Some(version) = self
            .runtime_repository
            .find_version(workflow_version_id)
            .await?
        else {
            return Ok(BindingAvailabilityInspection::unavailable(
                "WORKFLOW_VERSION_NOT_FOUND",
            ));
        };
        if !version
            .recipes
            .iter()
            .any(|recipe| recipe.recipe_id == recipe_id)
        {
            return Ok(BindingAvailabilityInspection::unavailable(
                "RECIPE_NOT_FOUND",
            ));
        }
        let state_available = self
            .runtime_state_repository
            .find_state(workflow_version_id)
            .await?
            .is_none_or(|state| state.enabled && !state.archived);
        if !state_available {
            return Ok(BindingAvailabilityInspection::unavailable(
                "VERSION_DISABLED",
            ));
        }

        // Migration 028 will persist library_state on workflows. Until that
        // port exists, an all-archived logical Workflow is the legacy REMOVED
        // representation; a non-current version remains available otherwise.
        let versions = self.runtime_repository.list_versions().await?;
        let states = self.runtime_state_repository.list_states().await?;
        let available = versions
            .iter()
            .filter(|candidate| candidate.workflow_id == version.workflow_id)
            .any(|candidate| {
                states
                    .iter()
                    .find(|state| state.workflow_version_id == candidate.workflow_version_id)
                    .is_none_or(|state| !state.archived)
            });
        Ok(BindingAvailabilityInspection {
            available,
            reasons: if available {
                Vec::new()
            } else {
                vec!["WORKFLOW_REMOVED".to_owned()]
            },
        })
    }

    async fn ensure_project(
        &self,
        project_id: &str,
    ) -> Result<(), ProjectWorkflowBindingServiceError> {
        if self
            .project_repository
            .find_by_id(project_id)
            .await?
            .is_none()
        {
            return Err(ProjectWorkflowBindingServiceError::ProjectNotFound(
                project_id.to_owned(),
            ));
        }
        Ok(())
    }

    async fn inspect_availability(
        &self,
        binding: &ProjectWorkflowBindingRecord,
    ) -> Result<BindingAvailabilityInspection, ProjectWorkflowBindingServiceError> {
        self.inspect_workflow_availability(&binding.workflow_version_id, &binding.recipe_id)
            .await
    }

    async fn validate_binding_input(
        &self,
        _project_id: &str,
        stage: String,
        mode: String,
        workflow_version_id: String,
        recipe_id: String,
        keys: &mut HashSet<(String, String)>,
    ) -> Result<(String, String, String, String), ProjectWorkflowBindingServiceError> {
        let stage = stage.trim().to_owned();
        let mode = mode.trim().to_owned();
        let workflow_version_id = workflow_version_id.trim().to_owned();
        let recipe_id = recipe_id.trim().to_owned();
        validate_binding_shape(&stage, &mode, &workflow_version_id, &recipe_id, keys)?;
        let version = self
            .runtime_repository
            .find_version(&workflow_version_id)
            .await?
            .ok_or_else(|| {
                ProjectWorkflowBindingServiceError::Invalid(format!(
                    "PROJECT_WORKFLOW_WORKFLOW_NOT_FOUND: workflow version {workflow_version_id} was not found"
                ))
            })?;
        if !version
            .recipes
            .iter()
            .any(|recipe| recipe.recipe_id == recipe_id)
        {
            let recipe_exists_elsewhere = self
                .runtime_repository
                .list_versions()
                .await?
                .into_iter()
                .any(|candidate| {
                    candidate
                        .recipes
                        .iter()
                        .any(|recipe| recipe.recipe_id == recipe_id)
                });
            let code = if recipe_exists_elsewhere {
                "PROJECT_WORKFLOW_RECIPE_MISMATCH"
            } else {
                "PROJECT_WORKFLOW_RECIPE_NOT_FOUND"
            };
            return Err(ProjectWorkflowBindingServiceError::Invalid(format!(
                "{code}: recipe {recipe_id} is not available in workflow version {workflow_version_id}"
            )));
        }
        let availability = self
            .inspect_workflow_availability(&workflow_version_id, &recipe_id)
            .await?;
        if !availability.available {
            return Err(ProjectWorkflowBindingServiceError::Invalid(format!(
                "PROJECT_WORKFLOW_WORKFLOW_UNAVAILABLE: workflow version {workflow_version_id} is unavailable ({})",
                availability.reasons.join(", ")
            )));
        }
        Ok((stage, mode, workflow_version_id, recipe_id))
    }

    async fn revision_conflict(
        &self,
        project_id: &str,
        stage: &str,
        mode: &str,
        expected_binding_instance_id: Option<String>,
        expected_revision: Option<i64>,
    ) -> Result<ProjectWorkflowBindingServiceError, ProjectWorkflowBindingServiceError> {
        let current = self
            .binding_repository
            .find_slot(project_id, stage, mode)
            .await?;
        Ok(ProjectWorkflowBindingServiceError::Conflict(
            self.revision_conflict_details(
                project_id,
                stage,
                mode,
                expected_binding_instance_id,
                expected_revision,
                current,
            ),
        ))
    }

    fn revision_conflict_details(
        &self,
        project_id: &str,
        stage: &str,
        mode: &str,
        expected_binding_instance_id: Option<String>,
        expected_revision: Option<i64>,
        current: Option<ProjectWorkflowBindingRecord>,
    ) -> ProjectWorkflowBindingConflict {
        ProjectWorkflowBindingConflict {
            project_id: project_id.to_owned(),
            stage: stage.to_owned(),
            mode: mode.to_owned(),
            expected_binding_instance_id,
            expected_revision,
            current_binding_instance_id: current
                .as_ref()
                .map(|value| value.binding_instance_id.clone()),
            current_revision: current.as_ref().map(|value| value.revision),
            current_workflow_version_id: current
                .as_ref()
                .map(|value| value.workflow_version_id.clone()),
            current_recipe_id: current.map(|value| value.recipe_id),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindingAvailabilityInspection {
    pub available: bool,
    pub reasons: Vec<String>,
}

impl BindingAvailabilityInspection {
    fn unavailable(reason: &str) -> Self {
        Self {
            available: false,
            reasons: vec![reason.to_owned()],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectWorkflowBindingConflict {
    pub project_id: String,
    pub stage: String,
    pub mode: String,
    pub expected_binding_instance_id: Option<String>,
    pub expected_revision: Option<i64>,
    pub current_binding_instance_id: Option<String>,
    pub current_revision: Option<i64>,
    pub current_workflow_version_id: Option<String>,
    pub current_recipe_id: Option<String>,
}

fn new_binding_instance_id() -> String {
    format!("bnd_{}", Uuid::new_v4().simple())
}

fn is_unique_constraint(error: &RepositoryError) -> bool {
    match error {
        RepositoryError::Database { message } | RepositoryError::Integrity { message } => {
            message.to_ascii_lowercase().contains("unique")
        }
        _ => false,
    }
}

fn validate_binding_shape(
    stage: &str,
    mode: &str,
    workflow_version_id: &str,
    recipe_id: &str,
    keys: &mut HashSet<(String, String)>,
) -> Result<(), ProjectWorkflowBindingServiceError> {
    if !matches!(stage, IMAGE_STAGE | VIDEO_STAGE) {
        return Err(ProjectWorkflowBindingServiceError::Invalid(format!(
            "PROJECT_WORKFLOW_INVALID_STAGE: stage {stage} is not supported"
        )));
    }
    if mode != DEFAULT_MODE && !VIDEO_MODES.contains(&mode) {
        return Err(ProjectWorkflowBindingServiceError::Invalid(format!(
            "PROJECT_WORKFLOW_INVALID_MODE: mode {mode} is not supported"
        )));
    }
    if stage == IMAGE_STAGE && mode != DEFAULT_MODE {
        return Err(ProjectWorkflowBindingServiceError::Invalid(
            "PROJECT_WORKFLOW_IMAGE_MODE_INVALID: image stage only supports DEFAULT".to_owned(),
        ));
    }
    if workflow_version_id.is_empty() || recipe_id.is_empty() {
        return Err(ProjectWorkflowBindingServiceError::Invalid(
            "PROJECT_WORKFLOW_EMPTY_REFERENCE: workflowVersionId and recipeId are required"
                .to_owned(),
        ));
    }
    if !keys.insert((stage.to_owned(), mode.to_owned())) {
        return Err(ProjectWorkflowBindingServiceError::Invalid(format!(
            "PROJECT_WORKFLOW_DUPLICATE_BINDING: binding {stage}/{mode} is duplicated"
        )));
    }
    Ok(())
}

fn config_from_bindings(
    project_id: &str,
    bindings: Vec<ProjectWorkflowBindingView>,
) -> ProjectWorkflowConfigView {
    let image_default = bindings
        .iter()
        .find(|binding| binding.stage == IMAGE_STAGE && binding.mode == DEFAULT_MODE)
        .cloned();
    let video_default = bindings
        .iter()
        .find(|binding| binding.stage == VIDEO_STAGE && binding.mode == DEFAULT_MODE)
        .cloned();
    let video_mode_overrides = bindings
        .into_iter()
        .filter(|binding| binding.stage == VIDEO_STAGE && binding.mode != DEFAULT_MODE)
        .collect();
    ProjectWorkflowConfigView {
        project_id: project_id.to_owned(),
        image_default,
        video_default,
        video_mode_overrides,
    }
}

#[derive(Debug)]
pub enum ProjectWorkflowBindingServiceError {
    ProjectNotFound(String),
    Invalid(String),
    Conflict(ProjectWorkflowBindingConflict),
    Registry(String),
    Repository(RepositoryError),
}

impl fmt::Display for ProjectWorkflowBindingServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectNotFound(project_id) => {
                write!(
                    formatter,
                    "PROJECT_NOT_FOUND: project {project_id} was not found"
                )
            }
            Self::Invalid(message) => formatter.write_str(message),
            Self::Conflict(_) => formatter.write_str(PROJECT_WORKFLOW_BINDING_REVISION_CONFLICT),
            Self::Registry(message) => write!(formatter, "WORKFLOW_REGISTRY_ERROR: {message}"),
            Self::Repository(error) => error.fmt(formatter),
        }
    }
}

impl Error for ProjectWorkflowBindingServiceError {}

impl From<RepositoryError> for ProjectWorkflowBindingServiceError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ProjectWorkflowBindingInput, ProjectWorkflowBindingService,
        ProjectWorkflowBindingServiceError, ProjectWorkflowConfigUpdateRequest,
    };
    use crate::application::ports::{
        Clock, ProjectRepository, ProjectWorkflowBindingRepository, WorkflowRuntimeRepository,
        WorkflowRuntimeStateRepository,
    };
    use crate::infrastructure::database::{
        initialize, repositories::test_support, SqliteProjectRepository,
        SqliteProjectWorkflowBindingRepository, SqliteWorkflowRuntimeRepository,
        SqliteWorkflowRuntimeStateRepository,
    };
    use chrono::{DateTime, Utc};
    use std::sync::Arc;
    use tempfile::tempdir;

    struct FixedClock;
    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            "2026-01-01T00:00:00Z".parse().unwrap()
        }
    }

    /// W-31: keep the TempDir alive for the whole test; the service derefs
    /// so existing call sites stay unchanged.
    struct Fixture {
        _dir: tempfile::TempDir,
        service: ProjectWorkflowBindingService,
    }

    impl std::ops::Deref for Fixture {
        type Target = ProjectWorkflowBindingService;
        fn deref(&self) -> &Self::Target {
            &self.service
        }
    }

    async fn setup() -> Fixture {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        let project_repository: Arc<dyn ProjectRepository> =
            Arc::new(SqliteProjectRepository::new(pool.clone()));
        let binding_repository: Arc<dyn ProjectWorkflowBindingRepository> =
            Arc::new(SqliteProjectWorkflowBindingRepository::new(pool.clone()));
        let runtime_repository: Arc<dyn WorkflowRuntimeRepository> =
            Arc::new(SqliteWorkflowRuntimeRepository::new(pool.clone()));
        let state_repository: Arc<dyn WorkflowRuntimeStateRepository> =
            Arc::new(SqliteWorkflowRuntimeStateRepository::new(pool));
        Fixture {
            _dir: directory,
            service: ProjectWorkflowBindingService::new(
                binding_repository,
                project_repository,
                runtime_repository,
                state_repository,
                Arc::new(FixedClock),
            ),
        }
    }

    fn request(stage: &str, mode: &str) -> ProjectWorkflowConfigUpdateRequest {
        ProjectWorkflowConfigUpdateRequest {
            bindings: vec![ProjectWorkflowBindingInput {
                stage: stage.to_owned(),
                mode: mode.to_owned(),
                workflow_version_id: "workflow-version-1".to_owned(),
                recipe_id: "recipe-1".to_owned(),
            }],
        }
    }

    #[tokio::test]
    async fn rejects_invalid_and_duplicate_bindings_before_replacement() {
        let service = setup().await;
        let error = service
            .replace("project-1", request("IMAGE", "VIDEO"))
            .await
            .unwrap_err();
        assert!(error
            .to_string()
            .starts_with("PROJECT_WORKFLOW_INVALID_MODE"));

        let error = service
            .replace(
                "project-1",
                ProjectWorkflowConfigUpdateRequest {
                    bindings: vec![
                        ProjectWorkflowBindingInput {
                            stage: "VIDEO".to_owned(),
                            mode: "DEFAULT".to_owned(),
                            workflow_version_id: "workflow-version-1".to_owned(),
                            recipe_id: "recipe-1".to_owned(),
                        },
                        ProjectWorkflowBindingInput {
                            stage: "VIDEO".to_owned(),
                            mode: "DEFAULT".to_owned(),
                            workflow_version_id: "workflow-version-1".to_owned(),
                            recipe_id: "recipe-1".to_owned(),
                        },
                    ],
                },
            )
            .await
            .unwrap_err();
        assert!(error
            .to_string()
            .starts_with("PROJECT_WORKFLOW_DUPLICATE_BINDING"));
    }

    #[tokio::test]
    async fn replaces_and_reports_current_binding_availability() {
        let service = setup().await;
        let config = service
            .replace("project-1", request("IMAGE", "DEFAULT"))
            .await
            .unwrap();
        assert_eq!(config.image_default.as_ref().unwrap().recipe_id, "recipe-1");
        assert!(config.image_default.as_ref().unwrap().available);
        assert!(config.video_default.is_none());
    }

    #[tokio::test]
    async fn rejects_missing_workflow_and_recipe_mismatch() {
        let service = setup().await;
        let mut missing = request("VIDEO", "DEFAULT");
        missing.bindings[0].workflow_version_id = "missing".to_owned();
        let error = service.replace("project-1", missing).await.unwrap_err();
        assert!(error
            .to_string()
            .starts_with("PROJECT_WORKFLOW_WORKFLOW_NOT_FOUND"));

        let mut missing_recipe = request("VIDEO", "DEFAULT");
        missing_recipe.bindings[0].recipe_id = "missing".to_owned();
        let error = service
            .replace("project-1", missing_recipe)
            .await
            .unwrap_err();
        assert!(error
            .to_string()
            .starts_with("PROJECT_WORKFLOW_RECIPE_NOT_FOUND"));
    }

    #[test]
    fn service_error_keeps_project_not_found_stable() {
        let error = ProjectWorkflowBindingServiceError::ProjectNotFound("p".to_owned());
        assert_eq!(
            error.to_string(),
            "PROJECT_NOT_FOUND: project p was not found"
        );
    }
}
