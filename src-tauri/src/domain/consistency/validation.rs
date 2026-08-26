use super::{
    binding::{BindingRole, ShotProfileBinding, ShotReferenceSetBinding},
    reference_set::{ReferenceSet, ReferenceSetItem},
};
use crate::domain::consistency::profile::ProfileType;
use serde_json::Value;
use std::{collections::HashSet, error::Error, fmt};

pub const MAX_PROFILE_NAME_CHARS: usize = 120;
pub const MAX_DESCRIPTION_CHARS: usize = 4_000;
pub const MAX_PROMPT_FRAGMENT_CHARS: usize = 20_000;
pub const MAX_METADATA_BYTES: usize = 64 * 1024;
pub const MAX_REFERENCE_ROLE_CHARS: usize = 120;

/// Stable, diagnostic validation failure for the consistency data contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsistencyValidationError {
    code: &'static str,
    field: String,
    message: String,
}

/// Short name kept for callers that prefer the generic validation terminology.
pub type ValidationError = ConsistencyValidationError;

impl ConsistencyValidationError {
    pub fn new(code: &'static str, field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code,
            field: field.into(),
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn field(&self) -> &str {
        &self.field
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ConsistencyValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}: {}", self.code, self.field, self.message)
    }
}

impl Error for ConsistencyValidationError {}

pub fn validate_profile_name(value: &str) -> Result<(), ConsistencyValidationError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ConsistencyValidationError::new(
            "INVALID_PROFILE_NAME",
            "name",
            "must not be empty after trimming",
        ));
    }
    if trimmed.chars().count() > MAX_PROFILE_NAME_CHARS {
        return Err(ConsistencyValidationError::new(
            "INVALID_PROFILE_NAME",
            "name",
            format!("must be at most {MAX_PROFILE_NAME_CHARS} Unicode scalar characters"),
        ));
    }
    Ok(())
}

/// Validates optional descriptive text. The original value is never normalized or truncated.
pub fn validate_optional_text(
    field: &str,
    value: Option<&str>,
) -> Result<(), ConsistencyValidationError> {
    if let Some(value) = value {
        if value.chars().count() > MAX_DESCRIPTION_CHARS {
            return Err(ConsistencyValidationError::new(
                "INVALID_OPTIONAL_TEXT",
                field,
                format!("must be at most {MAX_DESCRIPTION_CHARS} Unicode scalar characters"),
            ));
        }
    }
    Ok(())
}

pub fn validate_prompt_fragment(
    field: &str,
    value: &str,
) -> Result<(), ConsistencyValidationError> {
    if value.chars().count() > MAX_PROMPT_FRAGMENT_CHARS {
        return Err(ConsistencyValidationError::new(
            "INVALID_PROMPT_FRAGMENT",
            field,
            format!("must be at most {MAX_PROMPT_FRAGMENT_CHARS} Unicode scalar characters"),
        ));
    }
    Ok(())
}

pub fn validate_metadata_json(value: &str) -> Result<(), ConsistencyValidationError> {
    if value.len() > MAX_METADATA_BYTES {
        return Err(ConsistencyValidationError::new(
            "INVALID_METADATA_JSON",
            "metadata_json",
            format!("must be at most {MAX_METADATA_BYTES} bytes"),
        ));
    }

    let parsed: Value = serde_json::from_str(value).map_err(|error| {
        ConsistencyValidationError::new(
            "INVALID_METADATA_JSON",
            "metadata_json",
            format!("must be valid JSON object: {error}"),
        )
    })?;

    if !parsed.is_object() {
        return Err(ConsistencyValidationError::new(
            "INVALID_METADATA_JSON",
            "metadata_json",
            "must be a JSON object, not an array or scalar",
        ));
    }
    Ok(())
}

pub fn validate_reference_set(
    reference_set: &ReferenceSet,
) -> Result<(), ConsistencyValidationError> {
    validate_profile_name(&reference_set.name)?;
    validate_optional_text("description", Some(&reference_set.description))?;

    match (
        reference_set.owner_profile_type.as_ref(),
        reference_set.owner_profile_id.as_deref(),
    ) {
        (Some(_), Some(profile_id)) if !profile_id.trim().is_empty() => {}
        (None, None) => {}
        (Some(_), None) | (None, Some(_)) => {
            return Err(ConsistencyValidationError::new(
                "INVALID_REFERENCE_SET_OWNER",
                "owner_profile",
                "owner_profile_type and owner_profile_id must be set together",
            ))
        }
        (Some(_), Some(_)) => {
            return Err(ConsistencyValidationError::new(
                "INVALID_REFERENCE_SET_OWNER",
                "owner_profile_id",
                "must not be empty",
            ))
        }
    }
    Ok(())
}

pub fn validate_reference_set_items(
    items: &[ReferenceSetItem],
) -> Result<(), ConsistencyValidationError> {
    let mut asset_ids = HashSet::with_capacity(items.len());
    let mut ordinals = HashSet::with_capacity(items.len());
    let mut primary_count = 0usize;

    for item in items {
        if item.asset_id.trim().is_empty() {
            return Err(ConsistencyValidationError::new(
                "INVALID_REFERENCE_SET_ITEM",
                "asset_id",
                "must not be empty",
            ));
        }
        if !asset_ids.insert(item.asset_id.as_str()) {
            return Err(ConsistencyValidationError::new(
                "INVALID_REFERENCE_SET_ITEM",
                "asset_id",
                "must be unique within a reference set",
            ));
        }
        if item.ordinal < 0 {
            return Err(ConsistencyValidationError::new(
                "INVALID_REFERENCE_SET_ITEM",
                "ordinal",
                "must be non-negative",
            ));
        }
        if !ordinals.insert(item.ordinal) {
            return Err(ConsistencyValidationError::new(
                "INVALID_REFERENCE_SET_ITEM",
                "ordinal",
                "must be unique within a reference set",
            ));
        }
        if let Some(role) = item.role.as_deref() {
            if role.trim().is_empty() {
                return Err(ConsistencyValidationError::new(
                    "INVALID_REFERENCE_SET_ITEM",
                    "role",
                    "must not be empty when supplied",
                ));
            }
            if role.chars().count() > MAX_REFERENCE_ROLE_CHARS {
                return Err(ConsistencyValidationError::new(
                    "INVALID_REFERENCE_SET_ITEM",
                    "role",
                    format!("must be at most {MAX_REFERENCE_ROLE_CHARS} Unicode scalar characters"),
                ));
            }
        }
        if item.is_primary {
            primary_count += 1;
            if primary_count > 1 {
                return Err(ConsistencyValidationError::new(
                    "INVALID_REFERENCE_SET_ITEM",
                    "is_primary",
                    "at most one item may be primary",
                ));
            }
        }
    }

    let mut sorted_ordinals: Vec<_> = ordinals.into_iter().collect();
    sorted_ordinals.sort_unstable();
    for (expected, actual) in sorted_ordinals.into_iter().enumerate() {
        if actual != expected as i64 {
            return Err(ConsistencyValidationError::new(
                "INVALID_REFERENCE_SET_ITEM",
                "ordinal",
                "must be a contiguous sequence starting at zero",
            ));
        }
    }
    Ok(())
}

pub fn validate_profile_binding(
    binding: &ShotProfileBinding,
) -> Result<(), ConsistencyValidationError> {
    if binding.ordinal < 0 {
        return Err(ConsistencyValidationError::new(
            "INVALID_SHOT_PROFILE_BINDING",
            "ordinal",
            "must be non-negative",
        ));
    }

    let expected_profile_type = match binding.role {
        BindingRole::Character => ProfileType::Character,
        BindingRole::Scene => ProfileType::Scene,
        BindingRole::Prop => ProfileType::Prop,
        BindingRole::Style => ProfileType::Style,
        BindingRole::ShotReference => {
            return Err(ConsistencyValidationError::new(
                "INVALID_SHOT_PROFILE_BINDING",
                "role",
                "SHOT_REFERENCE is only valid for reference-set bindings",
            ))
        }
    };

    if &binding.profile_type != &expected_profile_type {
        return Err(ConsistencyValidationError::new(
            "INVALID_SHOT_PROFILE_BINDING",
            "profile_type",
            format!(
                "role {} requires profile type {:?}",
                binding.role.as_str(),
                expected_profile_type
            ),
        ));
    }

    if binding.costume_variant_id.is_some() && binding.role != BindingRole::Character {
        return Err(ConsistencyValidationError::new(
            "INVALID_SHOT_PROFILE_BINDING",
            "costume_variant_id",
            "a costume variant may only be attached to a CHARACTER binding",
        ));
    }
    Ok(())
}

pub fn validate_reference_set_binding(
    binding: &ShotReferenceSetBinding,
) -> Result<(), ConsistencyValidationError> {
    if binding.ordinal < 0 {
        return Err(ConsistencyValidationError::new(
            "INVALID_SHOT_REFERENCE_SET_BINDING",
            "ordinal",
            "must be non-negative",
        ));
    }
    Ok(())
}
