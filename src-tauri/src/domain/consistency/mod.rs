//! Consistency asset system domain contracts.
//!
//! This module contains only serializable domain data, stable identifiers, and
//! validation primitives. Persistence, resolution, readiness, and commands
//! belong to later DEV-048+ layers.

pub mod binding;
pub mod ids;
pub mod profile;
pub mod reference_set;
pub mod scope_binding;
pub mod validation;

#[cfg(test)]
mod tests;

pub use binding::{
    BindingDomainError, BindingRole, InheritanceMode, ShotProfileBinding, ShotReferenceSetBinding,
};
pub use ids::{
    generate_consistency_id, validate_consistency_id, ConsistencyIdKind,
    ConsistencyIdValidationError,
};
pub use profile::{
    CharacterProfile, ConsistencyProfileRecord, CostumeVariant, ProfileDomainError,
    ProfileRevision, ProfileRevisionStatus, ProfileType, PropProfile, SceneProfile, StyleProfile,
};
pub use reference_set::{
    ReferenceSet, ReferenceSetItem, ReferenceSetPurpose, ReferenceSetPurposeError,
};
pub use scope_binding::{
    ConsistencyScopeType, ConsistencyScopeTypeError, ScopedProfileBinding,
    ScopedReferenceSetBinding,
};
pub use validation::{
    validate_metadata_json, validate_optional_text, validate_profile_binding,
    validate_profile_name, validate_prompt_fragment, validate_reference_set,
    validate_reference_set_binding, validate_reference_set_items, ConsistencyValidationError,
    ValidationError, MAX_DESCRIPTION_CHARS, MAX_METADATA_BYTES, MAX_PROFILE_NAME_CHARS,
    MAX_PROMPT_FRAGMENT_CHARS, MAX_REFERENCE_ROLE_CHARS,
};
