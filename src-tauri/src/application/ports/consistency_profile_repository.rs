use super::RepositoryError;
use crate::domain::consistency::{
    ConsistencyProfileRecord, CostumeVariant, ProfileRevision, ProfileType,
};
use async_trait::async_trait;

/// Persistence boundary for project-scoped consistency profiles and their
/// immutable revision history.
///
/// This is a port only. The SQLite implementation belongs to DEV-048.
#[async_trait]
pub trait ConsistencyProfileRepository: Send + Sync {
    async fn list_profiles(
        &self,
        project_id: &str,
        profile_type: ProfileType,
    ) -> Result<Vec<ConsistencyProfileRecord>, RepositoryError>;

    async fn find_profile(
        &self,
        project_id: &str,
        profile_type: ProfileType,
        profile_id: &str,
    ) -> Result<Option<ConsistencyProfileRecord>, RepositoryError>;

    async fn insert_profile(
        &self,
        profile: &ConsistencyProfileRecord,
    ) -> Result<(), RepositoryError>;

    async fn update_profile(
        &self,
        profile: &ConsistencyProfileRecord,
    ) -> Result<bool, RepositoryError>;

    async fn delete_profile(
        &self,
        project_id: &str,
        profile_type: ProfileType,
        profile_id: &str,
    ) -> Result<bool, RepositoryError>;

    async fn list_costume_variants(
        &self,
        character_profile_id: &str,
    ) -> Result<Vec<CostumeVariant>, RepositoryError>;

    async fn find_costume_variant(
        &self,
        costume_variant_id: &str,
    ) -> Result<Option<CostumeVariant>, RepositoryError>;

    async fn insert_costume_variant(
        &self,
        costume_variant: &CostumeVariant,
    ) -> Result<(), RepositoryError>;

    async fn update_costume_variant(
        &self,
        costume_variant: &CostumeVariant,
    ) -> Result<bool, RepositoryError>;

    async fn delete_costume_variant(
        &self,
        costume_variant_id: &str,
    ) -> Result<bool, RepositoryError>;

    async fn list_profile_revisions(
        &self,
        profile_type: ProfileType,
        profile_id: &str,
    ) -> Result<Vec<ProfileRevision>, RepositoryError>;

    async fn find_profile_revision(
        &self,
        revision_id: &str,
    ) -> Result<Option<ProfileRevision>, RepositoryError>;

    /// Inserts a revision. Revision content is immutable: there is
    /// intentionally no update method for `ProfileRevision`.
    async fn insert_profile_revision(
        &self,
        revision: &ProfileRevision,
    ) -> Result<(), RepositoryError>;
}
