use super::RepositoryError;
use async_trait::async_trait;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct GenerationDefinition {
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub workflow_version: String,
    pub workflow_sha256: String,
    pub recipe_version: String,
    pub recipe_sha256: String,
    pub package_name: Option<String>,
    pub package_source_path: Option<String>,
    pub workflow_json: Value,
    pub recipe_yaml: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvailableGenerationDefinition {
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub recipe_version: String,
    pub name: String,
    pub category: String,
    pub mode: String,
    pub recipe_yaml: String,
}

#[async_trait]
pub trait GenerationDefinitionRepository: Send + Sync {
    async fn find(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
    ) -> Result<Option<GenerationDefinition>, RepositoryError>;

    /// Exact tuple lookup for new production-input boundaries. Implementations
    /// with lifecycle state should reject archived or removed definitions.
    async fn find_active(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
    ) -> Result<Option<GenerationDefinition>, RepositoryError> {
        self.find(workflow_version_id, recipe_id).await
    }

    /// Distinguishes an unavailable workflow version from an unavailable
    /// recipe pair for strict external production-input validation.
    async fn workflow_version_is_active(
        &self,
        _workflow_version_id: &str,
    ) -> Result<bool, RepositoryError> {
        Ok(false)
    }

    /// Loads the requested workflow-version/recipe pairs in bulk.
    ///
    /// The default keeps existing fakes and adapters source-compatible. SQL
    /// repositories should override this with a set-based implementation.
    async fn find_many(
        &self,
        pairs: &[(String, String)],
    ) -> Result<Vec<GenerationDefinition>, RepositoryError> {
        let mut definitions = Vec::new();
        for (workflow_version_id, recipe_id) in pairs {
            if let Some(definition) = self.find(workflow_version_id, recipe_id).await? {
                definitions.push(definition);
            }
        }
        Ok(definitions)
    }

    async fn list_available(&self) -> Result<Vec<AvailableGenerationDefinition>, RepositoryError>;
}
