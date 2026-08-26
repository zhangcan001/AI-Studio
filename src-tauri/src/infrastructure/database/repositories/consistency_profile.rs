use super::{format_datetime, map_domain_error, map_sqlx_error, parse_datetime};
use crate::application::ports::{ConsistencyProfileRepository, RepositoryError};
use crate::domain::consistency::{
    CharacterProfile, ConsistencyProfileRecord, CostumeVariant, ProfileRevision,
    ProfileRevisionStatus, ProfileType, PropProfile, SceneProfile, StyleProfile,
};
use async_trait::async_trait;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct SqliteConsistencyProfileRepository {
    pool: SqlitePool,
}

impl SqliteConsistencyProfileRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct CharacterProfileRow {
    id: String,
    project_id: String,
    name: String,
    description: String,
    canonical_prompt: String,
    negative_prompt: String,
    default_style_profile_id: Option<String>,
    default_reference_set_id: Option<String>,
    active_revision_id: Option<String>,
    metadata_json: String,
    created_at: String,
    updated_at: String,
}

impl CharacterProfileRow {
    fn into_domain(self) -> Result<CharacterProfile, RepositoryError> {
        Ok(CharacterProfile {
            id: self.id,
            project_id: self.project_id,
            name: self.name,
            description: self.description,
            canonical_prompt: self.canonical_prompt,
            negative_prompt: self.negative_prompt,
            default_style_profile_id: self.default_style_profile_id,
            default_reference_set_id: self.default_reference_set_id,
            active_revision_id: self.active_revision_id,
            metadata_json: self.metadata_json,
            created_at: parse_datetime("character profile created_at", &self.created_at)?,
            updated_at: parse_datetime("character profile updated_at", &self.updated_at)?,
        })
    }
}

#[derive(sqlx::FromRow)]
struct SceneProfileRow {
    id: String,
    project_id: String,
    name: String,
    description: String,
    environment_prompt: String,
    lighting_prompt: Option<String>,
    negative_prompt: Option<String>,
    default_style_profile_id: Option<String>,
    default_reference_set_id: Option<String>,
    active_revision_id: Option<String>,
    created_at: String,
    updated_at: String,
}

impl SceneProfileRow {
    fn into_domain(self) -> Result<SceneProfile, RepositoryError> {
        Ok(SceneProfile {
            id: self.id,
            project_id: self.project_id,
            name: self.name,
            description: self.description,
            environment_prompt: self.environment_prompt,
            lighting_prompt: self.lighting_prompt,
            negative_prompt: self.negative_prompt,
            default_style_profile_id: self.default_style_profile_id,
            default_reference_set_id: self.default_reference_set_id,
            active_revision_id: self.active_revision_id,
            created_at: parse_datetime("scene profile created_at", &self.created_at)?,
            updated_at: parse_datetime("scene profile updated_at", &self.updated_at)?,
        })
    }
}

#[derive(sqlx::FromRow)]
struct PropProfileRow {
    id: String,
    project_id: String,
    name: String,
    description: String,
    canonical_prompt: String,
    material_prompt: Option<String>,
    scale_prompt: Option<String>,
    default_reference_set_id: Option<String>,
    active_revision_id: Option<String>,
    created_at: String,
    updated_at: String,
}

impl PropProfileRow {
    fn into_domain(self) -> Result<PropProfile, RepositoryError> {
        Ok(PropProfile {
            id: self.id,
            project_id: self.project_id,
            name: self.name,
            description: self.description,
            canonical_prompt: self.canonical_prompt,
            material_prompt: self.material_prompt,
            scale_prompt: self.scale_prompt,
            default_reference_set_id: self.default_reference_set_id,
            active_revision_id: self.active_revision_id,
            created_at: parse_datetime("prop profile created_at", &self.created_at)?,
            updated_at: parse_datetime("prop profile updated_at", &self.updated_at)?,
        })
    }
}

#[derive(sqlx::FromRow)]
struct StyleProfileRow {
    id: String,
    project_id: String,
    name: String,
    style_prompt: String,
    color_prompt: Option<String>,
    line_prompt: Option<String>,
    negative_prompt: Option<String>,
    output_notes: Option<String>,
    active_revision_id: Option<String>,
    created_at: String,
    updated_at: String,
}

impl StyleProfileRow {
    fn into_domain(self) -> Result<StyleProfile, RepositoryError> {
        Ok(StyleProfile {
            id: self.id,
            project_id: self.project_id,
            name: self.name,
            style_prompt: self.style_prompt,
            color_prompt: self.color_prompt,
            line_prompt: self.line_prompt,
            negative_prompt: self.negative_prompt,
            output_notes: self.output_notes,
            active_revision_id: self.active_revision_id,
            created_at: parse_datetime("style profile created_at", &self.created_at)?,
            updated_at: parse_datetime("style profile updated_at", &self.updated_at)?,
        })
    }
}

#[derive(sqlx::FromRow)]
struct CostumeVariantRow {
    id: String,
    character_profile_id: String,
    name: String,
    prompt_fragment: String,
    reference_set_id: Option<String>,
    is_default: i64,
    ordinal: i64,
    active_revision_id: Option<String>,
    created_at: String,
    updated_at: String,
}

impl CostumeVariantRow {
    fn into_domain(self) -> Result<CostumeVariant, RepositoryError> {
        Ok(CostumeVariant {
            id: self.id,
            character_profile_id: self.character_profile_id,
            name: self.name,
            prompt_fragment: self.prompt_fragment,
            reference_set_id: self.reference_set_id,
            is_default: bool_from_sqlite("costume variant is_default", self.is_default)?,
            ordinal: self.ordinal,
            active_revision_id: self.active_revision_id,
            created_at: parse_datetime("costume variant created_at", &self.created_at)?,
            updated_at: parse_datetime("costume variant updated_at", &self.updated_at)?,
        })
    }
}

#[derive(sqlx::FromRow)]
struct ProfileRevisionRow {
    id: String,
    profile_type: String,
    profile_id: String,
    revision_number: i64,
    content_json: String,
    content_sha256: String,
    status: String,
    created_at: String,
    created_by: Option<String>,
}

impl ProfileRevisionRow {
    fn into_domain(self) -> Result<ProfileRevision, RepositoryError> {
        let profile_type = ProfileType::try_from_db(&self.profile_type)
            .map_err(|error| map_domain_error("profile revision profile_type", error))?;
        let status = ProfileRevisionStatus::try_from_db(&self.status)
            .map_err(|error| map_domain_error("profile revision status", error))?;
        Ok(ProfileRevision {
            id: self.id,
            profile_type,
            profile_id: self.profile_id,
            revision_number: self.revision_number,
            content_json: self.content_json,
            content_sha256: self.content_sha256,
            status,
            created_at: parse_datetime("profile revision created_at", &self.created_at)?,
            created_by: self.created_by,
        })
    }
}

fn bool_from_sqlite(field: &str, value: i64) -> Result<bool, RepositoryError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(RepositoryError::serialization(
            field,
            format!("expected SQLite boolean 0 or 1, got {other}"),
        )),
    }
}

async fn list_profiles_for_type(
    pool: &SqlitePool,
    project_id: &str,
    profile_type: ProfileType,
) -> Result<Vec<ConsistencyProfileRecord>, RepositoryError> {
    match profile_type {
        ProfileType::Character => sqlx::query_as::<_, CharacterProfileRow>(
            "SELECT id, project_id, name, description, canonical_prompt, negative_prompt,
                    default_style_profile_id, default_reference_set_id, active_revision_id,
                    metadata_json, created_at, updated_at
             FROM character_profiles
             WHERE project_id = ?
             ORDER BY updated_at DESC, created_at DESC, id ASC",
        )
        .bind(project_id)
        .fetch_all(pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(|row| row.into_domain().map(ConsistencyProfileRecord::Character))
        .collect(),
        ProfileType::Scene => sqlx::query_as::<_, SceneProfileRow>(
            "SELECT id, project_id, name, description, environment_prompt, lighting_prompt,
                    negative_prompt, default_style_profile_id, default_reference_set_id,
                    active_revision_id, created_at, updated_at
             FROM scene_profiles
             WHERE project_id = ?
             ORDER BY updated_at DESC, created_at DESC, id ASC",
        )
        .bind(project_id)
        .fetch_all(pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(|row| row.into_domain().map(ConsistencyProfileRecord::Scene))
        .collect(),
        ProfileType::Prop => sqlx::query_as::<_, PropProfileRow>(
            "SELECT id, project_id, name, description, canonical_prompt, material_prompt,
                    scale_prompt, default_reference_set_id, active_revision_id,
                    created_at, updated_at
             FROM prop_profiles
             WHERE project_id = ?
             ORDER BY updated_at DESC, created_at DESC, id ASC",
        )
        .bind(project_id)
        .fetch_all(pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(|row| row.into_domain().map(ConsistencyProfileRecord::Prop))
        .collect(),
        ProfileType::Style => sqlx::query_as::<_, StyleProfileRow>(
            "SELECT id, project_id, name, style_prompt, color_prompt, line_prompt,
                    negative_prompt, output_notes, active_revision_id, created_at, updated_at
             FROM style_profiles
             WHERE project_id = ?
             ORDER BY updated_at DESC, created_at DESC, id ASC",
        )
        .bind(project_id)
        .fetch_all(pool)
        .await
        .map_err(map_sqlx_error)?
        .into_iter()
        .map(|row| row.into_domain().map(ConsistencyProfileRecord::Style))
        .collect(),
    }
}

async fn find_profile_for_type(
    pool: &SqlitePool,
    project_id: &str,
    profile_type: ProfileType,
    profile_id: &str,
) -> Result<Option<ConsistencyProfileRecord>, RepositoryError> {
    match profile_type {
        ProfileType::Character => sqlx::query_as::<_, CharacterProfileRow>(
            "SELECT id, project_id, name, description, canonical_prompt, negative_prompt,
                    default_style_profile_id, default_reference_set_id, active_revision_id,
                    metadata_json, created_at, updated_at
             FROM character_profiles
             WHERE project_id = ? AND id = ?",
        )
        .bind(project_id)
        .bind(profile_id)
        .fetch_optional(pool)
        .await
        .map_err(map_sqlx_error)?
        .map(|row| row.into_domain().map(ConsistencyProfileRecord::Character))
        .transpose(),
        ProfileType::Scene => sqlx::query_as::<_, SceneProfileRow>(
            "SELECT id, project_id, name, description, environment_prompt, lighting_prompt,
                    negative_prompt, default_style_profile_id, default_reference_set_id,
                    active_revision_id, created_at, updated_at
             FROM scene_profiles
             WHERE project_id = ? AND id = ?",
        )
        .bind(project_id)
        .bind(profile_id)
        .fetch_optional(pool)
        .await
        .map_err(map_sqlx_error)?
        .map(|row| row.into_domain().map(ConsistencyProfileRecord::Scene))
        .transpose(),
        ProfileType::Prop => sqlx::query_as::<_, PropProfileRow>(
            "SELECT id, project_id, name, description, canonical_prompt, material_prompt,
                    scale_prompt, default_reference_set_id, active_revision_id,
                    created_at, updated_at
             FROM prop_profiles
             WHERE project_id = ? AND id = ?",
        )
        .bind(project_id)
        .bind(profile_id)
        .fetch_optional(pool)
        .await
        .map_err(map_sqlx_error)?
        .map(|row| row.into_domain().map(ConsistencyProfileRecord::Prop))
        .transpose(),
        ProfileType::Style => sqlx::query_as::<_, StyleProfileRow>(
            "SELECT id, project_id, name, style_prompt, color_prompt, line_prompt,
                    negative_prompt, output_notes, active_revision_id, created_at, updated_at
             FROM style_profiles
             WHERE project_id = ? AND id = ?",
        )
        .bind(project_id)
        .bind(profile_id)
        .fetch_optional(pool)
        .await
        .map_err(map_sqlx_error)?
        .map(|row| row.into_domain().map(ConsistencyProfileRecord::Style))
        .transpose(),
    }
}

async fn insert_profile_record(
    pool: &SqlitePool,
    profile: &ConsistencyProfileRecord,
) -> Result<(), RepositoryError> {
    match profile {
        ConsistencyProfileRecord::Character(profile) => {
            sqlx::query(
                "INSERT INTO character_profiles
                 (id, project_id, name, description, canonical_prompt, negative_prompt,
                  default_style_profile_id, default_reference_set_id, active_revision_id,
                  metadata_json, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&profile.id)
            .bind(&profile.project_id)
            .bind(&profile.name)
            .bind(&profile.description)
            .bind(&profile.canonical_prompt)
            .bind(&profile.negative_prompt)
            .bind(&profile.default_style_profile_id)
            .bind(&profile.default_reference_set_id)
            .bind(&profile.active_revision_id)
            .bind(&profile.metadata_json)
            .bind(format_datetime(profile.created_at))
            .bind(format_datetime(profile.updated_at))
            .execute(pool)
            .await
            .map_err(map_sqlx_error)?;
        }
        ConsistencyProfileRecord::Scene(profile) => {
            sqlx::query(
                "INSERT INTO scene_profiles
                 (id, project_id, name, description, environment_prompt, lighting_prompt,
                  negative_prompt, default_style_profile_id, default_reference_set_id,
                  active_revision_id, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&profile.id)
            .bind(&profile.project_id)
            .bind(&profile.name)
            .bind(&profile.description)
            .bind(&profile.environment_prompt)
            .bind(&profile.lighting_prompt)
            .bind(&profile.negative_prompt)
            .bind(&profile.default_style_profile_id)
            .bind(&profile.default_reference_set_id)
            .bind(&profile.active_revision_id)
            .bind(format_datetime(profile.created_at))
            .bind(format_datetime(profile.updated_at))
            .execute(pool)
            .await
            .map_err(map_sqlx_error)?;
        }
        ConsistencyProfileRecord::Prop(profile) => {
            sqlx::query(
                "INSERT INTO prop_profiles
                 (id, project_id, name, description, canonical_prompt, material_prompt,
                  scale_prompt, default_reference_set_id, active_revision_id, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&profile.id)
            .bind(&profile.project_id)
            .bind(&profile.name)
            .bind(&profile.description)
            .bind(&profile.canonical_prompt)
            .bind(&profile.material_prompt)
            .bind(&profile.scale_prompt)
            .bind(&profile.default_reference_set_id)
            .bind(&profile.active_revision_id)
            .bind(format_datetime(profile.created_at))
            .bind(format_datetime(profile.updated_at))
            .execute(pool)
            .await
            .map_err(map_sqlx_error)?;
        }
        ConsistencyProfileRecord::Style(profile) => {
            sqlx::query(
                "INSERT INTO style_profiles
                 (id, project_id, name, style_prompt, color_prompt, line_prompt,
                  negative_prompt, output_notes, active_revision_id, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&profile.id)
            .bind(&profile.project_id)
            .bind(&profile.name)
            .bind(&profile.style_prompt)
            .bind(&profile.color_prompt)
            .bind(&profile.line_prompt)
            .bind(&profile.negative_prompt)
            .bind(&profile.output_notes)
            .bind(&profile.active_revision_id)
            .bind(format_datetime(profile.created_at))
            .bind(format_datetime(profile.updated_at))
            .execute(pool)
            .await
            .map_err(map_sqlx_error)?;
        }
    }
    Ok(())
}

async fn update_profile_record(
    pool: &SqlitePool,
    profile: &ConsistencyProfileRecord,
) -> Result<bool, RepositoryError> {
    let result = match profile {
        ConsistencyProfileRecord::Character(profile) => {
            sqlx::query(
                "UPDATE character_profiles
             SET name = ?, description = ?, canonical_prompt = ?, negative_prompt = ?,
                 default_style_profile_id = ?, default_reference_set_id = ?,
                 active_revision_id = ?, metadata_json = ?, updated_at = ?
             WHERE id = ? AND project_id = ?",
            )
            .bind(&profile.name)
            .bind(&profile.description)
            .bind(&profile.canonical_prompt)
            .bind(&profile.negative_prompt)
            .bind(&profile.default_style_profile_id)
            .bind(&profile.default_reference_set_id)
            .bind(&profile.active_revision_id)
            .bind(&profile.metadata_json)
            .bind(format_datetime(profile.updated_at))
            .bind(&profile.id)
            .bind(&profile.project_id)
            .execute(pool)
            .await
        }
        ConsistencyProfileRecord::Scene(profile) => {
            sqlx::query(
                "UPDATE scene_profiles
             SET name = ?, description = ?, environment_prompt = ?, lighting_prompt = ?,
                 negative_prompt = ?, default_style_profile_id = ?,
                 default_reference_set_id = ?, active_revision_id = ?, updated_at = ?
             WHERE id = ? AND project_id = ?",
            )
            .bind(&profile.name)
            .bind(&profile.description)
            .bind(&profile.environment_prompt)
            .bind(&profile.lighting_prompt)
            .bind(&profile.negative_prompt)
            .bind(&profile.default_style_profile_id)
            .bind(&profile.default_reference_set_id)
            .bind(&profile.active_revision_id)
            .bind(format_datetime(profile.updated_at))
            .bind(&profile.id)
            .bind(&profile.project_id)
            .execute(pool)
            .await
        }
        ConsistencyProfileRecord::Prop(profile) => {
            sqlx::query(
                "UPDATE prop_profiles
             SET name = ?, description = ?, canonical_prompt = ?, material_prompt = ?,
                 scale_prompt = ?, default_reference_set_id = ?, active_revision_id = ?,
                 updated_at = ?
             WHERE id = ? AND project_id = ?",
            )
            .bind(&profile.name)
            .bind(&profile.description)
            .bind(&profile.canonical_prompt)
            .bind(&profile.material_prompt)
            .bind(&profile.scale_prompt)
            .bind(&profile.default_reference_set_id)
            .bind(&profile.active_revision_id)
            .bind(format_datetime(profile.updated_at))
            .bind(&profile.id)
            .bind(&profile.project_id)
            .execute(pool)
            .await
        }
        ConsistencyProfileRecord::Style(profile) => {
            sqlx::query(
                "UPDATE style_profiles
             SET name = ?, style_prompt = ?, color_prompt = ?, line_prompt = ?,
                 negative_prompt = ?, output_notes = ?, active_revision_id = ?, updated_at = ?
             WHERE id = ? AND project_id = ?",
            )
            .bind(&profile.name)
            .bind(&profile.style_prompt)
            .bind(&profile.color_prompt)
            .bind(&profile.line_prompt)
            .bind(&profile.negative_prompt)
            .bind(&profile.output_notes)
            .bind(&profile.active_revision_id)
            .bind(format_datetime(profile.updated_at))
            .bind(&profile.id)
            .bind(&profile.project_id)
            .execute(pool)
            .await
        }
    }
    .map_err(map_sqlx_error)?;
    Ok(result.rows_affected() == 1)
}

async fn ensure_profile_not_referenced(
    pool: &SqlitePool,
    project_id: &str,
    profile_type: ProfileType,
    profile_id: &str,
) -> Result<(), RepositoryError> {
    let profile_type = profile_type.as_str();
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM shot_profile_bindings AS binding
         INNER JOIN shots AS shot ON shot.id = binding.shot_id
         WHERE shot.project_id = ? AND binding.profile_type = ? AND binding.profile_id = ?",
    )
    .bind(project_id)
    .bind(profile_type)
    .bind(profile_id)
    .fetch_one(pool)
    .await
    .map_err(map_sqlx_error)?;
    if count > 0 {
        return Err(RepositoryError::integrity(format!(
            "profile {profile_id} is referenced by {count} shot profile binding(s)"
        )));
    }

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM reference_sets
         WHERE project_id = ? AND owner_profile_type = ? AND owner_profile_id = ?",
    )
    .bind(project_id)
    .bind(profile_type)
    .bind(profile_id)
    .fetch_one(pool)
    .await
    .map_err(map_sqlx_error)?;
    if count > 0 {
        return Err(RepositoryError::integrity(format!(
            "profile {profile_id} owns {count} reference set(s)"
        )));
    }

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM profile_revisions
         WHERE profile_type = ? AND profile_id = ?",
    )
    .bind(profile_type)
    .bind(profile_id)
    .fetch_one(pool)
    .await
    .map_err(map_sqlx_error)?;
    if count > 0 {
        return Err(RepositoryError::integrity(format!(
            "profile {profile_id} has {count} immutable revision(s)"
        )));
    }

    if profile_type == ProfileType::Character.as_str() {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM costume_variants WHERE character_profile_id = ?",
        )
        .bind(profile_id)
        .fetch_one(pool)
        .await
        .map_err(map_sqlx_error)?;
        if count > 0 {
            return Err(RepositoryError::integrity(format!(
                "character profile {profile_id} has {count} costume variant(s)"
            )));
        }
    }

    if profile_type == ProfileType::Style.as_str() {
        let count: i64 = sqlx::query_scalar(
            "SELECT
                 (SELECT COUNT(*) FROM character_profiles WHERE default_style_profile_id = ?)
                 + (SELECT COUNT(*) FROM scene_profiles WHERE default_style_profile_id = ?)",
        )
        .bind(profile_id)
        .bind(profile_id)
        .fetch_one(pool)
        .await
        .map_err(map_sqlx_error)?;
        if count > 0 {
            return Err(RepositoryError::integrity(format!(
                "style profile {profile_id} is used as a default by {count} profile(s)"
            )));
        }
    }

    Ok(())
}

async fn insert_costume_variant_record(
    pool: &SqlitePool,
    variant: &CostumeVariant,
) -> Result<(), RepositoryError> {
    sqlx::query(
        "INSERT INTO costume_variants
         (id, character_profile_id, name, prompt_fragment, reference_set_id, is_default,
          ordinal, active_revision_id, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&variant.id)
    .bind(&variant.character_profile_id)
    .bind(&variant.name)
    .bind(&variant.prompt_fragment)
    .bind(&variant.reference_set_id)
    .bind(if variant.is_default { 1_i64 } else { 0_i64 })
    .bind(variant.ordinal)
    .bind(&variant.active_revision_id)
    .bind(format_datetime(variant.created_at))
    .bind(format_datetime(variant.updated_at))
    .execute(pool)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

async fn update_costume_variant_record(
    pool: &SqlitePool,
    variant: &CostumeVariant,
) -> Result<bool, RepositoryError> {
    let result = sqlx::query(
        "UPDATE costume_variants
         SET name = ?, prompt_fragment = ?, reference_set_id = ?, is_default = ?,
             ordinal = ?, active_revision_id = ?, updated_at = ?
         WHERE id = ? AND character_profile_id = ?",
    )
    .bind(&variant.name)
    .bind(&variant.prompt_fragment)
    .bind(&variant.reference_set_id)
    .bind(if variant.is_default { 1_i64 } else { 0_i64 })
    .bind(variant.ordinal)
    .bind(&variant.active_revision_id)
    .bind(format_datetime(variant.updated_at))
    .bind(&variant.id)
    .bind(&variant.character_profile_id)
    .execute(pool)
    .await
    .map_err(map_sqlx_error)?;
    Ok(result.rows_affected() == 1)
}

async fn ensure_costume_not_referenced(
    pool: &SqlitePool,
    costume_variant_id: &str,
) -> Result<(), RepositoryError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM shot_profile_bindings WHERE costume_variant_id = ?",
    )
    .bind(costume_variant_id)
    .fetch_one(pool)
    .await
    .map_err(map_sqlx_error)?;
    if count > 0 {
        return Err(RepositoryError::integrity(format!(
            "costume variant {costume_variant_id} is referenced by {count} shot binding(s)"
        )));
    }
    Ok(())
}

#[async_trait]
impl ConsistencyProfileRepository for SqliteConsistencyProfileRepository {
    async fn list_profiles(
        &self,
        project_id: &str,
        profile_type: ProfileType,
    ) -> Result<Vec<ConsistencyProfileRecord>, RepositoryError> {
        list_profiles_for_type(&self.pool, project_id, profile_type).await
    }

    async fn find_profile(
        &self,
        project_id: &str,
        profile_type: ProfileType,
        profile_id: &str,
    ) -> Result<Option<ConsistencyProfileRecord>, RepositoryError> {
        find_profile_for_type(&self.pool, project_id, profile_type, profile_id).await
    }

    async fn insert_profile(
        &self,
        profile: &ConsistencyProfileRecord,
    ) -> Result<(), RepositoryError> {
        insert_profile_record(&self.pool, profile).await
    }

    async fn update_profile(
        &self,
        profile: &ConsistencyProfileRecord,
    ) -> Result<bool, RepositoryError> {
        update_profile_record(&self.pool, profile).await
    }

    async fn delete_profile(
        &self,
        project_id: &str,
        profile_type: ProfileType,
        profile_id: &str,
    ) -> Result<bool, RepositoryError> {
        if find_profile_for_type(&self.pool, project_id, profile_type, profile_id)
            .await?
            .is_none()
        {
            return Ok(false);
        }

        ensure_profile_not_referenced(&self.pool, project_id, profile_type, profile_id).await?;
        let result = match profile_type {
            ProfileType::Character => {
                sqlx::query("DELETE FROM character_profiles WHERE project_id = ? AND id = ?")
                    .bind(project_id)
                    .bind(profile_id)
                    .execute(&self.pool)
                    .await
            }
            ProfileType::Scene => {
                sqlx::query("DELETE FROM scene_profiles WHERE project_id = ? AND id = ?")
                    .bind(project_id)
                    .bind(profile_id)
                    .execute(&self.pool)
                    .await
            }
            ProfileType::Prop => {
                sqlx::query("DELETE FROM prop_profiles WHERE project_id = ? AND id = ?")
                    .bind(project_id)
                    .bind(profile_id)
                    .execute(&self.pool)
                    .await
            }
            ProfileType::Style => {
                sqlx::query("DELETE FROM style_profiles WHERE project_id = ? AND id = ?")
                    .bind(project_id)
                    .bind(profile_id)
                    .execute(&self.pool)
                    .await
            }
        }
        .map_err(map_sqlx_error)?;
        Ok(result.rows_affected() == 1)
    }

    async fn list_costume_variants(
        &self,
        character_profile_id: &str,
    ) -> Result<Vec<CostumeVariant>, RepositoryError> {
        let rows = sqlx::query_as::<_, CostumeVariantRow>(
            "SELECT id, character_profile_id, name, prompt_fragment, reference_set_id,
                    is_default, ordinal, active_revision_id, created_at, updated_at
             FROM costume_variants
             WHERE character_profile_id = ?
             ORDER BY ordinal ASC, name ASC, id ASC",
        )
        .bind(character_profile_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(CostumeVariantRow::into_domain)
            .collect()
    }

    async fn find_costume_variant(
        &self,
        costume_variant_id: &str,
    ) -> Result<Option<CostumeVariant>, RepositoryError> {
        sqlx::query_as::<_, CostumeVariantRow>(
            "SELECT id, character_profile_id, name, prompt_fragment, reference_set_id,
                    is_default, ordinal, active_revision_id, created_at, updated_at
             FROM costume_variants
             WHERE id = ?",
        )
        .bind(costume_variant_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .map(CostumeVariantRow::into_domain)
        .transpose()
    }

    async fn insert_costume_variant(
        &self,
        costume_variant: &CostumeVariant,
    ) -> Result<(), RepositoryError> {
        insert_costume_variant_record(&self.pool, costume_variant).await
    }

    async fn update_costume_variant(
        &self,
        costume_variant: &CostumeVariant,
    ) -> Result<bool, RepositoryError> {
        update_costume_variant_record(&self.pool, costume_variant).await
    }

    async fn delete_costume_variant(
        &self,
        costume_variant_id: &str,
    ) -> Result<bool, RepositoryError> {
        if self
            .find_costume_variant(costume_variant_id)
            .await?
            .is_none()
        {
            return Ok(false);
        }
        ensure_costume_not_referenced(&self.pool, costume_variant_id).await?;
        let result = sqlx::query("DELETE FROM costume_variants WHERE id = ?")
            .bind(costume_variant_id)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        Ok(result.rows_affected() == 1)
    }

    async fn list_profile_revisions(
        &self,
        profile_type: ProfileType,
        profile_id: &str,
    ) -> Result<Vec<ProfileRevision>, RepositoryError> {
        let rows = sqlx::query_as::<_, ProfileRevisionRow>(
            "SELECT id, profile_type, profile_id, revision_number, content_json,
                    content_sha256, status, created_at, created_by
             FROM profile_revisions
             WHERE profile_type = ? AND profile_id = ?
             ORDER BY revision_number ASC, id ASC",
        )
        .bind(profile_type.as_str())
        .bind(profile_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        rows.into_iter()
            .map(ProfileRevisionRow::into_domain)
            .collect()
    }

    async fn find_profile_revision(
        &self,
        revision_id: &str,
    ) -> Result<Option<ProfileRevision>, RepositoryError> {
        sqlx::query_as::<_, ProfileRevisionRow>(
            "SELECT id, profile_type, profile_id, revision_number, content_json,
                    content_sha256, status, created_at, created_by
             FROM profile_revisions
             WHERE id = ?",
        )
        .bind(revision_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .map(ProfileRevisionRow::into_domain)
        .transpose()
    }

    async fn insert_profile_revision(
        &self,
        revision: &ProfileRevision,
    ) -> Result<(), RepositoryError> {
        if revision.revision_number < 1 {
            return Err(RepositoryError::integrity(
                "profile revision_number must be at least 1",
            ));
        }
        if revision.content_sha256.trim().is_empty() {
            return Err(RepositoryError::integrity(
                "profile revision content_sha256 must not be empty",
            ));
        }
        sqlx::query(
            "INSERT INTO profile_revisions
             (id, profile_type, profile_id, revision_number, content_json, content_sha256,
              status, created_at, created_by)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&revision.id)
        .bind(revision.profile_type.as_str())
        .bind(&revision.profile_id)
        .bind(revision.revision_number)
        .bind(&revision.content_json)
        .bind(&revision.content_sha256)
        .bind(revision.status.as_str())
        .bind(format_datetime(revision.created_at))
        .bind(&revision.created_by)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }
}
