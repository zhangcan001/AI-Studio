use super::RepositoryError;
use crate::domain::consistency::{
    ConsistencyScopeType, ScopedProfileBinding, ScopedReferenceSetBinding,
};
use async_trait::async_trait;

#[async_trait]
pub trait ConsistencyScopeRepository: Send + Sync {
    async fn list_profile_bindings_for_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<ScopedProfileBinding>, RepositoryError>;

    async fn list_reference_set_bindings_for_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<ScopedReferenceSetBinding>, RepositoryError>;

    async fn replace_profile_bindings(
        &self,
        project_id: &str,
        scope_type: ConsistencyScopeType,
        scope_id: &str,
        bindings: &[ScopedProfileBinding],
    ) -> Result<(), RepositoryError>;

    async fn replace_reference_set_bindings(
        &self,
        project_id: &str,
        scope_type: ConsistencyScopeType,
        scope_id: &str,
        bindings: &[ScopedReferenceSetBinding],
    ) -> Result<(), RepositoryError>;

    /// Atomically replaces both direct binding collections when the concrete
    /// repository supports a transaction.  The default keeps small fakes and
    /// older adapters source-compatible; SQLite overrides it below.
    async fn replace_binding_pack(
        &self,
        project_id: &str,
        scope_type: ConsistencyScopeType,
        scope_id: &str,
        profile_bindings: &[ScopedProfileBinding],
        reference_set_bindings: &[ScopedReferenceSetBinding],
    ) -> Result<(), RepositoryError> {
        self.replace_profile_bindings(project_id, scope_type, scope_id, profile_bindings)
            .await?;
        self.replace_reference_set_bindings(
            project_id,
            scope_type,
            scope_id,
            reference_set_bindings,
        )
        .await
    }
}
