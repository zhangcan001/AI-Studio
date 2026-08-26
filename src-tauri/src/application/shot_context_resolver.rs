//! Batch resolver for the draft shot context.
//!
//! The resolver deliberately performs all persistence reads up front.  The
//! per-shot phase is pure map traversal, which keeps a 500-shot resolve from
//! turning into an N+1 read path.

use crate::application::ports::{
    AssetRepository, Clock, ConsistencyProfileRepository, ConsistencyScopeRepository,
    ProductionStructureRepository, ProjectRepository, ReferenceSetRepository, RepositoryError,
    ShotConsistencyRepository, ShotData, ShotRepository,
};
use crate::application::prompt_context_builder::{
    build_prompt_context, select_stage_prompt, PromptContextInput, PromptFragmentInput,
};
use crate::application::shot_reference_pack_builder::{
    build_shot_reference_pack, compute_context_hash, merge_profile_bindings,
    merge_reference_set_bindings, order_reference_assets, reference_set_content_hash,
    ProfileBindingCandidate, ReferenceAssetCandidate, ReferenceSetBindingCandidate,
    ReferenceSetContentHashItem, ShotReferencePackInput,
};
use crate::domain::consistency::{
    BindingRole, ConsistencyProfileRecord, ConsistencyScopeType, CostumeVariant, ProfileRevision,
    ProfileType, ReferenceSet, ReferenceSetItem, ScopedProfileBinding, ScopedReferenceSetBinding,
};
use crate::domain::shot::ShotStage;
use crate::domain::shot_context::{
    ContextDiagnostic, ContextHashInput, ContextSourceScope, LegacyContext, ResolvedCharacter,
    ResolvedOutputSpec, ResolvedProfile, ResolvedProfiles, ResolvedProp, ResolvedReferenceSet,
    ResolvedScene, ResolvedShotContext, ResolvedStructure, ResolvedStructureNode, ResolvedStyle,
    ResolvedWorkflowContext, ResolverIdentity, SourceTrace,
};
use crate::domain::{Asset, AssetId};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

pub const CONTEXT_BATCH_LIMIT: usize = 500;

#[derive(Debug)]
pub enum ShotContextResolverError {
    ContextBatchLimit { limit: usize },
    ProjectNotFound(String),
    ShotNotFound(String),
    Repository(RepositoryError),
}

impl fmt::Display for ShotContextResolverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContextBatchLimit { limit } => {
                write!(
                    formatter,
                    "CONTEXT_BATCH_LIMIT: at most {limit} shots may be resolved"
                )
            }
            Self::ProjectNotFound(id) => write!(formatter, "CONTEXT_PROJECT_NOT_FOUND: {id}"),
            Self::ShotNotFound(id) => write!(formatter, "CONTEXT_SHOT_NOT_FOUND: {id}"),
            Self::Repository(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for ShotContextResolverError {}

impl From<RepositoryError> for ShotContextResolverError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

pub struct ShotContextResolver {
    project_repository: Arc<dyn ProjectRepository>,
    structure_repository: Arc<dyn ProductionStructureRepository>,
    shot_repository: Arc<dyn ShotRepository>,
    scope_repository: Arc<dyn ConsistencyScopeRepository>,
    profile_repository: Arc<dyn ConsistencyProfileRepository>,
    reference_set_repository: Arc<dyn ReferenceSetRepository>,
    shot_consistency_repository: Arc<dyn ShotConsistencyRepository>,
    asset_repository: Arc<dyn AssetRepository>,
    clock: Arc<dyn Clock>,
}

impl ShotContextResolver {
    pub fn new(
        project_repository: Arc<dyn ProjectRepository>,
        structure_repository: Arc<dyn ProductionStructureRepository>,
        shot_repository: Arc<dyn ShotRepository>,
        scope_repository: Arc<dyn ConsistencyScopeRepository>,
        profile_repository: Arc<dyn ConsistencyProfileRepository>,
        reference_set_repository: Arc<dyn ReferenceSetRepository>,
        shot_consistency_repository: Arc<dyn ShotConsistencyRepository>,
        asset_repository: Arc<dyn AssetRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            project_repository,
            structure_repository,
            shot_repository,
            scope_repository,
            profile_repository,
            reference_set_repository,
            shot_consistency_repository,
            asset_repository,
            clock,
        }
    }

    pub async fn resolve_draft(
        &self,
        project_id: &str,
        shot_id: &str,
        stage: ShotStage,
    ) -> Result<ResolvedShotContext, ShotContextResolverError> {
        self.resolve_many_draft(project_id, &[shot_id.to_owned()], stage)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| ShotContextResolverError::ShotNotFound(shot_id.to_owned()))
    }

    pub async fn resolve_many_draft(
        &self,
        project_id: &str,
        shot_ids: &[String],
        stage: ShotStage,
    ) -> Result<Vec<ResolvedShotContext>, ShotContextResolverError> {
        if shot_ids.len() > CONTEXT_BATCH_LIMIT {
            return Err(ShotContextResolverError::ContextBatchLimit {
                limit: CONTEXT_BATCH_LIMIT,
            });
        }
        if shot_ids.is_empty() {
            return Ok(Vec::new());
        }
        if self
            .project_repository
            .find_by_id(project_id)
            .await?
            .is_none()
        {
            return Err(ShotContextResolverError::ProjectNotFound(
                project_id.to_owned(),
            ));
        }

        let tree = self.structure_repository.load_tree_data(project_id).await?;
        let shots = self.shot_repository.list(project_id).await?;
        let requested = shot_ids.iter().collect::<HashSet<_>>();
        let selected = shots
            .iter()
            .filter(|data| requested.contains(&data.shot.id))
            .collect::<Vec<_>>();
        if let Some(missing) = shot_ids
            .iter()
            .find(|id| !selected.iter().any(|data| data.shot.id == **id))
        {
            return Err(ShotContextResolverError::ShotNotFound(missing.clone()));
        }

        let scope_profiles = self
            .scope_repository
            .list_profile_bindings_for_project(project_id)
            .await?;
        let scope_reference_sets = self
            .scope_repository
            .list_reference_set_bindings_for_project(project_id)
            .await?;
        let requested_ids = shot_ids.to_vec();
        let shot_profiles = self
            .shot_consistency_repository
            .list_profile_bindings_many(&requested_ids)
            .await?;
        let shot_reference_sets = self
            .shot_consistency_repository
            .list_reference_set_bindings_many(&requested_ids)
            .await?;

        let mut profiles = HashMap::<(ProfileType, String), ConsistencyProfileRecord>::new();
        for profile_type in [
            ProfileType::Character,
            ProfileType::Scene,
            ProfileType::Prop,
            ProfileType::Style,
        ] {
            for profile in self
                .profile_repository
                .list_profiles(project_id, profile_type)
                .await?
            {
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
            .map(|value| (value.id.clone(), value))
            .collect::<HashMap<_, _>>();

        let reference_sets = self
            .reference_set_repository
            .list_reference_sets(project_id, None)
            .await?;
        let reference_set_ids = reference_sets
            .iter()
            .map(|value| value.id.clone())
            .collect::<Vec<_>>();
        let items = self
            .reference_set_repository
            .list_items_many(&reference_set_ids)
            .await?
            .into_iter()
            .fold(
                HashMap::<String, Vec<ReferenceSetItem>>::new(),
                |mut map, item| {
                    map.entry(item.reference_set_id.clone())
                        .or_default()
                        .push(item);
                    map
                },
            );

        let revision_ids = profiles
            .values()
            .filter_map(active_revision_id)
            .chain(
                costumes
                    .values()
                    .filter_map(|value| value.active_revision_id.clone()),
            )
            .chain(
                reference_sets
                    .iter()
                    .filter_map(|value| value.active_revision_id.clone()),
            )
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let revisions = self
            .profile_repository
            .find_profile_revisions_many(&revision_ids)
            .await?
            .into_iter()
            .map(|value| (value.id.clone(), value))
            .collect::<HashMap<_, _>>();

        let mut asset_ids = HashSet::<String>::new();
        for values in items.values() {
            asset_ids.extend(values.iter().map(|item| item.asset_id.clone()));
        }
        for data in &selected {
            asset_ids.extend(
                data.reference_assets
                    .iter()
                    .filter(|value| value.stage == stage)
                    .map(|value| value.asset_id.clone()),
            );
        }
        let parsed_asset_ids = asset_ids
            .iter()
            .filter_map(|value| AssetId::parse(value.clone()).ok())
            .collect::<Vec<_>>();
        let assets = self
            .asset_repository
            .find_many_by_ids(&parsed_asset_ids)
            .await?
            .into_iter()
            .map(|value| (value.id.as_str().to_owned(), value))
            .collect::<HashMap<_, _>>();

        let snapshots = LoadedContext {
            project_id,
            stage,
            tree: &tree,
            scope_profiles: &scope_profiles,
            scope_reference_sets: &scope_reference_sets,
            shot_profiles: &shot_profiles,
            shot_reference_sets: &shot_reference_sets,
            profiles: &profiles,
            costumes: &costumes,
            revisions: &revisions,
            reference_sets: &reference_sets,
            items: &items,
            assets: &assets,
        };
        let resolved_at = self.clock.now();
        shot_ids
            .iter()
            .map(|shot_id| {
                let data = selected
                    .iter()
                    .find(|value| value.shot.id == *shot_id)
                    .expect("selected shots were checked above");
                build_context(data, &snapshots, resolved_at)
            })
            .collect()
    }
}

struct LoadedContext<'a> {
    project_id: &'a str,
    stage: ShotStage,
    tree: &'a crate::application::ports::ProductionStructureTreeData,
    scope_profiles: &'a [ScopedProfileBinding],
    scope_reference_sets: &'a [ScopedReferenceSetBinding],
    shot_profiles: &'a [crate::domain::consistency::ShotProfileBinding],
    shot_reference_sets: &'a [crate::domain::consistency::ShotReferenceSetBinding],
    profiles: &'a HashMap<(ProfileType, String), ConsistencyProfileRecord>,
    costumes: &'a HashMap<String, CostumeVariant>,
    revisions: &'a HashMap<String, ProfileRevision>,
    reference_sets: &'a [ReferenceSet],
    items: &'a HashMap<String, Vec<ReferenceSetItem>>,
    assets: &'a HashMap<String, Asset>,
}

fn build_context(
    data: &ShotData,
    loaded: &LoadedContext<'_>,
    resolved_at: DateTime<Utc>,
) -> Result<ResolvedShotContext, ShotContextResolverError> {
    let shot_id = data.shot.id.as_str();
    let (series_id, episode_id, scene_id) = hierarchy_ids(loaded.tree, shot_id);
    let mut profile_candidates = Vec::new();
    let mut reference_candidates = Vec::new();
    for binding in loaded.scope_profiles {
        let scope = scope_for(binding.scope_type);
        if scope_id(
            scope,
            loaded.project_id,
            series_id.as_deref(),
            episode_id.as_deref(),
            scene_id.as_deref(),
        )
        .as_deref()
            != Some(binding.scope_id.as_str())
        {
            continue;
        }
        profile_candidates.push(ProfileBindingCandidate {
            binding_id: binding.id.clone(),
            scope,
            scope_id: binding.scope_id.clone(),
            role: binding.role,
            profile_type: binding.profile_type,
            profile_id: binding.profile_id.clone(),
            costume_variant_id: binding.costume_variant_id.clone(),
            ordinal: binding.ordinal,
            inheritance_mode: binding.inheritance_mode,
        });
    }
    for binding in loaded.scope_reference_sets {
        let scope = scope_for(binding.scope_type);
        if scope_id(
            scope,
            loaded.project_id,
            series_id.as_deref(),
            episode_id.as_deref(),
            scene_id.as_deref(),
        )
        .as_deref()
            != Some(binding.scope_id.as_str())
        {
            continue;
        }
        reference_candidates.push(ReferenceSetBindingCandidate {
            binding_id: binding.id.clone(),
            scope,
            scope_id: binding.scope_id.clone(),
            role: binding.role,
            reference_set_id: binding.reference_set_id.clone(),
            ordinal: binding.ordinal,
            required: binding.required,
            inheritance_mode: binding.inheritance_mode,
        });
    }
    profile_candidates.extend(
        loaded
            .shot_profiles
            .iter()
            .filter(|value| value.shot_id == shot_id)
            .map(|value| ProfileBindingCandidate {
                binding_id: value.id.clone(),
                scope: ContextSourceScope::Shot,
                scope_id: shot_id.to_owned(),
                role: value.role,
                profile_type: value.profile_type,
                profile_id: value.profile_id.clone(),
                costume_variant_id: value.costume_variant_id.clone(),
                ordinal: value.ordinal,
                inheritance_mode: value.inheritance_mode,
            }),
    );
    reference_candidates.extend(
        loaded
            .shot_reference_sets
            .iter()
            .filter(|value| value.shot_id == shot_id)
            .map(|value| ReferenceSetBindingCandidate {
                binding_id: value.id.clone(),
                scope: ContextSourceScope::Shot,
                scope_id: shot_id.to_owned(),
                role: value.role,
                reference_set_id: value.reference_set_id.clone(),
                ordinal: value.ordinal,
                required: value.required,
                inheritance_mode: value.inheritance_mode,
            }),
    );

    let mut diagnostics = Vec::new();
    let merged_profiles = merge_profile_bindings(&profile_candidates);
    let merged_reference_sets = merge_reference_set_bindings(&reference_candidates);
    diagnostics.extend(merged_profiles.diagnostics.clone());
    diagnostics.extend(merged_reference_sets.diagnostics.clone());
    single_role_diagnostics(&merged_profiles.bindings, &mut diagnostics);
    single_role_reference_diagnostics(&merged_reference_sets.bindings, &mut diagnostics);

    let mut resolved_profiles = ResolvedProfiles::default();
    let mut profile_reference_ids =
        Vec::<(BindingRole, String, Option<String>, SourceTrace, i64, bool)>::new();
    let mut prompt = PromptContextInput::default();
    for binding in &merged_profiles.bindings {
        let Some(profile) = loaded
            .profiles
            .get(&(binding.profile_type, binding.profile_id.clone()))
        else {
            diagnostics.push(ContextDiagnostic::error(
                "CONTEXT_PROFILE_NOT_FOUND",
                format!("profile {}", binding.profile_id),
            ));
            continue;
        };
        if profile.project_id() != loaded.project_id {
            diagnostics.push(ContextDiagnostic::error(
                "CONTEXT_PROFILE_PROJECT_MISMATCH",
                profile.id().to_owned(),
            ));
            continue;
        }
        if profile.profile_type() != binding.profile_type {
            diagnostics.push(ContextDiagnostic::error(
                "CONTEXT_PROFILE_NOT_FOUND",
                format!("profile type mismatch {}", binding.profile_id),
            ));
            continue;
        }
        let (revision_id, content_hash) = profile_hash(profile, loaded.revisions, &mut diagnostics);
        let (text, negative, defaults) = profile_text(profile);
        let mut costume = None;
        if let Some(costume_id) = binding.costume_variant_id.as_deref() {
            match loaded.costumes.get(costume_id) {
                Some(value)
                    if value.character_profile_id == binding.profile_id
                        && binding.role == BindingRole::Character =>
                {
                    costume = Some(value.id.clone());
                    if !value.prompt_fragment.trim().is_empty() {
                        prompt.costumes.push(PromptFragmentInput {
                            text: value.prompt_fragment.clone(),
                            negative_prompt: None,
                            source_scope: binding.source.scope,
                            source_entity_id: value.id.clone(),
                            revision_id: value.active_revision_id.clone(),
                            ordinal: value.ordinal,
                        });
                    }
                    if let Some(reference_set_id) = &value.reference_set_id {
                        profile_reference_ids.push((
                            BindingRole::Character,
                            reference_set_id.clone(),
                            Some(binding.profile_id.clone()),
                            binding.source.clone(),
                            binding.ordinal,
                            true,
                        ));
                    }
                }
                _ => diagnostics.push(ContextDiagnostic::error(
                    "CONTEXT_COSTUME_MISMATCH",
                    costume_id.to_owned(),
                )),
            }
        }
        profile_reference_ids.extend(defaults.into_iter().map(|id| {
            (
                binding.role,
                id,
                Some(binding.profile_id.clone()),
                binding.source.clone(),
                binding.ordinal,
                true,
            )
        }));
        let resolved = ResolvedProfile {
            profile_id: binding.profile_id.clone(),
            profile_type: binding.profile_type,
            ordinal: binding.ordinal,
            revision_id: revision_id.clone(),
            content_hash: content_hash.clone(),
            prompt: text.clone(),
            negative_prompt: negative.clone(),
            costume_variant_id: costume.clone(),
            source: binding.source.clone(),
        };
        match binding.role {
            BindingRole::Character => {
                resolved_profiles.characters.push(resolved.clone());
                prompt.characters.push(fragment(&resolved, true));
            }
            BindingRole::Scene => {
                resolved_profiles.scene = Some(resolved.clone());
                prompt.scene.push(fragment(&resolved, true));
            }
            BindingRole::Prop => {
                resolved_profiles.props.push(resolved.clone());
                prompt.props.push(fragment(&resolved, true));
            }
            BindingRole::Style => {
                resolved_profiles.style = Some(resolved.clone());
                prompt.global_style.push(fragment(&resolved, true));
            }
            BindingRole::ShotReference => diagnostics.push(ContextDiagnostic::error(
                "CONTEXT_PROFILE_NOT_FOUND",
                "SHOT_REFERENCE cannot bind a profile",
            )),
        }
    }
    if let Some(scene) = resolved_profiles.scene.as_ref() {
        if let Some(record) = loaded
            .profiles
            .get(&(ProfileType::Scene, scene.profile_id.clone()))
        {
            if let ConsistencyProfileRecord::Scene(value) = record {
                if let Some(text) = value
                    .lighting_prompt
                    .as_deref()
                    .filter(|text| !text.trim().is_empty())
                {
                    prompt.lighting.push(PromptFragmentInput::new(
                        text,
                        scene.source.scope,
                        scene.profile_id.clone(),
                    ));
                }
            }
        }
    } else if let Some(scene_id) = scene_id.as_deref() {
        if let Some(scene) = loaded
            .tree
            .scenes
            .iter()
            .find(|value| value.id.as_str() == scene_id)
        {
            if !scene.description.trim().is_empty() {
                prompt.scene.push(PromptFragmentInput {
                    text: scene.description.clone(),
                    negative_prompt: None,
                    source_scope: ContextSourceScope::Legacy,
                    source_entity_id: scene_id.to_owned(),
                    revision_id: None,
                    ordinal: scene.ordinal as i64,
                });
            }
        }
    }
    let mut resolved_reference_sets = Vec::new();
    let mut reference_hashes = BTreeMap::new();
    for (role, id, _profile_id, source, ordinal, required) in profile_reference_ids
        .into_iter()
        .chain(merged_reference_sets.bindings.iter().map(|value| {
            (
                value.role,
                value.reference_set_id.clone(),
                None,
                value.source.clone(),
                value.ordinal,
                value.required,
            )
        }))
    {
        if resolved_reference_sets
            .iter()
            .any(|value: &ResolvedReferenceSet| value.reference_set_id == id && value.role == role)
        {
            continue;
        }
        let Some(reference_set) = loaded.reference_sets.iter().find(|value| value.id == id) else {
            diagnostics.push(ContextDiagnostic::error(
                "CONTEXT_REFERENCE_SET_NOT_FOUND",
                id.clone(),
            ));
            continue;
        };
        if reference_set.project_id != loaded.project_id {
            diagnostics.push(ContextDiagnostic::error(
                "CONTEXT_REFERENCE_PROJECT_MISMATCH",
                id.clone(),
            ));
            continue;
        }
        let set_items = loaded.items.get(&id).cloned().unwrap_or_default();
        let hash_items = set_items
            .iter()
            .filter_map(|item| {
                loaded
                    .assets
                    .get(&item.asset_id)
                    .map(|asset| ReferenceSetContentHashItem {
                        asset_id: item.asset_id.clone(),
                        asset_sha256: asset.sha256.clone(),
                        ordinal: item.ordinal,
                        role: item.role.clone(),
                        is_primary: item.is_primary,
                    })
            })
            .collect::<Vec<_>>();
        let content_hash =
            reference_set_content_hash(&reference_set.id, reference_set.purpose, hash_items);
        reference_hashes.insert(reference_set.id.clone(), content_hash.clone());
        resolved_reference_sets.push(ResolvedReferenceSet {
            reference_set_id: id.clone(),
            role,
            ordinal,
            required,
            source: source.clone(),
            content_hash,
        });
    }
    // DEV-049 §49: only a valid, project-scoped ReferenceSet binding makes
    // the new pack authoritative over legacy shot references. Profile-only
    // bindings still enrich prompts, but do not suppress legacy assets.
    let new_pack = !resolved_reference_sets.is_empty();
    resolved_reference_sets.sort_by_key(|value| {
        (
            role_rank(value.role),
            value.ordinal,
            value.reference_set_id.clone(),
        )
    });

    let mut asset_candidates = Vec::new();
    if new_pack {
        for reference_set in &resolved_reference_sets {
            for item in loaded
                .items
                .get(&reference_set.reference_set_id)
                .into_iter()
                .flatten()
            {
                match loaded.assets.get(&item.asset_id) {
                    Some(asset)
                        if asset.project_id == loaded.project_id
                            && asset.asset_type == crate::domain::AssetType::Image =>
                    {
                        asset_candidates.push(ReferenceAssetCandidate {
                            asset_id: item.asset_id.clone(),
                            sha256: asset.sha256.clone(),
                            role: reference_set.role,
                            binding_ordinal: reference_set.ordinal,
                            set_ordinal: item.ordinal,
                            source_reference_set_id: reference_set.reference_set_id.clone(),
                            source_profile_id: None,
                            source_scope: reference_set.source.scope,
                        })
                    }
                    Some(asset) if asset.project_id != loaded.project_id => {
                        diagnostics.push(ContextDiagnostic::error(
                            "CONTEXT_ASSET_PROJECT_MISMATCH",
                            item.asset_id.clone(),
                        ))
                    }
                    Some(_) => diagnostics.push(ContextDiagnostic::error(
                        "CONTEXT_IMAGE_REQUIRED",
                        item.asset_id.clone(),
                    )),
                    None => diagnostics.push(ContextDiagnostic::error(
                        "CONTEXT_ASSET_NOT_FOUND",
                        item.asset_id.clone(),
                    )),
                }
            }
        }
    } else {
        for item in data
            .reference_assets
            .iter()
            .filter(|value| value.stage == loaded.stage)
        {
            match loaded.assets.get(&item.asset_id) {
                Some(asset)
                    if asset.project_id == loaded.project_id
                        && asset.asset_type == crate::domain::AssetType::Image =>
                {
                    asset_candidates.push(ReferenceAssetCandidate {
                        asset_id: item.asset_id.clone(),
                        sha256: asset.sha256.clone(),
                        role: BindingRole::ShotReference,
                        binding_ordinal: item.ordinal,
                        set_ordinal: item.ordinal,
                        source_reference_set_id: format!("legacy:{shot_id}"),
                        source_profile_id: None,
                        source_scope: ContextSourceScope::Legacy,
                    })
                }
                Some(asset) if asset.project_id != loaded.project_id => {
                    diagnostics.push(ContextDiagnostic::error(
                        "CONTEXT_ASSET_PROJECT_MISMATCH",
                        item.asset_id.clone(),
                    ))
                }
                Some(_) => diagnostics.push(ContextDiagnostic::error(
                    "CONTEXT_IMAGE_REQUIRED",
                    item.asset_id.clone(),
                )),
                None => diagnostics.push(ContextDiagnostic::error(
                    "CONTEXT_ASSET_NOT_FOUND",
                    item.asset_id.clone(),
                )),
            }
        }
    }
    let reference_assets = order_reference_assets(asset_candidates);
    let stage_prompt = stage_prompt(data, loaded.stage);
    prompt.shot_action.push(PromptFragmentInput::new(
        stage_prompt.clone(),
        ContextSourceScope::Shot,
        shot_id.to_owned(),
    ));
    let workflow = workflow(data, loaded.stage);
    prompt.camera.extend(scalar_prompt(
        &workflow.scalar_values,
        "camera",
        ContextSourceScope::Shot,
        shot_id,
    ));
    prompt.output_specification.extend(output_prompt(
        &workflow.scalar_values,
        ContextSourceScope::Shot,
        shot_id,
    ));
    let output = output_spec(&workflow.scalar_values);
    let prompt_context = build_prompt_context(&prompt);
    let structure = structure(loaded.tree, &data.shot, shot_id);
    let pack = build_shot_reference_pack(ShotReferencePackInput {
        shot_id: shot_id.to_owned(),
        characters: resolved_profiles
            .characters
            .iter()
            .map(to_character)
            .collect(),
        scene: resolved_profiles.scene.as_ref().map(to_scene),
        props: resolved_profiles.props.iter().map(to_prop).collect(),
        style: resolved_profiles.style.as_ref().map(to_style),
        reference_sets: resolved_reference_sets.clone(),
        prompt_context: prompt_context.clone(),
        source_trace: merged_profiles
            .bindings
            .iter()
            .map(|value| value.source.clone())
            .chain(
                merged_reference_sets
                    .bindings
                    .iter()
                    .map(|value| value.source.clone()),
            )
            .collect(),
    });
    let context_hash = compute_context_hash(&ContextHashInput {
        project_id: loaded.project_id.to_owned(),
        structure: structure.clone(),
        stage: loaded.stage.as_str().to_owned(),
        profile_ids: profile_ids(&resolved_profiles),
        profile_content_hashes: profile_hashes(&resolved_profiles),
        costume_ids: costume_ids(&resolved_profiles),
        reference_set_content_hashes: reference_hashes.clone(),
        asset_ids: reference_assets
            .iter()
            .map(|value| value.asset_id.clone())
            .collect(),
        asset_sha256: reference_assets
            .iter()
            .map(|value| value.sha256.clone())
            .collect(),
        ordered_prompt_segments: prompt_context.segments.clone(),
        negative_prompt: prompt_context.negative_prompt.clone(),
        workflow_version_id: workflow.workflow_version_id.clone(),
        recipe_id: workflow.recipe_id.clone(),
        scalar_values: workflow.scalar_values.clone(),
        output: output.clone(),
    });
    let partial = diagnostics.iter().any(|value| {
        value.severity == crate::domain::shot_context::ContextDiagnosticSeverity::Error
    });
    Ok(ResolvedShotContext {
        project_id: loaded.project_id.to_owned(),
        structure,
        stage: loaded.stage,
        reference_pack: pack,
        profiles: resolved_profiles,
        reference_assets,
        prompt_context,
        workflow,
        output,
        legacy: LegacyContext {
            has_reference_pack: new_pack,
            uses_legacy_shot_references: !new_pack
                && data
                    .reference_assets
                    .iter()
                    .any(|value| value.stage == loaded.stage),
            prompt: Some(stage_prompt).filter(|value| !value.is_empty()),
        },
        diagnostics,
        partial,
        resolver_identity: ResolverIdentity {
            resolved_at: Some(resolved_at),
            context_hash,
            reference_set_content_hashes: reference_hashes,
        },
    })
}

fn scope_for(scope: ConsistencyScopeType) -> ContextSourceScope {
    match scope {
        ConsistencyScopeType::Project => ContextSourceScope::Project,
        ConsistencyScopeType::Series => ContextSourceScope::Series,
        ConsistencyScopeType::Episode => ContextSourceScope::Episode,
        ConsistencyScopeType::Scene => ContextSourceScope::Scene,
    }
}
fn scope_id(
    scope: ContextSourceScope,
    project: &str,
    series: Option<&str>,
    episode: Option<&str>,
    scene: Option<&str>,
) -> Option<String> {
    Some(match scope {
        ContextSourceScope::Project => project.to_owned(),
        ContextSourceScope::Series => series?.to_owned(),
        ContextSourceScope::Episode => episode?.to_owned(),
        ContextSourceScope::Scene => scene?.to_owned(),
        _ => return None,
    })
}
fn role_rank(role: BindingRole) -> usize {
    match role {
        BindingRole::Character => 0,
        BindingRole::Scene => 1,
        BindingRole::Prop => 2,
        BindingRole::Style => 3,
        BindingRole::ShotReference => 4,
    }
}
fn active_revision_id(value: &ConsistencyProfileRecord) -> Option<String> {
    match value {
        ConsistencyProfileRecord::Character(v) => v.active_revision_id.clone(),
        ConsistencyProfileRecord::Scene(v) => v.active_revision_id.clone(),
        ConsistencyProfileRecord::Prop(v) => v.active_revision_id.clone(),
        ConsistencyProfileRecord::Style(v) => v.active_revision_id.clone(),
    }
}
fn profile_text(value: &ConsistencyProfileRecord) -> (String, Option<String>, Vec<String>) {
    match value {
        ConsistencyProfileRecord::Character(v) => (
            v.canonical_prompt.clone(),
            Some(v.negative_prompt.clone()),
            v.default_reference_set_id.clone().into_iter().collect(),
        ),
        ConsistencyProfileRecord::Scene(v) => (
            v.environment_prompt.clone(),
            v.negative_prompt.clone(),
            v.default_reference_set_id.clone().into_iter().collect(),
        ),
        ConsistencyProfileRecord::Prop(v) => {
            let text = [
                Some(v.canonical_prompt.clone()),
                v.material_prompt.clone(),
                v.scale_prompt.clone(),
            ]
            .into_iter()
            .flatten()
            .filter(|value| !value.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");
            (
                text,
                None,
                v.default_reference_set_id.clone().into_iter().collect(),
            )
        }
        ConsistencyProfileRecord::Style(v) => {
            let text = [
                Some(v.style_prompt.clone()),
                v.color_prompt.clone(),
                v.line_prompt.clone(),
                v.output_notes.clone(),
            ]
            .into_iter()
            .flatten()
            .filter(|value| !value.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");
            (text, v.negative_prompt.clone(), Vec::new())
        }
    }
}
fn profile_hash(
    value: &ConsistencyProfileRecord,
    revisions: &HashMap<String, ProfileRevision>,
    diagnostics: &mut Vec<ContextDiagnostic>,
) -> (Option<String>, String) {
    let revision_id = active_revision_id(value);
    if let Some(id) = revision_id.as_deref() {
        match revisions.get(id) {
            Some(revision)
                if revision.profile_id == value.id()
                    && revision.profile_type == value.profile_type() =>
            {
                let actual = crate::application::shot_reference_pack_builder::hex_sha256(
                    revision.content_json.as_bytes(),
                );
                if !actual.eq_ignore_ascii_case(&revision.content_sha256) {
                    diagnostics.push(ContextDiagnostic::error(
                        "CONTEXT_PROFILE_REVISION_HASH_MISMATCH",
                        id.to_owned(),
                    ));
                }
                return (Some(id.to_owned()), revision.content_sha256.clone());
            }
            Some(_) => diagnostics.push(ContextDiagnostic::error(
                "CONTEXT_PROFILE_REVISION_HASH_MISMATCH",
                id.to_owned(),
            )),
            None => diagnostics.push(ContextDiagnostic::error(
                "CONTEXT_PROFILE_REVISION_MISSING",
                id.to_owned(),
            )),
        }
    } else {
        diagnostics.push(ContextDiagnostic::warning(
            "CONTEXT_PROFILE_REVISION_MISSING",
            format!("profile {} uses live content", value.id()),
        ));
    }
    let bytes = serde_json::to_vec(value).expect("profile DTO is serializable");
    (
        revision_id,
        crate::application::shot_reference_pack_builder::hex_sha256(&bytes),
    )
}
fn fragment(value: &ResolvedProfile, negative: bool) -> PromptFragmentInput {
    PromptFragmentInput {
        text: value.prompt.clone(),
        negative_prompt: if negative {
            value.negative_prompt.clone()
        } else {
            None
        },
        source_scope: value.source.scope,
        source_entity_id: value.profile_id.clone(),
        revision_id: value.revision_id.clone(),
        ordinal: value.ordinal,
    }
}
fn stage_prompt(data: &ShotData, stage: ShotStage) -> String {
    let image = data
        .stage_prompts
        .iter()
        .find(|value| value.stage == ShotStage::Image)
        .map(|value| value.prompt_text.as_str());
    let video = data
        .stage_prompts
        .iter()
        .find(|value| value.stage == ShotStage::Video)
        .map(|value| value.prompt_text.as_str());
    select_stage_prompt(stage, image, video, &data.shot.prompt_text)
}
fn workflow(data: &ShotData, stage: ShotStage) -> ResolvedWorkflowContext {
    data.stage_configs
        .iter()
        .find(|value| value.stage == stage)
        .map(|value| ResolvedWorkflowContext {
            workflow_version_id: Some(value.workflow_version_id.clone()),
            recipe_id: Some(value.recipe_id.clone()),
            scalar_values: value.scalar_values.clone(),
        })
        .unwrap_or_default()
}
fn scalar_prompt(
    values: &Value,
    key: &str,
    scope: ContextSourceScope,
    id: &str,
) -> Vec<PromptFragmentInput> {
    values
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(|value| vec![PromptFragmentInput::new(value, scope, id.to_owned())])
        .unwrap_or_default()
}
fn output_prompt(values: &Value, scope: ContextSourceScope, id: &str) -> Vec<PromptFragmentInput> {
    let mut parts = Vec::new();
    for key in [
        "width",
        "height",
        "count",
        "durationSeconds",
        "duration_seconds",
    ] {
        if let Some(value) = values.get(key) {
            if !value.is_null() {
                parts.push(format!("{key}={value}"));
            }
        }
    }
    if parts.is_empty() {
        Vec::new()
    } else {
        vec![PromptFragmentInput::new(
            parts.join(", "),
            scope,
            id.to_owned(),
        )]
    }
}
fn output_spec(values: &Value) -> ResolvedOutputSpec {
    ResolvedOutputSpec {
        width: values.get("width").and_then(Value::as_i64),
        height: values.get("height").and_then(Value::as_i64),
        count: values.get("count").and_then(Value::as_i64),
        duration_seconds: values
            .get("durationSeconds")
            .or_else(|| values.get("duration_seconds"))
            .and_then(Value::as_f64),
    }
}
fn hierarchy_ids(
    tree: &crate::application::ports::ProductionStructureTreeData,
    shot_id: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    let Some(assignment) = tree
        .assignments
        .iter()
        .find(|value| value.shot_id == shot_id)
    else {
        return (None, None, None);
    };
    let Some(scene) = tree
        .scenes
        .iter()
        .find(|value| value.id.as_str() == assignment.scene_id.as_str())
    else {
        return (None, None, None);
    };
    let Some(episode) = tree
        .episodes
        .iter()
        .find(|value| value.id.as_str() == scene.episode_id.as_str())
    else {
        return (None, None, Some(scene.id.as_str().to_owned()));
    };
    let series = tree
        .series
        .iter()
        .find(|value| value.id.as_str() == episode.series_id.as_str())
        .map(|value| value.id.as_str().to_owned());
    (
        series,
        Some(episode.id.as_str().to_owned()),
        Some(scene.id.as_str().to_owned()),
    )
}
fn structure(
    tree: &crate::application::ports::ProductionStructureTreeData,
    shot: &crate::application::ports::ShotRecord,
    shot_id: &str,
) -> ResolvedStructure {
    let (series_id, episode_id, scene_id) = hierarchy_ids(tree, shot_id);
    let series = series_id.and_then(|id| {
        tree.series
            .iter()
            .find(|value| value.id.as_str() == id)
            .map(|value| ResolvedStructureNode {
                id: id.clone(),
                ordinal: value.ordinal,
                name: value.name.clone(),
            })
    });
    let episode = episode_id.and_then(|id| {
        tree.episodes
            .iter()
            .find(|value| value.id.as_str() == id)
            .map(|value| ResolvedStructureNode {
                id: id.clone(),
                ordinal: value.ordinal,
                name: value.name.clone(),
            })
    });
    let scene = scene_id.and_then(|id| {
        tree.scenes
            .iter()
            .find(|value| value.id.as_str() == id)
            .map(|value| ResolvedStructureNode {
                id: id.clone(),
                ordinal: value.ordinal,
                name: value.name.clone(),
            })
    });
    ResolvedStructure {
        series,
        episode,
        scene,
        shot: ResolvedStructureNode {
            id: shot.id.clone(),
            ordinal: shot.ordinal.max(0) as u32,
            name: shot.name.clone(),
        },
    }
}
fn profile_ids(value: &ResolvedProfiles) -> Vec<String> {
    value
        .characters
        .iter()
        .chain(value.scene.iter())
        .chain(value.props.iter())
        .chain(value.style.iter())
        .map(|value| value.profile_id.clone())
        .collect()
}
fn profile_hashes(value: &ResolvedProfiles) -> Vec<String> {
    value
        .characters
        .iter()
        .chain(value.scene.iter())
        .chain(value.props.iter())
        .chain(value.style.iter())
        .map(|value| value.content_hash.clone())
        .collect()
}
fn costume_ids(value: &ResolvedProfiles) -> Vec<String> {
    value
        .characters
        .iter()
        .filter_map(|value| value.costume_variant_id.clone())
        .collect()
}
fn to_character(value: &ResolvedProfile) -> ResolvedCharacter {
    ResolvedCharacter {
        profile_id: value.profile_id.clone(),
        costume_variant_id: value.costume_variant_id.clone(),
        ordinal: value.ordinal,
        reference_set_ids: Vec::new(),
        source: value.source.clone(),
        revision_id: value.revision_id.clone(),
        content_hash: value.content_hash.clone(),
        prompt: value.prompt.clone(),
        negative_prompt: value.negative_prompt.clone(),
    }
}
fn to_scene(value: &ResolvedProfile) -> ResolvedScene {
    ResolvedScene {
        profile_id: Some(value.profile_id.clone()),
        reference_set_ids: Vec::new(),
        source: Some(value.source.clone()),
        revision_id: value.revision_id.clone(),
        content_hash: Some(value.content_hash.clone()),
        prompt: value.prompt.clone(),
        lighting_prompt: None,
        negative_prompt: value.negative_prompt.clone(),
    }
}
fn to_prop(value: &ResolvedProfile) -> ResolvedProp {
    ResolvedProp {
        profile_id: value.profile_id.clone(),
        ordinal: value.ordinal,
        reference_set_ids: Vec::new(),
        source: value.source.clone(),
        revision_id: value.revision_id.clone(),
        content_hash: value.content_hash.clone(),
        prompt: value.prompt.clone(),
        negative_prompt: value.negative_prompt.clone(),
    }
}
fn to_style(value: &ResolvedProfile) -> ResolvedStyle {
    ResolvedStyle {
        profile_id: Some(value.profile_id.clone()),
        reference_set_ids: Vec::new(),
        source: Some(value.source.clone()),
        revision_id: value.revision_id.clone(),
        content_hash: Some(value.content_hash.clone()),
        prompt: value.prompt.clone(),
        negative_prompt: value.negative_prompt.clone(),
    }
}
fn single_role_diagnostics(
    values: &[crate::application::shot_reference_pack_builder::MergedProfileBinding],
    diagnostics: &mut Vec<ContextDiagnostic>,
) {
    for role in [BindingRole::Scene, BindingRole::Style] {
        if values.iter().filter(|value| value.role == role).count() > 1 {
            diagnostics.push(ContextDiagnostic::error(
                "CONTEXT_SINGLE_ROLE_CONFLICT",
                role.as_str(),
            ));
        }
    }
}
fn single_role_reference_diagnostics(
    values: &[crate::application::shot_reference_pack_builder::MergedReferenceSetBinding],
    diagnostics: &mut Vec<ContextDiagnostic>,
) {
    for role in [BindingRole::Scene, BindingRole::Style] {
        if values.iter().filter(|value| value.role == role).count() > 1 {
            diagnostics.push(ContextDiagnostic::error(
                "CONTEXT_SINGLE_ROLE_CONFLICT",
                role.as_str(),
            ));
        }
    }
}
