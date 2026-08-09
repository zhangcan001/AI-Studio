use crate::application::ports::{AssetTag, Clock, OrganizationRepository, RepositoryError};
use std::{error::Error, fmt, sync::Arc};
use uuid::Uuid;

pub struct OrganizationService {
    repository: Arc<dyn OrganizationRepository>,
    clock: Arc<dyn Clock>,
}

impl OrganizationService {
    pub fn new(repository: Arc<dyn OrganizationRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repository, clock }
    }

    pub async fn list_tags(&self, project_id: &str) -> Result<Vec<AssetTag>, OrganizationError> {
        validate_id(project_id, "project")?;
        Ok(self.repository.list_tags(project_id).await?)
    }

    pub async fn create_tag(
        &self,
        project_id: &str,
        name: &str,
    ) -> Result<AssetTag, OrganizationError> {
        validate_id(project_id, "project")?;
        let (name, normalized_name) = normalize_name(name, 32, "ASSET_TAG")?;
        let now = self.clock.now();
        Ok(self
            .repository
            .create_tag(AssetTag {
                id: format!("tag_{}", Uuid::new_v4()),
                project_id: project_id.to_owned(),
                name,
                normalized_name,
                created_at: now,
                updated_at: now,
            })
            .await?)
    }

    pub async fn rename_tag(
        &self,
        project_id: &str,
        tag_id: &str,
        name: &str,
    ) -> Result<AssetTag, OrganizationError> {
        validate_id(project_id, "project")?;
        validate_id(tag_id, "tag")?;
        let (name, normalized_name) = normalize_name(name, 32, "ASSET_TAG")?;
        self.repository
            .update_tag(
                project_id,
                tag_id,
                &name,
                &normalized_name,
                self.clock.now(),
            )
            .await?
            .ok_or_else(|| {
                OrganizationError::NotFound(format!(
                    "ASSET_TAG_NOT_FOUND: tag {tag_id} was not found"
                ))
            })
    }

    pub async fn delete_tag(
        &self,
        project_id: &str,
        tag_id: &str,
    ) -> Result<(), OrganizationError> {
        validate_id(project_id, "project")?;
        validate_id(tag_id, "tag")?;
        if !self.repository.delete_tag(project_id, tag_id).await? {
            return Err(OrganizationError::NotFound(format!(
                "ASSET_TAG_NOT_FOUND: tag {tag_id} was not found"
            )));
        }
        Ok(())
    }

    pub async fn assign_tag(
        &self,
        project_id: &str,
        asset_id: &str,
        tag_id: &str,
    ) -> Result<(), OrganizationError> {
        validate_three(project_id, asset_id, tag_id)?;
        Ok(self
            .repository
            .assign_tag(project_id, asset_id, tag_id, self.clock.now())
            .await?)
    }

    pub async fn remove_tag(
        &self,
        project_id: &str,
        asset_id: &str,
        tag_id: &str,
    ) -> Result<(), OrganizationError> {
        validate_three(project_id, asset_id, tag_id)?;
        Ok(self
            .repository
            .remove_tag(project_id, asset_id, tag_id)
            .await?)
    }

    pub async fn set_favorite(
        &self,
        project_id: &str,
        asset_id: &str,
        favorite: bool,
    ) -> Result<(), OrganizationError> {
        validate_id(project_id, "project")?;
        validate_id(asset_id, "asset")?;
        Ok(self
            .repository
            .set_favorite(project_id, asset_id, favorite, self.clock.now())
            .await?)
    }
}

fn validate_three(project_id: &str, asset_id: &str, tag_id: &str) -> Result<(), OrganizationError> {
    validate_id(project_id, "project")?;
    validate_id(asset_id, "asset")?;
    validate_id(tag_id, "tag")
}

fn validate_id(value: &str, kind: &str) -> Result<(), OrganizationError> {
    if value.trim().is_empty() {
        return Err(OrganizationError::InvalidInput(format!(
            "INVALID_{}_ID: id must not be empty",
            kind.to_ascii_uppercase()
        )));
    }
    Ok(())
}

pub(crate) fn normalize_name(
    value: &str,
    max: usize,
    prefix: &str,
) -> Result<(String, String), OrganizationError> {
    let name = value.trim();
    if name.is_empty() || name.contains(['\r', '\n']) {
        return Err(OrganizationError::InvalidInput(format!(
            "{prefix}_NAME_INVALID: name must be a single non-empty line"
        )));
    }
    if name.chars().count() > max {
        return Err(OrganizationError::InvalidInput(format!(
            "{prefix}_NAME_TOO_LONG: name must be at most {max} characters"
        )));
    }
    Ok((name.to_owned(), name.to_lowercase()))
}

#[derive(Debug)]
pub enum OrganizationError {
    InvalidInput(String),
    NotFound(String),
    Repository(RepositoryError),
}

impl fmt::Display for OrganizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) | Self::NotFound(message) => formatter.write_str(message),
            Self::Repository(error) => write!(formatter, "{error}"),
        }
    }
}
impl Error for OrganizationError {}
impl From<RepositoryError> for OrganizationError {
    fn from(value: RepositoryError) -> Self {
        Self::Repository(value)
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_name;

    #[test]
    fn trims_chinese_and_normalizes_unicode_case() {
        assert_eq!(
            normalize_name("  人物  ", 32, "TAG").unwrap(),
            ("人物".to_owned(), "人物".to_owned())
        );
        assert_eq!(
            normalize_name("Character", 32, "TAG").unwrap().1,
            "character"
        );
        assert!(normalize_name("bad\nname", 32, "TAG").is_err());
    }
}
