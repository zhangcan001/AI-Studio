use crate::{
    app_state::AppState,
    application::{
        consistency_profile_service::{
            ConsistencyProfileError, CreateCharacterProfileRequest, CreateCostumeVariantRequest,
            CreatePropProfileRequest, CreateSceneProfileRequest, CreateStyleProfileRequest,
            UpdateCharacterProfileRequest, UpdateCostumeVariantRequest, UpdatePropProfileRequest,
            UpdateSceneProfileRequest, UpdateStyleProfileRequest,
        },
        reference_set_service::{
            CreateReferenceSetRequest, ReferenceSetDetailView, ReferenceSetError,
            ReferenceSetItemRequest, UpdateReferenceSetRequest,
        },
    },
    domain::consistency::{ConsistencyProfileRecord, ProfileType, ReferenceSetPurpose},
    error::AppError,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyProfileView {
    pub id: String,
    pub project_id: String,
    pub profile_type: String,
    pub name: String,
    pub description: String,
    pub canonical_prompt: Option<String>,
    pub negative_prompt: Option<String>,
    pub environment_prompt: Option<String>,
    pub lighting_prompt: Option<String>,
    pub material_prompt: Option<String>,
    pub scale_prompt: Option<String>,
    pub style_prompt: Option<String>,
    pub color_prompt: Option<String>,
    pub line_prompt: Option<String>,
    pub output_notes: Option<String>,
    pub metadata_json: Option<String>,
    pub default_reference_set_id: Option<String>,
    pub default_style_profile_id: Option<String>,
    pub active_revision_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CostumeVariantView {
    pub id: String,
    pub character_profile_id: String,
    pub name: String,
    pub prompt_fragment: String,
    pub reference_set_id: Option<String>,
    pub is_default: bool,
    pub ordinal: i64,
    pub active_revision_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceSetView {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub purpose: String,
    pub description: String,
    pub owner_profile_type: Option<String>,
    pub owner_profile_id: Option<String>,
    pub active_revision_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceSetItemView {
    pub asset_id: String,
    pub ordinal: i64,
    pub role: Option<String>,
    pub is_primary: bool,
    pub asset_name: String,
    pub thumbnail_available: bool,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceSetDetailViewDto {
    pub reference_set: ReferenceSetView,
    pub items: Vec<ReferenceSetItemView>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterProfileCreateRequest {
    pub project_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub canonical_prompt: String,
    #[serde(default)]
    pub negative_prompt: String,
    #[serde(default)]
    pub default_style_profile_id: Option<String>,
    #[serde(default)]
    pub default_reference_set_id: Option<String>,
    #[serde(default = "default_metadata_json")]
    pub metadata_json: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterProfileUpdateRequest {
    pub project_id: String,
    pub profile_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub canonical_prompt: String,
    #[serde(default)]
    pub negative_prompt: String,
    #[serde(default)]
    pub default_style_profile_id: Option<String>,
    #[serde(default)]
    pub default_reference_set_id: Option<String>,
    #[serde(default = "default_metadata_json")]
    pub metadata_json: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneProfileCreateRequest {
    pub project_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub environment_prompt: String,
    #[serde(default)]
    pub lighting_prompt: Option<String>,
    #[serde(default)]
    pub negative_prompt: Option<String>,
    #[serde(default)]
    pub default_style_profile_id: Option<String>,
    #[serde(default)]
    pub default_reference_set_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneProfileUpdateRequest {
    pub project_id: String,
    pub profile_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub environment_prompt: String,
    #[serde(default)]
    pub lighting_prompt: Option<String>,
    #[serde(default)]
    pub negative_prompt: Option<String>,
    #[serde(default)]
    pub default_style_profile_id: Option<String>,
    #[serde(default)]
    pub default_reference_set_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropProfileCreateRequest {
    pub project_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub canonical_prompt: String,
    #[serde(default)]
    pub material_prompt: Option<String>,
    #[serde(default)]
    pub scale_prompt: Option<String>,
    #[serde(default)]
    pub default_reference_set_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropProfileUpdateRequest {
    pub project_id: String,
    pub profile_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub canonical_prompt: String,
    #[serde(default)]
    pub material_prompt: Option<String>,
    #[serde(default)]
    pub scale_prompt: Option<String>,
    #[serde(default)]
    pub default_reference_set_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleProfileCreateRequest {
    pub project_id: String,
    pub name: String,
    #[serde(default)]
    pub style_prompt: String,
    #[serde(default)]
    pub color_prompt: Option<String>,
    #[serde(default)]
    pub line_prompt: Option<String>,
    #[serde(default)]
    pub negative_prompt: Option<String>,
    #[serde(default)]
    pub output_notes: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleProfileUpdateRequest {
    pub project_id: String,
    pub profile_id: String,
    pub name: String,
    #[serde(default)]
    pub style_prompt: String,
    #[serde(default)]
    pub color_prompt: Option<String>,
    #[serde(default)]
    pub line_prompt: Option<String>,
    #[serde(default)]
    pub negative_prompt: Option<String>,
    #[serde(default)]
    pub output_notes: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CostumeVariantCreateRequest {
    pub project_id: String,
    pub character_profile_id: String,
    pub name: String,
    #[serde(default)]
    pub prompt_fragment: String,
    #[serde(default)]
    pub reference_set_id: Option<String>,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default)]
    pub ordinal: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CostumeVariantUpdateRequest {
    pub project_id: String,
    pub costume_variant_id: String,
    pub name: String,
    #[serde(default)]
    pub prompt_fragment: String,
    #[serde(default)]
    pub reference_set_id: Option<String>,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default)]
    pub ordinal: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceSetItemRequestDto {
    pub asset_id: String,
    pub ordinal: i64,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub is_primary: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceSetCreateRequest {
    pub project_id: String,
    pub name: String,
    pub purpose: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub owner_profile_type: Option<String>,
    #[serde(default)]
    pub owner_profile_id: Option<String>,
    #[serde(default)]
    pub items: Vec<ReferenceSetItemRequestDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceSetUpdateRequest {
    pub project_id: String,
    pub reference_set_id: String,
    pub name: String,
    pub purpose: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub owner_profile_type: Option<String>,
    #[serde(default)]
    pub owner_profile_id: Option<String>,
    #[serde(default)]
    pub items: Vec<ReferenceSetItemRequestDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceSetCreateFromAnchorRequest {
    pub project_id: String,
    pub anchor_id: String,
    #[serde(default)]
    pub new_name: Option<String>,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn consistency_profile_list(
    state: State<'_, AppState>,
    project_id: String,
    profile_type: Option<String>,
) -> Result<Vec<ConsistencyProfileView>, AppError> {
    super::validate_project_id(&project_id)?;
    let profile_type = parse_optional_profile_type(profile_type)?;
    state
        .consistency_profile_service
        .list(&project_id, profile_type)
        .await
        .map_err(map_profile_error)
        .map(|profiles| profiles.into_iter().map(profile_view).collect())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn consistency_profile_get(
    state: State<'_, AppState>,
    project_id: String,
    profile_type: String,
    profile_id: String,
) -> Result<ConsistencyProfileView, AppError> {
    super::validate_project_id(&project_id)?;
    let profile_type = parse_profile_type(&profile_type)?;
    state
        .consistency_profile_service
        .get(&project_id, profile_type, &profile_id)
        .await
        .map_err(map_profile_error)
        .map(profile_view)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn character_profile_create(
    state: State<'_, AppState>,
    request: CharacterProfileCreateRequest,
) -> Result<ConsistencyProfileView, AppError> {
    super::validate_project_id(&request.project_id)?;
    state
        .consistency_profile_service
        .create_character(CreateCharacterProfileRequest {
            project_id: request.project_id,
            name: request.name,
            description: request.description,
            canonical_prompt: request.canonical_prompt,
            negative_prompt: request.negative_prompt,
            default_style_profile_id: request.default_style_profile_id,
            default_reference_set_id: request.default_reference_set_id,
            metadata_json: request.metadata_json,
        })
        .await
        .map_err(map_profile_error)
        .map(|profile| profile_view(ConsistencyProfileRecord::Character(profile)))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn character_profile_update(
    state: State<'_, AppState>,
    request: CharacterProfileUpdateRequest,
) -> Result<ConsistencyProfileView, AppError> {
    super::validate_project_id(&request.project_id)?;
    state
        .consistency_profile_service
        .update_character(UpdateCharacterProfileRequest {
            project_id: request.project_id,
            profile_id: request.profile_id,
            name: request.name,
            description: request.description,
            canonical_prompt: request.canonical_prompt,
            negative_prompt: request.negative_prompt,
            default_style_profile_id: request.default_style_profile_id,
            default_reference_set_id: request.default_reference_set_id,
            metadata_json: request.metadata_json,
        })
        .await
        .map_err(map_profile_error)
        .map(|profile| profile_view(ConsistencyProfileRecord::Character(profile)))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn scene_profile_create(
    state: State<'_, AppState>,
    request: SceneProfileCreateRequest,
) -> Result<ConsistencyProfileView, AppError> {
    super::validate_project_id(&request.project_id)?;
    state
        .consistency_profile_service
        .create_scene(CreateSceneProfileRequest {
            project_id: request.project_id,
            name: request.name,
            description: request.description,
            environment_prompt: request.environment_prompt,
            lighting_prompt: request.lighting_prompt,
            negative_prompt: request.negative_prompt,
            default_style_profile_id: request.default_style_profile_id,
            default_reference_set_id: request.default_reference_set_id,
        })
        .await
        .map_err(map_profile_error)
        .map(|profile| profile_view(ConsistencyProfileRecord::Scene(profile)))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn scene_profile_update(
    state: State<'_, AppState>,
    request: SceneProfileUpdateRequest,
) -> Result<ConsistencyProfileView, AppError> {
    super::validate_project_id(&request.project_id)?;
    state
        .consistency_profile_service
        .update_scene(UpdateSceneProfileRequest {
            project_id: request.project_id,
            profile_id: request.profile_id,
            name: request.name,
            description: request.description,
            environment_prompt: request.environment_prompt,
            lighting_prompt: request.lighting_prompt,
            negative_prompt: request.negative_prompt,
            default_style_profile_id: request.default_style_profile_id,
            default_reference_set_id: request.default_reference_set_id,
        })
        .await
        .map_err(map_profile_error)
        .map(|profile| profile_view(ConsistencyProfileRecord::Scene(profile)))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn prop_profile_create(
    state: State<'_, AppState>,
    request: PropProfileCreateRequest,
) -> Result<ConsistencyProfileView, AppError> {
    super::validate_project_id(&request.project_id)?;
    state
        .consistency_profile_service
        .create_prop(CreatePropProfileRequest {
            project_id: request.project_id,
            name: request.name,
            description: request.description,
            canonical_prompt: request.canonical_prompt,
            material_prompt: request.material_prompt,
            scale_prompt: request.scale_prompt,
            default_reference_set_id: request.default_reference_set_id,
        })
        .await
        .map_err(map_profile_error)
        .map(|profile| profile_view(ConsistencyProfileRecord::Prop(profile)))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn prop_profile_update(
    state: State<'_, AppState>,
    request: PropProfileUpdateRequest,
) -> Result<ConsistencyProfileView, AppError> {
    super::validate_project_id(&request.project_id)?;
    state
        .consistency_profile_service
        .update_prop(UpdatePropProfileRequest {
            project_id: request.project_id,
            profile_id: request.profile_id,
            name: request.name,
            description: request.description,
            canonical_prompt: request.canonical_prompt,
            material_prompt: request.material_prompt,
            scale_prompt: request.scale_prompt,
            default_reference_set_id: request.default_reference_set_id,
        })
        .await
        .map_err(map_profile_error)
        .map(|profile| profile_view(ConsistencyProfileRecord::Prop(profile)))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn style_profile_create(
    state: State<'_, AppState>,
    request: StyleProfileCreateRequest,
) -> Result<ConsistencyProfileView, AppError> {
    super::validate_project_id(&request.project_id)?;
    state
        .consistency_profile_service
        .create_style(CreateStyleProfileRequest {
            project_id: request.project_id,
            name: request.name,
            style_prompt: request.style_prompt,
            color_prompt: request.color_prompt,
            line_prompt: request.line_prompt,
            negative_prompt: request.negative_prompt,
            output_notes: request.output_notes,
        })
        .await
        .map_err(map_profile_error)
        .map(|profile| profile_view(ConsistencyProfileRecord::Style(profile)))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn style_profile_update(
    state: State<'_, AppState>,
    request: StyleProfileUpdateRequest,
) -> Result<ConsistencyProfileView, AppError> {
    super::validate_project_id(&request.project_id)?;
    state
        .consistency_profile_service
        .update_style(UpdateStyleProfileRequest {
            project_id: request.project_id,
            profile_id: request.profile_id,
            name: request.name,
            style_prompt: request.style_prompt,
            color_prompt: request.color_prompt,
            line_prompt: request.line_prompt,
            negative_prompt: request.negative_prompt,
            output_notes: request.output_notes,
        })
        .await
        .map_err(map_profile_error)
        .map(|profile| profile_view(ConsistencyProfileRecord::Style(profile)))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn consistency_profile_delete(
    state: State<'_, AppState>,
    project_id: String,
    profile_type: String,
    profile_id: String,
) -> Result<(), AppError> {
    super::validate_project_id(&project_id)?;
    let profile_type = parse_profile_type(&profile_type)?;
    state
        .asset_usage_service
        .ensure_profile_deletable(&project_id, profile_type, &profile_id)
        .await
        .map_err(map_usage_error)?;
    state
        .consistency_profile_service
        .delete(&project_id, profile_type, &profile_id)
        .await
        .map_err(map_profile_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn costume_variant_list(
    state: State<'_, AppState>,
    project_id: String,
    character_profile_id: String,
) -> Result<Vec<CostumeVariantView>, AppError> {
    super::validate_project_id(&project_id)?;
    state
        .consistency_profile_service
        .list_costumes(&project_id, &character_profile_id)
        .await
        .map_err(map_profile_error)
        .map(|costumes| costumes.into_iter().map(costume_view).collect())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn costume_variant_get(
    state: State<'_, AppState>,
    project_id: String,
    costume_variant_id: String,
) -> Result<CostumeVariantView, AppError> {
    super::validate_project_id(&project_id)?;
    state
        .consistency_profile_service
        .get_costume(&project_id, &costume_variant_id)
        .await
        .map_err(map_profile_error)
        .map(costume_view)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn costume_variant_create(
    state: State<'_, AppState>,
    request: CostumeVariantCreateRequest,
) -> Result<CostumeVariantView, AppError> {
    super::validate_project_id(&request.project_id)?;
    state
        .consistency_profile_service
        .create_costume(CreateCostumeVariantRequest {
            project_id: request.project_id,
            character_profile_id: request.character_profile_id,
            name: request.name,
            prompt_fragment: request.prompt_fragment,
            reference_set_id: request.reference_set_id,
            is_default: request.is_default,
            ordinal: request.ordinal,
        })
        .await
        .map_err(map_profile_error)
        .map(costume_view)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn costume_variant_update(
    state: State<'_, AppState>,
    request: CostumeVariantUpdateRequest,
) -> Result<CostumeVariantView, AppError> {
    super::validate_project_id(&request.project_id)?;
    state
        .consistency_profile_service
        .update_costume(UpdateCostumeVariantRequest {
            project_id: request.project_id,
            costume_variant_id: request.costume_variant_id,
            name: request.name,
            prompt_fragment: request.prompt_fragment,
            reference_set_id: request.reference_set_id,
            is_default: request.is_default,
            ordinal: request.ordinal,
        })
        .await
        .map_err(map_profile_error)
        .map(costume_view)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn costume_variant_delete(
    state: State<'_, AppState>,
    project_id: String,
    costume_variant_id: String,
) -> Result<(), AppError> {
    super::validate_project_id(&project_id)?;
    state
        .consistency_profile_service
        .delete_costume(&project_id, &costume_variant_id)
        .await
        .map_err(map_profile_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn reference_set_list(
    state: State<'_, AppState>,
    project_id: String,
    purpose: Option<String>,
) -> Result<Vec<ReferenceSetView>, AppError> {
    super::validate_project_id(&project_id)?;
    let purpose = parse_optional_reference_set_purpose(purpose)?;
    state
        .reference_set_service
        .list(&project_id, purpose)
        .await
        .map_err(map_reference_set_error)
        .map(|sets| sets.into_iter().map(reference_set_view).collect())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn reference_set_detail_get(
    state: State<'_, AppState>,
    project_id: String,
    reference_set_id: String,
) -> Result<ReferenceSetDetailViewDto, AppError> {
    super::validate_project_id(&project_id)?;
    state
        .reference_set_service
        .get_detail(&project_id, &reference_set_id)
        .await
        .map_err(map_reference_set_error)
        .map(reference_set_detail_view)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn reference_set_create(
    state: State<'_, AppState>,
    request: ReferenceSetCreateRequest,
) -> Result<ReferenceSetView, AppError> {
    super::validate_project_id(&request.project_id)?;
    let purpose = parse_reference_set_purpose(&request.purpose)?;
    let owner_profile_type = parse_optional_profile_type(request.owner_profile_type)?;
    state
        .reference_set_service
        .create(CreateReferenceSetRequest {
            project_id: request.project_id,
            name: request.name,
            purpose,
            description: request.description,
            owner_profile_type,
            owner_profile_id: request.owner_profile_id,
            items: into_reference_set_items(request.items),
        })
        .await
        .map_err(map_reference_set_error)
        .map(|reference_set| reference_set_view(reference_set))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn reference_set_update(
    state: State<'_, AppState>,
    request: ReferenceSetUpdateRequest,
) -> Result<ReferenceSetView, AppError> {
    super::validate_project_id(&request.project_id)?;
    let purpose = parse_reference_set_purpose(&request.purpose)?;
    let owner_profile_type = parse_optional_profile_type(request.owner_profile_type)?;
    state
        .reference_set_service
        .update(UpdateReferenceSetRequest {
            project_id: request.project_id,
            reference_set_id: request.reference_set_id,
            name: request.name,
            purpose,
            description: request.description,
            owner_profile_type,
            owner_profile_id: request.owner_profile_id,
            items: into_reference_set_items(request.items),
        })
        .await
        .map_err(map_reference_set_error)
        .map(reference_set_view)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn reference_set_delete(
    state: State<'_, AppState>,
    project_id: String,
    reference_set_id: String,
) -> Result<(), AppError> {
    super::validate_project_id(&project_id)?;
    state
        .asset_usage_service
        .ensure_reference_set_deletable(&project_id, &reference_set_id)
        .await
        .map_err(map_usage_error)?;
    state
        .reference_set_service
        .delete(&project_id, &reference_set_id)
        .await
        .map_err(map_reference_set_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn reference_set_create_from_anchor(
    state: State<'_, AppState>,
    request: ReferenceSetCreateFromAnchorRequest,
) -> Result<ReferenceSetView, AppError> {
    super::validate_project_id(&request.project_id)?;
    state
        .reference_set_service
        .create_from_anchor(&request.project_id, &request.anchor_id, request.new_name)
        .await
        .map_err(map_reference_set_error)
        .map(reference_set_view)
}

fn default_metadata_json() -> String {
    "{}".to_owned()
}

fn parse_profile_type(value: &str) -> Result<ProfileType, AppError> {
    let normalized = value.trim().to_ascii_uppercase();
    ProfileType::try_from_str(&normalized).map_err(|error| {
        AppError::invalid_input(format!("CONSISTENCY_PROFILE_TYPE_INVALID: {error}"))
    })
}

fn parse_optional_profile_type(value: Option<String>) -> Result<Option<ProfileType>, AppError> {
    match value.as_deref().map(str::trim) {
        None | Some("") | Some("ALL") => Ok(None),
        Some(value) => parse_profile_type(value).map(Some),
    }
}

fn parse_reference_set_purpose(value: &str) -> Result<ReferenceSetPurpose, AppError> {
    let normalized = value.trim().to_ascii_uppercase();
    ReferenceSetPurpose::try_from_db(&normalized)
        .map_err(|error| AppError::invalid_input(format!("REFERENCE_SET_PURPOSE_INVALID: {error}")))
}

fn parse_optional_reference_set_purpose(
    value: Option<String>,
) -> Result<Option<ReferenceSetPurpose>, AppError> {
    match value.as_deref().map(str::trim) {
        None | Some("") | Some("ALL") => Ok(None),
        Some(value) => parse_reference_set_purpose(value).map(Some),
    }
}

fn into_reference_set_items(
    items: Vec<ReferenceSetItemRequestDto>,
) -> Vec<ReferenceSetItemRequest> {
    items
        .into_iter()
        .map(|item| ReferenceSetItemRequest {
            asset_id: item.asset_id,
            ordinal: item.ordinal,
            role: item.role,
            is_primary: item.is_primary,
        })
        .collect()
}

fn profile_view(record: ConsistencyProfileRecord) -> ConsistencyProfileView {
    match record {
        ConsistencyProfileRecord::Character(profile) => ConsistencyProfileView {
            id: profile.id,
            project_id: profile.project_id,
            profile_type: ProfileType::Character.as_str().to_owned(),
            name: profile.name,
            description: profile.description,
            canonical_prompt: Some(profile.canonical_prompt),
            negative_prompt: Some(profile.negative_prompt),
            environment_prompt: None,
            lighting_prompt: None,
            material_prompt: None,
            scale_prompt: None,
            style_prompt: None,
            color_prompt: None,
            line_prompt: None,
            output_notes: None,
            metadata_json: Some(profile.metadata_json),
            default_reference_set_id: profile.default_reference_set_id,
            default_style_profile_id: profile.default_style_profile_id,
            active_revision_id: profile.active_revision_id,
            created_at: profile.created_at,
            updated_at: profile.updated_at,
        },
        ConsistencyProfileRecord::Scene(profile) => ConsistencyProfileView {
            id: profile.id,
            project_id: profile.project_id,
            profile_type: ProfileType::Scene.as_str().to_owned(),
            name: profile.name,
            description: profile.description,
            canonical_prompt: None,
            negative_prompt: profile.negative_prompt.clone(),
            environment_prompt: Some(profile.environment_prompt),
            lighting_prompt: profile.lighting_prompt,
            material_prompt: None,
            scale_prompt: None,
            style_prompt: None,
            color_prompt: None,
            line_prompt: None,
            output_notes: None,
            metadata_json: None,
            default_reference_set_id: profile.default_reference_set_id,
            default_style_profile_id: profile.default_style_profile_id,
            active_revision_id: profile.active_revision_id,
            created_at: profile.created_at,
            updated_at: profile.updated_at,
        },
        ConsistencyProfileRecord::Prop(profile) => ConsistencyProfileView {
            id: profile.id,
            project_id: profile.project_id,
            profile_type: ProfileType::Prop.as_str().to_owned(),
            name: profile.name,
            description: profile.description,
            canonical_prompt: Some(profile.canonical_prompt),
            negative_prompt: None,
            environment_prompt: None,
            lighting_prompt: None,
            material_prompt: profile.material_prompt,
            scale_prompt: profile.scale_prompt,
            style_prompt: None,
            color_prompt: None,
            line_prompt: None,
            output_notes: None,
            metadata_json: None,
            default_reference_set_id: profile.default_reference_set_id,
            default_style_profile_id: None,
            active_revision_id: profile.active_revision_id,
            created_at: profile.created_at,
            updated_at: profile.updated_at,
        },
        ConsistencyProfileRecord::Style(profile) => ConsistencyProfileView {
            id: profile.id,
            project_id: profile.project_id,
            profile_type: ProfileType::Style.as_str().to_owned(),
            name: profile.name,
            description: String::new(),
            canonical_prompt: None,
            negative_prompt: profile.negative_prompt,
            environment_prompt: None,
            lighting_prompt: None,
            material_prompt: None,
            scale_prompt: None,
            style_prompt: Some(profile.style_prompt),
            color_prompt: profile.color_prompt,
            line_prompt: profile.line_prompt,
            output_notes: profile.output_notes,
            metadata_json: None,
            default_reference_set_id: None,
            default_style_profile_id: None,
            active_revision_id: profile.active_revision_id,
            created_at: profile.created_at,
            updated_at: profile.updated_at,
        },
    }
}

fn costume_view(costume: crate::domain::consistency::CostumeVariant) -> CostumeVariantView {
    CostumeVariantView {
        id: costume.id,
        character_profile_id: costume.character_profile_id,
        name: costume.name,
        prompt_fragment: costume.prompt_fragment,
        reference_set_id: costume.reference_set_id,
        is_default: costume.is_default,
        ordinal: costume.ordinal,
        active_revision_id: costume.active_revision_id,
        created_at: costume.created_at,
        updated_at: costume.updated_at,
    }
}

fn reference_set_view(reference_set: crate::domain::consistency::ReferenceSet) -> ReferenceSetView {
    ReferenceSetView {
        id: reference_set.id,
        project_id: reference_set.project_id,
        name: reference_set.name,
        purpose: reference_set.purpose.as_str().to_owned(),
        description: reference_set.description,
        owner_profile_type: reference_set
            .owner_profile_type
            .map(|profile_type| profile_type.as_str().to_owned()),
        owner_profile_id: reference_set.owner_profile_id,
        active_revision_id: reference_set.active_revision_id,
        created_at: reference_set.created_at,
        updated_at: reference_set.updated_at,
    }
}

fn reference_set_detail_view(detail: ReferenceSetDetailView) -> ReferenceSetDetailViewDto {
    ReferenceSetDetailViewDto {
        reference_set: reference_set_view(detail.reference_set),
        items: detail
            .items
            .into_iter()
            .map(|item| ReferenceSetItemView {
                asset_id: item.asset_id,
                ordinal: item.ordinal,
                role: item.role,
                is_primary: item.is_primary,
                asset_name: item.asset_name,
                thumbnail_available: item.thumbnail_available,
                width: item.width,
                height: item.height,
            })
            .collect(),
    }
}

fn map_profile_error(error: ConsistencyProfileError) -> AppError {
    match error {
        ConsistencyProfileError::InvalidInput(message) => AppError::invalid_input(message),
        ConsistencyProfileError::NotFound(message) => map_consistency_not_found(message),
        ConsistencyProfileError::ProjectMismatch(message) => AppError::invalid_input(message),
        ConsistencyProfileError::Conflict(message) => AppError::invalid_input(message),
        ConsistencyProfileError::Repository(error) => super::map_repository_error(&error),
    }
}

fn map_reference_set_error(error: ReferenceSetError) -> AppError {
    match error {
        ReferenceSetError::InvalidInput(message) => AppError::invalid_input(message),
        ReferenceSetError::NotFound(message) => map_consistency_not_found(message),
        ReferenceSetError::ProjectMismatch(message) => AppError::invalid_input(message),
        ReferenceSetError::AssetNotFound(message) => AppError::asset_not_found(message),
        ReferenceSetError::ImageRequired(message) => AppError::invalid_input(message),
        ReferenceSetError::Conflict(message) => AppError::invalid_input(message),
        ReferenceSetError::Repository(error) => super::map_repository_error(&error),
    }
}

fn map_consistency_not_found(message: String) -> AppError {
    if message.contains("PROJECT_NOT_FOUND") {
        AppError::project_not_found(message)
    } else {
        AppError::database(message)
    }
}

// The three usage commands are kept at the bottom of this module so their
// public wire contract is local and stable while the application service owns
// the SQL/query implementation.  The service is read-only; no command creates
// a pool or reaches a production/generation boundary.

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetUsageItemView {
    pub entity_type: String,
    pub entity_id: String,
    pub display_name: String,
    pub relation_type: String,
    pub scope_type: Option<String>,
    pub scope_id: Option<String>,
    pub shot_id: Option<String>,
    pub profile_type: Option<String>,
    pub reference_set_id: Option<String>,
    pub blocking: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetUsageSummaryView {
    pub asset_id: String,
    pub total: usize,
    pub blocking_count: usize,
    pub reference_sets: Vec<AssetUsageItemView>,
    pub profiles: Vec<AssetUsageItemView>,
    pub shots: Vec<AssetUsageItemView>,
    pub legacy_references: Vec<AssetUsageItemView>,
    pub production_history: Vec<AssetUsageItemView>,
    pub items: Vec<AssetUsageItemView>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUsageSummaryView {
    pub profile_type: String,
    pub profile_id: String,
    pub total: usize,
    pub blocking_count: usize,
    pub shot_bindings: Vec<AssetUsageItemView>,
    pub scope_bindings: Vec<AssetUsageItemView>,
    pub reference_sets: Vec<AssetUsageItemView>,
    pub default_style_profiles: Vec<AssetUsageItemView>,
    pub costume_variants: Vec<AssetUsageItemView>,
    pub related_profiles: Vec<AssetUsageItemView>,
    pub items: Vec<AssetUsageItemView>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceSetUsageSummaryView {
    pub reference_set_id: String,
    pub total: usize,
    pub blocking_count: usize,
    pub profile_defaults: Vec<AssetUsageItemView>,
    pub costume_variants: Vec<AssetUsageItemView>,
    pub shot_bindings: Vec<AssetUsageItemView>,
    pub scope_bindings: Vec<AssetUsageItemView>,
    pub owner: Option<AssetUsageItemView>,
    pub item_count: usize,
    pub items: Vec<AssetUsageItemView>,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn asset_usage_get(
    state: State<'_, AppState>,
    project_id: String,
    asset_id: String,
) -> Result<AssetUsageSummaryView, AppError> {
    super::validate_project_id(&project_id)?;
    let usage = state
        .asset_usage_service
        .asset_usage(&project_id, &asset_id)
        .await
        .map_err(map_usage_error)?;
    into_wire_usage(usage)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn profile_usage_get(
    state: State<'_, AppState>,
    project_id: String,
    profile_type: String,
    profile_id: String,
) -> Result<ProfileUsageSummaryView, AppError> {
    super::validate_project_id(&project_id)?;
    let profile_type = parse_profile_type(&profile_type)?;
    let usage = state
        .asset_usage_service
        .profile_usage(&project_id, profile_type, &profile_id)
        .await
        .map_err(map_usage_error)?;
    into_wire_profile_usage(usage)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn reference_set_usage_get(
    state: State<'_, AppState>,
    project_id: String,
    reference_set_id: String,
) -> Result<ReferenceSetUsageSummaryView, AppError> {
    super::validate_project_id(&project_id)?;
    let usage = state
        .asset_usage_service
        .reference_set_usage(&project_id, &reference_set_id)
        .await
        .map_err(map_usage_error)?;
    into_wire_reference_set_usage(usage)
}

fn map_usage_error<E: std::fmt::Display>(error: E) -> AppError {
    let message = error.to_string();
    if message.contains("NOT_FOUND") {
        AppError::database(message)
    } else if message.contains("REPOSITORY") || message.contains("database") {
        AppError::database(message)
    } else {
        AppError::invalid_input(message)
    }
}

fn into_wire_usage<T: Serialize>(usage: T) -> Result<AssetUsageSummaryView, AppError> {
    serde_json::from_value(serde_json::to_value(usage).map_err(|error| {
        AppError::internal(format!(
            "asset usage response serialization failed: {error}"
        ))
    })?)
    .map_err(|error| AppError::internal(format!("asset usage response shape invalid: {error}")))
}

fn into_wire_profile_usage<T: Serialize>(usage: T) -> Result<ProfileUsageSummaryView, AppError> {
    serde_json::from_value(serde_json::to_value(usage).map_err(|error| {
        AppError::internal(format!(
            "profile usage response serialization failed: {error}"
        ))
    })?)
    .map_err(|error| AppError::internal(format!("profile usage response shape invalid: {error}")))
}

fn into_wire_reference_set_usage<T: Serialize>(
    usage: T,
) -> Result<ReferenceSetUsageSummaryView, AppError> {
    serde_json::from_value(serde_json::to_value(usage).map_err(|error| {
        AppError::internal(format!(
            "reference set usage response serialization failed: {error}"
        ))
    })?)
    .map_err(|error| {
        AppError::internal(format!(
            "reference set usage response shape invalid: {error}"
        ))
    })
}
