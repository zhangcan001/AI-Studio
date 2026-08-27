use crate::application::ports::{
    Clock, ConsistencyProfileRepository, ConsistencyScopeRepository, ProductionStructureRepository,
    ProductionStructureTreeData, ReferenceSetRepository, RepositoryError,
};
use crate::domain::consistency::{
    generate_consistency_id, validate_consistency_id, BindingRole, ConsistencyIdKind,
    ConsistencyProfileRecord, ConsistencyScopeType, CostumeVariant, InheritanceMode, ProfileType,
    ReferenceSet, ReferenceSetPurpose, ScopedProfileBinding, ScopedReferenceSetBinding,
};
use chrono::{DateTime, Utc};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
    sync::Arc,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsistencyProfileBindingInput {
    pub id: Option<String>,
    pub role: BindingRole,
    pub profile_type: ProfileType,
    pub profile_id: String,
    pub costume_variant_id: Option<String>,
    pub ordinal: i64,
    pub inheritance_mode: InheritanceMode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsistencyReferenceSetBindingInput {
    pub id: Option<String>,
    pub role: BindingRole,
    pub reference_set_id: String,
    pub ordinal: i64,
    pub required: bool,
    pub inheritance_mode: InheritanceMode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsistencyScopeBindingPack {
    pub project_id: String,
    pub scope_type: ConsistencyScopeType,
    pub scope_id: String,
    pub scope_name: String,
    pub ancestors: Vec<ConsistencyScopeAncestor>,
    pub direct_profile_bindings: Vec<ScopedProfileBinding>,
    pub direct_reference_set_bindings: Vec<ScopedReferenceSetBinding>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsistencyScopeAncestor {
    pub scope_type: ConsistencyScopeType,
    pub scope_id: String,
    pub scope_name: String,
    pub profile_bindings: Vec<ScopedProfileBinding>,
    pub reference_set_bindings: Vec<ScopedReferenceSetBinding>,
}

pub struct ConsistencyScopeBindingService {
    repository: Arc<dyn ConsistencyScopeRepository>,
    profile_repository: Arc<dyn ConsistencyProfileRepository>,
    reference_set_repository: Arc<dyn ReferenceSetRepository>,
    structure_repository: Arc<dyn ProductionStructureRepository>,
    clock: Arc<dyn Clock>,
}

impl ConsistencyScopeBindingService {
    pub fn new(
        repository: Arc<dyn ConsistencyScopeRepository>,
        profile_repository: Arc<dyn ConsistencyProfileRepository>,
        reference_set_repository: Arc<dyn ReferenceSetRepository>,
        structure_repository: Arc<dyn ProductionStructureRepository>,
    ) -> Self {
        Self::new_with_clock(
            repository,
            profile_repository,
            reference_set_repository,
            structure_repository,
            Arc::new(UtcClock),
        )
    }

    pub fn new_with_clock(
        repository: Arc<dyn ConsistencyScopeRepository>,
        profile_repository: Arc<dyn ConsistencyProfileRepository>,
        reference_set_repository: Arc<dyn ReferenceSetRepository>,
        structure_repository: Arc<dyn ProductionStructureRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repository,
            profile_repository,
            reference_set_repository,
            structure_repository,
            clock,
        }
    }

    pub async fn get_binding_pack(
        &self,
        project_id: &str,
        scope_type: ConsistencyScopeType,
        scope_id: &str,
    ) -> Result<ConsistencyScopeBindingPack, ConsistencyScopeBindingError> {
        let tree = self.structure_repository.load_tree_data(project_id).await?;
        validate_scope_membership(project_id, scope_type, scope_id, &tree)?;
        let (current, ancestors) = scope_path(project_id, scope_type, scope_id, &tree)?;
        let profiles = self
            .repository
            .list_profile_bindings_for_project(project_id)
            .await?;
        let reference_sets = self
            .repository
            .list_reference_set_bindings_for_project(project_id)
            .await?;

        Ok(ConsistencyScopeBindingPack {
            project_id: project_id.to_owned(),
            scope_type,
            scope_id: scope_id.to_owned(),
            scope_name: current.name,
            ancestors: ancestors
                .into_iter()
                .map(|ancestor| ConsistencyScopeAncestor {
                    scope_type: ancestor.scope_type,
                    scope_id: ancestor.scope_id.clone(),
                    scope_name: ancestor.name,
                    profile_bindings: profiles
                        .iter()
                        .filter(|binding| {
                            binding.scope_type == ancestor.scope_type
                                && binding.scope_id == ancestor.scope_id
                        })
                        .cloned()
                        .collect(),
                    reference_set_bindings: reference_sets
                        .iter()
                        .filter(|binding| {
                            binding.scope_type == ancestor.scope_type
                                && binding.scope_id == ancestor.scope_id
                        })
                        .cloned()
                        .collect(),
                })
                .collect(),
            direct_profile_bindings: profiles
                .into_iter()
                .filter(|binding| binding.scope_type == scope_type && binding.scope_id == scope_id)
                .collect(),
            direct_reference_set_bindings: reference_sets
                .into_iter()
                .filter(|binding| binding.scope_type == scope_type && binding.scope_id == scope_id)
                .collect(),
        })
    }

    pub async fn replace_binding_pack(
        &self,
        project_id: &str,
        scope_type: ConsistencyScopeType,
        scope_id: &str,
        profile_inputs: &[ConsistencyProfileBindingInput],
        reference_set_inputs: &[ConsistencyReferenceSetBindingInput],
    ) -> Result<(), ConsistencyScopeBindingError> {
        let tree = self.structure_repository.load_tree_data(project_id).await?;
        validate_scope_membership(project_id, scope_type, scope_id, &tree)?;
        let (_current, ancestors) = scope_path(project_id, scope_type, scope_id, &tree)?;
        let existing_profiles = self
            .repository
            .list_profile_bindings_for_project(project_id)
            .await?
            .into_iter()
            .filter(|binding| binding.scope_type == scope_type && binding.scope_id == scope_id)
            .collect::<Vec<_>>();
        let existing_reference_sets = self
            .repository
            .list_reference_set_bindings_for_project(project_id)
            .await?
            .into_iter()
            .filter(|binding| binding.scope_type == scope_type && binding.scope_id == scope_id)
            .collect::<Vec<_>>();
        let all_profiles = self
            .repository
            .list_profile_bindings_for_project(project_id)
            .await?;
        let all_reference_sets = self
            .repository
            .list_reference_set_bindings_for_project(project_id)
            .await?;
        let ancestor_profiles = all_profiles
            .iter()
            .filter(|binding| {
                ancestors.iter().any(|ancestor| {
                    binding.scope_type == ancestor.scope_type
                        && binding.scope_id == ancestor.scope_id
                })
            })
            .cloned()
            .collect::<Vec<_>>();
        let ancestor_reference_sets = all_reference_sets
            .iter()
            .filter(|binding| {
                ancestors.iter().any(|ancestor| {
                    binding.scope_type == ancestor.scope_type
                        && binding.scope_id == ancestor.scope_id
                })
            })
            .cloned()
            .collect::<Vec<_>>();

        validate_remove_profile_targets(profile_inputs, &ancestor_profiles)?;
        validate_remove_reference_targets(reference_set_inputs, &ancestor_reference_sets)?;
        let now = self.clock.now();
        let profiles = materialize_scope_profiles(
            project_id,
            scope_type,
            scope_id,
            profile_inputs,
            &existing_profiles,
            now,
        )?;
        let reference_sets = materialize_scope_reference_sets(
            project_id,
            scope_type,
            scope_id,
            reference_set_inputs,
            &existing_reference_sets,
            now,
        )?;
        validate_profile_conflicts(scope_type, scope_id, &profiles)?;
        validate_reference_set_conflicts(scope_type, scope_id, &reference_sets)?;
        self.validate_profiles(project_id, &profiles).await?;
        self.validate_reference_sets(project_id, &reference_sets)
            .await?;
        self.repository
            .replace_binding_pack(project_id, scope_type, scope_id, &profiles, &reference_sets)
            .await?;
        Ok(())
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
            if binding
                .costume_variant_id
                .as_deref()
                .is_some_and(|value| value.trim().is_empty())
            {
                return Err(ConsistencyScopeBindingError::invalid(
                    "CONSISTENCY_SCOPE_COSTUME_ID_INVALID: costume variant id must not be empty",
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
            if profile.project_id() != project_id {
                return Err(ConsistencyScopeBindingError::project_mismatch(format!(
                    "CONSISTENCY_SCOPE_PROFILE_PROJECT_MISMATCH: profile {} belongs to another project",
                    binding.profile_id
                )));
            }

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
                if !reference_purpose_allowed(binding.role, reference_set.purpose) {
                    return Err(ConsistencyScopeBindingError::invalid(format!(
                        "CONSISTENCY_SCOPE_REFERENCE_PURPOSE_INVALID: role {} cannot use {} reference set",
                        binding.role.as_str(),
                        reference_set.purpose.as_str()
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

fn reference_purpose_allowed(role: BindingRole, purpose: ReferenceSetPurpose) -> bool {
    match role {
        BindingRole::Character => {
            matches!(
                purpose,
                ReferenceSetPurpose::Character | ReferenceSetPurpose::Costume
            )
        }
        BindingRole::Scene => purpose == ReferenceSetPurpose::Scene,
        BindingRole::Prop => purpose == ReferenceSetPurpose::Prop,
        BindingRole::Style => purpose == ReferenceSetPurpose::Style,
        BindingRole::ShotReference => purpose == ReferenceSetPurpose::Shot,
    }
}

fn validate_profile_conflicts(
    scope_type: ConsistencyScopeType,
    scope_id: &str,
    bindings: &[ScopedProfileBinding],
) -> Result<(), ConsistencyScopeBindingError> {
    let mut slots: HashMap<(BindingRole, i64), &str> = HashMap::new();
    let mut single_role_counts = HashMap::<BindingRole, usize>::new();
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
        if matches!(binding.role, BindingRole::Scene | BindingRole::Style)
            && binding.inheritance_mode != InheritanceMode::Remove
        {
            let count = single_role_counts.entry(binding.role).or_default();
            *count += 1;
            if *count > 1 {
                return Err(ConsistencyScopeBindingError::conflict(format!(
                    "CONTEXT_PROFILE_ORDINAL_CONFLICT: role {} has multiple effective direct bindings",
                    binding.role.as_str()
                )));
            }
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
    let mut single_role_counts = HashMap::<BindingRole, usize>::new();
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
        if matches!(binding.role, BindingRole::Scene | BindingRole::Style)
            && binding.inheritance_mode != InheritanceMode::Remove
        {
            let count = single_role_counts.entry(binding.role).or_default();
            *count += 1;
            if *count > 1 {
                return Err(ConsistencyScopeBindingError::conflict(format!(
                    "CONTEXT_REFERENCE_ORDINAL_CONFLICT: role {} has multiple effective direct bindings",
                    binding.role.as_str()
                )));
            }
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

struct UtcClock;

impl Clock for UtcClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

#[derive(Clone)]
struct ScopePathNode {
    scope_type: ConsistencyScopeType,
    scope_id: String,
    name: String,
}

fn scope_path(
    project_id: &str,
    scope_type: ConsistencyScopeType,
    scope_id: &str,
    tree: &ProductionStructureTreeData,
) -> Result<(ScopePathNode, Vec<ScopePathNode>), ConsistencyScopeBindingError> {
    let project = ScopePathNode {
        scope_type: ConsistencyScopeType::Project,
        scope_id: project_id.to_owned(),
        name: project_id.to_owned(),
    };
    match scope_type {
        ConsistencyScopeType::Project => Ok((project, Vec::new())),
        ConsistencyScopeType::Series => {
            let series = tree
                .series
                .iter()
                .find(|value| value.id.as_str() == scope_id)
                .ok_or_else(|| {
                    ConsistencyScopeBindingError::not_found(
                        "CONSISTENCY_SCOPE_SERIES_NOT_FOUND: series is not in structure tree",
                    )
                })?;
            Ok((
                ScopePathNode {
                    scope_type,
                    scope_id: series.id.as_str().to_owned(),
                    name: series.name.clone(),
                },
                vec![project],
            ))
        }
        ConsistencyScopeType::Episode => {
            let episode = tree
                .episodes
                .iter()
                .find(|value| value.id.as_str() == scope_id)
                .ok_or_else(|| {
                    ConsistencyScopeBindingError::not_found(
                        "CONSISTENCY_SCOPE_EPISODE_NOT_FOUND: episode is not in structure tree",
                    )
                })?;
            let series = tree
                .series
                .iter()
                .find(|value| value.id == episode.series_id)
                .ok_or_else(|| {
                    ConsistencyScopeBindingError::project_mismatch(
                        "CONSISTENCY_SCOPE_EPISODE_PROJECT_MISMATCH: episode parent is not in project",
                    )
                })?;
            Ok((
                ScopePathNode {
                    scope_type,
                    scope_id: episode.id.as_str().to_owned(),
                    name: episode.name.clone(),
                },
                vec![
                    project,
                    ScopePathNode {
                        scope_type: ConsistencyScopeType::Series,
                        scope_id: series.id.as_str().to_owned(),
                        name: series.name.clone(),
                    },
                ],
            ))
        }
        ConsistencyScopeType::Scene => {
            let scene = tree
                .scenes
                .iter()
                .find(|value| value.id.as_str() == scope_id)
                .ok_or_else(|| {
                    ConsistencyScopeBindingError::not_found(
                        "CONSISTENCY_SCOPE_SCENE_NOT_FOUND: scene is not in structure tree",
                    )
                })?;
            let episode = tree
                .episodes
                .iter()
                .find(|value| value.id == scene.episode_id)
                .ok_or_else(|| {
                    ConsistencyScopeBindingError::project_mismatch(
                        "CONSISTENCY_SCOPE_SCENE_PROJECT_MISMATCH: episode parent is not in project",
                    )
                })?;
            let series = tree
                .series
                .iter()
                .find(|value| value.id == episode.series_id)
                .ok_or_else(|| {
                    ConsistencyScopeBindingError::project_mismatch(
                        "CONSISTENCY_SCOPE_SCENE_PROJECT_MISMATCH: series parent is not in project",
                    )
                })?;
            Ok((
                ScopePathNode {
                    scope_type,
                    scope_id: scene.id.as_str().to_owned(),
                    name: scene.name.clone(),
                },
                vec![
                    project,
                    ScopePathNode {
                        scope_type: ConsistencyScopeType::Series,
                        scope_id: series.id.as_str().to_owned(),
                        name: series.name.clone(),
                    },
                    ScopePathNode {
                        scope_type: ConsistencyScopeType::Episode,
                        scope_id: episode.id.as_str().to_owned(),
                        name: episode.name.clone(),
                    },
                ],
            ))
        }
    }
}

fn materialize_scope_profiles(
    project_id: &str,
    scope_type: ConsistencyScopeType,
    scope_id: &str,
    inputs: &[ConsistencyProfileBindingInput],
    existing: &[ScopedProfileBinding],
    now: DateTime<Utc>,
) -> Result<Vec<ScopedProfileBinding>, ConsistencyScopeBindingError> {
    let existing_by_id = existing
        .iter()
        .map(|binding| (binding.id.as_str(), binding))
        .collect::<HashMap<_, _>>();
    let mut ids = HashSet::new();
    inputs
        .iter()
        .map(|input| {
            validate_write_mode(input.inheritance_mode, "profile")?;
            if input.profile_id.trim().is_empty() {
                return Err(ConsistencyScopeBindingError::invalid(
                    "CONSISTENCY_SCOPE_PROFILE_ID_INVALID: profile id must not be empty",
                ));
            }
            if input
                .costume_variant_id
                .as_deref()
                .is_some_and(|value| value.trim().is_empty())
            {
                return Err(ConsistencyScopeBindingError::invalid(
                    "CONSISTENCY_SCOPE_COSTUME_ID_INVALID: costume variant id must not be empty",
                ));
            }
            let id = match input.id.as_deref() {
                Some(id) => {
                    validate_consistency_id(ConsistencyIdKind::HierarchyProfileBinding, id)
                        .map_err(|error| ConsistencyScopeBindingError::invalid(error.to_string()))?;
                    if !existing_by_id.contains_key(id) {
                        return Err(ConsistencyScopeBindingError::not_found(format!(
                            "CONSISTENCY_BINDING_ID_NOT_FOUND: profile binding {id} is not a direct binding at this scope"
                        )));
                    }
                    id.to_owned()
                }
                None => generate_consistency_id(ConsistencyIdKind::HierarchyProfileBinding),
            };
            if !ids.insert(id.clone()) {
                return Err(ConsistencyScopeBindingError::conflict(
                    "CONSISTENCY_BINDING_ID_DUPLICATE: profile binding ids must be unique",
                ));
            }
            let created_at = existing_by_id
                .get(id.as_str())
                .map(|binding| binding.created_at)
                .unwrap_or(now);
            Ok(ScopedProfileBinding {
                id,
                project_id: project_id.to_owned(),
                scope_type,
                scope_id: scope_id.to_owned(),
                role: input.role,
                profile_type: input.profile_type,
                profile_id: input.profile_id.clone(),
                costume_variant_id: input.costume_variant_id.clone(),
                ordinal: input.ordinal,
                inheritance_mode: input.inheritance_mode,
                created_at,
                updated_at: now,
            })
        })
        .collect()
}

fn materialize_scope_reference_sets(
    project_id: &str,
    scope_type: ConsistencyScopeType,
    scope_id: &str,
    inputs: &[ConsistencyReferenceSetBindingInput],
    existing: &[ScopedReferenceSetBinding],
    now: DateTime<Utc>,
) -> Result<Vec<ScopedReferenceSetBinding>, ConsistencyScopeBindingError> {
    let existing_by_id = existing
        .iter()
        .map(|binding| (binding.id.as_str(), binding))
        .collect::<HashMap<_, _>>();
    let mut ids = HashSet::new();
    inputs
        .iter()
        .map(|input| {
            validate_write_mode(input.inheritance_mode, "reference-set")?;
            if input.reference_set_id.trim().is_empty() {
                return Err(ConsistencyScopeBindingError::invalid(
                    "CONSISTENCY_SCOPE_REFERENCE_SET_ID_INVALID: reference set id must not be empty",
                ));
            }
            let id = match input.id.as_deref() {
                Some(id) => {
                    validate_consistency_id(ConsistencyIdKind::HierarchyReferenceSetBinding, id)
                        .map_err(|error| ConsistencyScopeBindingError::invalid(error.to_string()))?;
                    if !existing_by_id.contains_key(id) {
                        return Err(ConsistencyScopeBindingError::not_found(format!(
                            "CONSISTENCY_BINDING_ID_NOT_FOUND: reference-set binding {id} is not a direct binding at this scope"
                        )));
                    }
                    id.to_owned()
                }
                None => generate_consistency_id(ConsistencyIdKind::HierarchyReferenceSetBinding),
            };
            if !ids.insert(id.clone()) {
                return Err(ConsistencyScopeBindingError::conflict(
                    "CONSISTENCY_BINDING_ID_DUPLICATE: reference-set binding ids must be unique",
                ));
            }
            let created_at = existing_by_id
                .get(id.as_str())
                .map(|binding| binding.created_at)
                .unwrap_or(now);
            Ok(ScopedReferenceSetBinding {
                id,
                project_id: project_id.to_owned(),
                scope_type,
                scope_id: scope_id.to_owned(),
                role: input.role,
                reference_set_id: input.reference_set_id.clone(),
                ordinal: input.ordinal,
                required: input.required,
                inheritance_mode: input.inheritance_mode,
                created_at,
                updated_at: now,
            })
        })
        .collect()
}

fn validate_write_mode(
    mode: InheritanceMode,
    entity: &str,
) -> Result<(), ConsistencyScopeBindingError> {
    if mode == InheritanceMode::Inherited {
        return Err(ConsistencyScopeBindingError::invalid(format!(
            "CONSISTENCY_BINDING_INHERITED_NOT_WRITABLE: {entity} bindings with INHERITED mode are read-only"
        )));
    }
    Ok(())
}

fn validate_remove_profile_targets(
    inputs: &[ConsistencyProfileBindingInput],
    ancestors: &[ScopedProfileBinding],
) -> Result<(), ConsistencyScopeBindingError> {
    for input in inputs
        .iter()
        .filter(|input| input.inheritance_mode == InheritanceMode::Remove)
    {
        let found = ancestors.iter().any(|binding| {
            binding.role == input.role
                && binding.profile_type == input.profile_type
                && binding.profile_id == input.profile_id
                && binding.inheritance_mode != InheritanceMode::Remove
        });
        if !found {
            return Err(ConsistencyScopeBindingError::invalid(format!(
                "CONSISTENCY_BINDING_REMOVE_TARGET_INVALID: profile {} must point to an ancestor direct binding",
                input.profile_id
            )));
        }
    }
    Ok(())
}

fn validate_remove_reference_targets(
    inputs: &[ConsistencyReferenceSetBindingInput],
    ancestors: &[ScopedReferenceSetBinding],
) -> Result<(), ConsistencyScopeBindingError> {
    for input in inputs
        .iter()
        .filter(|input| input.inheritance_mode == InheritanceMode::Remove)
    {
        let found = ancestors.iter().any(|binding| {
            binding.role == input.role
                && binding.reference_set_id == input.reference_set_id
                && binding.inheritance_mode != InheritanceMode::Remove
        });
        if !found {
            return Err(ConsistencyScopeBindingError::invalid(format!(
                "CONSISTENCY_BINDING_REMOVE_TARGET_INVALID: reference set {} must point to an ancestor direct binding",
                input.reference_set_id
            )));
        }
    }
    Ok(())
}
