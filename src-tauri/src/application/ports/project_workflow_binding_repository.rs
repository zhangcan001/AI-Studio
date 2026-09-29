use super::RepositoryError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectWorkflowBindingRecord {
    pub project_id: String,
    pub stage: String,
    pub mode: String,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub binding_instance_id: String,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[async_trait]
pub trait ProjectWorkflowBindingRepository: Send + Sync {
    async fn list_for_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<ProjectWorkflowBindingRecord>, RepositoryError>;

    async fn replace_for_project(
        &self,
        project_id: &str,
        bindings: &[ProjectWorkflowBindingRecord],
    ) -> Result<(), RepositoryError>;

    async fn find_slot(
        &self,
        project_id: &str,
        stage: &str,
        mode: &str,
    ) -> Result<Option<ProjectWorkflowBindingRecord>, RepositoryError> {
        Ok(self
            .list_for_project(project_id)
            .await?
            .into_iter()
            .find(|binding| binding.stage == stage && binding.mode == mode))
    }

    async fn insert_slot(
        &self,
        binding: &ProjectWorkflowBindingRecord,
    ) -> Result<(), RepositoryError> {
        let _ = binding;
        Err(RepositoryError::integrity(
            "project workflow binding repository does not support slot inserts",
        ))
    }

    async fn update_slot(
        &self,
        project_id: &str,
        stage: &str,
        mode: &str,
        expected_binding_instance_id: &str,
        expected_revision: i64,
        workflow_version_id: &str,
        recipe_id: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<u64, RepositoryError> {
        let _ = (
            project_id,
            stage,
            mode,
            expected_binding_instance_id,
            expected_revision,
            workflow_version_id,
            recipe_id,
            updated_at,
        );
        Err(RepositoryError::integrity(
            "project workflow binding repository does not support slot updates",
        ))
    }

    async fn delete_slot(
        &self,
        project_id: &str,
        stage: &str,
        mode: &str,
        expected_binding_instance_id: &str,
        expected_revision: i64,
    ) -> Result<u64, RepositoryError> {
        let _ = (
            project_id,
            stage,
            mode,
            expected_binding_instance_id,
            expected_revision,
        );
        Err(RepositoryError::integrity(
            "project workflow binding repository does not support slot deletes",
        ))
    }

    /// Delete only the records represented by the snapshot. Infrastructure
    /// implementations must include instance id and revision in the DELETE
    /// predicate so a later recreate cannot be removed accidentally.
    async fn clear_exact_bindings(
        &self,
        bindings: &[ProjectWorkflowBindingRecord],
    ) -> Result<u64, RepositoryError> {
        let mut cleared = 0;
        for binding in bindings {
            cleared += self
                .delete_slot(
                    &binding.project_id,
                    &binding.stage,
                    &binding.mode,
                    &binding.binding_instance_id,
                    binding.revision,
                )
                .await?;
        }
        Ok(cleared)
    }

    async fn list_for_workflow_version(
        &self,
        workflow_version_id: &str,
    ) -> Result<Vec<ProjectWorkflowBindingRecord>, RepositoryError>;

    async fn clear_by_workflow_version(
        &self,
        workflow_version_id: &str,
    ) -> Result<u64, RepositoryError>;
}
