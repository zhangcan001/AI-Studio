use crate::application::ports::{
    Clock, ProjectRepository, ProjectWorkflowBindingRecord, ProjectWorkflowBindingRepository,
    RepositoryError, WorkflowRuntimeRepository, WorkflowRuntimeStateRepository,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, error::Error, fmt, sync::Arc};

pub const IMAGE_STAGE: &str = "IMAGE";
pub const VIDEO_STAGE: &str = "VIDEO";
pub const DEFAULT_MODE: &str = "DEFAULT";

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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub available: bool,
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
        }
    }

    pub async fn get(
        &self,
        project_id: &str,
    ) -> Result<ProjectWorkflowConfigView, ProjectWorkflowBindingServiceError> {
        self.ensure_project(project_id).await?;
        let bindings = self.binding_repository.list_for_project(project_id).await?;
        let mut views = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let available = self.is_available(&binding).await?;
            views.push(ProjectWorkflowBindingView {
                stage: binding.stage,
                mode: binding.mode,
                workflow_version_id: binding.workflow_version_id,
                recipe_id: binding.recipe_id,
                created_at: binding.created_at,
                updated_at: binding.updated_at,
                available,
            });
        }
        Ok(config_from_bindings(project_id, views))
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

            let state = self
                .runtime_state_repository
                .find_state(&workflow_version_id)
                .await?;
            if !version.is_current
                || state
                    .as_ref()
                    .is_some_and(|state| !state.enabled || state.archived)
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
                created_at: now,
                updated_at: now,
            });
        }

        self.binding_repository
            .replace_for_project(project_id, &records)
            .await?;
        self.get(project_id).await
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

    async fn is_available(
        &self,
        binding: &ProjectWorkflowBindingRecord,
    ) -> Result<bool, ProjectWorkflowBindingServiceError> {
        let Some(version) = self
            .runtime_repository
            .find_version(&binding.workflow_version_id)
            .await?
        else {
            return Ok(false);
        };
        if !version.is_current
            || !version
                .recipes
                .iter()
                .any(|recipe| recipe.recipe_id == binding.recipe_id)
        {
            return Ok(false);
        }
        Ok(self
            .runtime_state_repository
            .find_state(&binding.workflow_version_id)
            .await?
            .is_none_or(|state| state.enabled && !state.archived))
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

    async fn setup() -> ProjectWorkflowBindingService {
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
        ProjectWorkflowBindingService::new(
            binding_repository,
            project_repository,
            runtime_repository,
            state_repository,
            Arc::new(FixedClock),
        )
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
