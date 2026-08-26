use super::RepositoryError;
use crate::domain::consistency::{ShotProfileBinding, ShotReferenceSetBinding};
use async_trait::async_trait;

/// Persistence boundary for the shot's consistency reference pack.
///
/// Both replacement operations are atomic persistence contracts. They do not
/// resolve inheritance or readiness; those concerns belong to later services.
#[async_trait]
pub trait ShotConsistencyRepository: Send + Sync {
    async fn list_profile_bindings(
        &self,
        shot_id: &str,
    ) -> Result<Vec<ShotProfileBinding>, RepositoryError>;

    /// Atomically replaces all profile bindings for one shot.
    async fn replace_profile_bindings(
        &self,
        shot_id: &str,
        bindings: &[ShotProfileBinding],
    ) -> Result<(), RepositoryError>;

    async fn list_reference_set_bindings(
        &self,
        shot_id: &str,
    ) -> Result<Vec<ShotReferenceSetBinding>, RepositoryError>;

    /// Atomically replaces all reference-set bindings for one shot.
    async fn replace_reference_set_bindings(
        &self,
        shot_id: &str,
        bindings: &[ShotReferenceSetBinding],
    ) -> Result<(), RepositoryError>;
}
