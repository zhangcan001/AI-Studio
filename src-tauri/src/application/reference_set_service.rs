use crate::application::ports::{
    AssetRepository, Clock, ConsistencyProfileRepository, ProjectRepository,
    ReferenceAnchorRepository, ReferenceSetRepository, RepositoryError,
};
use crate::domain::consistency::{
    validate_reference_set, validate_reference_set_items, ProfileType, ReferenceSet,
    ReferenceSetItem, ReferenceSetPurpose,
};
use crate::domain::{AssetId, AssetType, ReferenceAnchorId, ReferenceAnchorKind};
use std::{collections::HashMap, error::Error, fmt, sync::Arc};

pub const MAX_REFERENCE_SET_ITEMS: usize = 20;

#[derive(Clone, Debug)]
pub struct ReferenceSetItemRequest {
    pub asset_id: String,
    pub ordinal: i64,
    pub role: Option<String>,
    pub is_primary: bool,
}

pub type ReferenceSetItemInput = ReferenceSetItemRequest;
pub type CreateReferenceSetItemRequest = ReferenceSetItemRequest;

#[derive(Clone, Debug)]
pub struct CreateReferenceSetRequest {
    pub project_id: String,
    pub name: String,
    pub purpose: ReferenceSetPurpose,
    pub description: String,
    pub owner_profile_type: Option<ProfileType>,
    pub owner_profile_id: Option<String>,
    pub items: Vec<ReferenceSetItemRequest>,
}

#[derive(Clone, Debug)]
pub struct UpdateReferenceSetRequest {
    pub project_id: String,
    pub reference_set_id: String,
    pub name: String,
    pub purpose: ReferenceSetPurpose,
    pub description: String,
    pub owner_profile_type: Option<ProfileType>,
    pub owner_profile_id: Option<String>,
    pub items: Vec<ReferenceSetItemRequest>,
}

pub struct ReferenceSetService {
    repository: Arc<dyn ReferenceSetRepository>,
    profile_repository: Arc<dyn ConsistencyProfileRepository>,
    asset_repository: Arc<dyn AssetRepository>,
    reference_anchor_repository: Arc<dyn ReferenceAnchorRepository>,
    project_repository: Arc<dyn ProjectRepository>,
    clock: Arc<dyn Clock>,
}

impl ReferenceSetService {
    pub fn new(
        repository: Arc<dyn ReferenceSetRepository>,
        profile_repository: Arc<dyn ConsistencyProfileRepository>,
        asset_repository: Arc<dyn AssetRepository>,
        reference_anchor_repository: Arc<dyn ReferenceAnchorRepository>,
        project_repository: Arc<dyn ProjectRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repository,
            profile_repository,
            asset_repository,
            reference_anchor_repository,
            project_repository,
            clock,
        }
    }

    pub async fn list(
        &self,
        project_id: &str,
        purpose: Option<ReferenceSetPurpose>,
    ) -> Result<Vec<ReferenceSet>, ReferenceSetError> {
        let project_id = self.ensure_project(project_id).await?;
        Ok(self
            .repository
            .list_reference_sets(&project_id, purpose)
            .await?)
    }

    pub async fn get(
        &self,
        project_id: &str,
        reference_set_id: &str,
    ) -> Result<ReferenceSet, ReferenceSetError> {
        let project_id = self.ensure_project(project_id).await?;
        let reference_set_id = required_id(reference_set_id, "reference set")?;
        self.repository
            .find_reference_set(&project_id, &reference_set_id)
            .await?
            .ok_or_else(|| ReferenceSetError::not_found(reference_set_id))
    }

    pub async fn create(
        &self,
        request: CreateReferenceSetRequest,
    ) -> Result<ReferenceSet, ReferenceSetError> {
        let project_id = self.ensure_project(&request.project_id).await?;
        let name = validate_name(&request.name)?;
        let owner_profile_id = normalize_optional_id(request.owner_profile_id)?;
        validate_owner_pair(request.owner_profile_type, owner_profile_id.as_deref())?;
        let now = self.clock.now();
        let reference_set = ReferenceSet {
            id: crate::domain::consistency::generate_consistency_id(
                crate::domain::consistency::ConsistencyIdKind::ReferenceSet,
            ),
            project_id,
            name,
            purpose: request.purpose,
            description: request.description,
            owner_profile_type: request.owner_profile_type,
            owner_profile_id,
            active_revision_id: None,
            created_at: now,
            updated_at: now,
        };
        validate_reference_set(&reference_set).map_err(ReferenceSetError::from)?;
        self.validate_owner(&reference_set).await?;
        let items = self
            .build_items(
                &reference_set.project_id,
                &reference_set.id,
                request.items,
                now,
            )
            .await?;

        self.repository.insert_reference_set(&reference_set).await?;
        self.repository
            .replace_items(&reference_set.id, &items)
            .await?;
        Ok(reference_set)
    }

    pub async fn update(
        &self,
        request: UpdateReferenceSetRequest,
    ) -> Result<ReferenceSet, ReferenceSetError> {
        let project_id = self.ensure_project(&request.project_id).await?;
        let reference_set_id = required_id(&request.reference_set_id, "reference set")?;
        let existing = self
            .repository
            .find_reference_set(&project_id, &reference_set_id)
            .await?
            .ok_or_else(|| ReferenceSetError::not_found(reference_set_id.clone()))?;
        if existing.project_id != project_id {
            return Err(ReferenceSetError::project_mismatch(format!(
                "reference set {reference_set_id} belongs to another project"
            )));
        }
        let name = validate_name(&request.name)?;
        let owner_profile_id = normalize_optional_id(request.owner_profile_id)?;
        validate_owner_pair(request.owner_profile_type, owner_profile_id.as_deref())?;
        let updated_at = self.clock.now();
        let reference_set = ReferenceSet {
            id: existing.id,
            project_id: existing.project_id,
            name,
            purpose: request.purpose,
            description: request.description,
            owner_profile_type: request.owner_profile_type,
            owner_profile_id,
            active_revision_id: existing.active_revision_id,
            created_at: existing.created_at,
            updated_at,
        };
        validate_reference_set(&reference_set).map_err(ReferenceSetError::from)?;
        self.validate_owner(&reference_set).await?;
        let items = self
            .build_items(
                &reference_set.project_id,
                &reference_set.id,
                request.items,
                updated_at,
            )
            .await?;

        if !self.repository.update_reference_set(&reference_set).await? {
            return Err(ReferenceSetError::not_found(reference_set.id));
        }
        self.repository
            .replace_items(&reference_set.id, &items)
            .await?;
        Ok(reference_set)
    }

    pub async fn delete(
        &self,
        project_id: &str,
        reference_set_id: &str,
    ) -> Result<(), ReferenceSetError> {
        let project_id = self.ensure_project(project_id).await?;
        let reference_set_id = required_id(reference_set_id, "reference set")?;
        self.repository
            .find_reference_set(&project_id, &reference_set_id)
            .await?
            .ok_or_else(|| ReferenceSetError::not_found(reference_set_id.clone()))?;
        if !self
            .repository
            .delete_reference_set(&project_id, &reference_set_id)
            .await?
        {
            return Err(ReferenceSetError::not_found(reference_set_id));
        }
        Ok(())
    }

    pub async fn replace_items(
        &self,
        project_id: &str,
        reference_set_id: &str,
        items: Vec<ReferenceSetItemRequest>,
    ) -> Result<Vec<ReferenceSetItem>, ReferenceSetError> {
        let project_id = self.ensure_project(project_id).await?;
        let reference_set_id = required_id(reference_set_id, "reference set")?;
        let reference_set = self
            .repository
            .find_reference_set(&project_id, &reference_set_id)
            .await?
            .ok_or_else(|| ReferenceSetError::not_found(reference_set_id.clone()))?;
        if reference_set.project_id != project_id {
            return Err(ReferenceSetError::project_mismatch(format!(
                "reference set {reference_set_id} belongs to another project"
            )));
        }
        let items = self
            .build_items(&project_id, &reference_set_id, items, self.clock.now())
            .await?;
        self.repository
            .replace_items(&reference_set_id, &items)
            .await?;
        Ok(items)
    }

    pub async fn create_from_anchor(
        &self,
        project_id: &str,
        anchor_id: &str,
        new_name: Option<String>,
    ) -> Result<ReferenceSet, ReferenceSetError> {
        let project_id = self.ensure_project(project_id).await?;
        let anchor_id = ReferenceAnchorId::parse(anchor_id.trim().to_owned()).map_err(|error| {
            ReferenceSetError::invalid_input(format!("REFERENCE_ANCHOR_ID_INVALID: {error}"))
        })?;
        let anchor = self
            .reference_anchor_repository
            .find(&project_id, &anchor_id)
            .await?
            .ok_or_else(|| ReferenceSetError::not_found(anchor_id.to_string()))?;
        if anchor.anchor.project_id != project_id {
            return Err(ReferenceSetError::project_mismatch(format!(
                "reference anchor {} belongs to another project",
                anchor.anchor.id
            )));
        }

        let purpose = match anchor.anchor.kind {
            ReferenceAnchorKind::Character => ReferenceSetPurpose::Character,
            ReferenceAnchorKind::Scene => ReferenceSetPurpose::Scene,
            ReferenceAnchorKind::Prop => ReferenceSetPurpose::Prop,
            ReferenceAnchorKind::Style => ReferenceSetPurpose::Style,
        };
        let mut ordered_assets = anchor.assets;
        ordered_assets.sort_by_key(|asset| asset.ordinal);
        let items = ordered_assets
            .into_iter()
            .enumerate()
            .map(|(index, asset)| ReferenceSetItemRequest {
                asset_id: asset.asset_id.as_str().to_owned(),
                ordinal: index as i64,
                role: None,
                is_primary: index == 0,
            })
            .collect();

        self.create(CreateReferenceSetRequest {
            project_id,
            name: new_name.unwrap_or(anchor.anchor.name),
            purpose,
            description: anchor.anchor.description,
            owner_profile_type: None,
            owner_profile_id: None,
            items,
        })
        .await
    }

    async fn ensure_project(&self, project_id: &str) -> Result<String, ReferenceSetError> {
        let project_id = required_id(project_id, "project")?;
        if self
            .project_repository
            .find_by_id(&project_id)
            .await?
            .is_none()
        {
            return Err(ReferenceSetError::not_found(format!(
                "PROJECT_NOT_FOUND: {project_id}"
            )));
        }
        Ok(project_id)
    }

    async fn validate_owner(&self, reference_set: &ReferenceSet) -> Result<(), ReferenceSetError> {
        let Some(profile_type) = reference_set.owner_profile_type else {
            return Ok(());
        };
        let profile_id = reference_set.owner_profile_id.as_deref().ok_or_else(|| {
            ReferenceSetError::invalid_input(
                "REFERENCE_SET_OWNER_INVALID: owner profile id is required".to_owned(),
            )
        })?;
        let expected_type = expected_owner_type(reference_set.purpose);
        if expected_type != Some(profile_type) {
            return Err(ReferenceSetError::invalid_input(format!(
                "REFERENCE_SET_OWNER_TYPE_MISMATCH: purpose {:?} cannot be owned by {:?}",
                reference_set.purpose, profile_type
            )));
        }
        let profile = self
            .profile_repository
            .find_profile(&reference_set.project_id, profile_type, profile_id)
            .await?
            .ok_or_else(|| {
                ReferenceSetError::not_found(format!("PROFILE_NOT_FOUND: {profile_id}"))
            })?;
        if profile.project_id() != reference_set.project_id {
            return Err(ReferenceSetError::project_mismatch(format!(
                "profile {profile_id} belongs to another project"
            )));
        }
        if profile.profile_type() != profile_type {
            return Err(ReferenceSetError::invalid_input(format!(
                "REFERENCE_SET_OWNER_TYPE_MISMATCH: profile {profile_id} has type {:?}",
                profile.profile_type()
            )));
        }
        Ok(())
    }

    async fn build_items(
        &self,
        project_id: &str,
        reference_set_id: &str,
        requests: Vec<ReferenceSetItemRequest>,
        created_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<ReferenceSetItem>, ReferenceSetError> {
        if requests.len() > MAX_REFERENCE_SET_ITEMS {
            return Err(ReferenceSetError::invalid_input(format!(
                "REFERENCE_SET_ITEM_LIMIT: at most {MAX_REFERENCE_SET_ITEMS} images are allowed"
            )));
        }

        let mut parsed_ids = Vec::with_capacity(requests.len());
        let mut items = Vec::with_capacity(requests.len());
        for request in requests {
            let asset_id = AssetId::parse(request.asset_id.trim().to_owned()).map_err(|error| {
                ReferenceSetError::invalid_input(format!("REFERENCE_SET_ASSET_ID_INVALID: {error}"))
            })?;
            parsed_ids.push(asset_id.clone());
            items.push(ReferenceSetItem {
                reference_set_id: reference_set_id.to_owned(),
                asset_id: asset_id.as_str().to_owned(),
                ordinal: request.ordinal,
                role: request.role,
                is_primary: request.is_primary,
                created_at,
            });
        }
        validate_reference_set_items(&items).map_err(ReferenceSetError::from)?;

        let assets = self.asset_repository.find_many_by_ids(&parsed_ids).await?;
        let assets_by_id: HashMap<&str, _> = assets
            .iter()
            .map(|asset| (asset.id.as_str(), asset))
            .collect();
        for asset_id in &parsed_ids {
            let asset = assets_by_id
                .get(asset_id.as_str())
                .ok_or_else(|| ReferenceSetError::asset_not_found(asset_id.as_str().to_owned()))?;
            if asset.project_id != project_id {
                return Err(ReferenceSetError::project_mismatch(format!(
                    "asset {} belongs to another project",
                    asset_id.as_str()
                )));
            }
            if asset.asset_type != AssetType::Image {
                return Err(ReferenceSetError::image_required(
                    asset_id.as_str().to_owned(),
                ));
            }
        }
        Ok(items)
    }
}

fn expected_owner_type(purpose: ReferenceSetPurpose) -> Option<ProfileType> {
    match purpose {
        ReferenceSetPurpose::Character => Some(ProfileType::Character),
        ReferenceSetPurpose::Costume => Some(ProfileType::Character),
        ReferenceSetPurpose::Scene => Some(ProfileType::Scene),
        ReferenceSetPurpose::Prop => Some(ProfileType::Prop),
        ReferenceSetPurpose::Style => Some(ProfileType::Style),
        ReferenceSetPurpose::Shot => None,
    }
}

fn validate_name(value: &str) -> Result<String, ReferenceSetError> {
    crate::domain::consistency::validate_profile_name(value).map_err(ReferenceSetError::from)?;
    Ok(value.trim().to_owned())
}

fn validate_owner_pair(
    profile_type: Option<ProfileType>,
    profile_id: Option<&str>,
) -> Result<(), ReferenceSetError> {
    match (profile_type, profile_id) {
        (Some(_), Some(profile_id)) if !profile_id.trim().is_empty() => Ok(()),
        (None, None) => Ok(()),
        _ => Err(ReferenceSetError::invalid_input(
            "REFERENCE_SET_OWNER_INVALID: owner_profile_type and owner_profile_id must be set together"
                .to_owned(),
        )),
    }
}

fn normalize_optional_id(value: Option<String>) -> Result<Option<String>, ReferenceSetError> {
    value
        .map(|value| required_id(&value, "owner profile"))
        .transpose()
}

fn required_id(value: &str, kind: &str) -> Result<String, ReferenceSetError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ReferenceSetError::invalid_input(format!(
            "INVALID_{}_ID: id must not be empty",
            kind.replace(' ', "_").to_ascii_uppercase()
        )));
    }
    Ok(value.to_owned())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReferenceSetError {
    InvalidInput(String),
    NotFound(String),
    ProjectMismatch(String),
    AssetNotFound(String),
    ImageRequired(String),
    Conflict(String),
    Repository(RepositoryError),
}

pub type ReferenceSetServiceError = ReferenceSetError;

impl ReferenceSetError {
    fn invalid_input(message: String) -> Self {
        Self::InvalidInput(message)
    }

    fn not_found(message: String) -> Self {
        Self::NotFound(message)
    }

    fn project_mismatch(message: String) -> Self {
        Self::ProjectMismatch(message)
    }

    fn asset_not_found(message: String) -> Self {
        Self::AssetNotFound(message)
    }

    fn image_required(message: String) -> Self {
        Self::ImageRequired(message)
    }
}

impl fmt::Display for ReferenceSetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => {
                write!(formatter, "REFERENCE_SET_INVALID_INPUT: {message}")
            }
            Self::NotFound(message) => write!(formatter, "REFERENCE_SET_NOT_FOUND: {message}"),
            Self::ProjectMismatch(message) => {
                write!(formatter, "REFERENCE_SET_PROJECT_MISMATCH: {message}")
            }
            Self::AssetNotFound(message) => {
                write!(formatter, "REFERENCE_SET_ASSET_NOT_FOUND: {message}")
            }
            Self::ImageRequired(message) => {
                write!(formatter, "REFERENCE_SET_IMAGE_REQUIRED: {message}")
            }
            Self::Conflict(message) => write!(formatter, "REFERENCE_SET_CONFLICT: {message}"),
            Self::Repository(error) => write!(formatter, "REFERENCE_SET_REPOSITORY: {error}"),
        }
    }
}

impl Error for ReferenceSetError {}

impl From<RepositoryError> for ReferenceSetError {
    fn from(error: RepositoryError) -> Self {
        match error {
            RepositoryError::NotFound { entity, id } => Self::NotFound(format!("{entity} {id}")),
            RepositoryError::Integrity { message } => Self::Conflict(message),
            error => Self::Repository(error),
        }
    }
}

impl From<crate::domain::consistency::ConsistencyValidationError> for ReferenceSetError {
    fn from(error: crate::domain::consistency::ConsistencyValidationError) -> Self {
        Self::InvalidInput(error.to_string())
    }
}
