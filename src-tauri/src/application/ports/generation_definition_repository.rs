use super::RepositoryError;
use async_trait::async_trait;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct GenerationDefinition {
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub workflow_json: Value,
    pub recipe_yaml: String,
}

#[async_trait]
pub trait GenerationDefinitionRepository: Send + Sync {
    async fn find(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
    ) -> Result<Option<GenerationDefinition>, RepositoryError>;
}
