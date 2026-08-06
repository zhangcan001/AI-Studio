use serde::{Deserialize, Serialize};

/// The on-disk manifest contract used by the existing runtime package loader.
/// Keep this type small and shared so onboarding cannot drift from runtime.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowManifest {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub workflow_version: String,
    pub recipe_version: String,
    pub category: String,
    pub mode: String,
}

impl WorkflowManifest {
    pub fn parse(yaml: &str) -> Result<Self, String> {
        yaml_serde::from_str(yaml).map_err(|error| format!("invalid manifest.yaml: {error}"))
    }

    pub fn to_yaml(&self) -> Result<String, String> {
        yaml_serde::to_string(self).map_err(|error| format!("serialize manifest.yaml: {error}"))
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err(format!(
                "unsupported manifest schema_version {}",
                self.schema_version
            ));
        }
        if !self.id.starts_with("wfl_") || self.id.len() <= 4 {
            return Err("manifest id must start with wfl_".to_owned());
        }
        for (field, value) in [
            ("name", self.name.as_str()),
            ("workflow_version", self.workflow_version.as_str()),
            ("recipe_version", self.recipe_version.as_str()),
            ("category", self.category.as_str()),
            ("mode", self.mode.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("manifest {field} must not be empty"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::WorkflowManifest;

    #[test]
    fn manifest_yaml_round_trip_preserves_runtime_contract() {
        let manifest = WorkflowManifest {
            schema_version: 1,
            id: "wfl_round_trip".to_owned(),
            name: "Round Trip".to_owned(),
            workflow_version: "1.2.3".to_owned(),
            recipe_version: "1.0.0".to_owned(),
            category: "image".to_owned(),
            mode: "text_to_image".to_owned(),
        };
        let yaml = manifest.to_yaml().expect("manifest should serialize");
        let parsed = WorkflowManifest::parse(&yaml).expect("manifest should parse");
        assert_eq!(parsed, manifest);
        parsed.validate().expect("manifest should validate");
    }
}
