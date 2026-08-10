use crate::application::ports::{
    AvailableGenerationDefinition, GenerationDefinitionRepository, RepositoryError,
};
use crate::compiler::RecipeParser;
use crate::domain::{InputDefinition, OutputType, SeedDefault};
use serde::Serialize;
use std::{error::Error, fmt, sync::Arc};

pub struct GenerationCatalogService {
    repository: Arc<dyn GenerationDefinitionRepository>,
}

impl GenerationCatalogService {
    pub fn new(repository: Arc<dyn GenerationDefinitionRepository>) -> Self {
        Self { repository }
    }

    pub async fn list(&self) -> Result<Vec<RecipeViewModel>, GenerationCatalogError> {
        let definitions = self.repository.list_available().await?;
        definitions
            .into_iter()
            .map(RecipeViewModel::try_from_definition)
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeViewModel {
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub name: String,
    pub category: String,
    pub mode: String,
    pub fields: Vec<FieldViewModel>,
    pub output_types: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FieldViewModel {
    #[serde(rename = "textarea")]
    Textarea {
        key: String,
        label: String,
        required: bool,
        default: String,
    },
    #[serde(rename = "integer")]
    Integer {
        key: String,
        label: String,
        required: bool,
        default: Option<i64>,
        min: Option<i64>,
        max: Option<i64>,
    },
    #[serde(rename = "seed")]
    Seed {
        key: String,
        label: String,
        #[serde(rename = "defaultMode")]
        default_mode: String,
        #[serde(rename = "defaultValue")]
        default_value: Option<String>,
        #[serde(rename = "minValue")]
        min_value: Option<String>,
        #[serde(rename = "maxValue")]
        max_value: Option<String>,
    },
    #[serde(rename = "image")]
    Image {
        key: String,
        label: String,
        required: bool,
    },
    #[serde(rename = "images")]
    Images {
        key: String,
        label: String,
        required: bool,
        #[serde(rename = "minItems")]
        min_items: usize,
        #[serde(rename = "maxItems")]
        max_items: usize,
    },
    #[serde(rename = "video")]
    Video {
        key: String,
        label: String,
        required: bool,
    },
    #[serde(rename = "audio")]
    Audio {
        key: String,
        label: String,
        required: bool,
    },
    #[serde(rename = "videos")]
    Videos {
        key: String,
        label: String,
        required: bool,
        #[serde(rename = "minItems")]
        min_items: usize,
        #[serde(rename = "maxItems")]
        max_items: usize,
    },
    #[serde(rename = "audios")]
    Audios {
        key: String,
        label: String,
        required: bool,
        #[serde(rename = "minItems")]
        min_items: usize,
        #[serde(rename = "maxItems")]
        max_items: usize,
    },
}

impl RecipeViewModel {
    fn try_from_definition(
        definition: AvailableGenerationDefinition,
    ) -> Result<Self, GenerationCatalogError> {
        let recipe = RecipeParser::parse(&definition.recipe_yaml)
            .map_err(|error| GenerationCatalogError::InvalidRecipe(error.to_string()))?;
        let output_types = recipe
            .outputs
            .iter()
            .map(|output| match output.output_type {
                OutputType::Image => "image".to_owned(),
                OutputType::Video => "video".to_owned(),
            })
            .collect();
        let fields = recipe
            .inputs
            .into_iter()
            .map(|(key, definition)| match definition {
                InputDefinition::TextArea {
                    label,
                    required,
                    default,
                } => FieldViewModel::Textarea {
                    key,
                    label,
                    required,
                    default: default.unwrap_or_default(),
                },
                InputDefinition::Integer {
                    label,
                    required,
                    default,
                    min,
                    max,
                } => FieldViewModel::Integer {
                    key,
                    label,
                    required,
                    default,
                    min,
                    max,
                },
                InputDefinition::Seed {
                    label,
                    default,
                    min,
                    max,
                } => {
                    let (default_mode, default_value) = match default {
                        SeedDefault::Random => ("random".to_owned(), None),
                        SeedDefault::Fixed(value) => ("fixed".to_owned(), Some(value.to_string())),
                    };
                    FieldViewModel::Seed {
                        key,
                        label,
                        default_mode,
                        default_value,
                        min_value: min.map(|value| value.to_string()),
                        max_value: max.map(|value| value.to_string()),
                    }
                }
                InputDefinition::Image { label, required } => FieldViewModel::Image {
                    key,
                    label,
                    required,
                },
                InputDefinition::Images {
                    label,
                    required,
                    min_items,
                    max_items,
                } => FieldViewModel::Images {
                    key,
                    label,
                    required,
                    min_items,
                    max_items,
                },
                InputDefinition::Video { label, required } => FieldViewModel::Video {
                    key,
                    label,
                    required,
                },
                InputDefinition::Audio { label, required } => FieldViewModel::Audio {
                    key,
                    label,
                    required,
                },
                InputDefinition::Videos {
                    label,
                    required,
                    min_items,
                    max_items,
                } => FieldViewModel::Videos {
                    key,
                    label,
                    required,
                    min_items,
                    max_items,
                },
                InputDefinition::Audios {
                    label,
                    required,
                    min_items,
                    max_items,
                } => FieldViewModel::Audios {
                    key,
                    label,
                    required,
                    min_items,
                    max_items,
                },
            })
            .collect();

        Ok(Self {
            workflow_id: definition.workflow_id,
            workflow_version_id: definition.workflow_version_id,
            recipe_id: definition.recipe_id,
            name: definition.name,
            category: definition.category,
            mode: definition.mode,
            fields,
            output_types,
        })
    }
}

#[derive(Debug)]
pub enum GenerationCatalogError {
    Repository(RepositoryError),
    InvalidRecipe(String),
}

impl fmt::Display for GenerationCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Repository(error) => write!(formatter, "{error}"),
            Self::InvalidRecipe(message) => write!(formatter, "RECIPE_INVALID: {message}"),
        }
    }
}

impl Error for GenerationCatalogError {}

impl From<RepositoryError> for GenerationCatalogError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{FieldViewModel, GenerationCatalogService};
    use crate::infrastructure::database::{
        initialize, repositories::test_support, SqliteGenerationDefinitionRepository,
    };
    use serde_json::to_string;
    use tempfile::tempdir;

    #[tokio::test]
    async fn returns_ui_fields_without_workflow_binding_details() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        sqlx::query("UPDATE recipes SET recipe_yaml = ? WHERE id = 'recipe-1'")
            .bind(
                "schema_version: 1\nid: recipe\nname: Recipe\nworkflow:\n  file: workflow_api.json\ninputs:\n  prompt:\n    type: textarea\n    label: Prompt\n    required: true\n    default: ''\n  steps:\n    type: integer\n    label: Steps\n    required: true\n    default: 20\n    min: 1\n    max: 100\n  seed:\n    type: seed\n    label: Seed\n    default: random\nbindings: []\noutputs: []\n",
            )
            .execute(&pool)
            .await
            .unwrap();
        let repository = std::sync::Arc::new(SqliteGenerationDefinitionRepository::new(pool));
        let catalog = GenerationCatalogService::new(repository);
        let view = catalog.list().await.unwrap().remove(0);
        assert!(matches!(view.fields[0], FieldViewModel::Textarea { .. }));
        let json = to_string(&view).unwrap();
        assert!(json.contains("textarea"));
        assert!(json.contains("\"defaultMode\":\"random\""));
        assert!(json.contains("\"defaultValue\":null"));
        assert!(!json.contains("node"));
        assert!(!json.contains("class_type"));
        assert!(!json.contains("binding"));
    }

    #[tokio::test]
    async fn preserves_fixed_seed_default_as_decimal_string() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        sqlx::query("UPDATE recipes SET recipe_yaml = ? WHERE id = 'recipe-1'")
            .bind(
                "schema_version: 1\nid: recipe\nname: Recipe\nworkflow:\n  file: workflow_api.json\ninputs:\n  seed:\n    type: seed\n    label: Seed\n    default: 18446744073709551615\nbindings: []\noutputs: []\n",
            )
            .execute(&pool)
            .await
            .unwrap();

        let repository = std::sync::Arc::new(SqliteGenerationDefinitionRepository::new(pool));
        let catalog = GenerationCatalogService::new(repository);
        let view = catalog.list().await.unwrap().remove(0);
        assert_eq!(
            view.fields,
            vec![FieldViewModel::Seed {
                key: "seed".to_owned(),
                label: "Seed".to_owned(),
                default_mode: "fixed".to_owned(),
                default_value: Some("18446744073709551615".to_owned()),
                min_value: None,
                max_value: None,
            }]
        );
        let json = to_string(&view).unwrap();
        assert!(json.contains("\"defaultValue\":\"18446744073709551615\""));
        assert!(!json.contains("1.8446744073709552e19"));
    }

    #[tokio::test]
    async fn serializes_seed_range_as_decimal_strings() {
        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        sqlx::query("UPDATE recipes SET recipe_yaml = ? WHERE id = 'recipe-1'")
            .bind(
                "schema_version: 1\nid: recipe\nname: Recipe\nworkflow:\n  file: workflow_api.json\ninputs:\n  seed:\n    type: seed\n    label: Seed\n    default: random\n    min: 0\n    max: 18446744073709551615\nbindings: []\noutputs: []\n",
            )
            .execute(&pool)
            .await
            .unwrap();

        let repository = std::sync::Arc::new(SqliteGenerationDefinitionRepository::new(pool));
        let catalog = GenerationCatalogService::new(repository);
        let view = catalog.list().await.unwrap().remove(0);
        let json = to_string(&view).unwrap();

        assert!(json.contains("\"minValue\":\"0\""));
        assert!(json.contains("\"maxValue\":\"18446744073709551615\""));
        assert!(!json.contains("1.8446744073709552e19"));
    }
}
