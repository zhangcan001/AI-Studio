use crate::application::consistency_scope_binding_service::{
    ConsistencyProfileBindingInput, ConsistencyReferenceSetBindingInput,
};
use crate::application::ports::{
    Clock, ConsistencyProfileRepository, ReferenceSetRepository, RepositoryError,
    ShotConsistencyRepository, ShotRepository,
};
use crate::domain::consistency::{
    generate_consistency_id, validate_consistency_id, BindingRole, ConsistencyIdKind,
    ConsistencyProfileRecord, InheritanceMode, ProfileType, ReferenceSetPurpose,
    ShotProfileBinding, ShotReferenceSetBinding,
};
use chrono::{DateTime, Utc};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
    sync::Arc,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShotConsistencyBindingPack {
    pub project_id: String,
    pub shot_id: String,
    pub profile_bindings: Vec<ShotProfileBinding>,
    pub reference_set_bindings: Vec<ShotReferenceSetBinding>,
}

pub struct ShotConsistencyBindingService {
    repository: Arc<dyn ShotConsistencyRepository>,
    shot_repository: Arc<dyn ShotRepository>,
    profile_repository: Arc<dyn ConsistencyProfileRepository>,
    reference_set_repository: Arc<dyn ReferenceSetRepository>,
    clock: Arc<dyn Clock>,
}

impl ShotConsistencyBindingService {
    pub fn new(
        repository: Arc<dyn ShotConsistencyRepository>,
        shot_repository: Arc<dyn ShotRepository>,
        profile_repository: Arc<dyn ConsistencyProfileRepository>,
        reference_set_repository: Arc<dyn ReferenceSetRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repository,
            shot_repository,
            profile_repository,
            reference_set_repository,
            clock,
        }
    }

    pub async fn get_binding_pack(
        &self,
        project_id: &str,
        shot_id: &str,
    ) -> Result<ShotConsistencyBindingPack, ShotConsistencyBindingError> {
        self.ensure_shot(project_id, shot_id).await?;
        Ok(ShotConsistencyBindingPack {
            project_id: project_id.to_owned(),
            shot_id: shot_id.to_owned(),
            profile_bindings: self.repository.list_profile_bindings(shot_id).await?,
            reference_set_bindings: self.repository.list_reference_set_bindings(shot_id).await?,
        })
    }

    pub async fn replace_binding_pack(
        &self,
        project_id: &str,
        shot_id: &str,
        profile_inputs: &[ConsistencyProfileBindingInput],
        reference_set_inputs: &[ConsistencyReferenceSetBindingInput],
    ) -> Result<(), ShotConsistencyBindingError> {
        self.ensure_shot(project_id, shot_id).await?;
        let existing_profiles = self.repository.list_profile_bindings(shot_id).await?;
        let existing_reference_sets = self.repository.list_reference_set_bindings(shot_id).await?;

        let now = self.clock.now();
        let profiles = materialize_profiles(shot_id, profile_inputs, &existing_profiles, now)?;
        let reference_sets = materialize_reference_sets(
            shot_id,
            reference_set_inputs,
            &existing_reference_sets,
            now,
        )?;
        validate_profile_conflicts(&profiles)?;
        validate_reference_set_conflicts(&reference_sets)?;
        self.validate_profiles(project_id, &profiles).await?;
        self.validate_reference_sets(project_id, &reference_sets)
            .await?;
        self.repository
            .replace_binding_pack(shot_id, &profiles, &reference_sets)
            .await?;
        Ok(())
    }

    async fn ensure_shot(
        &self,
        project_id: &str,
        shot_id: &str,
    ) -> Result<(), ShotConsistencyBindingError> {
        let Some(data) = self.shot_repository.find(project_id, shot_id).await? else {
            return Err(ShotConsistencyBindingError::not_found(format!(
                "CONSISTENCY_SHOT_NOT_FOUND: shot {shot_id} is not in project {project_id}"
            )));
        };
        if data.shot.project_id != project_id {
            return Err(ShotConsistencyBindingError::project_mismatch(format!(
                "CONSISTENCY_SHOT_PROJECT_MISMATCH: shot {shot_id} belongs to another project"
            )));
        }
        Ok(())
    }

    async fn validate_profiles(
        &self,
        project_id: &str,
        bindings: &[ShotProfileBinding],
    ) -> Result<(), ShotConsistencyBindingError> {
        let requested_types = bindings
            .iter()
            .map(|binding| binding.profile_type)
            .collect::<HashSet<_>>();
        let mut profiles = HashMap::<(ProfileType, String), ConsistencyProfileRecord>::new();
        for profile_type in requested_types {
            for profile in self
                .profile_repository
                .list_profiles(project_id, profile_type)
                .await?
            {
                if profile.project_id() != project_id {
                    return Err(ShotConsistencyBindingError::project_mismatch(format!(
                        "CONSISTENCY_PROFILE_PROJECT_MISMATCH: profile {} belongs to another project",
                        profile.id()
                    )));
                }
                profiles.insert((profile_type, profile.id().to_owned()), profile);
            }
        }

        let character_ids = profiles
            .values()
            .filter_map(|profile| match profile {
                ConsistencyProfileRecord::Character(value) => Some(value.id.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let costumes = self
            .profile_repository
            .list_costume_variants_many(&character_ids)
            .await?
            .into_iter()
            .map(|variant| (variant.id.clone(), variant))
            .collect::<HashMap<_, _>>();

        for binding in bindings {
            let expected = profile_type_for_role(binding.role).ok_or_else(|| {
                ShotConsistencyBindingError::invalid(
                    "CONSISTENCY_PROFILE_ROLE_INVALID: SHOT_REFERENCE is not a profile role",
                )
            })?;
            if binding.profile_type != expected {
                return Err(ShotConsistencyBindingError::invalid(format!(
                    "CONSISTENCY_PROFILE_TYPE_MISMATCH: role {} requires {}",
                    binding.role.as_str(),
                    expected.as_str()
                )));
            }
            validate_profile_id(binding.profile_type, &binding.profile_id)?;
            let Some(profile) = profiles.get(&(binding.profile_type, binding.profile_id.clone()))
            else {
                return Err(ShotConsistencyBindingError::project_mismatch(format!(
                    "CONSISTENCY_PROFILE_PROJECT_MISMATCH: profile {} is not in project {}",
                    binding.profile_id, project_id
                )));
            };
            if let Some(costume_id) = binding.costume_variant_id.as_deref() {
                if binding.role != BindingRole::Character {
                    return Err(ShotConsistencyBindingError::invalid(
                        "CONSISTENCY_COSTUME_ROLE_INVALID: costume variants require CHARACTER",
                    ));
                }
                if !matches!(profile, ConsistencyProfileRecord::Character(_)) {
                    return Err(ShotConsistencyBindingError::invalid(
                        "CONSISTENCY_COSTUME_PROFILE_INVALID: costume requires CharacterProfile",
                    ));
                }
                let Some(costume) = costumes.get(costume_id) else {
                    return Err(ShotConsistencyBindingError::project_mismatch(format!(
                        "CONSISTENCY_COSTUME_PROJECT_MISMATCH: costume variant {costume_id} is not owned by project {project_id}"
                    )));
                };
                if costume.character_profile_id != binding.profile_id {
                    return Err(ShotConsistencyBindingError::invalid(format!(
                        "CONSISTENCY_COSTUME_OWNER_INVALID: costume variant {costume_id} does not belong to profile {}",
                        binding.profile_id
                    )));
                }
            }
        }
        Ok(())
    }

    async fn validate_reference_sets(
        &self,
        project_id: &str,
        bindings: &[ShotReferenceSetBinding],
    ) -> Result<(), ShotConsistencyBindingError> {
        let reference_sets = self
            .reference_set_repository
            .list_reference_sets(project_id, None)
            .await?
            .into_iter()
            .map(|reference_set| (reference_set.id.clone(), reference_set))
            .collect::<HashMap<_, _>>();
        for binding in bindings {
            validate_consistency_id(ConsistencyIdKind::ReferenceSet, &binding.reference_set_id)
                .map_err(|error| ShotConsistencyBindingError::invalid(error.to_string()))?;
            let Some(reference_set) = reference_sets.get(&binding.reference_set_id) else {
                return Err(ShotConsistencyBindingError::project_mismatch(format!(
                    "CONSISTENCY_REFERENCE_SET_PROJECT_MISMATCH: reference set {} is not in project {}",
                    binding.reference_set_id, project_id
                )));
            };
            if reference_set.project_id != project_id {
                return Err(ShotConsistencyBindingError::project_mismatch(format!(
                    "CONSISTENCY_REFERENCE_SET_PROJECT_MISMATCH: reference set {} belongs to another project",
                    binding.reference_set_id
                )));
            }
            if !reference_purpose_allowed(binding.role, reference_set.purpose) {
                return Err(ShotConsistencyBindingError::invalid(format!(
                    "CONSISTENCY_REFERENCE_SET_PURPOSE_INVALID: role {} cannot use {} reference set",
                    binding.role.as_str(),
                    reference_set.purpose.as_str()
                )));
            }
        }
        Ok(())
    }
}

fn materialize_profiles(
    shot_id: &str,
    inputs: &[ConsistencyProfileBindingInput],
    existing: &[ShotProfileBinding],
    now: DateTime<Utc>,
) -> Result<Vec<ShotProfileBinding>, ShotConsistencyBindingError> {
    let existing_by_id = existing
        .iter()
        .map(|binding| (binding.id.as_str(), binding))
        .collect::<HashMap<_, _>>();
    let mut ids = HashSet::new();
    inputs
        .iter()
        .map(|input| {
            validate_write_mode(input.inheritance_mode, "profile")?;
            let id = materialize_id(
                input.id.as_deref(),
                ConsistencyIdKind::ShotProfileBinding,
                &existing_by_id,
                "profile",
            )?;
            if !ids.insert(id.clone()) {
                return Err(ShotConsistencyBindingError::conflict(
                    "CONSISTENCY_BINDING_ID_DUPLICATE: profile binding ids must be unique",
                ));
            }
            let created_at = existing_by_id
                .get(id.as_str())
                .map(|binding| binding.created_at)
                .unwrap_or(now);
            Ok(ShotProfileBinding {
                id,
                shot_id: shot_id.to_owned(),
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

fn materialize_reference_sets(
    shot_id: &str,
    inputs: &[ConsistencyReferenceSetBindingInput],
    existing: &[ShotReferenceSetBinding],
    now: DateTime<Utc>,
) -> Result<Vec<ShotReferenceSetBinding>, ShotConsistencyBindingError> {
    let existing_by_id = existing
        .iter()
        .map(|binding| (binding.id.as_str(), binding))
        .collect::<HashMap<_, _>>();
    let mut ids = HashSet::new();
    inputs
        .iter()
        .map(|input| {
            validate_write_mode(input.inheritance_mode, "reference-set")?;
            let id = materialize_id(
                input.id.as_deref(),
                ConsistencyIdKind::ShotReferenceSetBinding,
                &existing_by_id,
                "reference-set",
            )?;
            if !ids.insert(id.clone()) {
                return Err(ShotConsistencyBindingError::conflict(
                    "CONSISTENCY_BINDING_ID_DUPLICATE: reference-set binding ids must be unique",
                ));
            }
            let created_at = existing_by_id
                .get(id.as_str())
                .map(|binding| binding.created_at)
                .unwrap_or(now);
            Ok(ShotReferenceSetBinding {
                id,
                shot_id: shot_id.to_owned(),
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

fn materialize_id<T>(
    input_id: Option<&str>,
    kind: ConsistencyIdKind,
    existing: &HashMap<&str, &T>,
    entity: &str,
) -> Result<String, ShotConsistencyBindingError> {
    match input_id {
        Some(id) => {
            validate_consistency_id(kind, id)
                .map_err(|error| ShotConsistencyBindingError::invalid(error.to_string()))?;
            if !existing.contains_key(id) {
                return Err(ShotConsistencyBindingError::not_found(format!(
                    "CONSISTENCY_BINDING_ID_NOT_FOUND: {entity} binding {id} is not a direct binding for this shot"
                )));
            }
            Ok(id.to_owned())
        }
        None => Ok(generate_consistency_id(kind)),
    }
}

fn validate_write_mode(
    mode: InheritanceMode,
    entity: &str,
) -> Result<(), ShotConsistencyBindingError> {
    if mode == InheritanceMode::Inherited {
        return Err(ShotConsistencyBindingError::invalid(format!(
            "CONSISTENCY_BINDING_INHERITED_NOT_WRITABLE: {entity} bindings with INHERITED mode are read-only"
        )));
    }
    Ok(())
}

fn validate_profile_id(
    profile_type: ProfileType,
    profile_id: &str,
) -> Result<(), ShotConsistencyBindingError> {
    let kind = match profile_type {
        ProfileType::Character => ConsistencyIdKind::CharacterProfile,
        ProfileType::Scene => ConsistencyIdKind::SceneProfile,
        ProfileType::Prop => ConsistencyIdKind::PropProfile,
        ProfileType::Style => ConsistencyIdKind::StyleProfile,
    };
    validate_consistency_id(kind, profile_id)
        .map_err(|error| ShotConsistencyBindingError::invalid(error.to_string()))
}

fn profile_type_for_role(role: BindingRole) -> Option<ProfileType> {
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
    bindings: &[ShotProfileBinding],
) -> Result<(), ShotConsistencyBindingError> {
    let mut slots = HashMap::<(BindingRole, i64), &str>::new();
    let mut single_role_counts = HashMap::<BindingRole, usize>::new();
    for binding in bindings {
        if binding.ordinal < 0 || binding.profile_id.trim().is_empty() {
            return Err(ShotConsistencyBindingError::invalid(
                "CONSISTENCY_PROFILE_BINDING_INVALID: profile id and ordinal are invalid",
            ));
        }
        if binding
            .costume_variant_id
            .as_deref()
            .map(str::is_empty)
            .unwrap_or(false)
        {
            return Err(ShotConsistencyBindingError::invalid(
                "CONSISTENCY_COSTUME_ID_INVALID: costume variant id must not be empty",
            ));
        }
        if let Some(previous) = slots.insert((binding.role, binding.ordinal), &binding.profile_id) {
            let code = if previous == binding.profile_id {
                "CONSISTENCY_PROFILE_ORDINAL_DUPLICATE"
            } else {
                "CONTEXT_PROFILE_ORDINAL_CONFLICT"
            };
            return Err(ShotConsistencyBindingError::conflict(format!(
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
                return Err(ShotConsistencyBindingError::conflict(format!(
                    "CONTEXT_PROFILE_ORDINAL_CONFLICT: role {} has multiple effective direct bindings",
                    binding.role.as_str()
                )));
            }
        }
    }
    Ok(())
}

fn validate_reference_set_conflicts(
    bindings: &[ShotReferenceSetBinding],
) -> Result<(), ShotConsistencyBindingError> {
    let mut slots = HashMap::<(BindingRole, i64), &str>::new();
    let mut single_role_counts = HashMap::<BindingRole, usize>::new();
    for binding in bindings {
        if binding.ordinal < 0 || binding.reference_set_id.trim().is_empty() {
            return Err(ShotConsistencyBindingError::invalid(
                "CONSISTENCY_REFERENCE_SET_BINDING_INVALID: reference set id and ordinal are invalid",
            ));
        }
        if let Some(previous) =
            slots.insert((binding.role, binding.ordinal), &binding.reference_set_id)
        {
            let code = if previous == binding.reference_set_id {
                "CONSISTENCY_REFERENCE_ORDINAL_DUPLICATE"
            } else {
                "CONTEXT_REFERENCE_ORDINAL_CONFLICT"
            };
            return Err(ShotConsistencyBindingError::conflict(format!(
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
                return Err(ShotConsistencyBindingError::conflict(format!(
                    "CONTEXT_REFERENCE_ORDINAL_CONFLICT: role {} has multiple effective direct bindings",
                    binding.role.as_str()
                )));
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
pub enum ShotConsistencyBindingError {
    InvalidInput(String),
    NotFound(String),
    ProjectMismatch(String),
    Conflict(String),
    Repository(RepositoryError),
}

impl ShotConsistencyBindingError {
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

impl fmt::Display for ShotConsistencyBindingError {
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

impl Error for ShotConsistencyBindingError {}

impl From<RepositoryError> for ShotConsistencyBindingError {
    fn from(value: RepositoryError) -> Self {
        Self::Repository(value)
    }
}
