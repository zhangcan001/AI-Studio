use super::RepositoryError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetTag {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub normalized_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssetOrganization {
    pub is_favorite: bool,
    pub tags: Vec<AssetTag>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTemplate {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing)]
    pub normalized_name: String,
    pub description: Option<String>,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub values: Value,
    pub available: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewProjectTemplate {
    pub id: String,
    pub name: String,
    pub normalized_name: String,
    pub description: Option<String>,
    pub workflow_version_id: String,
    pub recipe_id: String,
    pub values: Value,
    pub now: DateTime<Utc>,
}

#[async_trait]
pub trait OrganizationRepository: Send + Sync {
    async fn list_tags(&self, project_id: &str) -> Result<Vec<AssetTag>, RepositoryError>;
    async fn create_tag(&self, tag: AssetTag) -> Result<AssetTag, RepositoryError>;
    async fn update_tag(
        &self,
        project_id: &str,
        tag_id: &str,
        name: &str,
        normalized_name: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<Option<AssetTag>, RepositoryError>;
    async fn delete_tag(&self, project_id: &str, tag_id: &str) -> Result<bool, RepositoryError>;
    async fn assign_tag(
        &self,
        project_id: &str,
        asset_id: &str,
        tag_id: &str,
        created_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError>;
    async fn remove_tag(
        &self,
        project_id: &str,
        asset_id: &str,
        tag_id: &str,
    ) -> Result<(), RepositoryError>;
    async fn set_favorite(
        &self,
        project_id: &str,
        asset_id: &str,
        favorite: bool,
        created_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError>;
    async fn bulk_set_favorite(
        &self,
        project_id: &str,
        asset_ids: &[String],
        favorite: bool,
        created_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError>;
    async fn bulk_add_tag(
        &self,
        project_id: &str,
        asset_ids: &[String],
        tag_id: &str,
        created_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError>;
    async fn bulk_remove_tag(
        &self,
        project_id: &str,
        asset_ids: &[String],
        tag_id: &str,
    ) -> Result<(), RepositoryError>;
    async fn organization_for_assets(
        &self,
        project_id: &str,
        asset_ids: &[String],
    ) -> Result<HashMap<String, AssetOrganization>, RepositoryError>;
    async fn create_template(
        &self,
        template: NewProjectTemplate,
    ) -> Result<ProjectTemplate, RepositoryError>;
    async fn list_templates(&self) -> Result<Vec<ProjectTemplate>, RepositoryError>;
    async fn find_template(
        &self,
        template_id: &str,
    ) -> Result<Option<ProjectTemplate>, RepositoryError>;
    async fn update_template(
        &self,
        template_id: &str,
        name: &str,
        normalized_name: &str,
        description: Option<&str>,
        updated_at: DateTime<Utc>,
    ) -> Result<Option<ProjectTemplate>, RepositoryError>;
    async fn delete_template(&self, template_id: &str) -> Result<bool, RepositoryError>;
}
