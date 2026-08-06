use super::RepositoryError;
use async_trait::async_trait;

/// Read-only runtime evidence used by the technical Workflow Workspace.
/// It deliberately reports only whether a version has a successful task.
#[async_trait]
pub trait WorkflowRunRepository: Send + Sync {
    async fn has_successful_run(
        &self,
        workflow_id: &str,
        workflow_version: &str,
    ) -> Result<bool, RepositoryError>;
}
