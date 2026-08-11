use crate::domain::{InputDefinition, Recipe, RecipeError, WorkflowDocument, WorkflowError};
use std::collections::BTreeSet;

pub struct RecipeValidator;

impl RecipeValidator {
    pub fn validate(recipe: &Recipe) -> Result<(), RecipeError> {
        if recipe.schema_version != 1 {
            return Err(RecipeError::UnsupportedSchema {
                found: recipe.schema_version,
            });
        }

        if recipe.id.trim().is_empty() {
            return Err(RecipeError::invalid("id must not be empty"));
        }

        if recipe.name.trim().is_empty() {
            return Err(RecipeError::invalid("name must not be empty"));
        }

        if !recipe.workflow.is_safe_relative_path() {
            return Err(RecipeError::invalid(format!(
                "workflow.file must be a safe relative path without ..: \"{}\"",
                recipe.workflow.file
            )));
        }

        for (key, definition) in &recipe.inputs {
            if key.trim().is_empty() {
                return Err(RecipeError::invalid("input key must not be empty"));
            }

            if definition.label().trim().is_empty() {
                return Err(RecipeError::invalid(format!(
                    "input \"{key}\" label must not be empty"
                )));
            }

            match definition {
                InputDefinition::Integer {
                    default,
                    min,
                    max,
                    step,
                    ..
                } => {
                    if step.is_some_and(|step| step <= 0) {
                        return Err(RecipeError::invalid(format!(
                            "input \"{key}\" step must be greater than zero"
                        )));
                    }
                    if let (Some(min), Some(max)) = (min, max) {
                        if min > max {
                            return Err(RecipeError::invalid(format!(
                                "input \"{key}\" min {min} must be less than or equal to max {max}"
                            )));
                        }
                    }

                    if let Some(default) = default {
                        if min.is_some_and(|min| *default < min)
                            || max.is_some_and(|max| *default > max)
                        {
                            return Err(RecipeError::invalid(format!(
                                "input \"{key}\" default {default} is outside its declared range"
                            )));
                        }
                        if step.is_some_and(|step| *default % step != 0) {
                            return Err(RecipeError::invalid(format!(
                                "input \"{key}\" default {default} is not aligned to step {}",
                                step.expect("step checked above")
                            )));
                        }
                    }
                    if let Some(step) = step {
                        if min.is_some_and(|min| min % step != 0)
                            || max.is_some_and(|max| max % step != 0)
                        {
                            return Err(RecipeError::invalid(format!(
                                "input \"{key}\" min/max must be aligned to step {step}"
                            )));
                        }
                    }
                }
                InputDefinition::Seed {
                    default, min, max, ..
                } => {
                    validate_unsigned_range(key, *min, *max)?;
                    if let crate::domain::SeedDefault::Fixed(default) = default {
                        if is_outside_unsigned_range(*default, *min, *max) {
                            return Err(RecipeError::invalid(format!(
                                "input \"{key}\" default {default} is outside its declared range"
                            )));
                        }
                    }
                }
                InputDefinition::Images {
                    required,
                    min_items,
                    max_items,
                    ..
                }
                | InputDefinition::Videos {
                    required,
                    min_items,
                    max_items,
                    ..
                }
                | InputDefinition::Audios {
                    required,
                    min_items,
                    max_items,
                    ..
                } => validate_plural_input(key, *required, *min_items, *max_items)?,
                InputDefinition::TextArea { .. }
                | InputDefinition::Image { .. }
                | InputDefinition::Video { .. }
                | InputDefinition::Audio { .. } => {}
            }
        }

        for binding in &recipe.bindings {
            if !recipe.inputs.contains_key(&binding.source) {
                return Err(RecipeError::invalid(format!(
                    "binding source \"{}\" is not declared in inputs",
                    binding.source
                )));
            }
            if let Some(item_index) = binding.item_index {
                let Some(definition) = recipe.inputs.get(&binding.source) else {
                    return Err(RecipeError::invalid(format!(
                        "binding \"{}\" item requires a plural media input",
                        binding.source
                    )));
                };
                let (min_items, max_items, kind) = match definition {
                    InputDefinition::Images {
                        min_items,
                        max_items,
                        ..
                    } => (*min_items, *max_items, "images"),
                    InputDefinition::Videos {
                        min_items,
                        max_items,
                        ..
                    } => (*min_items, *max_items, "videos"),
                    InputDefinition::Audios {
                        min_items,
                        max_items,
                        ..
                    } => (*min_items, *max_items, "audios"),
                    _ => {
                        return Err(RecipeError::invalid(format!(
                            "binding \"{}\" item requires a plural media input",
                            binding.source
                        )))
                    }
                };
                if item_index >= max_items || item_index >= min_items {
                    return Err(RecipeError::invalid(format!(
                        "binding \"{}\" item {} must be within the declared minimum and maximum {kind} slots",
                        binding.source, item_index,
                    )));
                }
            }
            if binding.target.node.trim().is_empty() {
                return Err(RecipeError::invalid(format!(
                    "binding \"{}\" target node must not be empty",
                    binding.source
                )));
            }
            if binding.target.input.trim().is_empty() {
                return Err(RecipeError::invalid(format!(
                    "binding \"{}\" target input must not be empty",
                    binding.source
                )));
            }
        }

        let mut output_ids = BTreeSet::new();
        for output in &recipe.outputs {
            if output.id.trim().is_empty() {
                return Err(RecipeError::invalid("output id must not be empty"));
            }
            if !output_ids.insert(&output.id) {
                return Err(RecipeError::invalid(format!(
                    "output id \"{}\" is duplicated",
                    output.id
                )));
            }
            if output.node.trim().is_empty() {
                return Err(RecipeError::invalid(format!(
                    "output \"{}\" node must not be empty",
                    output.id
                )));
            }
        }

        Ok(())
    }
}

fn validate_plural_input(
    key: &str,
    required: bool,
    min_items: usize,
    max_items: usize,
) -> Result<(), RecipeError> {
    if max_items == 0 || max_items > 32 {
        return Err(RecipeError::invalid(format!(
            "input \"{key}\" max_items must be between 1 and 32"
        )));
    }
    if min_items > max_items || (required && min_items == 0) {
        return Err(RecipeError::invalid(format!(
            "input \"{key}\" min_items must be less than or equal to max_items and required lists must have at least one item"
        )));
    }
    Ok(())
}

fn validate_unsigned_range(
    key: &str,
    min: Option<u64>,
    max: Option<u64>,
) -> Result<(), RecipeError> {
    if let (Some(min), Some(max)) = (min, max) {
        if min > max {
            return Err(RecipeError::invalid(format!(
                "input \"{key}\" min {min} must be less than or equal to max {max}"
            )));
        }
    }
    Ok(())
}

fn is_outside_unsigned_range(value: u64, min: Option<u64>, max: Option<u64>) -> bool {
    min.is_some_and(|min| value < min) || max.is_some_and(|max| value > max)
}

pub struct WorkflowValidator;

impl WorkflowValidator {
    pub fn validate(workflow: &WorkflowDocument) -> Result<(), WorkflowError> {
        let root = workflow
            .value()
            .as_object()
            .ok_or_else(|| WorkflowError::invalid("workflow root must be a JSON object"))?;

        for (node_id, node_value) in root {
            if !is_valid_node_id(node_id) {
                return Err(WorkflowError::invalid(format!(
                    "node id \"{node_id}\" must be a numeric string"
                )));
            }

            let node = node_value.as_object().ok_or_else(|| {
                WorkflowError::invalid(format!("node \"{node_id}\" must be a JSON object"))
            })?;

            let inputs = node.get("inputs").ok_or_else(|| {
                WorkflowError::invalid(format!("node \"{node_id}\" is missing inputs"))
            })?;
            if !inputs.is_object() {
                return Err(WorkflowError::invalid(format!(
                    "node \"{node_id}\" inputs must be a JSON object"
                )));
            }

            let class_type = node.get("class_type").ok_or_else(|| {
                WorkflowError::invalid(format!("node \"{node_id}\" is missing class_type"))
            })?;
            if !class_type.is_string() {
                return Err(WorkflowError::invalid(format!(
                    "node \"{node_id}\" class_type must be a string"
                )));
            }
        }

        Ok(())
    }
}

fn is_valid_node_id(node_id: &str) -> bool {
    !node_id.is_empty()
        && node_id.bytes().all(|byte| byte.is_ascii_digit())
        && node_id.parse::<u64>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::{RecipeValidator, WorkflowValidator};
    use crate::compiler::RecipeParser;
    use crate::domain::{InputDefinition, RecipeError, WorkflowDocument, WorkflowError};
    use serde_json::json;

    const RECIPE_WITH_RANGE: &str = r#"
schema_version: 1
id: range_test
name: Range Test
workflow:
  file: workflow.json
inputs:
  steps:
    type: integer
    label: Steps
    required: true
    default: 20
    min: 1
    max: 100
bindings: []
outputs: []
"#;

    const RECIPE_WITH_SEED_RANGE: &str = r#"
schema_version: 1
id: seed_range
name: Seed Range
workflow:
  file: workflow.json
inputs:
  seed:
    type: seed
    label: Seed
    default: random
    min: 10
    max: 20
bindings: []
outputs: []
"#;

    #[test]
    fn rejects_unsupported_schema() {
        let mut recipe = RecipeParser::parse(RECIPE_WITH_RANGE).expect("recipe should parse");
        recipe.schema_version = 2;

        let error = RecipeValidator::validate(&recipe).expect_err("schema must be rejected");

        assert!(matches!(error, RecipeError::UnsupportedSchema { found: 2 }));
        assert!(error.to_string().contains("only version 1"));
    }

    #[test]
    fn rejects_integer_default_outside_range() {
        let mut recipe = RecipeParser::parse(RECIPE_WITH_RANGE).expect("recipe should parse");
        if let Some(crate::domain::InputDefinition::Integer { default, .. }) =
            recipe.inputs.get_mut("steps")
        {
            *default = Some(101);
        }

        let error = RecipeValidator::validate(&recipe).expect_err("default must be rejected");

        assert!(matches!(error, RecipeError::Invalid { .. }));
        assert!(error.to_string().contains("default 101"));
    }

    #[test]
    fn rejects_min_greater_than_max() {
        let mut recipe = RecipeParser::parse(RECIPE_WITH_RANGE).expect("recipe should parse");
        if let Some(crate::domain::InputDefinition::Integer { min, max, .. }) =
            recipe.inputs.get_mut("steps")
        {
            *min = Some(100);
            *max = Some(10);
        }

        let error = RecipeValidator::validate(&recipe).expect_err("range must be rejected");

        assert!(matches!(error, RecipeError::Invalid { .. }));
    }

    #[test]
    fn rejects_invalid_integer_step_contract() {
        for replacement in [
            ("    step: 4\n", "    step: 0\n"),
            ("    step: 4\n", "    step: -4\n"),
            ("    default: 20\n", "    default: 21\n"),
        ] {
            let yaml = RECIPE_WITH_RANGE
                .replace("    max: 100\n", "    max: 100\n    step: 4\n")
                .replace(replacement.0, replacement.1);
            let recipe = RecipeParser::parse(&yaml).expect("recipe should parse");
            assert!(RecipeValidator::validate(&recipe).is_err());
        }
    }

    #[test]
    fn accepts_aligned_integer_step_contract() {
        let yaml = RECIPE_WITH_RANGE
            .replace("    min: 1\n", "    min: 4\n")
            .replace("    max: 100\n", "    max: 100\n    step: 4\n");
        let recipe = RecipeParser::parse(&yaml).expect("recipe should parse");

        RecipeValidator::validate(&recipe).expect("aligned step should be valid");
    }

    #[test]
    fn accepts_valid_seed_range() {
        let recipe = RecipeParser::parse(RECIPE_WITH_SEED_RANGE).expect("recipe should parse");

        RecipeValidator::validate(&recipe).expect("seed range should be valid");
    }

    #[test]
    fn rejects_seed_min_greater_than_max() {
        let mut recipe = RecipeParser::parse(RECIPE_WITH_SEED_RANGE).expect("recipe should parse");
        if let Some(InputDefinition::Seed { min, max, .. }) = recipe.inputs.get_mut("seed") {
            *min = Some(20);
            *max = Some(10);
        }

        let error = RecipeValidator::validate(&recipe).expect_err("seed range must be rejected");

        assert!(matches!(error, RecipeError::Invalid { .. }));
    }

    #[test]
    fn rejects_fixed_seed_default_outside_range() {
        let yaml = RECIPE_WITH_SEED_RANGE
            .replace("default: random", "default: 999")
            .replace("max: 20", "max: 100");
        let recipe = RecipeParser::parse(&yaml).expect("recipe should parse");

        let error = RecipeValidator::validate(&recipe)
            .expect_err("fixed seed default must be inside its declared range");

        assert!(matches!(error, RecipeError::Invalid { .. }));
        assert!(error.to_string().contains("default 999"));
    }

    #[test]
    fn rejects_unsafe_workflow_path() {
        let mut recipe = RecipeParser::parse(RECIPE_WITH_RANGE).expect("recipe should parse");
        recipe.workflow.file = "../workflow.json".to_owned();

        let error = RecipeValidator::validate(&recipe).expect_err("path must be rejected");

        assert!(matches!(error, RecipeError::Invalid { .. }));
        assert!(error.to_string().contains("safe relative path"));
    }

    #[test]
    fn validates_workflow_shape() {
        let workflow = WorkflowDocument::parse(json!({
            "3": {
                "inputs": {"seed": 1},
                "class_type": "KSampler"
            }
        }))
        .expect("root should parse");

        WorkflowValidator::validate(&workflow).expect("workflow should validate");
    }

    #[test]
    fn rejects_non_object_workflow_root() {
        let error = WorkflowDocument::parse(json!([])).expect_err("root must be object");

        assert!(matches!(error, WorkflowError::Invalid { .. }));
    }

    #[test]
    fn rejects_invalid_workflow_nodes() {
        let cases = [
            json!({"hello": {"inputs": {}, "class_type": "Node"}}),
            json!({"3": "not a node"}),
            json!({"3": {"class_type": "Node"}}),
            json!({"3": {"inputs": [], "class_type": "Node"}}),
            json!({"3": {"inputs": {}}}),
            json!({"3": {"inputs": {}, "class_type": 3}}),
        ];

        for value in cases {
            let workflow = WorkflowDocument::parse(value).expect("root should parse");
            assert!(WorkflowValidator::validate(&workflow).is_err());
        }
    }
}
