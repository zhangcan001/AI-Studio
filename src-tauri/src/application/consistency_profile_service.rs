use crate::application::ports::{
    Clock, ConsistencyProfileRepository, ProjectRepository, ReferenceSetRepository, RepositoryError,
};
use crate::domain::consistency::{
    generate_consistency_id, validate_metadata_json, validate_optional_text, validate_profile_name,
    validate_prompt_fragment, CharacterProfile, ConsistencyIdKind, ConsistencyProfileRecord,
    ConsistencyValidationError, CostumeVariant, ProfileType, PropProfile, ReferenceSetPurpose,
    SceneProfile, StyleProfile,
};
use std::{error::Error, fmt, sync::Arc};

#[derive(Clone, Debug)]
pub struct CreateCharacterProfileRequest {
    pub project_id: String,
    pub name: String,
    pub description: String,
    pub canonical_prompt: String,
    pub negative_prompt: String,
    pub default_style_profile_id: Option<String>,
    pub default_reference_set_id: Option<String>,
    pub metadata_json: String,
}

#[derive(Clone, Debug)]
pub struct UpdateCharacterProfileRequest {
    pub project_id: String,
    pub profile_id: String,
    pub name: String,
    pub description: String,
    pub canonical_prompt: String,
    pub negative_prompt: String,
    pub default_style_profile_id: Option<String>,
    pub default_reference_set_id: Option<String>,
    pub metadata_json: String,
}

#[derive(Clone, Debug)]
pub struct CreateSceneProfileRequest {
    pub project_id: String,
    pub name: String,
    pub description: String,
    pub environment_prompt: String,
    pub lighting_prompt: Option<String>,
    pub negative_prompt: Option<String>,
    pub default_style_profile_id: Option<String>,
    pub default_reference_set_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct UpdateSceneProfileRequest {
    pub project_id: String,
    pub profile_id: String,
    pub name: String,
    pub description: String,
    pub environment_prompt: String,
    pub lighting_prompt: Option<String>,
    pub negative_prompt: Option<String>,
    pub default_style_profile_id: Option<String>,
    pub default_reference_set_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CreatePropProfileRequest {
    pub project_id: String,
    pub name: String,
    pub description: String,
    pub canonical_prompt: String,
    pub material_prompt: Option<String>,
    pub scale_prompt: Option<String>,
    pub default_reference_set_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct UpdatePropProfileRequest {
    pub project_id: String,
    pub profile_id: String,
    pub name: String,
    pub description: String,
    pub canonical_prompt: String,
    pub material_prompt: Option<String>,
    pub scale_prompt: Option<String>,
    pub default_reference_set_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CreateStyleProfileRequest {
    pub project_id: String,
    pub name: String,
    pub style_prompt: String,
    pub color_prompt: Option<String>,
    pub line_prompt: Option<String>,
    pub negative_prompt: Option<String>,
    pub output_notes: Option<String>,
}

#[derive(Clone, Debug)]
pub struct UpdateStyleProfileRequest {
    pub project_id: String,
    pub profile_id: String,
    pub name: String,
    pub style_prompt: String,
    pub color_prompt: Option<String>,
    pub line_prompt: Option<String>,
    pub negative_prompt: Option<String>,
    pub output_notes: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CreateCostumeVariantRequest {
    pub project_id: String,
    pub character_profile_id: String,
    pub name: String,
    pub prompt_fragment: String,
    pub reference_set_id: Option<String>,
    pub is_default: bool,
    pub ordinal: i64,
}

#[derive(Clone, Debug)]
pub struct UpdateCostumeVariantRequest {
    pub project_id: String,
    pub costume_variant_id: String,
    pub name: String,
    pub prompt_fragment: String,
    pub reference_set_id: Option<String>,
    pub is_default: bool,
    pub ordinal: i64,
}

pub struct ConsistencyProfileService {
    repository: Arc<dyn ConsistencyProfileRepository>,
    reference_set_repository: Arc<dyn ReferenceSetRepository>,
    project_repository: Arc<dyn ProjectRepository>,
    clock: Arc<dyn Clock>,
}

impl ConsistencyProfileService {
    pub fn new(
        repository: Arc<dyn ConsistencyProfileRepository>,
        reference_set_repository: Arc<dyn ReferenceSetRepository>,
        project_repository: Arc<dyn ProjectRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repository,
            reference_set_repository,
            project_repository,
            clock,
        }
    }

    pub async fn list(
        &self,
        project_id: &str,
        profile_type: Option<ProfileType>,
    ) -> Result<Vec<ConsistencyProfileRecord>, ConsistencyProfileError> {
        let project_id = self.ensure_project(project_id).await?;
        let mut profiles = Vec::new();
        for profile_type in profile_type
            .map(|profile_type| vec![profile_type])
            .unwrap_or_else(|| {
                vec![
                    ProfileType::Character,
                    ProfileType::Scene,
                    ProfileType::Prop,
                    ProfileType::Style,
                ]
            })
        {
            profiles.extend(
                self.repository
                    .list_profiles(&project_id, profile_type)
                    .await?,
            );
        }
        Ok(profiles)
    }

    pub async fn get(
        &self,
        project_id: &str,
        profile_type: ProfileType,
        profile_id: &str,
    ) -> Result<ConsistencyProfileRecord, ConsistencyProfileError> {
        let project_id = self.ensure_project(project_id).await?;
        let profile_id = required_id(profile_id, "profile")?;
        self.repository
            .find_profile(&project_id, profile_type, &profile_id)
            .await?
            .ok_or_else(|| ConsistencyProfileError::not_found(profile_id))
    }

    pub async fn create_character(
        &self,
        request: CreateCharacterProfileRequest,
    ) -> Result<CharacterProfile, ConsistencyProfileError> {
        let project_id = self.ensure_project(&request.project_id).await?;
        let name = validate_name(&request.name)?;
        validate_profile_text(
            &request.description,
            &[
                (&request.canonical_prompt, "canonical_prompt"),
                (&request.negative_prompt, "negative_prompt"),
            ],
            &request.metadata_json,
        )?;
        self.validate_style_reference(&project_id, request.default_style_profile_id.as_deref())
            .await?;
        self.validate_reference_relation(
            &project_id,
            request.default_reference_set_id.as_deref(),
            ReferenceSetPurpose::Character,
        )
        .await?;

        let now = self.clock.now();
        let profile = CharacterProfile {
            id: generate_consistency_id(ConsistencyIdKind::CharacterProfile),
            project_id,
            name,
            description: request.description,
            canonical_prompt: request.canonical_prompt,
            negative_prompt: request.negative_prompt,
            default_style_profile_id: normalize_optional_id(request.default_style_profile_id)?,
            default_reference_set_id: normalize_optional_id(request.default_reference_set_id)?,
            active_revision_id: None,
            metadata_json: request.metadata_json,
            created_at: now,
            updated_at: now,
        };
        self.repository
            .insert_profile(&ConsistencyProfileRecord::Character(profile.clone()))
            .await?;
        Ok(profile)
    }

    pub async fn create_scene(
        &self,
        request: CreateSceneProfileRequest,
    ) -> Result<SceneProfile, ConsistencyProfileError> {
        let project_id = self.ensure_project(&request.project_id).await?;
        let name = validate_name(&request.name)?;
        validate_profile_texts(
            &request.description,
            &[(&request.environment_prompt, "environment_prompt")],
            &[
                (&request.lighting_prompt, "lighting_prompt"),
                (&request.negative_prompt, "negative_prompt"),
            ],
        )?;
        self.validate_style_reference(&project_id, request.default_style_profile_id.as_deref())
            .await?;
        self.validate_reference_relation(
            &project_id,
            request.default_reference_set_id.as_deref(),
            ReferenceSetPurpose::Scene,
        )
        .await?;

        let now = self.clock.now();
        let profile = SceneProfile {
            id: generate_consistency_id(ConsistencyIdKind::SceneProfile),
            project_id,
            name,
            description: request.description,
            environment_prompt: request.environment_prompt,
            lighting_prompt: request.lighting_prompt,
            negative_prompt: request.negative_prompt,
            default_style_profile_id: normalize_optional_id(request.default_style_profile_id)?,
            default_reference_set_id: normalize_optional_id(request.default_reference_set_id)?,
            active_revision_id: None,
            created_at: now,
            updated_at: now,
        };
        self.repository
            .insert_profile(&ConsistencyProfileRecord::Scene(profile.clone()))
            .await?;
        Ok(profile)
    }

    pub async fn create_prop(
        &self,
        request: CreatePropProfileRequest,
    ) -> Result<PropProfile, ConsistencyProfileError> {
        let project_id = self.ensure_project(&request.project_id).await?;
        let name = validate_name(&request.name)?;
        validate_profile_texts(
            &request.description,
            &[(&request.canonical_prompt, "canonical_prompt")],
            &[
                (&request.material_prompt, "material_prompt"),
                (&request.scale_prompt, "scale_prompt"),
            ],
        )?;
        self.validate_reference_relation(
            &project_id,
            request.default_reference_set_id.as_deref(),
            ReferenceSetPurpose::Prop,
        )
        .await?;

        let now = self.clock.now();
        let profile = PropProfile {
            id: generate_consistency_id(ConsistencyIdKind::PropProfile),
            project_id,
            name,
            description: request.description,
            canonical_prompt: request.canonical_prompt,
            material_prompt: request.material_prompt,
            scale_prompt: request.scale_prompt,
            default_reference_set_id: normalize_optional_id(request.default_reference_set_id)?,
            active_revision_id: None,
            created_at: now,
            updated_at: now,
        };
        self.repository
            .insert_profile(&ConsistencyProfileRecord::Prop(profile.clone()))
            .await?;
        Ok(profile)
    }

    pub async fn create_style(
        &self,
        request: CreateStyleProfileRequest,
    ) -> Result<StyleProfile, ConsistencyProfileError> {
        let project_id = self.ensure_project(&request.project_id).await?;
        let name = validate_name(&request.name)?;
        validate_prompt_fragment_field("style_prompt", &request.style_prompt)?;
        validate_optional_prompts(&[
            (&request.color_prompt, "color_prompt"),
            (&request.line_prompt, "line_prompt"),
            (&request.negative_prompt, "negative_prompt"),
            (&request.output_notes, "output_notes"),
        ])?;

        let now = self.clock.now();
        let profile = StyleProfile {
            id: generate_consistency_id(ConsistencyIdKind::StyleProfile),
            project_id,
            name,
            style_prompt: request.style_prompt,
            color_prompt: request.color_prompt,
            line_prompt: request.line_prompt,
            negative_prompt: request.negative_prompt,
            output_notes: request.output_notes,
            active_revision_id: None,
            created_at: now,
            updated_at: now,
        };
        self.repository
            .insert_profile(&ConsistencyProfileRecord::Style(profile.clone()))
            .await?;
        Ok(profile)
    }

    pub async fn update_character(
        &self,
        request: UpdateCharacterProfileRequest,
    ) -> Result<CharacterProfile, ConsistencyProfileError> {
        let project_id = self.ensure_project(&request.project_id).await?;
        let profile_id = required_id(&request.profile_id, "profile")?;
        let existing = self
            .repository
            .find_profile(&project_id, ProfileType::Character, &profile_id)
            .await?
            .ok_or_else(|| ConsistencyProfileError::not_found(profile_id.clone()))?;
        let existing = match existing {
            ConsistencyProfileRecord::Character(profile) => profile,
            _ => return Err(ConsistencyProfileError::type_mismatch(profile_id)),
        };
        let name = validate_name(&request.name)?;
        validate_profile_text(
            &request.description,
            &[
                (&request.canonical_prompt, "canonical_prompt"),
                (&request.negative_prompt, "negative_prompt"),
            ],
            &request.metadata_json,
        )?;
        self.validate_style_reference(&project_id, request.default_style_profile_id.as_deref())
            .await?;
        self.validate_reference_relation(
            &project_id,
            request.default_reference_set_id.as_deref(),
            ReferenceSetPurpose::Character,
        )
        .await?;

        let profile = CharacterProfile {
            id: existing.id,
            project_id: existing.project_id,
            name,
            description: request.description,
            canonical_prompt: request.canonical_prompt,
            negative_prompt: request.negative_prompt,
            default_style_profile_id: normalize_optional_id(request.default_style_profile_id)?,
            default_reference_set_id: normalize_optional_id(request.default_reference_set_id)?,
            active_revision_id: existing.active_revision_id,
            metadata_json: request.metadata_json,
            created_at: existing.created_at,
            updated_at: self.clock.now(),
        };
        self.persist_profile_update(ConsistencyProfileRecord::Character(profile.clone()))
            .await?;
        Ok(profile)
    }

    pub async fn update_scene(
        &self,
        request: UpdateSceneProfileRequest,
    ) -> Result<SceneProfile, ConsistencyProfileError> {
        let project_id = self.ensure_project(&request.project_id).await?;
        let profile_id = required_id(&request.profile_id, "profile")?;
        let existing = self
            .repository
            .find_profile(&project_id, ProfileType::Scene, &profile_id)
            .await?
            .ok_or_else(|| ConsistencyProfileError::not_found(profile_id.clone()))?;
        let existing = match existing {
            ConsistencyProfileRecord::Scene(profile) => profile,
            _ => return Err(ConsistencyProfileError::type_mismatch(profile_id)),
        };
        let name = validate_name(&request.name)?;
        validate_profile_texts(
            &request.description,
            &[(&request.environment_prompt, "environment_prompt")],
            &[
                (&request.lighting_prompt, "lighting_prompt"),
                (&request.negative_prompt, "negative_prompt"),
            ],
        )?;
        self.validate_style_reference(&project_id, request.default_style_profile_id.as_deref())
            .await?;
        self.validate_reference_relation(
            &project_id,
            request.default_reference_set_id.as_deref(),
            ReferenceSetPurpose::Scene,
        )
        .await?;

        let profile = SceneProfile {
            id: existing.id,
            project_id: existing.project_id,
            name,
            description: request.description,
            environment_prompt: request.environment_prompt,
            lighting_prompt: request.lighting_prompt,
            negative_prompt: request.negative_prompt,
            default_style_profile_id: normalize_optional_id(request.default_style_profile_id)?,
            default_reference_set_id: normalize_optional_id(request.default_reference_set_id)?,
            active_revision_id: existing.active_revision_id,
            created_at: existing.created_at,
            updated_at: self.clock.now(),
        };
        self.persist_profile_update(ConsistencyProfileRecord::Scene(profile.clone()))
            .await?;
        Ok(profile)
    }

    pub async fn update_prop(
        &self,
        request: UpdatePropProfileRequest,
    ) -> Result<PropProfile, ConsistencyProfileError> {
        let project_id = self.ensure_project(&request.project_id).await?;
        let profile_id = required_id(&request.profile_id, "profile")?;
        let existing = self
            .repository
            .find_profile(&project_id, ProfileType::Prop, &profile_id)
            .await?
            .ok_or_else(|| ConsistencyProfileError::not_found(profile_id.clone()))?;
        let existing = match existing {
            ConsistencyProfileRecord::Prop(profile) => profile,
            _ => return Err(ConsistencyProfileError::type_mismatch(profile_id)),
        };
        let name = validate_name(&request.name)?;
        validate_profile_texts(
            &request.description,
            &[(&request.canonical_prompt, "canonical_prompt")],
            &[
                (&request.material_prompt, "material_prompt"),
                (&request.scale_prompt, "scale_prompt"),
            ],
        )?;
        self.validate_reference_relation(
            &project_id,
            request.default_reference_set_id.as_deref(),
            ReferenceSetPurpose::Prop,
        )
        .await?;

        let profile = PropProfile {
            id: existing.id,
            project_id: existing.project_id,
            name,
            description: request.description,
            canonical_prompt: request.canonical_prompt,
            material_prompt: request.material_prompt,
            scale_prompt: request.scale_prompt,
            default_reference_set_id: normalize_optional_id(request.default_reference_set_id)?,
            active_revision_id: existing.active_revision_id,
            created_at: existing.created_at,
            updated_at: self.clock.now(),
        };
        self.persist_profile_update(ConsistencyProfileRecord::Prop(profile.clone()))
            .await?;
        Ok(profile)
    }

    pub async fn update_style(
        &self,
        request: UpdateStyleProfileRequest,
    ) -> Result<StyleProfile, ConsistencyProfileError> {
        let project_id = self.ensure_project(&request.project_id).await?;
        let profile_id = required_id(&request.profile_id, "profile")?;
        let existing = self
            .repository
            .find_profile(&project_id, ProfileType::Style, &profile_id)
            .await?
            .ok_or_else(|| ConsistencyProfileError::not_found(profile_id.clone()))?;
        let existing = match existing {
            ConsistencyProfileRecord::Style(profile) => profile,
            _ => return Err(ConsistencyProfileError::type_mismatch(profile_id)),
        };
        let name = validate_name(&request.name)?;
        validate_prompt_fragment_field("style_prompt", &request.style_prompt)?;
        validate_optional_prompts(&[
            (&request.color_prompt, "color_prompt"),
            (&request.line_prompt, "line_prompt"),
            (&request.negative_prompt, "negative_prompt"),
            (&request.output_notes, "output_notes"),
        ])?;

        let profile = StyleProfile {
            id: existing.id,
            project_id: existing.project_id,
            name,
            style_prompt: request.style_prompt,
            color_prompt: request.color_prompt,
            line_prompt: request.line_prompt,
            negative_prompt: request.negative_prompt,
            output_notes: request.output_notes,
            active_revision_id: existing.active_revision_id,
            created_at: existing.created_at,
            updated_at: self.clock.now(),
        };
        self.persist_profile_update(ConsistencyProfileRecord::Style(profile.clone()))
            .await?;
        Ok(profile)
    }

    pub async fn delete(
        &self,
        project_id: &str,
        profile_type: ProfileType,
        profile_id: &str,
    ) -> Result<(), ConsistencyProfileError> {
        let project_id = self.ensure_project(project_id).await?;
        let profile_id = required_id(profile_id, "profile")?;
        self.repository
            .find_profile(&project_id, profile_type, &profile_id)
            .await?
            .ok_or_else(|| ConsistencyProfileError::not_found(profile_id.clone()))?;
        if !self
            .repository
            .delete_profile(&project_id, profile_type, &profile_id)
            .await?
        {
            return Err(ConsistencyProfileError::not_found(profile_id));
        }
        Ok(())
    }

    pub async fn list_costumes(
        &self,
        project_id: &str,
        character_profile_id: &str,
    ) -> Result<Vec<CostumeVariant>, ConsistencyProfileError> {
        let project_id = self.ensure_project(project_id).await?;
        let character_profile_id = required_id(character_profile_id, "character profile")?;
        self.ensure_character(&project_id, &character_profile_id)
            .await?;
        Ok(self
            .repository
            .list_costume_variants(&character_profile_id)
            .await?)
    }

    pub async fn get_costume(
        &self,
        project_id: &str,
        costume_variant_id: &str,
    ) -> Result<CostumeVariant, ConsistencyProfileError> {
        let project_id = self.ensure_project(project_id).await?;
        let costume_variant_id = required_id(costume_variant_id, "costume variant")?;
        let costume = self
            .repository
            .find_costume_variant(&costume_variant_id)
            .await?
            .ok_or_else(|| ConsistencyProfileError::not_found(costume_variant_id))?;
        self.ensure_character(&project_id, &costume.character_profile_id)
            .await?;
        Ok(costume)
    }

    pub async fn create_costume(
        &self,
        request: CreateCostumeVariantRequest,
    ) -> Result<CostumeVariant, ConsistencyProfileError> {
        let project_id = self.ensure_project(&request.project_id).await?;
        let character_profile_id = required_id(&request.character_profile_id, "character profile")?;
        self.ensure_character(&project_id, &character_profile_id)
            .await?;
        let name = validate_name(&request.name)?;
        validate_prompt_fragment_field("prompt_fragment", &request.prompt_fragment)?;
        validate_ordinal(request.ordinal)?;
        self.validate_reference_relation(
            &project_id,
            request.reference_set_id.as_deref(),
            ReferenceSetPurpose::Costume,
        )
        .await?;

        let now = self.clock.now();
        let costume = CostumeVariant {
            id: generate_consistency_id(ConsistencyIdKind::CostumeVariant),
            character_profile_id,
            name,
            prompt_fragment: request.prompt_fragment,
            reference_set_id: normalize_optional_id(request.reference_set_id)?,
            is_default: request.is_default,
            ordinal: request.ordinal,
            active_revision_id: None,
            created_at: now,
            updated_at: now,
        };
        self.repository.insert_costume_variant(&costume).await?;
        Ok(costume)
    }

    pub async fn update_costume(
        &self,
        request: UpdateCostumeVariantRequest,
    ) -> Result<CostumeVariant, ConsistencyProfileError> {
        let project_id = self.ensure_project(&request.project_id).await?;
        let costume_variant_id = required_id(&request.costume_variant_id, "costume variant")?;
        let existing = self
            .repository
            .find_costume_variant(&costume_variant_id)
            .await?
            .ok_or_else(|| ConsistencyProfileError::not_found(costume_variant_id.clone()))?;
        self.ensure_character(&project_id, &existing.character_profile_id)
            .await?;
        let name = validate_name(&request.name)?;
        validate_prompt_fragment_field("prompt_fragment", &request.prompt_fragment)?;
        validate_ordinal(request.ordinal)?;
        self.validate_reference_relation(
            &project_id,
            request.reference_set_id.as_deref(),
            ReferenceSetPurpose::Costume,
        )
        .await?;

        let costume = CostumeVariant {
            id: existing.id,
            character_profile_id: existing.character_profile_id,
            name,
            prompt_fragment: request.prompt_fragment,
            reference_set_id: normalize_optional_id(request.reference_set_id)?,
            is_default: request.is_default,
            ordinal: request.ordinal,
            active_revision_id: existing.active_revision_id,
            created_at: existing.created_at,
            updated_at: self.clock.now(),
        };
        if !self.repository.update_costume_variant(&costume).await? {
            return Err(ConsistencyProfileError::not_found(costume_variant_id));
        }
        Ok(costume)
    }

    pub async fn delete_costume(
        &self,
        project_id: &str,
        costume_variant_id: &str,
    ) -> Result<(), ConsistencyProfileError> {
        let project_id = self.ensure_project(project_id).await?;
        let costume_variant_id = required_id(costume_variant_id, "costume variant")?;
        let costume = self
            .repository
            .find_costume_variant(&costume_variant_id)
            .await?
            .ok_or_else(|| ConsistencyProfileError::not_found(costume_variant_id.clone()))?;
        self.ensure_character(&project_id, &costume.character_profile_id)
            .await?;
        if !self
            .repository
            .delete_costume_variant(&costume_variant_id)
            .await?
        {
            return Err(ConsistencyProfileError::not_found(costume_variant_id));
        }
        Ok(())
    }

    async fn ensure_project(&self, project_id: &str) -> Result<String, ConsistencyProfileError> {
        let project_id = required_id(project_id, "project")?;
        if self
            .project_repository
            .find_by_id(&project_id)
            .await?
            .is_none()
        {
            return Err(ConsistencyProfileError::not_found(format!(
                "PROJECT_NOT_FOUND: {project_id}"
            )));
        }
        Ok(project_id)
    }

    async fn ensure_character(
        &self,
        project_id: &str,
        character_profile_id: &str,
    ) -> Result<CharacterProfile, ConsistencyProfileError> {
        let record = self
            .repository
            .find_profile(project_id, ProfileType::Character, character_profile_id)
            .await?
            .ok_or_else(|| {
                ConsistencyProfileError::project_mismatch(format!(
                    "character profile {character_profile_id} is not in project {project_id}"
                ))
            })?;
        match record {
            ConsistencyProfileRecord::Character(profile) => Ok(profile),
            _ => Err(ConsistencyProfileError::type_mismatch(
                character_profile_id.to_owned(),
            )),
        }
    }

    async fn validate_style_reference(
        &self,
        project_id: &str,
        style_profile_id: Option<&str>,
    ) -> Result<(), ConsistencyProfileError> {
        let Some(style_profile_id) = normalize_optional_id_ref(style_profile_id)? else {
            return Ok(());
        };
        let record = self
            .repository
            .find_profile(project_id, ProfileType::Style, &style_profile_id)
            .await?
            .ok_or_else(|| {
                ConsistencyProfileError::not_found(format!(
                    "STYLE_PROFILE_NOT_FOUND: {style_profile_id}"
                ))
            })?;
        if !matches!(record, ConsistencyProfileRecord::Style(_)) {
            return Err(ConsistencyProfileError::type_mismatch(style_profile_id));
        }
        Ok(())
    }

    async fn validate_reference_relation(
        &self,
        project_id: &str,
        reference_set_id: Option<&str>,
        expected_purpose: ReferenceSetPurpose,
    ) -> Result<(), ConsistencyProfileError> {
        let Some(reference_set_id) = normalize_optional_id_ref(reference_set_id)? else {
            return Ok(());
        };
        let reference_set = self
            .reference_set_repository
            .find_reference_set(project_id, &reference_set_id)
            .await?
            .ok_or_else(|| {
                ConsistencyProfileError::not_found(format!(
                    "REFERENCE_SET_NOT_FOUND: {reference_set_id}"
                ))
            })?;
        if reference_set.project_id != project_id {
            return Err(ConsistencyProfileError::project_mismatch(format!(
                "reference set {reference_set_id} belongs to another project"
            )));
        }
        if reference_set.purpose != expected_purpose {
            return Err(ConsistencyProfileError::invalid_input(format!(
                "REFERENCE_SET_PURPOSE_MISMATCH: expected {:?}, got {:?}",
                expected_purpose, reference_set.purpose
            )));
        }
        Ok(())
    }

    async fn persist_profile_update(
        &self,
        profile: ConsistencyProfileRecord,
    ) -> Result<(), ConsistencyProfileError> {
        if !self.repository.update_profile(&profile).await? {
            return Err(ConsistencyProfileError::not_found(profile.id().to_owned()));
        }
        Ok(())
    }
}

fn validate_name(value: &str) -> Result<String, ConsistencyProfileError> {
    validate_profile_name(value).map_err(ConsistencyProfileError::from)?;
    Ok(value.trim().to_owned())
}

fn validate_profile_text(
    description: &str,
    prompts: &[(&str, &str)],
    metadata_json: &str,
) -> Result<(), ConsistencyProfileError> {
    validate_optional_text("description", Some(description))
        .map_err(ConsistencyProfileError::from)?;
    for (value, field) in prompts {
        validate_prompt_fragment_field(field, value)?;
    }
    validate_metadata_json(metadata_json).map_err(ConsistencyProfileError::from)
}

fn validate_profile_texts(
    description: &str,
    required_prompts: &[(&str, &str)],
    optional_prompts: &[(&Option<String>, &str)],
) -> Result<(), ConsistencyProfileError> {
    validate_optional_text("description", Some(description))
        .map_err(ConsistencyProfileError::from)?;
    for (value, field) in required_prompts {
        validate_prompt_fragment_field(field, value)?;
    }
    validate_optional_prompts(optional_prompts)
}

fn validate_optional_prompts(
    prompts: &[(&Option<String>, &str)],
) -> Result<(), ConsistencyProfileError> {
    for (value, field) in prompts {
        if let Some(value) = value.as_deref() {
            validate_prompt_fragment_field(field, value)?;
        }
    }
    Ok(())
}

fn validate_prompt_fragment_field(field: &str, value: &str) -> Result<(), ConsistencyProfileError> {
    validate_prompt_fragment(field, value).map_err(ConsistencyProfileError::from)
}

fn normalize_optional_id(value: Option<String>) -> Result<Option<String>, ConsistencyProfileError> {
    value
        .map(|value| {
            let value = required_id(&value, "relation")?;
            Ok(value)
        })
        .transpose()
}

fn normalize_optional_id_ref(
    value: Option<&str>,
) -> Result<Option<String>, ConsistencyProfileError> {
    value
        .map(|value| required_id(value, "relation"))
        .transpose()
}

fn required_id(value: &str, kind: &str) -> Result<String, ConsistencyProfileError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ConsistencyProfileError::invalid_input(format!(
            "INVALID_{}_ID: id must not be empty",
            kind.replace(' ', "_").to_ascii_uppercase()
        )));
    }
    Ok(value.to_owned())
}

fn validate_ordinal(ordinal: i64) -> Result<(), ConsistencyProfileError> {
    if ordinal < 0 {
        return Err(ConsistencyProfileError::invalid_input(
            "INVALID_COSTUME_ORDINAL: ordinal must be non-negative".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConsistencyProfileError {
    InvalidInput(String),
    NotFound(String),
    ProjectMismatch(String),
    Conflict(String),
    Repository(RepositoryError),
}

pub type ConsistencyProfileServiceError = ConsistencyProfileError;

impl ConsistencyProfileError {
    fn invalid_input(message: String) -> Self {
        Self::InvalidInput(message)
    }

    fn not_found(message: String) -> Self {
        Self::NotFound(message)
    }

    fn project_mismatch(message: String) -> Self {
        Self::ProjectMismatch(message)
    }

    fn type_mismatch(profile_id: String) -> Self {
        Self::InvalidInput(format!(
            "CONSISTENCY_PROFILE_TYPE_MISMATCH: profile {profile_id} has an unexpected type"
        ))
    }
}

impl fmt::Display for ConsistencyProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => {
                write!(formatter, "CONSISTENCY_PROFILE_INVALID_INPUT: {message}")
            }
            Self::NotFound(message) => {
                write!(formatter, "CONSISTENCY_PROFILE_NOT_FOUND: {message}")
            }
            Self::ProjectMismatch(message) => {
                write!(formatter, "CONSISTENCY_PROFILE_PROJECT_MISMATCH: {message}")
            }
            Self::Conflict(message) => write!(formatter, "CONSISTENCY_PROFILE_CONFLICT: {message}"),
            Self::Repository(error) => write!(formatter, "CONSISTENCY_PROFILE_REPOSITORY: {error}"),
        }
    }
}

impl Error for ConsistencyProfileError {}

impl From<RepositoryError> for ConsistencyProfileError {
    fn from(error: RepositoryError) -> Self {
        match error {
            RepositoryError::NotFound { entity, id } => Self::NotFound(format!("{entity} {id}")),
            RepositoryError::Integrity { message } => Self::Conflict(message),
            error => Self::Repository(error),
        }
    }
}

impl From<ConsistencyValidationError> for ConsistencyProfileError {
    fn from(error: ConsistencyValidationError) -> Self {
        Self::InvalidInput(error.to_string())
    }
}
