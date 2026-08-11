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
        #[serde(default)]
        step: Option<i64>,
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
    #[serde(rename = "image")]
    Image {
        label: String,
        #[serde(default)]
        required: bool,
    },
    #[serde(rename = "images")]
    Images {
        label: String,
        #[serde(default)]
        required: bool,
        #[serde(default, rename = "min_items")]
        min_items: usize,
        #[serde(default = "default_max_items", rename = "max_items")]
        max_items: usize,
    },
    #[serde(rename = "video")]
    Video {
        label: String,
        #[serde(default)]
        required: bool,
    },
    #[serde(rename = "audio")]
    Audio {
        label: String,
        #[serde(default)]
        required: bool,
    },
    #[serde(rename = "videos")]
    Videos {
        label: String,
        #[serde(default)]
        required: bool,
        #[serde(default, rename = "min_items")]
        min_items: usize,
        #[serde(default = "default_max_items", rename = "max_items")]
        max_items: usize,
    },
    #[serde(rename = "audios")]
    Audios {
        label: String,
        #[serde(default)]
        required: bool,
        #[serde(default, rename = "min_items")]
        min_items: usize,
        #[serde(default = "default_max_items", rename = "max_items")]
        max_items: usize,
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
    #[serde(default, rename = "item")]
    item_index: Option<usize>,
    target: BindingTargetDto,
    #[serde(default, rename = "clear_targets")]
    clear_targets: Vec<BindingTargetDto>,
}

fn default_max_items() -> usize {
    8
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
    #[serde(rename = "video")]
    Video,
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
                item_index: binding.item_index,
                target: BindingTarget {
                    node: binding.target.node,
                    input: binding.target.input,
                },
                clear_targets: binding
                    .clear_targets
                    .into_iter()
                    .map(|target| BindingTarget {
                        node: target.node,
                        input: target.input,
                    })
                    .collect(),
            })
            .collect();

        let outputs = self
            .outputs
            .into_iter()
            .map(|output| OutputDefinition {
                id: output.id,
                output_type: match output.output_type {
                    OutputTypeDto::Image => OutputType::Image,
                    OutputTypeDto::Video => OutputType::Video,
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
                step,
            } => Ok(InputDefinition::Integer {
                label,
                required,
                default,
                min,
                max,
                step,
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
            Self::Image { label, required } => Ok(InputDefinition::Image { label, required }),
            Self::Images {
                label,
                required,
                min_items,
                max_items,
            } => Ok(InputDefinition::Images {
                label,
                required,
                min_items,
                max_items,
            }),
            Self::Video { label, required } => Ok(InputDefinition::Video { label, required }),
            Self::Audio { label, required } => Ok(InputDefinition::Audio { label, required }),
            Self::Videos {
                label,
                required,
                min_items,
                max_items,
            } => Ok(InputDefinition::Videos {
                label,
                required,
                min_items,
                max_items,
            }),
            Self::Audios {
                label,
                required,
                min_items,
                max_items,
            } => Ok(InputDefinition::Audios {
                label,
                required,
                min_items,
                max_items,
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
    use crate::domain::{InputDefinition, OutputType, RecipeError, SeedDefault};

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
    fn parses_integer_step_into_domain_types() {
        let yaml = VALID_RECIPE.replace("    max: 100\n", "    max: 100\n    step: 4\n");
        let recipe = RecipeParser::parse(&yaml).expect("recipe should parse");

        assert!(matches!(
            recipe.inputs.get("steps"),
            Some(InputDefinition::Integer { step: Some(4), .. })
        ));
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
    fn parses_image_input_in_schema_v1() {
        let yaml = VALID_RECIPE.replace(
            "  steps:\n",
            "  reference_image:\n    type: image\n    label: Reference Image\n    required: true\n  steps:\n",
        );
        let recipe = RecipeParser::parse(&yaml).expect("image recipe should parse");
        assert!(matches!(
            recipe.inputs.get("reference_image"),
            Some(InputDefinition::Image { required: true, .. })
        ));
        assert_eq!(recipe.schema_version, 1);
    }

    #[test]
    fn parses_ordered_images_input_and_item_binding() {
        let yaml = r#"
schema_version: 1
id: multi_image
name: Multi Image
workflow:
  file: workflow.json
inputs:
  references:
    type: images
    label: References
    required: true
    min_items: 2
    max_items: 4
bindings:
  - source: references
    item: 1
    target:
      node: "10"
      input: image
outputs: []
"#;
        let recipe = RecipeParser::parse(yaml).expect("multi-image recipe should parse");
        assert!(matches!(
            recipe.inputs.get("references"),
            Some(InputDefinition::Images {
                required: true,
                min_items: 2,
                max_items: 4,
                ..
            })
        ));
        assert_eq!(recipe.bindings[0].item_index, Some(1));
    }

    #[test]
    fn parses_video_audio_and_ordered_plural_media_inputs() {
        let yaml = r#"
schema_version: 1
id: media_inputs
name: Media Inputs
workflow:
  file: workflow.json
inputs:
  reference_video:
    type: video
    label: Reference Video
    required: true
  reference_audio:
    type: audio
    label: Reference Audio
    required: false
  reference_videos:
    type: videos
    label: Reference Videos
    required: false
    min_items: 0
    max_items: 3
  reference_audios:
    type: audios
    label: Reference Audios
    required: false
    min_items: 0
    max_items: 3
bindings:
  - source: reference_videos
    item: 0
    target:
      node: "10"
      input: video
outputs: []
"#;
        let recipe = RecipeParser::parse(yaml).expect("media recipe should parse");
        assert!(matches!(
            recipe.inputs.get("reference_video"),
            Some(InputDefinition::Video { required: true, .. })
        ));
        assert!(matches!(
            recipe.inputs.get("reference_audio"),
            Some(InputDefinition::Audio {
                required: false,
                ..
            })
        ));
        assert!(matches!(
            recipe.inputs.get("reference_videos"),
            Some(InputDefinition::Videos {
                min_items: 0,
                max_items: 3,
                ..
            })
        ));
        assert!(matches!(
            recipe.inputs.get("reference_audios"),
            Some(InputDefinition::Audios {
                min_items: 0,
                max_items: 3,
                ..
            })
        ));
        assert_eq!(recipe.bindings[0].item_index, Some(0));
    }

    #[test]
    fn parses_optional_binding_clear_targets() {
        let yaml = VALID_RECIPE.replace(
            "  - source: prompt\n    target:\n      node: \"6\"\n      input: text",
            "  - source: prompt\n    target:\n      node: \"6\"\n      input: text\n    clear_targets:\n      - node: \"14\"\n        input: first_frame",
        );
        let recipe = RecipeParser::parse(&yaml).expect("recipe with clear targets should parse");
        assert_eq!(recipe.bindings[0].clear_targets.len(), 1);
        assert_eq!(recipe.bindings[0].clear_targets[0].node, "14");
        assert_eq!(recipe.bindings[0].clear_targets[0].input, "first_frame");
    }

    #[test]
    fn parses_video_output_type_without_workflow_specific_conditionals() {
        let yaml = VALID_RECIPE.replace("type: image", "type: video");
        let recipe = RecipeParser::parse(&yaml).expect("video recipe should parse");
        assert_eq!(recipe.outputs[0].output_type, OutputType::Video);
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
