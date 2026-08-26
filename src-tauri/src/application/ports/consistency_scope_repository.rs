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
}
