use crate::application::ports::{
    Clock, GenerationDefinitionRepository, NewProjectTemplate, OrganizationRepository,
    ProjectTemplate, RepositoryError,
};
use crate::application::{
    organization_service::{normalize_name, OrganizationError},
    project_service::{ProjectService, ProjectServiceError, ProjectView},
};
use crate::compiler::RecipeParser;
use crate::domain::InputDefinition;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{error::Error, fmt, sync::Arc};
use uuid::Uuid;

pub struct ProjectTemplateService {
    repository: Arc<dyn OrganizationRepository>,
    definitions: Arc<dyn GenerationDefinitionRepository>,
    projects: Arc<ProjectService>,
    clock: Arc<dyn Clock>,
}

impl ProjectTemplateService {
    pub fn new(
        repository: Arc<dyn OrganizationRepository>,
        definitions: Arc<dyn GenerationDefinitionRepository>,
        projects: Arc<ProjectService>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repository,
            definitions,
            projects,
            clock,
        }
    }

    pub async fn list(&self) -> Result<Vec<ProjectTemplate>, ProjectTemplateError> {
        Ok(self.repository.list_templates().await?)
    }

    pub async fn create(
        &self,
        request: CreateProjectTemplate,
    ) -> Result<ProjectTemplate, ProjectTemplateError> {
        let (name, normalized_name) = normalize_template_name(&request.name)?;
        let description = normalize_description(request.description.as_deref())?;
        let definition = self
            .definitions
            .list_available()
            .await?
            .into_iter()
            .find(|definition| {
                definition.workflow_version_id == request.workflow_version_id
                    && definition.recipe_id == request.recipe_id
            })
            .ok_or_else(|| {
                ProjectTemplateError::Unavailable(
                    "PROJECT_TEMPLATE_WORKFLOW_UNAVAILABLE: workflow definition is not available"
                        .to_owned(),
                )
            })?;
        let values = sanitize_values(&definition.recipe_yaml, &request.values)?;
        Ok(self
            .repository
            .create_template(NewProjectTemplate {
                id: format!("ptm_{}", Uuid::new_v4()),
                name,
                normalized_name,
                description,
                workflow_version_id: request.workflow_version_id,
                recipe_id: request.recipe_id,
                values,
                now: self.clock.now(),
            })
            .await?)
    }

    pub async fn update(
        &self,
        template_id: &str,
        name: &str,
        description: Option<&str>,
    ) -> Result<ProjectTemplate, ProjectTemplateError> {
        let (name, normalized_name) = normalize_template_name(name)?;
        let description = normalize_description(description)?;
        self.repository
            .update_template(
                template_id,
                &name,
                &normalized_name,
                description.as_deref(),
                self.clock.now(),
            )
            .await?
            .ok_or_else(|| {
                ProjectTemplateError::NotFound(format!(
                    "PROJECT_TEMPLATE_NOT_FOUND: template {template_id} was not found"
                ))
            })
    }

    pub async fn delete(&self, template_id: &str) -> Result<(), ProjectTemplateError> {
        if !self.repository.delete_template(template_id).await? {
            return Err(ProjectTemplateError::NotFound(format!(
                "PROJECT_TEMPLATE_NOT_FOUND: template {template_id} was not found"
            )));
        }
        Ok(())
    }

    pub async fn create_project(
        &self,
        template_id: &str,
        name: &str,
        description: Option<&str>,
    ) -> Result<TemplateProjectResult, ProjectTemplateError> {
        let template = self
            .repository
            .find_template(template_id)
            .await?
            .ok_or_else(|| {
                ProjectTemplateError::NotFound(format!(
                    "PROJECT_TEMPLATE_NOT_FOUND: template {template_id} was not found"
                ))
            })?;
        if !template.available {
            return Err(ProjectTemplateError::Unavailable(
                "PROJECT_TEMPLATE_WORKFLOW_UNAVAILABLE: 工作流当前不可用".to_owned(),
            ));
        }
        let project = self
            .projects
            .create(name, description.or(template.description.as_deref()))
            .await?;
        Ok(TemplateProjectResult {
            project,
            workflow_version_id: template.workflow_version_id,
            recipe_id: template.recipe_id,
            values: template.values,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectTemplate {
    pub name: String,
    pub description: Option<String>,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub values: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateProjectResult {
    pub project: ProjectView,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub values: Value,
}

fn normalize_template_name(value: &str) -> Result<(String, String), ProjectTemplateError> {
    normalize_name(value, 80, "PROJECT_TEMPLATE").map_err(ProjectTemplateError::Organization)
}

fn normalize_description(value: Option<&str>) -> Result<Option<String>, ProjectTemplateError> {
    let value = value.map(str::trim).filter(|value| !value.is_empty());
    if value.is_some_and(|value| value.chars().count() > 500) {
        return Err(ProjectTemplateError::InvalidInput(
            "PROJECT_TEMPLATE_DESCRIPTION_TOO_LONG: description must be at most 500 characters"
                .to_owned(),
        ));
    }
    Ok(value.map(str::to_owned))
}

pub(crate) fn sanitize_values(
    recipe_yaml: &str,
    values: &Value,
) -> Result<Value, ProjectTemplateError> {
    let recipe = RecipeParser::parse(recipe_yaml)
        .map_err(|error| ProjectTemplateError::InvalidInput(format!("RECIPE_INVALID: {error}")))?;
    let source = values.as_object().ok_or_else(|| {
        ProjectTemplateError::InvalidInput(
            "PROJECT_TEMPLATE_VALUES_INVALID: values must be an object".to_owned(),
        )
    })?;
    let mut sanitized = Map::new();
    for (key, field) in recipe.inputs {
        let Some(value) = source.get(&key) else {
            continue;
        };
        let value_type = value.get("type").and_then(Value::as_str);
        let valid = match field {
            InputDefinition::TextArea { .. } => {
                value_type == Some("string") && value.get("value").is_some_and(Value::is_string)
            }
            InputDefinition::Integer { .. } => {
                value_type == Some("integer") && value.get("value").is_some_and(Value::is_i64)
            }
            InputDefinition::Number { .. } => {
                value_type == Some("number")
                    && value
                        .get("value")
                        .and_then(Value::as_f64)
                        .is_some_and(f64::is_finite)
            }
            InputDefinition::Seed { .. } => {
                value_type == Some("seed_random")
                    || (value_type == Some("seed_fixed")
                        && value.get("value").is_some_and(Value::is_string))
            }
            InputDefinition::Image { .. }
            | InputDefinition::Images { .. }
            | InputDefinition::Video { .. }
            | InputDefinition::Videos { .. }
            | InputDefinition::Audio { .. }
            | InputDefinition::Audios { .. } => false,
        };
        if valid {
            sanitized.insert(key, value.clone());
        }
    }
    Ok(Value::Object(sanitized))
}

#[derive(Debug)]
pub enum ProjectTemplateError {
    InvalidInput(String),
    NotFound(String),
    Unavailable(String),
    Organization(OrganizationError),
    Repository(RepositoryError),
    Project(ProjectServiceError),
}
impl fmt::Display for ProjectTemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(v) | Self::NotFound(v) | Self::Unavailable(v) => f.write_str(v),
            Self::Organization(v) => write!(f, "{v}"),
            Self::Repository(v) => write!(f, "{v}"),
            Self::Project(v) => write!(f, "{v}"),
        }
    }
}
impl Error for ProjectTemplateError {}
impl From<RepositoryError> for ProjectTemplateError {
    fn from(v: RepositoryError) -> Self {
        Self::Repository(v)
    }
}
impl From<ProjectServiceError> for ProjectTemplateError {
    fn from(v: ProjectServiceError) -> Self {
        Self::Project(v)
    }
}

#[cfg(test)]
mod tests {
    use super::{sanitize_values, ProjectTemplateService};
    use crate::application::ports::{NewProjectTemplate, OrganizationRepository};
    use crate::application::project_service::ProjectService;
    use crate::infrastructure::{
        database::{
            initialize, repositories::test_support, SqliteGenerationDefinitionRepository,
            SqliteOrganizationRepository, SqliteProjectRepository,
        },
        filesystem::FileSystemProjectDirectoryStore,
        time::SystemClock,
    };
    use chrono::Utc;
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::tempdir;
    #[test]
    fn preserves_scalars_and_excludes_every_media_field() {
        let yaml = "schema_version: 1\nid: r\nname: R\nworkflow:\n  file: w.json\ninputs:\n  prompt:\n    type: textarea\n    label: Prompt\n    required: false\n    default: ''\n  steps:\n    type: integer\n    label: Steps\n    required: false\n    default: 10\n  seed:\n    type: seed\n    label: Seed\n    default: random\n  image:\n    type: image\n    label: Image\n    required: false\n  images:\n    type: images\n    label: Images\n    required: false\n  video:\n    type: video\n    label: Video\n    required: false\n  videos:\n    type: videos\n    label: Videos\n    required: false\n  audio:\n    type: audio\n    label: Audio\n    required: false\n  audios:\n    type: audios\n    label: Audios\n    required: false\nbindings: []\noutputs: []\n";
        let values = json!({"prompt":{"type":"string","value":"人物"},"steps":{"type":"integer","value":10},"seed":{"type":"seed_fixed","value":"42"},"image":{"type":"image_asset","assetId":"ast_old"},"images":{"type":"image_assets","assetIds":["ast_old"]},"video":{"type":"video_asset","assetId":"ast_old"},"videos":{"type":"video_assets","assetIds":["ast_old"]},"audio":{"type":"audio_asset","assetId":"ast_old"},"audios":{"type":"audio_assets","assetIds":["ast_old"]}});
        let sanitized = sanitize_values(yaml, &values).unwrap();
        assert_eq!(
            sanitized,
            json!({"prompt":{"type":"string","value":"人物"},"steps":{"type":"integer","value":10},"seed":{"type":"seed_fixed","value":"42"}})
        );
        assert!(!sanitized.to_string().contains("ast_old"));
    }

    #[tokio::test]
    async fn creates_new_project_and_draft_without_tasks_and_blocks_unavailable_workflow() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        let repository: Arc<dyn OrganizationRepository> =
            Arc::new(SqliteOrganizationRepository::new(pool.clone()));
        repository
            .create_template(NewProjectTemplate {
                id: "ptm_test".to_owned(),
                name: "测试模板".to_owned(),
                normalized_name: "测试模板".to_owned(),
                description: Some("说明".to_owned()),
                workflow_version_id: "workflow-version-1".to_owned(),
                recipe_id: "recipe-1".to_owned(),
                values: json!({"prompt":{"type":"string","value":"人物"}}),
                now: Utc::now(),
            })
            .await
            .unwrap();
        let clock = Arc::new(SystemClock);
        let project_service = Arc::new(ProjectService::new(
            Arc::new(SqliteProjectRepository::new(pool.clone())),
            Arc::new(FileSystemProjectDirectoryStore::new(
                directory.path().join("projects"),
            )),
            clock.clone(),
        ));
        std::fs::create_dir_all(directory.path().join("projects")).unwrap();
        let service = ProjectTemplateService::new(
            repository.clone(),
            Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone())),
            project_service,
            clock,
        );
        let result = service
            .create_project("ptm_test", "模板项目", None)
            .await
            .unwrap();
        assert_ne!(result.project.id, "project-1");
        assert_eq!(result.values["prompt"]["value"], "人物");
        let tasks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE project_id = ?")
            .bind(&result.project.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(tasks, 0);
        sqlx::query("UPDATE workflows SET current_version_id = NULL WHERE id = 'workflow-1'")
            .execute(&pool)
            .await
            .unwrap();
        assert!(!service.list().await.unwrap()[0].available);
        assert!(service
            .create_project("ptm_test", "不可用项目", None)
            .await
            .is_err());
    }
}
