use std::{error::Error, fmt};
use uuid::Uuid;

const DEFAULT_PROJECT_ID: &str = "prj_default";
const PROJECT_ID_PREFIX: &str = "prj_";

/// Validates the stable project identifier format used by application-owned projects.
///
/// Repository fixtures may use arbitrary identifiers in isolated tests, but values
/// crossing the application or filesystem boundary must be either the bootstrap
/// project or `prj_<hyphenated UUID>`.
pub fn validate_project_id(value: &str) -> Result<(), ProjectIdValidationError> {
    if value == DEFAULT_PROJECT_ID {
        return Ok(());
    }

    let Some(uuid_text) = value.strip_prefix(PROJECT_ID_PREFIX) else {
        return Err(ProjectIdValidationError::new(value));
    };

    let Ok(uuid) = Uuid::parse_str(uuid_text) else {
        return Err(ProjectIdValidationError::new(value));
    };

    if uuid_text.len() != 36
        || !uuid
            .hyphenated()
            .to_string()
            .eq_ignore_ascii_case(uuid_text)
    {
        return Err(ProjectIdValidationError::new(value));
    }

    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectIdValidationError {
    value: String,
}

impl ProjectIdValidationError {
    fn new(value: &str) -> Self {
        Self {
            value: value.to_owned(),
        }
    }
}

impl fmt::Display for ProjectIdValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "INVALID_PROJECT_ID: project id {:?} must be prj_default or prj_<UUID>",
            self.value
        )
    }
}

impl Error for ProjectIdValidationError {}

#[cfg(test)]
mod tests {
    use super::validate_project_id;

    #[test]
    fn accepts_default_and_hyphenated_uuid_project_ids() {
        assert!(validate_project_id("prj_default").is_ok());
        assert!(validate_project_id("prj_550e8400-e29b-41d4-a716-446655440000").is_ok());
        assert!(validate_project_id("prj_550E8400-E29B-41D4-A716-446655440000").is_ok());
    }

    #[test]
    fn rejects_arbitrary_and_path_like_project_ids() {
        for value in [
            "",
            " ",
            "default",
            "prj_",
            "prj_test",
            "project-1",
            "../prj_x",
            "prj/a",
            r"prj\a",
            "prj:a",
            "prj_123",
            "prj_not-a-uuid",
            "prj_550e8400e29b41d4a716446655440000",
        ] {
            assert!(validate_project_id(value).is_err(), "accepted {value:?}");
        }
    }
}
