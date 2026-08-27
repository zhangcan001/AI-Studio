use crate::app_state::AppState;
use crate::application::consistency_scope_binding_service::{
    ConsistencyProfileBindingInput as ScopeProfileBindingInput,
    ConsistencyReferenceSetBindingInput as ScopeReferenceSetBindingInput, ConsistencyScopeAncestor,
    ConsistencyScopeBindingError, ConsistencyScopeBindingPack,
};
use crate::application::shot_consistency_binding_service::{
    ShotConsistencyBindingError, ShotConsistencyBindingPack,
};
use crate::domain::consistency::{
    BindingRole, ConsistencyScopeType, InheritanceMode, ProfileType, ScopedProfileBinding,
    ScopedReferenceSetBinding, ShotProfileBinding, ShotReferenceSetBinding,
};
use crate::domain::shot_context::{
    ContextDiagnostic, LegacyContext, PromptContext, ResolvedCharacter, ResolvedOutputSpec,
    ResolvedProp, ResolvedReferenceAsset, ResolvedReferenceSet, ResolvedScene, ResolvedShotContext,
    ResolvedStageInput, ResolvedStructure, ResolvedStyle, ResolvedWorkflowContext, SourceTrace,
};
use crate::domain::ShotStage;
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyProfileBindingInput {
    pub id: Option<String>,
    pub role: String,
    pub profile_type: String,
    pub profile_id: String,
    #[serde(default)]
    pub costume_variant_id: Option<String>,
    pub ordinal: i64,
    pub inheritance_mode: String,
}

impl ConsistencyProfileBindingInput {
    fn to_domain(&self) -> Result<ScopeProfileBindingInput, AppError> {
        Ok(ScopeProfileBindingInput {
            id: self.id.clone(),
            role: parse_role(&self.role)?,
            profile_type: parse_profile_type(&self.profile_type)?,
            profile_id: self.profile_id.clone(),
            costume_variant_id: self.costume_variant_id.clone(),
            ordinal: self.ordinal,
            inheritance_mode: parse_inheritance_mode(&self.inheritance_mode)?,
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyReferenceSetBindingInput {
    pub id: Option<String>,
    pub role: String,
    pub reference_set_id: String,
    pub ordinal: i64,
    pub required: bool,
    pub inheritance_mode: String,
}

impl ConsistencyReferenceSetBindingInput {
    fn to_domain(&self) -> Result<ScopeReferenceSetBindingInput, AppError> {
        Ok(ScopeReferenceSetBindingInput {
            id: self.id.clone(),
            role: parse_role(&self.role)?,
            reference_set_id: self.reference_set_id.clone(),
            ordinal: self.ordinal,
            required: self.required,
            inheritance_mode: parse_inheritance_mode(&self.inheritance_mode)?,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyScopeBindingReplaceRequest {
    pub project_id: String,
    pub scope_type: String,
    pub scope_id: String,
    #[serde(default)]
    pub profile_bindings: Vec<ConsistencyProfileBindingInput>,
    #[serde(default)]
    pub reference_set_bindings: Vec<ConsistencyReferenceSetBindingInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotConsistencyBindingReplaceRequest {
    pub project_id: String,
    pub shot_id: String,
    #[serde(default)]
    pub profile_bindings: Vec<ConsistencyProfileBindingInput>,
    #[serde(default)]
    pub reference_set_bindings: Vec<ConsistencyReferenceSetBindingInput>,
}

pub type ConsistencyBindingPackReplaceRequest = ConsistencyScopeBindingReplaceRequest;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyScopeRefView {
    pub scope_type: String,
    pub scope_id: String,
    pub scope_name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyScopeProfileBindingView {
    pub id: String,
    pub project_id: String,
    pub scope_type: String,
    pub scope_id: String,
    pub role: String,
    pub profile_type: String,
    pub profile_id: String,
    pub costume_variant_id: Option<String>,
    pub ordinal: i64,
    pub inheritance_mode: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyScopeReferenceSetBindingView {
    pub id: String,
    pub project_id: String,
    pub scope_type: String,
    pub scope_id: String,
    pub role: String,
    pub reference_set_id: String,
    pub ordinal: i64,
    pub required: bool,
    pub inheritance_mode: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyScopeAncestorView {
    pub scope_type: String,
    pub scope_id: String,
    pub scope_name: String,
    pub profile_bindings: Vec<ConsistencyScopeProfileBindingView>,
    pub reference_set_bindings: Vec<ConsistencyScopeReferenceSetBindingView>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyScopeBindingPackView {
    pub project_id: String,
    pub scope: ConsistencyScopeRefView,
    pub ancestors: Vec<ConsistencyScopeAncestorView>,
    pub direct_profile_bindings: Vec<ConsistencyScopeProfileBindingView>,
    pub direct_reference_set_bindings: Vec<ConsistencyScopeReferenceSetBindingView>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotProfileBindingView {
    pub id: String,
    pub shot_id: String,
    pub role: String,
    pub profile_type: String,
    pub profile_id: String,
    pub costume_variant_id: Option<String>,
    pub ordinal: i64,
    pub inheritance_mode: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotReferenceSetBindingView {
    pub id: String,
    pub shot_id: String,
    pub role: String,
    pub reference_set_id: String,
    pub ordinal: i64,
    pub required: bool,
    pub inheritance_mode: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotStructurePathItemView {
    pub scope_type: String,
    pub scope_id: String,
    pub scope_name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotConsistencyBindingPackView {
    pub project_id: String,
    pub shot_id: String,
    pub profile_bindings: Vec<ShotProfileBindingView>,
    pub reference_set_bindings: Vec<ShotReferenceSetBindingView>,
    pub structure_path: Vec<ShotStructurePathItemView>,
    pub resolved_context_summary: Option<ShotContextDraftView>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotContextDraftView {
    pub project_id: String,
    pub structure: ResolvedStructure,
    pub stage: String,
    pub stage_input: ResolvedStageInput,
    pub characters: Vec<ResolvedCharacter>,
    pub scene: Option<ResolvedScene>,
    pub props: Vec<ResolvedProp>,
    pub style: Option<ResolvedStyle>,
    pub reference_sets: Vec<ResolvedReferenceSet>,
    pub reference_assets: Vec<ResolvedReferenceAsset>,
    pub prompt_context: PromptContext,
    pub source_trace: Vec<SourceTrace>,
    pub context_hash: String,
    pub legacy: LegacyContext,
    pub partial: bool,
    pub diagnostics: Vec<ContextDiagnostic>,
    pub workflow: ResolvedWorkflowContext,
    pub output: ResolvedOutputSpec,
    pub profiles: crate::domain::shot_context::ResolvedProfiles,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn consistency_scope_binding_get(
    state: State<'_, AppState>,
    project_id: String,
    scope_type: String,
    scope_id: String,
) -> Result<ConsistencyScopeBindingPackView, AppError> {
    super::validate_project_id(&project_id)?;
    let scope_type = parse_scope_type(&scope_type)?;
    state
        .consistency_scope_binding_service
        .get_binding_pack(&project_id, scope_type, &scope_id)
        .await
        .map(map_scope_pack)
        .map_err(map_scope_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn consistency_scope_binding_replace(
    state: State<'_, AppState>,
    request: ConsistencyScopeBindingReplaceRequest,
) -> Result<(), AppError> {
    super::validate_project_id(&request.project_id)?;
    let scope_type = parse_scope_type(&request.scope_type)?;
    let profiles = request
        .profile_bindings
        .iter()
        .map(ConsistencyProfileBindingInput::to_domain)
        .collect::<Result<Vec<_>, _>>()?;
    let reference_sets = request
        .reference_set_bindings
        .iter()
        .map(ConsistencyReferenceSetBindingInput::to_domain)
        .collect::<Result<Vec<_>, _>>()?;
    state
        .consistency_scope_binding_service
        .replace_binding_pack(
            &request.project_id,
            scope_type,
            &request.scope_id,
            &profiles,
            &reference_sets,
        )
        .await
        .map_err(map_scope_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn shot_consistency_binding_get(
    state: State<'_, AppState>,
    project_id: String,
    shot_id: String,
) -> Result<ShotConsistencyBindingPackView, AppError> {
    super::validate_project_id(&project_id)?;
    let pack = state
        .shot_consistency_binding_service
        .get_binding_pack(&project_id, &shot_id)
        .await
        .map_err(map_shot_error)?;
    let resolved = state
        .shot_context_resolver
        .resolve_draft(&project_id, &shot_id, ShotStage::Image)
        .await
        .ok();
    Ok(map_shot_pack(pack, resolved))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn shot_consistency_binding_replace(
    state: State<'_, AppState>,
    request: ShotConsistencyBindingReplaceRequest,
) -> Result<(), AppError> {
    super::validate_project_id(&request.project_id)?;
    let profiles = request
        .profile_bindings
        .iter()
        .map(ConsistencyProfileBindingInput::to_domain)
        .collect::<Result<Vec<_>, _>>()?;
    let reference_sets = request
        .reference_set_bindings
        .iter()
        .map(ConsistencyReferenceSetBindingInput::to_domain)
        .collect::<Result<Vec<_>, _>>()?;
    state
        .shot_consistency_binding_service
        .replace_binding_pack(
            &request.project_id,
            &request.shot_id,
            &profiles,
            &reference_sets,
        )
        .await
        .map_err(map_shot_error)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn shot_context_draft_get(
    state: State<'_, AppState>,
    project_id: String,
    shot_id: String,
    stage: String,
) -> Result<ShotContextDraftView, AppError> {
    super::validate_project_id(&project_id)?;
    let stage = ShotStage::try_from_str(&stage)
        .map_err(|error| AppError::invalid_input(format!("CONTEXT_STAGE_INVALID: {error}")))?;
    state
        .shot_context_resolver
        .resolve_draft(&project_id, &shot_id, stage)
        .await
        .map(map_context)
        .map_err(|error| AppError::invalid_input(error.to_string()))
}

fn map_scope_pack(pack: ConsistencyScopeBindingPack) -> ConsistencyScopeBindingPackView {
    ConsistencyScopeBindingPackView {
        project_id: pack.project_id,
        scope: ConsistencyScopeRefView {
            scope_type: pack.scope_type.as_str().to_owned(),
            scope_id: pack.scope_id,
            scope_name: pack.scope_name,
        },
        ancestors: pack.ancestors.into_iter().map(map_ancestor).collect(),
        direct_profile_bindings: pack
            .direct_profile_bindings
            .into_iter()
            .map(map_scope_profile)
            .collect(),
        direct_reference_set_bindings: pack
            .direct_reference_set_bindings
            .into_iter()
            .map(map_scope_reference_set)
            .collect(),
    }
}

fn map_ancestor(ancestor: ConsistencyScopeAncestor) -> ConsistencyScopeAncestorView {
    ConsistencyScopeAncestorView {
        scope_type: ancestor.scope_type.as_str().to_owned(),
        scope_id: ancestor.scope_id,
        scope_name: ancestor.scope_name,
        profile_bindings: ancestor
            .profile_bindings
            .into_iter()
            .map(map_scope_profile)
            .collect(),
        reference_set_bindings: ancestor
            .reference_set_bindings
            .into_iter()
            .map(map_scope_reference_set)
            .collect(),
    }
}

fn map_scope_profile(binding: ScopedProfileBinding) -> ConsistencyScopeProfileBindingView {
    ConsistencyScopeProfileBindingView {
        id: binding.id,
        project_id: binding.project_id,
        scope_type: binding.scope_type.as_str().to_owned(),
        scope_id: binding.scope_id,
        role: binding.role.as_str().to_owned(),
        profile_type: binding.profile_type.as_str().to_owned(),
        profile_id: binding.profile_id,
        costume_variant_id: binding.costume_variant_id,
        ordinal: binding.ordinal,
        inheritance_mode: binding.inheritance_mode.as_str().to_owned(),
        created_at: binding.created_at,
        updated_at: binding.updated_at,
    }
}

fn map_scope_reference_set(
    binding: ScopedReferenceSetBinding,
) -> ConsistencyScopeReferenceSetBindingView {
    ConsistencyScopeReferenceSetBindingView {
        id: binding.id,
        project_id: binding.project_id,
        scope_type: binding.scope_type.as_str().to_owned(),
        scope_id: binding.scope_id,
        role: binding.role.as_str().to_owned(),
        reference_set_id: binding.reference_set_id,
        ordinal: binding.ordinal,
        required: binding.required,
        inheritance_mode: binding.inheritance_mode.as_str().to_owned(),
        created_at: binding.created_at,
        updated_at: binding.updated_at,
    }
}

fn map_shot_pack(
    pack: ShotConsistencyBindingPack,
    context: Option<ResolvedShotContext>,
) -> ShotConsistencyBindingPackView {
    let structure_path = context.as_ref().map(structure_path).unwrap_or_default();
    let resolved_context_summary = context.map(map_context);
    ShotConsistencyBindingPackView {
        project_id: pack.project_id,
        shot_id: pack.shot_id,
        profile_bindings: pack
            .profile_bindings
            .into_iter()
            .map(map_shot_profile)
            .collect(),
        reference_set_bindings: pack
            .reference_set_bindings
            .into_iter()
            .map(map_shot_reference_set)
            .collect(),
        structure_path,
        resolved_context_summary,
    }
}

fn map_shot_profile(binding: ShotProfileBinding) -> ShotProfileBindingView {
    ShotProfileBindingView {
        id: binding.id,
        shot_id: binding.shot_id,
        role: binding.role.as_str().to_owned(),
        profile_type: binding.profile_type.as_str().to_owned(),
        profile_id: binding.profile_id,
        costume_variant_id: binding.costume_variant_id,
        ordinal: binding.ordinal,
        inheritance_mode: binding.inheritance_mode.as_str().to_owned(),
        created_at: binding.created_at,
        updated_at: binding.updated_at,
    }
}

fn map_shot_reference_set(binding: ShotReferenceSetBinding) -> ShotReferenceSetBindingView {
    ShotReferenceSetBindingView {
        id: binding.id,
        shot_id: binding.shot_id,
        role: binding.role.as_str().to_owned(),
        reference_set_id: binding.reference_set_id,
        ordinal: binding.ordinal,
        required: binding.required,
        inheritance_mode: binding.inheritance_mode.as_str().to_owned(),
        created_at: binding.created_at,
        updated_at: binding.updated_at,
    }
}

fn structure_path(context: &ResolvedShotContext) -> Vec<ShotStructurePathItemView> {
    let mut result = Vec::with_capacity(4);
    if let Some(series) = &context.structure.series {
        result.push(ShotStructurePathItemView {
            scope_type: "SERIES".to_owned(),
            scope_id: series.id.clone(),
            scope_name: series.name.clone(),
        });
    }
    if let Some(episode) = &context.structure.episode {
        result.push(ShotStructurePathItemView {
            scope_type: "EPISODE".to_owned(),
            scope_id: episode.id.clone(),
            scope_name: episode.name.clone(),
        });
    }
    if let Some(scene) = &context.structure.scene {
        result.push(ShotStructurePathItemView {
            scope_type: "SCENE".to_owned(),
            scope_id: scene.id.clone(),
            scope_name: scene.name.clone(),
        });
    }
    result.push(ShotStructurePathItemView {
        scope_type: "SHOT".to_owned(),
        scope_id: context.structure.shot.id.clone(),
        scope_name: context.structure.shot.name.clone(),
    });
    result
}

fn map_context(context: ResolvedShotContext) -> ShotContextDraftView {
    ShotContextDraftView {
        project_id: context.project_id,
        structure: context.structure,
        stage: context.stage.as_str().to_owned(),
        stage_input: context.stage_input,
        characters: context.reference_pack.characters,
        scene: context.reference_pack.scene,
        props: context.reference_pack.props,
        style: context.reference_pack.style,
        reference_sets: context.reference_pack.reference_sets,
        reference_assets: context.reference_assets,
        prompt_context: context.prompt_context,
        source_trace: context.reference_pack.source_trace,
        context_hash: context.resolver_identity.context_hash,
        legacy: context.legacy,
        partial: context.partial,
        diagnostics: context.diagnostics,
        workflow: context.workflow,
        output: context.output,
        profiles: context.profiles,
    }
}

fn parse_scope_type(value: &str) -> Result<ConsistencyScopeType, AppError> {
    ConsistencyScopeType::try_from_str(value).map_err(|error| {
        AppError::invalid_input(format!("CONSISTENCY_SCOPE_TYPE_INVALID: {error}"))
    })
}

fn parse_role(value: &str) -> Result<BindingRole, AppError> {
    BindingRole::try_from_db(value).map_err(|error| {
        AppError::invalid_input(format!("CONSISTENCY_BINDING_ROLE_INVALID: {error}"))
    })
}

fn parse_profile_type(value: &str) -> Result<ProfileType, AppError> {
    ProfileType::try_from_str(value).map_err(|error| {
        AppError::invalid_input(format!("CONSISTENCY_PROFILE_TYPE_INVALID: {error}"))
    })
}

fn parse_inheritance_mode(value: &str) -> Result<InheritanceMode, AppError> {
    InheritanceMode::try_from_db(value).map_err(|error| {
        AppError::invalid_input(format!("CONSISTENCY_INHERITANCE_MODE_INVALID: {error}"))
    })
}

fn map_scope_error(error: ConsistencyScopeBindingError) -> AppError {
    match error {
        ConsistencyScopeBindingError::Repository(repository) => {
            super::map_repository_error(&repository)
        }
        ConsistencyScopeBindingError::NotFound(message)
        | ConsistencyScopeBindingError::ProjectMismatch(message)
        | ConsistencyScopeBindingError::InvalidInput(message)
        | ConsistencyScopeBindingError::Conflict(message) => AppError::invalid_input(message),
    }
}

fn map_shot_error(error: ShotConsistencyBindingError) -> AppError {
    match error {
        ShotConsistencyBindingError::Repository(repository) => {
            super::map_repository_error(&repository)
        }
        ShotConsistencyBindingError::NotFound(message)
        | ShotConsistencyBindingError::ProjectMismatch(message)
        | ShotConsistencyBindingError::InvalidInput(message)
        | ShotConsistencyBindingError::Conflict(message) => AppError::invalid_input(message),
    }
}
