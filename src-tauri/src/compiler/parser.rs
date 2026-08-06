use crate::domain::{
    Binding, BindingTarget, InputDefinition, OutputDefinition, OutputType, Recipe, RecipeError,
    SeedDefault, WorkflowRef,
};
use serde::Deserialize;
use std::collections::BTreeMap;

pub struct RecipeParser;

impl RecipeParser {
    pub fn parse(yaml: &str) -> Result<Recipe, RecipeError> {
        let dto: RecipeFileDto = yaml_serde::from_str(yaml)
            .map_err(|error| RecipeError::parse(format!("invalid Recipe YAML: {error}")))?;

        dto.try_into_recipe()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecipeFileDto {
    schema_version: u32,
    id: String,
    name: String,
    workflow: WorkflowRefDto,
    inputs: BTreeMap<String, InputDefinitionDto>,
    bindings: Vec<BindingDto>,
    outputs: Vec<OutputDefinitionDto>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkflowRefDto {
    file: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum InputDefinitionDto {
    #[serde(rename = "textarea")]
    TextArea {
        label: String,
        #[serde(default)]
        required: bool,
        #[serde(default)]
        default: Option<String>,
    },
    #[serde(rename = "integer")]
    Integer {
        label: String,
        #[serde(default)]
        required: bool,
        #[serde(default)]
        default: Option<i64>,
        #[serde(default)]
        min: Option<i64>,
        #[serde(default)]
        max: Option<i64>,
    },
    #[serde(rename = "seed")]
    Seed {
        label: String,
        default: SeedDefaultDto,
        #[serde(default)]
        min: Option<u64>,
        #[serde(default)]
        max: Option<u64>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SeedDefaultDto {
    Text(String),
    Fixed(u64),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingDto {
    source: String,
    target: BindingTargetDto,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingTargetDto {
    node: String,
    input: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OutputDefinitionDto {
    id: String,
    #[serde(rename = "type")]
    output_type: OutputTypeDto,
    node: String,
    required: bool,
}

#[derive(Debug, Deserialize)]
enum OutputTypeDto {
    #[serde(rename = "image")]
    Image,
}

impl RecipeFileDto {
    fn try_into_recipe(self) -> Result<Recipe, RecipeError> {
        let inputs = self
            .inputs
            .into_iter()
            .map(|(key, definition)| {
                definition
                    .try_into_domain()
                    .map(|definition| (key, definition))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;

        let bindings = self
            .bindings
            .into_iter()
            .map(|binding| Binding {
                source: binding.source,
                target: BindingTarget {
                    node: binding.target.node,
                    input: binding.target.input,
                },
            })
            .collect();

        let outputs = self
            .outputs
            .into_iter()
            .map(|output| OutputDefinition {
                id: output.id,
                output_type: match output.output_type {
                    OutputTypeDto::Image => OutputType::Image,
                },
                node: output.node,
                required: output.required,
            })
            .collect();

        Ok(Recipe {
            schema_version: self.schema_version,
            id: self.id,
            name: self.name,
            workflow: WorkflowRef {
                file: self.workflow.file,
            },
            inputs,
            bindings,
            outputs,
        })
    }
}

impl InputDefinitionDto {
    fn try_into_domain(self) -> Result<InputDefinition, RecipeError> {
        match self {
            Self::TextArea {
                label,
                required,
                default,
            } => Ok(InputDefinition::TextArea {
                label,
                required,
                default,
            }),
            Self::Integer {
                label,
                required,
                default,
                min,
                max,
            } => Ok(InputDefinition::Integer {
                label,
                required,
                default,
                min,
                max,
            }),
            Self::Seed {
                label,
                default,
                min,
                max,
            } => Ok(InputDefinition::Seed {
                label,
                default: default.try_into_domain()?,
                min,
                max,
            }),
        }
    }
}

impl SeedDefaultDto {
    fn try_into_domain(self) -> Result<SeedDefault, RecipeError> {
        match self {
            Self::Text(value) if value.eq_ignore_ascii_case("random") => Ok(SeedDefault::Random),
            Self::Text(value) => Err(RecipeError::parse(format!(
                "unsupported seed default \"{value}\"; expected random or an unsigned integer"
            ))),
            Self::Fixed(value) => Ok(SeedDefault::Fixed(value)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RecipeParser;
    use crate::domain::{InputDefinition, RecipeError, SeedDefault};

    const VALID_RECIPE: &str = r#"
schema_version: 1
id: simple_t2i
name: Simple Text to Image
workflow:
  file: workflow_api.json
inputs:
  prompt:
    type: textarea
    label: 提示词
    required: true
    default: ""
  steps:
    type: integer
    label: Steps
    required: true
    default: 20
    min: 1
    max: 100
  seed:
    type: seed
    label: Seed
    default: random
bindings:
  - source: prompt
    target:
      node: "6"
      input: text
  - source: steps
    target:
      node: "3"
      input: steps
  - source: seed
    target:
      node: "3"
      input: seed
outputs:
  - id: generated_image
    type: image
    node: "9"
    required: true
"#;

    #[test]
    fn parses_valid_recipe_into_domain_types() {
        let recipe = RecipeParser::parse(VALID_RECIPE).expect("recipe should parse");

        assert_eq!(recipe.schema_version, 1);
        assert_eq!(recipe.workflow.file, "workflow_api.json");
        assert!(matches!(
            recipe.inputs.get("prompt"),
            Some(InputDefinition::TextArea { required: true, .. })
        ));
        assert!(matches!(
            recipe.inputs.get("seed"),
            Some(InputDefinition::Seed {
                default: SeedDefault::Random,
                ..
            })
        ));
        assert_eq!(recipe.bindings.len(), 3);
        assert_eq!(recipe.outputs.len(), 1);
    }

    #[test]
    fn parses_optional_seed_range_without_upgrading_schema() {
        let yaml = VALID_RECIPE.replace(
            "    default: random\n",
            "    default: random\n    min: 10\n    max: 20\n",
        );
        let recipe = RecipeParser::parse(&yaml).expect("recipe should parse");

        assert!(matches!(
            recipe.inputs.get("seed"),
            Some(InputDefinition::Seed {
                default: SeedDefault::Random,
                min: Some(10),
                max: Some(20),
                ..
            })
        ));
        assert_eq!(recipe.schema_version, 1);
    }

    #[test]
    fn unknown_input_type_is_a_parse_error() {
        let yaml = VALID_RECIPE.replace("type: textarea", "type: magic_slider");

        let error = RecipeParser::parse(&yaml).expect_err("unknown type must fail");

        assert!(matches!(error, RecipeError::Parse { .. }));
        assert!(error.to_string().contains("magic_slider"));
    }

    #[test]
    fn missing_required_recipe_field_is_a_parse_error() {
        let yaml = VALID_RECIPE.replace("name: Simple Text to Image\n", "");

        let error = RecipeParser::parse(&yaml).expect_err("missing name must fail");

        assert!(matches!(error, RecipeError::Parse { .. }));
    }
}
