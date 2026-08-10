use super::RepositoryError;
use async_trait::async_trait;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptEntryRecord {
    pub id: String,
    pub project_id: String,
    pub kind: String,
    pub name: String,
    pub normalized_name: String,
    pub tags_json: String,
    pub created_at: String,
    pub updated_at: String,
    pub version_count: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptVersionRecord {
    pub id: String,
    pub prompt_id: String,
    pub version: i64,
    pub text: String,
    pub created_at: String,
}

#[async_trait]
pub trait PromptLibraryRepository: Send + Sync {
    async fn list(
        &self,
        project_id: &str,
        kind: Option<&str>,
        keyword: Option<&str>,
        tag: Option<&str>,
    ) -> Result<Vec<PromptEntryRecord>, RepositoryError>;

    async fn find_by_id(
        &self,
        project_id: &str,
        prompt_id: &str,
    ) -> Result<Option<PromptEntryRecord>, RepositoryError>;

    async fn list_versions(
        &self,
        project_id: &str,
        prompt_id: &str,
    ) -> Result<Vec<PromptVersionRecord>, RepositoryError>;

    async fn create(
        &self,
        entry: &PromptEntryRecord,
        first_version: &PromptVersionRecord,
    ) -> Result<(), RepositoryError>;

    async fn append_version(
        &self,
        project_id: &str,
        prompt_id: &str,
        version_id: &str,
        text: &str,
        created_at: &str,
    ) -> Result<PromptVersionRecord, RepositoryError>;

    async fn update_metadata(
        &self,
        project_id: &str,
        prompt_id: &str,
        name: &str,
        normalized_name: &str,
        tags_json: &str,
        updated_at: &str,
    ) -> Result<Option<PromptEntryRecord>, RepositoryError>;

    async fn delete(&self, project_id: &str, prompt_id: &str) -> Result<bool, RepositoryError>;
}
