use super::RepositoryError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowRecipePromotionRecord {
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub promoted_at: DateTime<Utc>,
}

/// Persistence boundary for the optional, version-scoped promoted Recipe.
/// Promotion is deliberately separate from workflow-version currentness.
#[async_trait]
pub trait WorkflowRecipePromotionRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<WorkflowRecipePromotionRecord>, RepositoryError>;

    async fn promote(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
        promoted_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError>;

    async fn clear(
        &self,
        workflow_version_id: &str,
        recipe_id: &str,
    ) -> Result<(), RepositoryError>;
}
