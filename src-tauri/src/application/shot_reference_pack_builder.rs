//! Pure reference-pack merge and content-hash helpers.
use crate::domain::consistency::{BindingRole, InheritanceMode, ProfileType, ReferenceSetPurpose};
use crate::domain::shot_context::{
    ContextDiagnostic, ContextHashInput, ContextSourceScope, ResolvedCharacter, ResolvedProp,
    ResolvedReferenceAsset, ResolvedReferenceSet, ResolvedScene, ResolvedStyle, ShotReferencePack,
    SourceTrace,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileBindingCandidate {
    pub binding_id: String,
    pub scope: ContextSourceScope,
    pub scope_id: String,
    pub role: BindingRole,
    pub profile_type: ProfileType,
    pub profile_id: String,
    pub costume_variant_id: Option<String>,
    pub ordinal: i64,
    pub inheritance_mode: InheritanceMode,
}

impl ProfileBindingCandidate {
    pub fn source(&self) -> SourceTrace {
        SourceTrace {
            scope: self.scope,
            scope_id: self.scope_id.clone(),
            binding_id: Some(self.binding_id.clone()),
            entity_id: self.profile_id.clone(),
            inheritance_mode: self.inheritance_mode,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceSetBindingCandidate {
    pub binding_id: String,
    pub scope: ContextSourceScope,
    pub scope_id: String,
    pub role: BindingRole,
    pub reference_set_id: String,
    pub ordinal: i64,
    pub required: bool,
    pub inheritance_mode: InheritanceMode,
}

impl ReferenceSetBindingCandidate {
    pub fn source(&self) -> SourceTrace {
        SourceTrace {
            scope: self.scope,
            scope_id: self.scope_id.clone(),
            binding_id: Some(self.binding_id.clone()),
            entity_id: self.reference_set_id.clone(),
            inheritance_mode: self.inheritance_mode,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MergedProfileBinding {
    pub role: BindingRole,
    pub profile_type: ProfileType,
    pub profile_id: String,
    pub costume_variant_id: Option<String>,
    pub ordinal: i64,
    pub source: SourceTrace,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MergedReferenceSetBinding {
    pub role: BindingRole,
    pub reference_set_id: String,
    pub ordinal: i64,
    pub required: bool,
    pub source: SourceTrace,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProfileMergeResult {
    pub bindings: Vec<MergedProfileBinding>,
    pub tombstones: Vec<String>,
    pub diagnostics: Vec<ContextDiagnostic>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReferenceSetMergeResult {
    pub bindings: Vec<MergedReferenceSetBinding>,
    pub tombstones: Vec<String>,
    pub diagnostics: Vec<ContextDiagnostic>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ShotReferencePackInput {
    pub shot_id: String,
    pub characters: Vec<ResolvedCharacter>,
    pub scene: Option<ResolvedScene>,
    pub props: Vec<ResolvedProp>,
    pub style: Option<ResolvedStyle>,
    pub reference_sets: Vec<ResolvedReferenceSet>,
    pub prompt_context: crate::domain::shot_context::PromptContext,
    pub source_trace: Vec<SourceTrace>,
}

/// Creates a pack with stable collection ordering. Profile/revision lookup is
/// intentionally outside this pure builder.
pub fn build_shot_reference_pack(mut input: ShotReferencePackInput) -> ShotReferencePack {
    input.characters.sort_by(|left, right| {
        (left.ordinal, left.profile_id.as_str()).cmp(&(right.ordinal, right.profile_id.as_str()))
    });
    input.props.sort_by(|left, right| {
        (left.ordinal, left.profile_id.as_str()).cmp(&(right.ordinal, right.profile_id.as_str()))
    });
    input.reference_sets.sort_by(|left, right| {
        (
            role_rank(left.role),
            left.ordinal,
            left.reference_set_id.as_str(),
        )
            .cmp(&(
                role_rank(right.role),
                right.ordinal,
                right.reference_set_id.as_str(),
            ))
    });
    input.source_trace.sort_by(|left, right| {
        (
            left.scope.rank(),
            left.scope_id.as_str(),
            left.entity_id.as_str(),
            left.binding_id.as_deref().unwrap_or_default(),
        )
            .cmp(&(
                right.scope.rank(),
                right.scope_id.as_str(),
                right.entity_id.as_str(),
                right.binding_id.as_deref().unwrap_or_default(),
            ))
    });
    ShotReferencePack {
        shot_id: input.shot_id,
        characters: input.characters,
        scene: input.scene,
        props: input.props,
        style: input.style,
        reference_sets: input.reference_sets,
        prompt_context: input.prompt_context,
        source_trace: input.source_trace,
    }
}

/// Merges profile bindings in hierarchy order. Input order is not trusted: it
/// is normalized by scope rank, scope id, and binding id before applying rules.
pub fn merge_profile_bindings(bindings: &[ProfileBindingCandidate]) -> ProfileMergeResult {
    let mut ordered = bindings.to_vec();
    ordered.sort_by(|left, right| {
        (
            left.scope.rank(),
            left.scope_id.as_str(),
            left.binding_id.as_str(),
        )
            .cmp(&(
                right.scope.rank(),
                right.scope_id.as_str(),
                right.binding_id.as_str(),
            ))
    });

    let conflicts = profile_conflicts(&ordered);
    let mut active: HashMap<(BindingRole, String), MergedProfileBinding> = HashMap::new();
    let mut tombstones = HashSet::<(BindingRole, String)>::new();
    let mut diagnostics = conflicts
        .iter()
        .map(|conflict| {
            ContextDiagnostic::error(
                "CONTEXT_PROFILE_ORDINAL_CONFLICT",
                format!(
                    "multiple profiles occupy {} ordinal {} in scope {}",
                    conflict.role.as_str(),
                    conflict.ordinal,
                    conflict.scope_id
                ),
            )
            .with_source(conflict.scope, conflict.scope_id.clone())
        })
        .collect::<Vec<_>>();

    for binding in ordered {
        if conflicts.iter().any(|conflict| {
            conflict.scope == binding.scope
                && conflict.scope_id == binding.scope_id
                && conflict.role == binding.role
                && conflict.ordinal == binding.ordinal
        }) {
            continue;
        }

        let key = (binding.role, binding.profile_id.clone());
        match binding.inheritance_mode {
            InheritanceMode::Remove => {
                active.remove(&key);
                tombstones.insert(key);
            }
            InheritanceMode::Replace => {
                let source = binding.source();
                active.retain(|(role, _), _| *role != binding.role);
                tombstones.retain(|(role, _)| *role != binding.role);
                active.insert(
                    key,
                    MergedProfileBinding {
                        role: binding.role,
                        profile_type: binding.profile_type,
                        profile_id: binding.profile_id,
                        costume_variant_id: binding.costume_variant_id,
                        ordinal: binding.ordinal,
                        source,
                    },
                );
            }
            InheritanceMode::Explicit | InheritanceMode::Inherited => {
                if tombstones.contains(&key)
                    && binding.inheritance_mode != InheritanceMode::Explicit
                {
                    continue;
                }
                tombstones.remove(&key);
                let source = binding.source();
                active.insert(
                    key,
                    MergedProfileBinding {
                        role: binding.role,
                        profile_type: binding.profile_type,
                        profile_id: binding.profile_id,
                        costume_variant_id: binding.costume_variant_id,
                        ordinal: binding.ordinal,
                        source,
                    },
                );
            }
        }
    }

    let mut result = active.into_values().collect::<Vec<_>>();
    result.sort_by(|left, right| {
        (role_rank(left.role), left.ordinal, left.profile_id.as_str()).cmp(&(
            role_rank(right.role),
            right.ordinal,
            right.profile_id.as_str(),
        ))
    });
    let mut tombstones = tombstones
        .into_iter()
        .map(|(role, profile_id)| format!("{}:{profile_id}", role.as_str()))
        .collect::<Vec<_>>();
    tombstones.sort();
    diagnostics.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then(left.message.cmp(&right.message))
    });

    ProfileMergeResult {
        bindings: result,
        tombstones,
        diagnostics,
    }
}

/// Merges reusable reference-set bindings using the same parent-to-child
/// semantics as profile bindings.
pub fn merge_reference_set_bindings(
    bindings: &[ReferenceSetBindingCandidate],
) -> ReferenceSetMergeResult {
    let mut ordered = bindings.to_vec();
    ordered.sort_by(|left, right| {
        (
            left.scope.rank(),
            left.scope_id.as_str(),
            left.binding_id.as_str(),
        )
            .cmp(&(
                right.scope.rank(),
                right.scope_id.as_str(),
                right.binding_id.as_str(),
            ))
    });

    let conflicts = reference_conflicts(&ordered);
    let mut active: HashMap<(BindingRole, String), MergedReferenceSetBinding> = HashMap::new();
    let mut tombstones = HashSet::<(BindingRole, String)>::new();
    let mut diagnostics = conflicts
        .iter()
        .map(|conflict| {
            ContextDiagnostic::error(
                "CONTEXT_REFERENCE_ORDINAL_CONFLICT",
                format!(
                    "multiple reference sets occupy {} ordinal {} in scope {}",
                    conflict.role.as_str(),
                    conflict.ordinal,
                    conflict.scope_id
                ),
            )
            .with_source(conflict.scope, conflict.scope_id.clone())
        })
        .collect::<Vec<_>>();

    for binding in ordered {
        if conflicts.iter().any(|conflict| {
            conflict.scope == binding.scope
                && conflict.scope_id == binding.scope_id
                && conflict.role == binding.role
                && conflict.ordinal == binding.ordinal
        }) {
            continue;
        }
        let key = (binding.role, binding.reference_set_id.clone());
        match binding.inheritance_mode {
            InheritanceMode::Remove => {
                active.remove(&key);
                tombstones.insert(key);
            }
            InheritanceMode::Replace => {
                let source = binding.source();
                active.retain(|(role, _), _| *role != binding.role);
                tombstones.retain(|(role, _)| *role != binding.role);
                active.insert(
                    key,
                    MergedReferenceSetBinding {
                        role: binding.role,
                        reference_set_id: binding.reference_set_id,
                        ordinal: binding.ordinal,
                        required: binding.required,
                        source,
                    },
                );
            }
            InheritanceMode::Explicit | InheritanceMode::Inherited => {
                if tombstones.contains(&key)
                    && binding.inheritance_mode != InheritanceMode::Explicit
                {
                    continue;
                }
                tombstones.remove(&key);
                let source = binding.source();
                active.insert(
                    key,
                    MergedReferenceSetBinding {
                        role: binding.role,
                        reference_set_id: binding.reference_set_id,
                        ordinal: binding.ordinal,
                        required: binding.required,
                        source,
                    },
                );
            }
        }
    }

    let mut result = active.into_values().collect::<Vec<_>>();
    result.sort_by(|left, right| {
        (
            role_rank(left.role),
            left.ordinal,
            left.reference_set_id.as_str(),
        )
            .cmp(&(
                role_rank(right.role),
                right.ordinal,
                right.reference_set_id.as_str(),
            ))
    });
    let mut tombstones = tombstones
        .into_iter()
        .map(|(role, reference_set_id)| format!("{}:{reference_set_id}", role.as_str()))
        .collect::<Vec<_>>();
    tombstones.sort();
    diagnostics.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then(left.message.cmp(&right.message))
    });

    ReferenceSetMergeResult {
        bindings: result,
        tombstones,
        diagnostics,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProfileConflict {
    scope: ContextSourceScope,
    scope_id: String,
    role: BindingRole,
    ordinal: i64,
}

fn profile_conflicts(bindings: &[ProfileBindingCandidate]) -> Vec<ProfileConflict> {
    let mut slots: HashMap<(ContextSourceScope, String, BindingRole, i64), HashSet<String>> =
        HashMap::new();
    for binding in bindings {
        slots
            .entry((
                binding.scope,
                binding.scope_id.clone(),
                binding.role,
                binding.ordinal,
            ))
            .or_default()
            .insert(binding.profile_id.clone());
    }
    let mut conflicts = slots
        .into_iter()
        .filter_map(|((scope, scope_id, role, ordinal), ids)| {
            (ids.len() > 1).then_some(ProfileConflict {
                scope,
                scope_id,
                role,
                ordinal,
            })
        })
        .collect::<Vec<_>>();
    conflicts.sort_by(|left, right| {
        (
            left.scope.rank(),
            left.scope_id.as_str(),
            role_rank(left.role),
            left.ordinal,
        )
            .cmp(&(
                right.scope.rank(),
                right.scope_id.as_str(),
                role_rank(right.role),
                right.ordinal,
            ))
    });
    conflicts
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReferenceConflict {
    scope: ContextSourceScope,
    scope_id: String,
    role: BindingRole,
    ordinal: i64,
}

fn reference_conflicts(bindings: &[ReferenceSetBindingCandidate]) -> Vec<ReferenceConflict> {
    let mut slots: HashMap<(ContextSourceScope, String, BindingRole, i64), HashSet<String>> =
        HashMap::new();
    for binding in bindings {
        slots
            .entry((
                binding.scope,
                binding.scope_id.clone(),
                binding.role,
                binding.ordinal,
            ))
            .or_default()
            .insert(binding.reference_set_id.clone());
    }
    let mut conflicts = slots
        .into_iter()
        .filter_map(|((scope, scope_id, role, ordinal), ids)| {
            (ids.len() > 1).then_some(ReferenceConflict {
                scope,
                scope_id,
                role,
                ordinal,
            })
        })
        .collect::<Vec<_>>();
    conflicts.sort_by(|left, right| {
        (
            left.scope.rank(),
            left.scope_id.as_str(),
            role_rank(left.role),
            left.ordinal,
        )
            .cmp(&(
                right.scope.rank(),
                right.scope_id.as_str(),
                role_rank(right.role),
                right.ordinal,
            ))
    });
    conflicts
}

pub fn role_rank(role: BindingRole) -> usize {
    match role {
        BindingRole::Character => 0,
        BindingRole::Scene => 1,
        BindingRole::Prop => 2,
        BindingRole::Style => 3,
        BindingRole::ShotReference => 4,
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReferenceAssetCandidate {
    pub asset_id: String,
    pub sha256: String,
    pub role: BindingRole,
    pub binding_ordinal: i64,
    pub set_ordinal: i64,
    pub source_reference_set_id: String,
    pub source_profile_id: Option<String>,
    pub source_scope: ContextSourceScope,
}

pub fn order_reference_assets(
    mut candidates: Vec<ReferenceAssetCandidate>,
) -> Vec<ResolvedReferenceAsset> {
    candidates.sort_by(|left, right| {
        (
            role_rank(left.role),
            left.binding_ordinal,
            left.set_ordinal,
            left.asset_id.as_str(),
        )
            .cmp(&(
                role_rank(right.role),
                right.binding_ordinal,
                right.set_ordinal,
                right.asset_id.as_str(),
            ))
    });
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|candidate| seen.insert(candidate.asset_id.clone()))
        .map(|candidate| ResolvedReferenceAsset {
            asset_id: candidate.asset_id,
            sha256: candidate.sha256,
            role: candidate.role,
            ordinal: candidate.set_ordinal,
            source_reference_set_id: candidate.source_reference_set_id,
            source_profile_id: candidate.source_profile_id,
            source_scope: candidate.source_scope,
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReferenceSetContentHashItem {
    pub asset_id: String,
    pub asset_sha256: String,
    pub ordinal: i64,
    pub role: Option<String>,
    pub is_primary: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ReferenceSetContentHashInput {
    reference_set_id: String,
    purpose: ReferenceSetPurpose,
    items: Vec<ReferenceSetContentHashItem>,
}

pub fn reference_set_content_hash(
    reference_set_id: &str,
    purpose: ReferenceSetPurpose,
    mut items: Vec<ReferenceSetContentHashItem>,
) -> String {
    items.sort_by(|left, right| {
        (left.ordinal, left.asset_id.as_str()).cmp(&(right.ordinal, right.asset_id.as_str()))
    });
    let input = ReferenceSetContentHashInput {
        reference_set_id: reference_set_id.to_owned(),
        purpose,
        items,
    };
    let bytes = serde_json::to_vec(&input).expect("hash input is serializable");
    hex_sha256(&bytes)
}

pub fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Hashes only the canonical, generation-relevant context input. The input
/// deliberately has no resolved-at timestamp or diagnostics field.
pub fn compute_context_hash(input: &ContextHashInput) -> String {
    let bytes = serde_json::to_vec(input).expect("context hash input is serializable");
    hex_sha256(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(
        binding_id: &str,
        scope: ContextSourceScope,
        role: BindingRole,
        profile_id: &str,
        ordinal: i64,
        mode: InheritanceMode,
    ) -> ProfileBindingCandidate {
        ProfileBindingCandidate {
            binding_id: binding_id.to_owned(),
            scope,
            scope_id: scope.as_str().to_owned(),
            role,
            profile_type: match role {
                BindingRole::Character => ProfileType::Character,
                BindingRole::Scene => ProfileType::Scene,
                BindingRole::Prop => ProfileType::Prop,
                BindingRole::Style | BindingRole::ShotReference => ProfileType::Style,
            },
            profile_id: profile_id.to_owned(),
            costume_variant_id: None,
            ordinal,
            inheritance_mode: mode,
        }
    }

    #[test]
    fn merge_supports_remove_and_deeper_explicit_re_add() {
        let result = merge_profile_bindings(&[
            profile(
                "p",
                ContextSourceScope::Project,
                BindingRole::Character,
                "a",
                0,
                InheritanceMode::Inherited,
            ),
            profile(
                "e",
                ContextSourceScope::Episode,
                BindingRole::Character,
                "a",
                0,
                InheritanceMode::Remove,
            ),
            profile(
                "s",
                ContextSourceScope::Shot,
                BindingRole::Character,
                "a",
                0,
                InheritanceMode::Explicit,
            ),
        ]);
        assert_eq!(result.bindings.len(), 1);
        assert_eq!(result.bindings[0].source.scope, ContextSourceScope::Shot);
        assert!(result.tombstones.is_empty());
    }

    #[test]
    fn same_scope_ordinal_conflict_has_no_winner() {
        let result = merge_profile_bindings(&[
            profile(
                "a",
                ContextSourceScope::Scene,
                BindingRole::Style,
                "style-a",
                0,
                InheritanceMode::Explicit,
            ),
            profile(
                "b",
                ContextSourceScope::Scene,
                BindingRole::Style,
                "style-b",
                0,
                InheritanceMode::Explicit,
            ),
        ]);
        assert!(result.bindings.is_empty());
        assert_eq!(
            result.diagnostics[0].code,
            "CONTEXT_PROFILE_ORDINAL_CONFLICT"
        );
    }

    #[test]
    fn reference_assets_have_fixed_role_and_asset_order() {
        let assets = order_reference_assets(vec![
            ReferenceAssetCandidate {
                asset_id: "b".into(),
                sha256: "b".into(),
                role: BindingRole::Scene,
                binding_ordinal: 0,
                set_ordinal: 1,
                source_reference_set_id: "rs".into(),
                source_profile_id: None,
                source_scope: ContextSourceScope::Scene,
            },
            ReferenceAssetCandidate {
                asset_id: "a".into(),
                sha256: "a".into(),
                role: BindingRole::Character,
                binding_ordinal: 1,
                set_ordinal: 0,
                source_reference_set_id: "rs".into(),
                source_profile_id: None,
                source_scope: ContextSourceScope::Project,
            },
            ReferenceAssetCandidate {
                asset_id: "a".into(),
                sha256: "a".into(),
                role: BindingRole::Character,
                binding_ordinal: 1,
                set_ordinal: 1,
                source_reference_set_id: "rs".into(),
                source_profile_id: None,
                source_scope: ContextSourceScope::Project,
            },
        ]);
        assert_eq!(
            assets
                .iter()
                .map(|asset| asset.asset_id.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }
}
