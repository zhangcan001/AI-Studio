use crate::application::generation_input_preparer::GenerationInputValue;
use crate::application::ports::Clock;
use crate::application::ports::{
    AssetRepository, GenerationDefinitionRepository, PresetRepository, RepositoryError,
};
use crate::compiler::RecipeParser;
use crate::domain::{
    AssetId, AssetType, InputDefinition, Preset, PresetDomainError, PresetId, Recipe, SeedValue,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use std::{collections::BTreeMap, error::Error, fmt, sync::Arc};

pub struct PresetService {
    repository: Arc<dyn PresetRepository>,
    definition_repository: Arc<dyn GenerationDefinitionRepository>,
    asset_repository: Arc<dyn AssetRepository>,
    clock: Arc<dyn Clock>,
}

impl PresetService {
    pub fn new(
        repository: Arc<dyn PresetRepository>,
        definition_repository: Arc<dyn GenerationDefinitionRepository>,
        asset_repository: Arc<dyn AssetRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repository,
            definition_repository,
            asset_repository,
            clock,
        }
    }

    pub async fn list(
        &self,
        project_id: &str,
        workflow_version_id: &str,
        recipe_id: &str,
    ) -> Result<Vec<PresetView>, PresetServiceError> {
        validate_project_id(project_id)?;
        Ok(self
            .repository
            .list(project_id, workflow_version_id, recipe_id)
            .await?
            .into_iter()
            .map(PresetView::from)
            .collect())
    }

    pub async fn create(
        &self,
        project_id: &str,
        workflow_version_id: &str,
        recipe_id: &str,
        name: &str,
        values: &BTreeMap<String, GenerationInputValue>,
    ) -> Result<PresetView, PresetServiceError> {
        validate_project_id(project_id)?;
        let recipe = self.load_recipe(workflow_version_id, recipe_id).await?;
        self.validate_values(project_id, &recipe, values).await?;
        let name = normalize_name(name)?;
        if self
            .repository
            .find_by_name(project_id, workflow_version_id, recipe_id, &name)
            .await?
            .is_some()
        {
            return Err(PresetServiceError::NameConflict(name));
        }
        let now = self.clock.now();
        let preset = Preset::new(
            PresetId::new(),
            project_id,
            workflow_version_id,
            recipe_id,
            name,
            input_values_to_json(values),
            now,
        )?;
        self.repository.insert(&preset).await?;
        Ok(PresetView::from(preset))
    }

    pub async fn update(
        &self,
        project_id: &str,
        preset_id: &str,
        name: &str,
        values: &BTreeMap<String, GenerationInputValue>,
    ) -> Result<PresetView, PresetServiceError> {
        validate_project_id(project_id)?;
        let preset_id = PresetId::parse(preset_id.to_owned())
            .map_err(|error| PresetServiceError::InvalidPresetId(error.to_string()))?;
        let current = self
            .repository
            .find_by_id(project_id, &preset_id)
            .await?
            .ok_or_else(|| PresetServiceError::NotFound(preset_id.to_string()))?;
        let recipe = self
            .load_recipe(&current.workflow_version_id, &current.recipe_id)
            .await?;
        self.validate_values(project_id, &recipe, values).await?;
        let name = normalize_name(name)?;
        if let Some(existing) = self
            .repository
            .find_by_name(
                project_id,
                &current.workflow_version_id,
                &current.recipe_id,
                &name,
            )
            .await?
        {
            if existing.id != current.id {
                return Err(PresetServiceError::NameConflict(name));
            }
        }
        let updated = Preset {
            id: current.id,
            project_id: current.project_id,
            workflow_version_id: current.workflow_version_id,
            recipe_id: current.recipe_id,
            name,
            values_json: input_values_to_json(values),
            created_at: current.created_at,
            updated_at: self.clock.now(),
        };
        let updated = self
            .repository
            .update(&updated)
            .await?
            .ok_or_else(|| PresetServiceError::NotFound(updated.id.to_string()))?;
        Ok(PresetView::from(updated))
    }

    pub async fn delete(
        &self,
        project_id: &str,
        preset_id: &str,
    ) -> Result<(), PresetServiceError> {
        validate_project_id(project_id)?;
        let preset_id = PresetId::parse(preset_id.to_owned())
            .map_err(|error| PresetServiceError::InvalidPresetId(error.to_string()))?;
        if !self.repository.delete(project_id, &preset_id).await? {
            return Err(PresetServiceError::NotFound(preset_id.to_string()));
        }
        Ok(())
    }

    async fn load_recipe(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
    ) -> Result<Recipe, PresetServiceError> {
        let definition = self
            .definition_repository
            .find(workflow_version_id, recipe_id)
            .await?
            .ok_or_else(|| PresetServiceError::DefinitionNotFound {
                workflow_version_id: workflow_version_id.to_owned(),
                recipe_id: recipe_id.to_owned(),
            })?;
        RecipeParser::parse(&definition.recipe_yaml)
            .map_err(|error| PresetServiceError::InvalidRecipe(error.to_string()))
    }

    async fn validate_values(
        &self,
        project_id: &str,
        recipe: &Recipe,
        values: &BTreeMap<String, GenerationInputValue>,
    ) -> Result<(), PresetServiceError> {
        for (key, value) in values {
            let Some(definition) = recipe.inputs.get(key) else {
                return Err(PresetServiceError::ValuesInvalid(format!(
                    "unknown input \"{key}\""
                )));
            };
            match (definition, value) {
                (InputDefinition::TextArea { .. }, GenerationInputValue::Text(_)) => {}
                (
                    InputDefinition::Integer { min, max, .. },
                    GenerationInputValue::Integer(value),
                ) => {
                    if min.is_some_and(|min| *value < min) || max.is_some_and(|max| *value > max) {
                        return Err(PresetServiceError::ValuesInvalid(format!(
                            "input \"{key}\" is outside its recipe range"
                        )));
                    }
                }
                (InputDefinition::Seed { min, max, .. }, GenerationInputValue::Seed(seed)) => {
                    if let SeedValue::Fixed(value) = seed {
                        if min.is_some_and(|min| *value < min)
                            || max.is_some_and(|max| *value > max)
                        {
                            return Err(PresetServiceError::ValuesInvalid(format!(
                                "input \"{key}\" is outside its recipe range"
                            )));
                        }
                    }
                }
                (InputDefinition::Image { .. }, GenerationInputValue::ImageAsset(asset_id)) => {
                    self.validate_image_asset(project_id, key, asset_id).await?;
                }
                (
                    InputDefinition::Images {
                        min_items,
                        max_items,
                        required,
                        ..
                    },
                    GenerationInputValue::ImageAssets(asset_ids),
                ) => {
                    if asset_ids.len() > *max_items
                        || (*required && asset_ids.len() < *min_items)
                        || (!asset_ids.is_empty() && asset_ids.len() < *min_items)
                    {
                        return Err(PresetServiceError::ValuesInvalid(format!(
                            "input \"{key}\" must contain between {min_items} and {max_items} images"
                        )));
                    }
                    for asset_id in asset_ids {
                        self.validate_image_asset(project_id, key, asset_id).await?;
                    }
                }
                _ => {
                    return Err(PresetServiceError::ValuesInvalid(format!(
                        "input \"{key}\" has a value type that does not match the recipe"
                    )))
                }
            }
        }
        Ok(())
    }

    async fn validate_image_asset(
        &self,
        project_id: &str,
        key: &str,
        asset_id: &AssetId,
    ) -> Result<(), PresetServiceError> {
        let asset = self.asset_repository.find_by_id(asset_id).await?;
        let Some(asset) = asset else {
            return Err(PresetServiceError::ValuesInvalid(format!(
                "input \"{key}\" references missing asset {}",
                asset_id.as_str()
            )));
        };
        if asset.project_id != project_id {
            return Err(PresetServiceError::ValuesInvalid(format!(
                "input \"{key}\" references an asset from another project"
            )));
        }
        if asset.asset_type != AssetType::Image {
            return Err(PresetServiceError::ValuesInvalid(format!(
                "input \"{key}\" references a non-image asset"
            )));
        }
        Ok(())
    }
}

fn validate_project_id(project_id: &str) -> Result<(), PresetServiceError> {
    if project_id.trim().is_empty() {
        return Err(PresetServiceError::InvalidProjectId);
    }
    Ok(())
}

fn normalize_name(name: &str) -> Result<String, PresetServiceError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(PresetServiceError::NameRequired);
    }
    if name.chars().count() > 80 {
        return Err(PresetServiceError::NameTooLong);
    }
    Ok(name.to_owned())
}

pub fn input_values_to_json(values: &BTreeMap<String, GenerationInputValue>) -> Value {
    Value::Object(
        values
            .iter()
            .map(|(key, value)| (key.clone(), input_value_to_json(value)))
            .collect(),
    )
}

fn input_value_to_json(value: &GenerationInputValue) -> Value {
    match value {
        GenerationInputValue::Text(value) => {
            serde_json::json!({ "type": "string", "value": value })
        }
        GenerationInputValue::Integer(value) => {
            serde_json::json!({ "type": "integer", "value": value })
        }
        GenerationInputValue::Seed(SeedValue::Random) => {
            serde_json::json!({ "type": "seed_random" })
        }
        GenerationInputValue::Seed(SeedValue::Fixed(value)) => {
            serde_json::json!({ "type": "seed_fixed", "value": value.to_string() })
        }
        GenerationInputValue::ImageAsset(asset_id) => serde_json::json!({
            "type": "image_asset",
            "assetId": asset_id.as_str(),
        }),
        GenerationInputValue::ImageAssets(asset_ids) => serde_json::json!({
            "type": "image_assets",
            "assetIds": asset_ids.iter().map(AssetId::as_str).collect::<Vec<_>>(),
        }),
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetView {
    pub id: String,
    pub project_id: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub name: String,
    pub values: BTreeMap<String, Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Preset> for PresetView {
    fn from(preset: Preset) -> Self {
        let values = preset
            .values_json
            .as_object()
            .map(|object| {
                object
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            id: preset.id.to_string(),
            project_id: preset.project_id,
            workflow_version_id: preset.workflow_version_id,
            recipe_id: preset.recipe_id,
            name: preset.name,
            values,
            created_at: preset.created_at,
            updated_at: preset.updated_at,
        }
    }
}

#[derive(Debug)]
pub enum PresetServiceError {
    InvalidProjectId,
    InvalidPresetId(String),
    NameRequired,
    NameTooLong,
    NameConflict(String),
    NotFound(String),
    DefinitionNotFound {
        workflow_version_id: String,
        recipe_id: String,
    },
    InvalidRecipe(String),
    ValuesInvalid(String),
    Repository(RepositoryError),
    Domain(PresetDomainError),
}

impl fmt::Display for PresetServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProjectId => formatter.write_str("INVALID_PROJECT_ID: project id must not be empty"),
            Self::InvalidPresetId(message) => write!(formatter, "INVALID_PRESET_ID: {message}"),
            Self::NameRequired => formatter.write_str("PRESET_NAME_REQUIRED: preset name is required"),
            Self::NameTooLong => formatter.write_str("PRESET_NAME_TOO_LONG: preset name must be 80 characters or fewer"),
            Self::NameConflict(name) => write!(formatter, "PRESET_NAME_CONFLICT: preset name \"{name}\" already exists for this recipe"),
            Self::NotFound(id) => write!(formatter, "PRESET_NOT_FOUND: preset {id} was not found"),
            Self::DefinitionNotFound { workflow_version_id, recipe_id } => write!(formatter, "GENERATION_DEFINITION_NOT_FOUND: workflow version {workflow_version_id} and recipe {recipe_id}"),
            Self::InvalidRecipe(message) => write!(formatter, "RECIPE_INVALID: {message}"),
            Self::ValuesInvalid(message) => write!(formatter, "PRESET_VALUES_INVALID: {message}"),
            Self::Repository(error) => write!(formatter, "{error}"),
            Self::Domain(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for PresetServiceError {}

impl From<RepositoryError> for PresetServiceError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

impl From<PresetDomainError> for PresetServiceError {
    fn from(error: PresetDomainError) -> Self {
        Self::Domain(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{PresetService, PresetServiceError};
    use crate::application::generation_input_preparer::GenerationInputValue;
    use crate::application::ports::Clock;
    use crate::infrastructure::database::{
        initialize, repositories::test_support, SqliteAssetRepository,
        SqliteGenerationDefinitionRepository, SqlitePresetRepository,
    };
    use chrono::{DateTime, Utc};
    use std::{collections::BTreeMap, sync::Arc};
    use tempfile::{tempdir, TempDir};

    struct FixedClock;

    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            Utc::now()
        }
    }

    async fn service() -> (TempDir, PresetService) {
        let directory = tempdir().expect("temporary directory");
        let pool = initialize(&directory.path().join("app.db")).await.unwrap();
        test_support::seed_task_dependencies(&pool).await;
        sqlx::query("UPDATE recipes SET recipe_yaml = ? WHERE id = 'recipe-1'")
            .bind(
                "schema_version: 1\nid: demo\nname: Demo\nworkflow:\n  file: workflow_api.json\ninputs:\n  prompt:\n    type: textarea\n    label: Prompt\n    required: true\n  seed:\n    type: seed\n    label: Seed\n    default: random\nbindings: []\noutputs: []\n",
            )
            .execute(&pool)
            .await
            .unwrap();
        let service = PresetService::new(
            Arc::new(SqlitePresetRepository::new(pool.clone())),
            Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone())),
            Arc::new(SqliteAssetRepository::new(pool)),
            Arc::new(FixedClock),
        );
        (directory, service)
    }

    #[tokio::test]
    async fn preset_crud_is_recipe_scoped_and_duplicate_names_are_rejected() {
        let (_directory, service) = service().await;
        let values = BTreeMap::from([(
            "prompt".to_owned(),
            GenerationInputValue::Text("hello".to_owned()),
        )]);
        let created = service
            .create(
                "project-1",
                "workflow-version-1",
                "recipe-1",
                "  First  ",
                &values,
            )
            .await
            .unwrap();
        assert_eq!(created.name, "First");
        assert_eq!(
            service
                .list("project-2", "workflow-version-1", "recipe-1")
                .await
                .unwrap()
                .len(),
            0
        );
        assert!(matches!(
            service
                .create(
                    "project-1",
                    "workflow-version-1",
                    "recipe-1",
                    "First",
                    &values
                )
                .await,
            Err(PresetServiceError::NameConflict(_))
        ));
        let updated = service
            .update("project-1", &created.id, "Renamed", &values)
            .await
            .unwrap();
        assert_eq!(updated.name, "Renamed");
        service.delete("project-1", &created.id).await.unwrap();
        assert!(matches!(
            service.delete("project-1", &created.id).await,
            Err(PresetServiceError::NotFound(_))
        ));
    }
}
