use crate::application::ports::{
    ConsistencyProfileRepository, ConsistencyScopeRepository, ProductionStructureRepository,
    ProductionStructureTreeData, ReferenceSetRepository, RepositoryError,
};
use crate::domain::consistency::{
    BindingRole, ConsistencyProfileRecord, ConsistencyScopeType, CostumeVariant, ProfileType,
    ReferenceSet, ScopedProfileBinding, ScopedReferenceSetBinding,
};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
    sync::Arc,
};

pub struct ConsistencyScopeBindingService {
    repository: Arc<dyn ConsistencyScopeRepository>,
    profile_repository: Arc<dyn ConsistencyProfileRepository>,
    reference_set_repository: Arc<dyn ReferenceSetRepository>,
    structure_repository: Arc<dyn ProductionStructureRepository>,
}

impl ConsistencyScopeBindingService {
    pub fn new(
        repository: Arc<dyn ConsistencyScopeRepository>,
        profile_repository: Arc<dyn ConsistencyProfileRepository>,
        reference_set_repository: Arc<dyn ReferenceSetRepository>,
        structure_repository: Arc<dyn ProductionStructureRepository>,
    ) -> Self {
        Self {
            repository,
            profile_repository,
            reference_set_repository,
            structure_repository,
        }
    }

    pub async fn list_profile_bindings_for_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<ScopedProfileBinding>, ConsistencyScopeBindingError> {
        Ok(self
            .repository
            .list_profile_bindings_for_project(project_id)
            .await?)
    }

    pub async fn list_reference_set_bindings_for_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<ScopedReferenceSetBinding>, ConsistencyScopeBindingError> {
        Ok(self
            .repository
            .list_reference_set_bindings_for_project(project_id)
            .await?)
    }

    pub async fn replace_profile_bindings(
        &self,
        project_id: &str,
        scope_type: ConsistencyScopeType,
        scope_id: &str,
        bindings: &[ScopedProfileBinding],
    ) -> Result<(), ConsistencyScopeBindingError> {
        let tree = self.structure_repository.load_tree_data(project_id).await?;
        validate_scope_membership(project_id, scope_type, scope_id, &tree)?;
        validate_profile_conflicts(scope_type, scope_id, bindings)?;
        self.validate_profiles(project_id, bindings).await?;
        self.repository
            .replace_profile_bindings(project_id, scope_type, scope_id, bindings)
            .await?;
        Ok(())
    }

    pub async fn replace_reference_set_bindings(
        &self,
        project_id: &str,
        scope_type: ConsistencyScopeType,
        scope_id: &str,
        bindings: &[ScopedReferenceSetBinding],
    ) -> Result<(), ConsistencyScopeBindingError> {
        let tree = self.structure_repository.load_tree_data(project_id).await?;
        validate_scope_membership(project_id, scope_type, scope_id, &tree)?;
        validate_reference_set_conflicts(scope_type, scope_id, bindings)?;
        self.validate_reference_sets(project_id, bindings).await?;
        self.repository
            .replace_reference_set_bindings(project_id, scope_type, scope_id, bindings)
            .await?;
        Ok(())
    }

    async fn validate_profiles(
        &self,
        project_id: &str,
        bindings: &[ScopedProfileBinding],
    ) -> Result<(), ConsistencyScopeBindingError> {
        let mut requested_types = HashSet::new();
        for binding in bindings {
            if binding.role == BindingRole::ShotReference {
                return Err(ConsistencyScopeBindingError::invalid(
                    "CONSISTENCY_SCOPE_PROFILE_ROLE_INVALID: SHOT_REFERENCE is not a profile role",
                ));
            }
            let expected = expected_profile_type(binding.role).ok_or_else(|| {
                ConsistencyScopeBindingError::invalid(
                    "CONSISTENCY_SCOPE_PROFILE_ROLE_INVALID: unsupported profile role",
                )
            })?;
            if binding.profile_type != expected {
                return Err(ConsistencyScopeBindingError::invalid(format!(
                    "CONSISTENCY_SCOPE_PROFILE_TYPE_MISMATCH: role {} requires {}",
                    binding.role.as_str(),
                    expected.as_str()
                )));
            }
            if binding.ordinal < 0 || binding.profile_id.trim().is_empty() {
                return Err(ConsistencyScopeBindingError::invalid(
                    "CONSISTENCY_SCOPE_PROFILE_BINDING_INVALID: profile id and ordinal are invalid",
                ));
            }
            requested_types.insert(binding.profile_type);
        }

        let mut profiles = HashMap::new();
        for profile_type in requested_types {
            for profile in self
                .profile_repository
                .list_profiles(project_id, profile_type)
                .await?
            {
                profiles.insert((profile_type, profile.id().to_owned()), profile);
            }
        }

        let mut costume_cache: HashMap<String, HashMap<String, CostumeVariant>> = HashMap::new();
        for binding in bindings {
            let Some(profile) = profiles.get(&(binding.profile_type, binding.profile_id.clone()))
            else {
                return Err(ConsistencyScopeBindingError::project_mismatch(format!(
                    "CONSISTENCY_SCOPE_PROFILE_PROJECT_MISMATCH: profile {} is not in project {}",
                    binding.profile_id, project_id
                )));
            };

            if let Some(costume_variant_id) = binding.costume_variant_id.as_deref() {
                if binding.role != BindingRole::Character {
                    return Err(ConsistencyScopeBindingError::invalid(
                        "CONSISTENCY_SCOPE_COSTUME_ROLE_INVALID: costume variants require CHARACTER",
                    ));
                }
                let character_id = match profile {
                    ConsistencyProfileRecord::Character(character) => character.id.as_str(),
                    _ => {
                        return Err(ConsistencyScopeBindingError::invalid(
                            "CONSISTENCY_SCOPE_COSTUME_PROFILE_INVALID: costume requires CharacterProfile",
                        ))
                    }
                };
                if !costume_cache.contains_key(character_id) {
                    let variants = self
                        .profile_repository
                        .list_costume_variants(character_id)
                        .await?
                        .into_iter()
                        .map(|variant| (variant.id.clone(), variant))
                        .collect();
                    costume_cache.insert(character_id.to_owned(), variants);
                }
                let Some(variant) = costume_cache
                    .get(character_id)
                    .and_then(|variants| variants.get(costume_variant_id))
                else {
                    return Err(ConsistencyScopeBindingError::project_mismatch(format!(
                        "CONSISTENCY_SCOPE_COSTUME_PROJECT_MISMATCH: costume variant {} is not owned by {}",
                        costume_variant_id, character_id
                    )));
                };
                if variant.character_profile_id != character_id {
                    return Err(ConsistencyScopeBindingError::invalid(
                        "CONSISTENCY_SCOPE_COSTUME_OWNER_INVALID: costume variant owner mismatch",
                    ));
                }
            }
        }
        Ok(())
    }

    async fn validate_reference_sets(
        &self,
        project_id: &str,
        bindings: &[ScopedReferenceSetBinding],
    ) -> Result<(), ConsistencyScopeBindingError> {
        let mut cache: HashMap<String, ReferenceSet> = HashMap::new();
        for binding in bindings {
            if binding.ordinal < 0 || binding.reference_set_id.trim().is_empty() {
                return Err(ConsistencyScopeBindingError::invalid(
                    "CONSISTENCY_SCOPE_REFERENCE_BINDING_INVALID: reference set id and ordinal are invalid",
                ));
            }
            if !cache.contains_key(&binding.reference_set_id) {
                let reference_set = self
                    .reference_set_repository
                    .find_reference_set(project_id, &binding.reference_set_id)
                    .await?
                    .ok_or_else(|| {
                        ConsistencyScopeBindingError::project_mismatch(format!(
                            "CONSISTENCY_SCOPE_REFERENCE_PROJECT_MISMATCH: reference set {} is not in project {}",
                            binding.reference_set_id, project_id
                        ))
                    })?;
                if reference_set.project_id != project_id {
                    return Err(ConsistencyScopeBindingError::project_mismatch(format!(
                        "CONSISTENCY_SCOPE_REFERENCE_PROJECT_MISMATCH: reference set {} belongs to another project",
                        binding.reference_set_id
                    )));
                }
                cache.insert(binding.reference_set_id.clone(), reference_set);
            }
        }
        Ok(())
    }
}

fn expected_profile_type(role: BindingRole) -> Option<ProfileType> {
    match role {
        BindingRole::Character => Some(ProfileType::Character),
        BindingRole::Scene => Some(ProfileType::Scene),
        BindingRole::Prop => Some(ProfileType::Prop),
        BindingRole::Style => Some(ProfileType::Style),
        BindingRole::ShotReference => None,
    }
}

fn validate_profile_conflicts(
    scope_type: ConsistencyScopeType,
    scope_id: &str,
    bindings: &[ScopedProfileBinding],
) -> Result<(), ConsistencyScopeBindingError> {
    let mut slots: HashMap<(BindingRole, i64), &str> = HashMap::new();
    for binding in bindings {
        if binding.scope_type != scope_type || binding.scope_id != scope_id {
            return Err(ConsistencyScopeBindingError::invalid(
                "CONSISTENCY_SCOPE_PROFILE_SCOPE_MISMATCH: binding does not match replacement scope",
            ));
        }
        let key = (binding.role, binding.ordinal);
        if let Some(previous) = slots.insert(key, binding.profile_id.as_str()) {
            let code = if previous == binding.profile_id {
                "CONSISTENCY_SCOPE_PROFILE_DUPLICATE"
            } else {
                "CONTEXT_PROFILE_ORDINAL_CONFLICT"
            };
            return Err(ConsistencyScopeBindingError::conflict(format!(
                "{code}: role {} ordinal {} has multiple profiles",
                binding.role.as_str(),
                binding.ordinal
            )));
        }
    }
    Ok(())
}

fn validate_reference_set_conflicts(
    scope_type: ConsistencyScopeType,
    scope_id: &str,
    bindings: &[ScopedReferenceSetBinding],
) -> Result<(), ConsistencyScopeBindingError> {
    let mut slots: HashMap<(BindingRole, i64), &str> = HashMap::new();
    for binding in bindings {
        if binding.scope_type != scope_type || binding.scope_id != scope_id {
            return Err(ConsistencyScopeBindingError::invalid(
                "CONSISTENCY_SCOPE_REFERENCE_SCOPE_MISMATCH: binding does not match replacement scope",
            ));
        }
        let key = (binding.role, binding.ordinal);
        if let Some(previous) = slots.insert(key, binding.reference_set_id.as_str()) {
            let code = if previous == binding.reference_set_id {
                "CONSISTENCY_SCOPE_REFERENCE_DUPLICATE"
            } else {
                "CONTEXT_REFERENCE_ORDINAL_CONFLICT"
            };
            return Err(ConsistencyScopeBindingError::conflict(format!(
                "{code}: role {} ordinal {} has multiple reference sets",
                binding.role.as_str(),
                binding.ordinal
            )));
        }
    }
    Ok(())
}

fn validate_scope_membership(
    project_id: &str,
    scope_type: ConsistencyScopeType,
    scope_id: &str,
    tree: &ProductionStructureTreeData,
) -> Result<(), ConsistencyScopeBindingError> {
    if project_id.trim().is_empty() || scope_id.trim().is_empty() {
        return Err(ConsistencyScopeBindingError::invalid(
            "CONSISTENCY_SCOPE_ID_INVALID: project and scope ids must not be empty",
        ));
    }
    match scope_type {
        ConsistencyScopeType::Project if scope_id == project_id => Ok(()),
        ConsistencyScopeType::Project => Err(ConsistencyScopeBindingError::project_mismatch(
            "CONSISTENCY_SCOPE_PROJECT_MISMATCH: PROJECT scope_id must equal project_id",
        )),
        ConsistencyScopeType::Series => tree
            .series
            .iter()
            .any(|series| series.id.as_str() == scope_id && series.project_id == project_id)
            .then_some(())
            .ok_or_else(|| {
                ConsistencyScopeBindingError::project_mismatch(
                    "CONSISTENCY_SCOPE_SERIES_PROJECT_MISMATCH: series is not in project",
                )
            }),
        ConsistencyScopeType::Episode => {
            let Some(episode) = tree
                .episodes
                .iter()
                .find(|episode| episode.id.as_str() == scope_id)
            else {
                return Err(ConsistencyScopeBindingError::not_found(
                    "CONSISTENCY_SCOPE_EPISODE_NOT_FOUND: episode is not in structure tree",
                ));
            };
            let belongs = tree
                .series
                .iter()
                .any(|series| series.id == episode.series_id && series.project_id == project_id);
            if belongs {
                Ok(())
            } else {
                Err(ConsistencyScopeBindingError::project_mismatch(
                    "CONSISTENCY_SCOPE_EPISODE_PROJECT_MISMATCH: episode parent is not in project",
                ))
            }
        }
        ConsistencyScopeType::Scene => {
            let Some(scene) = tree
                .scenes
                .iter()
                .find(|scene| scene.id.as_str() == scope_id)
            else {
                return Err(ConsistencyScopeBindingError::not_found(
                    "CONSISTENCY_SCOPE_SCENE_NOT_FOUND: scene is not in structure tree",
                ));
            };
            let belongs = tree.episodes.iter().any(|episode| {
                episode.id == scene.episode_id
                    && tree.series.iter().any(|series| {
                        series.id == episode.series_id && series.project_id == project_id
                    })
            });
            if belongs {
                Ok(())
            } else {
                Err(ConsistencyScopeBindingError::project_mismatch(
                    "CONSISTENCY_SCOPE_SCENE_PROJECT_MISMATCH: scene ancestry is not in project",
                ))
            }
        }
    }
}

#[derive(Debug)]
pub enum ConsistencyScopeBindingError {
    InvalidInput(String),
    NotFound(String),
    ProjectMismatch(String),
    Conflict(String),
    Repository(RepositoryError),
}

impl ConsistencyScopeBindingError {
    fn invalid(message: impl Into<String>) -> Self {
        Self::InvalidInput(message.into())
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    fn project_mismatch(message: impl Into<String>) -> Self {
        Self::ProjectMismatch(message.into())
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict(message.into())
    }
}

impl fmt::Display for ConsistencyScopeBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message)
            | Self::NotFound(message)
            | Self::ProjectMismatch(message)
            | Self::Conflict(message) => formatter.write_str(message),
            Self::Repository(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for ConsistencyScopeBindingError {}

impl From<RepositoryError> for ConsistencyScopeBindingError {
    fn from(value: RepositoryError) -> Self {
        Self::Repository(value)
    }
}
