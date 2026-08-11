use crate::compiler::{
    BindingValidator, CompileError, RecipeValidator, SeedResolver, WorkflowValidator,
};
use crate::domain::{
    CompileRequest, InputDefinition, InputValue, Recipe, ResolvedInputValue, SeedDefault,
    SeedValue, WorkflowDocument,
};
use serde_json::{Number, Value};
use std::collections::BTreeMap;

pub struct WorkflowCompiler;

#[derive(Clone, Debug, PartialEq)]
pub struct CompileResult {
    pub workflow: Value,
    pub resolved_inputs: BTreeMap<String, ResolvedInputValue>,
    pub resolved_seed: Option<u64>,
}

impl WorkflowCompiler {
    pub fn compile(
        &self,
        workflow: &WorkflowDocument,
        recipe: &Recipe,
        request: &CompileRequest,
    ) -> Result<CompileResult, CompileError> {
        // Keep this order explicit: all validation happens before cloning or
        // resolving user values, so invalid inputs cannot produce a partial result.
        RecipeValidator::validate(recipe)?;
        WorkflowValidator::validate(workflow)?;
        BindingValidator::validate(recipe, workflow)?;

        let mut compiled = workflow.clone();
        let mut resolved_inputs = BTreeMap::new();
        let mut resolved_seed = None;
        let mut seed_resolver = SeedResolver::default();

        reject_unknown_inputs(recipe, request)?;

        for (input_key, definition) in &recipe.inputs {
            match definition {
                InputDefinition::TextArea {
                    required, default, ..
                } => {
                    let value = resolve_text_input(input_key, *required, default, request)?;
                    if let Some(value) = value {
                        resolved_inputs
                            .insert(input_key.clone(), ResolvedInputValue::String(value));
                    }
                }
                InputDefinition::Integer {
                    required,
                    default,
                    min,
                    max,
                    step,
                    ..
                } => {
                    let value = resolve_integer_input(
                        input_key, *required, *default, *min, *max, *step, request,
                    )?;
                    if let Some(value) = value {
                        resolved_inputs
                            .insert(input_key.clone(), ResolvedInputValue::Integer(value));
                    }
                }
                InputDefinition::Seed {
                    default, min, max, ..
                } => {
                    let seed_value = request
                        .values
                        .get(input_key)
                        .map(|value| match value {
                            InputValue::Seed(seed) => Ok(seed.clone()),
                            other => Err(type_mismatch(input_key, "seed", other)),
                        })
                        .transpose()?
                        .unwrap_or_else(|| match default {
                            SeedDefault::Random => SeedValue::Random,
                            SeedDefault::Fixed(seed) => SeedValue::Fixed(*seed),
                        });
                    if let SeedValue::Fixed(seed) = &seed_value {
                        validate_seed_input(input_key, *seed, *min, *max)?;
                    }
                    let seed = seed_resolver.resolve(input_key, &seed_value, *min, *max);
                    resolved_seed.get_or_insert(seed);
                    resolved_inputs.insert(input_key.clone(), ResolvedInputValue::Seed(seed));
                }
                InputDefinition::Image { required, .. } => {
                    if let Some(value) = resolve_single_media_input(
                        input_key,
                        *required,
                        request,
                        "image",
                        |value| match value {
                            InputValue::Image(value) => {
                                Some((value.clone(), ResolvedInputValue::Image(value)))
                            }
                            _ => None,
                        },
                    )? {
                        resolved_inputs.insert(input_key.clone(), value);
                    }
                }
                InputDefinition::Images {
                    required,
                    min_items,
                    max_items,
                    ..
                } => {
                    if let Some(value) = resolve_plural_media_input(
                        input_key,
                        *required,
                        *min_items,
                        *max_items,
                        request,
                        "images",
                        |value| match value {
                            InputValue::Images(value) => Some(ResolvedInputValue::Images(value)),
                            _ => None,
                        },
                    )? {
                        resolved_inputs.insert(input_key.clone(), value);
                    }
                }
                InputDefinition::Video { required, .. } => {
                    if let Some(value) = resolve_single_media_input(
                        input_key,
                        *required,
                        request,
                        "video",
                        |value| match value {
                            InputValue::Video(value) => {
                                Some((value.clone(), ResolvedInputValue::Video(value)))
                            }
                            _ => None,
                        },
                    )? {
                        resolved_inputs.insert(input_key.clone(), value);
                    }
                }
                InputDefinition::Audio { required, .. } => {
                    if let Some(value) = resolve_single_media_input(
                        input_key,
                        *required,
                        request,
                        "audio",
                        |value| match value {
                            InputValue::Audio(value) => {
                                Some((value.clone(), ResolvedInputValue::Audio(value)))
                            }
                            _ => None,
                        },
                    )? {
                        resolved_inputs.insert(input_key.clone(), value);
                    }
                }
                InputDefinition::Videos {
                    required,
                    min_items,
                    max_items,
                    ..
                } => {
                    if let Some(value) = resolve_plural_media_input(
                        input_key,
                        *required,
                        *min_items,
                        *max_items,
                        request,
                        "videos",
                        |value| match value {
                            InputValue::Videos(value) => Some(ResolvedInputValue::Videos(value)),
                            _ => None,
                        },
                    )? {
                        resolved_inputs.insert(input_key.clone(), value);
                    }
                }
                InputDefinition::Audios {
                    required,
                    min_items,
                    max_items,
                    ..
                } => {
                    if let Some(value) = resolve_plural_media_input(
                        input_key,
                        *required,
                        *min_items,
                        *max_items,
                        request,
                        "audios",
                        |value| match value {
                            InputValue::Audios(value) => Some(ResolvedInputValue::Audios(value)),
                            _ => None,
                        },
                    )? {
                        resolved_inputs.insert(input_key.clone(), value);
                    }
                }
            }
        }

        apply_bindings(&mut compiled, recipe, &resolved_inputs)?;
        WorkflowValidator::validate(&compiled)?;

        Ok(CompileResult {
            workflow: compiled.into_value(),
            resolved_inputs,
            resolved_seed,
        })
    }
}

fn reject_unknown_inputs(recipe: &Recipe, request: &CompileRequest) -> Result<(), CompileError> {
    for input_key in request.values.keys() {
        if !recipe.inputs.contains_key(input_key) {
            return Err(CompileError::UnknownInput {
                input: input_key.clone(),
            });
        }
    }

    Ok(())
}

fn resolve_text_input(
    input_key: &str,
    required: bool,
    default: &Option<String>,
    request: &CompileRequest,
) -> Result<Option<String>, CompileError> {
    let value = request
        .values
        .get(input_key)
        .map(|value| match value {
            InputValue::String(value) => Ok(value.clone()),
            other => Err(type_mismatch(input_key, "textarea", other)),
        })
        .transpose()?
        .or_else(|| default.clone());

    if required && value.as_deref().is_none_or(|value| value.trim().is_empty()) {
        return Err(CompileError::InputRequired {
            input: input_key.to_owned(),
        });
    }

    Ok(value)
}

fn resolve_integer_input(
    input_key: &str,
    required: bool,
    default: Option<i64>,
    min: Option<i64>,
    max: Option<i64>,
    step: Option<i64>,
    request: &CompileRequest,
) -> Result<Option<i64>, CompileError> {
    let value = request
        .values
        .get(input_key)
        .map(|value| match value {
            InputValue::Integer(value) => Ok(*value),
            other => Err(type_mismatch(input_key, "integer", other)),
        })
        .transpose()?
        .or(default);

    let Some(value) = value else {
        if required {
            return Err(CompileError::InputRequired {
                input: input_key.to_owned(),
            });
        }
        return Ok(None);
    };

    if min.is_some_and(|min| value < min) || max.is_some_and(|max| value > max) {
        return Err(CompileError::InputOutOfRange {
            input: input_key.to_owned(),
            value,
            min,
            max,
        });
    }

    if step.is_some_and(|step| value % step != 0) {
        return Err(CompileError::InputStepMismatch {
            input: input_key.to_owned(),
            value,
            step: step.expect("step checked above"),
        });
    }

    Ok(Some(value))
}

fn resolve_single_media_input<F>(
    input_key: &str,
    required: bool,
    request: &CompileRequest,
    expected: &str,
    resolve: F,
) -> Result<Option<ResolvedInputValue>, CompileError>
where
    F: FnOnce(InputValue) -> Option<(String, ResolvedInputValue)>,
{
    let value = request.values.get(input_key).cloned();
    let Some(value) = value else {
        if required {
            return Err(CompileError::InputRequired {
                input: input_key.to_owned(),
            });
        }
        return Ok(None);
    };
    let actual = value.clone();
    let Some((value, resolved)) = resolve(value) else {
        return Err(type_mismatch(input_key, expected, &actual));
    };
    let value = (!value.trim().is_empty()).then_some(resolved);
    if value.is_none() && required {
        return Err(CompileError::InputRequired {
            input: input_key.to_owned(),
        });
    }
    Ok(value)
}

fn resolve_plural_media_input<F>(
    input_key: &str,
    required: bool,
    min: usize,
    max: usize,
    request: &CompileRequest,
    expected: &str,
    resolve: F,
) -> Result<Option<ResolvedInputValue>, CompileError>
where
    F: FnOnce(InputValue) -> Option<ResolvedInputValue>,
{
    let Some(value) = request.values.get(input_key).cloned() else {
        if required {
            return Err(CompileError::InputRequired {
                input: input_key.to_owned(),
            });
        }
        return Ok(None);
    };
    let actual = value.clone();
    let Some(resolved) = resolve(value) else {
        return Err(type_mismatch(input_key, expected, &actual));
    };
    let count = match &resolved {
        ResolvedInputValue::Images(values)
        | ResolvedInputValue::Videos(values)
        | ResolvedInputValue::Audios(values) => values.len(),
        _ => 0,
    };
    if required && count < min {
        return Err(CompileError::InputRequired {
            input: input_key.to_owned(),
        });
    }
    if (count > 0 && count < min) || count > max {
        if required && count < min {
            return Err(CompileError::InputRequired {
                input: input_key.to_owned(),
            });
        }
        return Err(CompileError::InputCountOutOfRange {
            input: input_key.to_owned(),
            count,
            min,
            max,
        });
    }
    Ok((count > 0).then_some(resolved))
}

fn validate_seed_input(
    input_key: &str,
    value: u64,
    min: Option<u64>,
    max: Option<u64>,
) -> Result<(), CompileError> {
    if min.is_some_and(|min| value < min) || max.is_some_and(|max| value > max) {
        return Err(CompileError::SeedOutOfRange {
            input: input_key.to_owned(),
            value,
            min,
            max,
        });
    }

    Ok(())
}

fn type_mismatch(input_key: &str, expected: &str, value: &InputValue) -> CompileError {
    CompileError::InputTypeMismatch {
        input: input_key.to_owned(),
        expected: expected.to_owned(),
        actual: match value {
            InputValue::String(_) => "string",
            InputValue::Integer(_) => "integer",
            InputValue::Seed(_) => "seed",
            InputValue::Image(_) => "image",
            InputValue::Images(_) => "images",
            InputValue::Video(_) => "video",
            InputValue::Audio(_) => "audio",
            InputValue::Videos(_) => "videos",
            InputValue::Audios(_) => "audios",
        }
        .to_owned(),
    }
}

fn apply_bindings(
    workflow: &mut WorkflowDocument,
    recipe: &Recipe,
    resolved_inputs: &BTreeMap<String, ResolvedInputValue>,
) -> Result<(), CompileError> {
    for binding in &recipe.bindings {
        let Some(value) = resolved_inputs.get(&binding.source) else {
            // Optional inputs without a user value or recipe default preserve
            // the original Workflow value, as required by the precedence rule.
            continue;
        };

        let Some(inputs) = workflow.inputs_mut(&binding.target.node) else {
            return Err(CompileError::Internal {
                message: format!(
                    "validated binding target node \"{}\" became inaccessible",
                    binding.target.node
                ),
            });
        };

        let target_value = match (binding.item_index, value) {
            (Some(index), ResolvedInputValue::Images(values))
            | (Some(index), ResolvedInputValue::Videos(values))
            | (Some(index), ResolvedInputValue::Audios(values)) => values
                .get(index)
                .cloned()
                .map(Value::String)
                .ok_or_else(|| CompileError::Internal {
                    message: format!(
                        "validated media binding item {} became unavailable for {}",
                        index, binding.source
                    ),
                })?,
            (Some(_), _) => {
                return Err(CompileError::Internal {
                    message: format!(
                        "binding {} declared a media item but resolved a non-list value",
                        binding.source
                    ),
                })
            }
            (None, value) => resolved_value_to_json(value),
        };
        inputs.insert(binding.target.input.clone(), target_value);
    }

    Ok(())
}

fn resolved_value_to_json(value: &ResolvedInputValue) -> Value {
    match value {
        ResolvedInputValue::String(value) => Value::String(value.clone()),
        ResolvedInputValue::Integer(value) => Value::Number(Number::from(*value)),
        ResolvedInputValue::Seed(value) => Value::Number(Number::from(*value)),
        ResolvedInputValue::Image(value) => Value::String(value.clone()),
        ResolvedInputValue::Images(values) => {
            Value::Array(values.iter().cloned().map(Value::String).collect())
        }
        ResolvedInputValue::Video(value) | ResolvedInputValue::Audio(value) => {
            Value::String(value.clone())
        }
        ResolvedInputValue::Videos(values) | ResolvedInputValue::Audios(values) => {
            Value::Array(values.iter().cloned().map(Value::String).collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WorkflowCompiler;
    use crate::compiler::{CompileError, RecipeParser};
    use crate::domain::{
        CompileRequest, InputDefinition, InputValue, ResolvedInputValue, SeedValue,
        WorkflowDocument,
    };
    use serde_json::Value;
    use std::collections::BTreeMap;

    const RECIPE_YAML: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/simple_t2i/recipe.yaml"
    ));
    const WORKFLOW_JSON: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/simple_t2i/workflow_api.json"
    ));
    const EXPECTED_FIXED_JSON: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/simple_t2i/expected_compiled_fixed_seed.json"
    ));
    const RANGE_RECIPE_YAML: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/seed_range_t2i/recipe.yaml"
    ));
    const RANGE_WORKFLOW_JSON: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/seed_range_t2i/workflow_api.json"
    ));
    const I2I_RECIPE_YAML: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/simple_i2i/recipe.yaml"
    ));
    const I2I_WORKFLOW_JSON: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/simple_i2i/workflow_api.json"
    ));

    const MULTI_RECIPE_YAML: &str = r#"
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
    target:
      node: "9"
      input: images
  - source: references
    item: 1
    target:
      node: "10"
      input: image
outputs: []
"#;

    const KREA2_RESOLUTION_RECIPE_YAML: &str = r#"
schema_version: 1
id: kera2_resolution
name: Kera2 Resolution
workflow:
  file: workflow.json
inputs:
  prompt:
    type: textarea
    label: Prompt
    required: true
    default: ""
  width:
    type: integer
    label: Width
    required: true
    default: 1024
    min: 16
    max: 2048
    step: 8
  height:
    type: integer
    label: Height
    required: true
    default: 1024
    min: 16
    max: 2048
    step: 8
  seed:
    type: seed
    label: Seed
    default: random
bindings:
  - source: prompt
    target:
      node: "13"
      input: text
  - source: width
    target:
      node: "10"
      input: width
  - source: height
    target:
      node: "10"
      input: height
  - source: seed
    target:
      node: "2"
      input: seed
outputs:
  - id: generated_image
    type: image
    node: "11"
    required: true
"#;

    const KREA2_RESOLUTION_WORKFLOW_JSON: &str = r#"
{
  "2": {"inputs": {"seed": 1}, "class_type": "Seed"},
  "10": {"inputs": {"width": 768, "height": 1280}, "class_type": "EmptyLatentImage"},
  "11": {"inputs": {"images": ["10", 0]}, "class_type": "SaveImage"},
  "13": {"inputs": {"text": "original"}, "class_type": "CLIPTextEncode"}
}
"#;

    const H3_RESOLUTION_RECIPE_YAML: &str = r#"
schema_version: 1
id: minimax_h3_resolution
name: MiniMax H3 Resolution
workflow:
  file: workflow.json
inputs:
  duration_seconds:
    type: integer
    label: Duration
    required: true
    default: 5
    min: 1
    max: 15
    step: 1
  prompt:
    type: textarea
    label: Prompt
    required: true
    default: ""
  reference_image:
    type: image
    label: Reference Image
    required: true
  width:
    type: integer
    label: Width
    required: true
    default: 1344
    min: 32
    max: 2048
    step: 32
  height:
    type: integer
    label: Height
    required: true
    default: 768
    min: 32
    max: 2048
    step: 32
  seed:
    type: seed
    label: Seed
    default: random
bindings:
  - source: duration_seconds
    target:
      node: "22"
      input: value
  - source: prompt
    target:
      node: "14"
      input: prompt
  - source: reference_image
    target:
      node: "24"
      input: image
  - source: width
    target:
      node: "14"
      input: width
  - source: height
    target:
      node: "14"
      input: height
  - source: seed
    target:
      node: "15"
      input: noise_seed
outputs:
  - id: generated_video
    type: video
    node: "21"
    required: true
"#;

    const H3_RESOLUTION_WORKFLOW_JSON: &str = r#"
{
  "14": {"inputs": {"prompt": "original", "width": 1344, "height": 768, "length": 124, "ref_image": "original"}, "class_type": "MiniMaxH3ReferenceToVideo"},
  "15": {"inputs": {"noise_seed": 1}, "class_type": "RandomNoise"},
  "21": {"inputs": {"images": ["14", 0]}, "class_type": "SaveVideo"},
  "22": {"inputs": {"value": 5}, "class_type": "PrimitiveFloat"},
  "24": {"inputs": {"image": "original"}, "class_type": "LoadImage"}
}
"#;

    fn recipe_and_workflow() -> (crate::domain::Recipe, WorkflowDocument) {
        let recipe = RecipeParser::parse(RECIPE_YAML).expect("recipe should parse");
        let workflow_value: Value = serde_json::from_str(WORKFLOW_JSON).expect("JSON fixture");
        let workflow = WorkflowDocument::parse(workflow_value).expect("workflow should parse");
        (recipe, workflow)
    }

    fn fixed_request() -> CompileRequest {
        CompileRequest::new(BTreeMap::from([
            (
                "prompt".to_owned(),
                InputValue::String("TEST_PROMPT".to_owned()),
            ),
            ("steps".to_owned(), InputValue::Integer(37)),
            (
                "seed".to_owned(),
                InputValue::Seed(SeedValue::Fixed(123_456_789)),
            ),
        ]))
    }

    fn range_recipe_and_workflow() -> (crate::domain::Recipe, WorkflowDocument) {
        let recipe = RecipeParser::parse(RANGE_RECIPE_YAML).expect("range recipe should parse");
        let workflow_value: Value =
            serde_json::from_str(RANGE_WORKFLOW_JSON).expect("range workflow JSON");
        let workflow = WorkflowDocument::parse(workflow_value).expect("workflow should parse");
        (recipe, workflow)
    }

    fn seed_request(seed: SeedValue) -> CompileRequest {
        CompileRequest::new(BTreeMap::from([(
            "seed".to_owned(),
            InputValue::Seed(seed),
        )]))
    }

    #[test]
    fn multi_image_list_and_item_bindings_preserve_order() {
        let recipe = RecipeParser::parse(MULTI_RECIPE_YAML).expect("multi recipe should parse");
        let workflow = WorkflowDocument::parse(serde_json::json!({
            "9": {"inputs": {"images": "original-list"}, "class_type": "SaveImage"},
            "10": {"inputs": {"image": "original-image"}, "class_type": "LoadImage"}
        }))
        .expect("workflow should parse");
        let result = WorkflowCompiler
            .compile(
                &workflow,
                &recipe,
                &CompileRequest::new(BTreeMap::from([(
                    "references".to_owned(),
                    InputValue::Images(vec!["first.png".to_owned(), "second.png".to_owned()]),
                )])),
            )
            .expect("multi image compile should succeed");

        assert_eq!(
            result.workflow["9"]["inputs"]["images"],
            serde_json::json!(["first.png", "second.png"])
        );
        assert_eq!(result.workflow["10"]["inputs"]["image"], "second.png");
        assert_eq!(
            result.resolved_inputs.get("references"),
            Some(&ResolvedInputValue::Images(vec![
                "first.png".to_owned(),
                "second.png".to_owned()
            ]))
        );
    }

    #[test]
    fn media_list_and_item_bindings_preserve_order() {
        let recipe = RecipeParser::parse(
            r#"
schema_version: 1
id: media_order
name: Media Order
workflow:
  file: workflow.json
inputs:
  videos:
    type: videos
    label: Videos
    required: true
    min_items: 2
    max_items: 3
  audios:
    type: audios
    label: Audios
    required: false
    min_items: 0
    max_items: 3
bindings:
  - source: videos
    target:
      node: "9"
      input: videos
  - source: videos
    item: 1
    target:
      node: "10"
      input: video
  - source: audios
    target:
      node: "11"
      input: audios
outputs: []
"#,
        )
        .expect("media recipe should parse");
        let workflow = WorkflowDocument::parse(serde_json::json!({
            "9": {"inputs": {"videos": "original"}, "class_type": "VideoList"},
            "10": {"inputs": {"video": "original"}, "class_type": "VideoItem"},
            "11": {"inputs": {"audios": "original"}, "class_type": "AudioList"}
        }))
        .expect("workflow should parse");
        let result = WorkflowCompiler
            .compile(
                &workflow,
                &recipe,
                &CompileRequest::new(BTreeMap::from([
                    (
                        "videos".to_owned(),
                        InputValue::Videos(vec![
                            "video-a.mp4".to_owned(),
                            "video-b.mp4".to_owned(),
                        ]),
                    ),
                    (
                        "audios".to_owned(),
                        InputValue::Audios(vec!["audio-a.wav".to_owned()]),
                    ),
                ])),
            )
            .expect("media compile should succeed");
        assert_eq!(
            result.workflow["9"]["inputs"]["videos"],
            serde_json::json!(["video-a.mp4", "video-b.mp4"])
        );
        assert_eq!(result.workflow["10"]["inputs"]["video"], "video-b.mp4");
        assert_eq!(
            result.workflow["11"]["inputs"]["audios"],
            serde_json::json!(["audio-a.wav"])
        );
        assert_eq!(
            result.resolved_inputs["videos"],
            ResolvedInputValue::Videos(vec!["video-a.mp4".to_owned(), "video-b.mp4".to_owned()])
        );
        assert_eq!(
            result.resolved_inputs["audios"],
            ResolvedInputValue::Audios(vec!["audio-a.wav".to_owned()])
        );
    }

    #[test]
    fn prepared_image_string_binds_without_mutating_raw_workflow() {
        let recipe = RecipeParser::parse(I2I_RECIPE_YAML).expect("i2i recipe should parse");
        let workflow_value: Value = serde_json::from_str(I2I_WORKFLOW_JSON).unwrap();
        let workflow = WorkflowDocument::parse(workflow_value.clone()).unwrap();
        let result = WorkflowCompiler
            .compile(
                &workflow,
                &recipe,
                &CompileRequest::new(BTreeMap::from([
                    ("prompt".to_owned(), InputValue::String("hello".to_owned())),
                    (
                        "reference_image".to_owned(),
                        InputValue::Image("server_returned.png".to_owned()),
                    ),
                    ("steps".to_owned(), InputValue::Integer(20)),
                    ("seed".to_owned(), InputValue::Seed(SeedValue::Fixed(123))),
                ])),
            )
            .expect("prepared image should compile");

        assert_eq!(
            result.workflow["10"]["inputs"]["image"],
            "server_returned.png"
        );
        assert_eq!(workflow.value(), &workflow_value);
        assert_eq!(
            result.resolved_inputs.get("reference_image"),
            Some(&ResolvedInputValue::Image("server_returned.png".to_owned()))
        );
    }

    #[test]
    fn fixed_seed_compile_matches_golden_and_preserves_unbound_fields() {
        let (recipe, workflow) = recipe_and_workflow();
        let original = workflow.clone();

        let result = WorkflowCompiler
            .compile(&workflow, &recipe, &fixed_request())
            .expect("fixed compile should succeed");
        let expected: Value =
            serde_json::from_str(EXPECTED_FIXED_JSON).expect("golden fixture should parse");

        assert_eq!(result.workflow, expected);
        assert_eq!(workflow, original);
        assert_eq!(result.resolved_seed, Some(123_456_789));
        assert_eq!(
            result.resolved_inputs.get("seed"),
            Some(&ResolvedInputValue::Seed(123_456_789))
        );
    }

    #[test]
    fn random_seed_is_resolved_once_and_applied_consistently() {
        let (recipe, workflow) = recipe_and_workflow();
        let request = CompileRequest::new(BTreeMap::from([
            (
                "prompt".to_owned(),
                InputValue::String("RANDOM_PROMPT".to_owned()),
            ),
            ("steps".to_owned(), InputValue::Integer(20)),
        ]));

        let result = WorkflowCompiler
            .compile(&workflow, &recipe, &request)
            .expect("random compile should succeed");
        let resolved_seed = result.resolved_seed.expect("seed should resolve");

        assert_eq!(
            result.resolved_inputs.get("seed"),
            Some(&ResolvedInputValue::Seed(resolved_seed))
        );
        assert_eq!(
            result.workflow["3"]["inputs"]["seed"],
            Value::from(resolved_seed)
        );
    }

    #[test]
    fn seed_range_accepts_fixed_boundaries_and_random_values() {
        for seed in [10, 20] {
            let (recipe, workflow) = range_recipe_and_workflow();
            let result = WorkflowCompiler
                .compile(&workflow, &recipe, &seed_request(SeedValue::Fixed(seed)))
                .expect("range boundary should compile");
            assert_eq!(result.resolved_seed, Some(seed));
        }

        let (recipe, workflow) = range_recipe_and_workflow();
        let result = WorkflowCompiler
            .compile(&workflow, &recipe, &seed_request(SeedValue::Random))
            .expect("random range should compile");
        let seed = result.resolved_seed.expect("random seed should resolve");
        assert!((10..=20).contains(&seed));
    }

    #[test]
    fn fixed_seed_outside_recipe_range_returns_unsigned_error() {
        for seed in [9, 21] {
            let (recipe, workflow) = range_recipe_and_workflow();
            let error = WorkflowCompiler
                .compile(&workflow, &recipe, &seed_request(SeedValue::Fixed(seed)))
                .expect_err("out-of-range seed must fail");

            assert!(matches!(
                error,
                CompileError::SeedOutOfRange {
                    value,
                    min: Some(10),
                    max: Some(20),
                    ..
                } if value == seed
            ));
            assert_eq!(error.code(), "SEED_OUT_OF_RANGE");
        }
    }

    #[test]
    fn unbounded_recipe_preserves_u64_seed_values() {
        let (recipe, workflow) = recipe_and_workflow();
        let mut request = fixed_request();
        request.values.insert(
            "seed".to_owned(),
            InputValue::Seed(SeedValue::Fixed(u64::MAX)),
        );
        let result = WorkflowCompiler
            .compile(&workflow, &recipe, &request)
            .expect("unbounded seed should accept u64::MAX");

        assert_eq!(result.resolved_seed, Some(u64::MAX));
        assert_eq!(
            result.workflow["3"]["inputs"]["seed"],
            Value::from(u64::MAX)
        );
    }

    #[test]
    fn rejects_unknown_user_input() {
        let (recipe, workflow) = recipe_and_workflow();
        let mut values = fixed_request().values;
        values.insert("stepps".to_owned(), InputValue::Integer(20));

        let error = WorkflowCompiler
            .compile(&workflow, &recipe, &CompileRequest::new(values))
            .expect_err("unknown input must fail");

        assert!(matches!(error, CompileError::UnknownInput { .. }));
        assert!(error.to_string().contains("stepps"));
    }

    #[test]
    fn rejects_required_input_without_value_or_default() {
        let (mut recipe, workflow) = recipe_and_workflow();
        if let Some(InputDefinition::TextArea { default, .. }) = recipe.inputs.get_mut("prompt") {
            *default = None;
        }
        let request = CompileRequest::new(BTreeMap::from([
            ("steps".to_owned(), InputValue::Integer(20)),
            ("seed".to_owned(), InputValue::Seed(SeedValue::Fixed(1))),
        ]));

        let error = WorkflowCompiler
            .compile(&workflow, &recipe, &request)
            .expect_err("required input must fail");

        assert!(matches!(error, CompileError::InputRequired { .. }));
        assert!(error.to_string().contains("prompt"));
    }

    #[test]
    fn enforces_integer_range_boundaries() {
        for steps in [1, 100] {
            let (recipe, workflow) = recipe_and_workflow();
            let mut request = fixed_request();
            request
                .values
                .insert("steps".to_owned(), InputValue::Integer(steps));
            assert!(WorkflowCompiler
                .compile(&workflow, &recipe, &request)
                .is_ok());
        }

        for steps in [0, 101] {
            let (recipe, workflow) = recipe_and_workflow();
            let mut request = fixed_request();
            request
                .values
                .insert("steps".to_owned(), InputValue::Integer(steps));
            let error = WorkflowCompiler
                .compile(&workflow, &recipe, &request)
                .expect_err("out-of-range value must fail");
            assert!(matches!(error, CompileError::InputOutOfRange { .. }));
        }
    }

    #[test]
    fn compiles_krea2_resolution_inputs_without_rounding() {
        let recipe =
            RecipeParser::parse(KREA2_RESOLUTION_RECIPE_YAML).expect("recipe should parse");
        let workflow = WorkflowDocument::parse(
            serde_json::from_str(KREA2_RESOLUTION_WORKFLOW_JSON).expect("workflow should parse"),
        )
        .expect("workflow should validate");
        let request = CompileRequest::new(BTreeMap::from([
            (
                "prompt".to_owned(),
                InputValue::String("portrait".to_owned()),
            ),
            ("width".to_owned(), InputValue::Integer(1280)),
            ("height".to_owned(), InputValue::Integer(720)),
            ("seed".to_owned(), InputValue::Seed(SeedValue::Fixed(42))),
        ]));

        let result = WorkflowCompiler
            .compile(&workflow, &recipe, &request)
            .expect("Krea2 resolution should compile");
        assert_eq!(result.workflow["10"]["inputs"]["width"], 1280);
        assert_eq!(result.workflow["10"]["inputs"]["height"], 720);

        let invalid = CompileRequest::new(BTreeMap::from([
            (
                "prompt".to_owned(),
                InputValue::String("portrait".to_owned()),
            ),
            ("width".to_owned(), InputValue::Integer(1270)),
            ("height".to_owned(), InputValue::Integer(720)),
            ("seed".to_owned(), InputValue::Seed(SeedValue::Fixed(42))),
        ]));
        let error = WorkflowCompiler
            .compile(&workflow, &recipe, &invalid)
            .expect_err("misaligned Krea2 width must fail");
        assert!(matches!(error, CompileError::InputStepMismatch { .. }));
    }

    #[test]
    fn compiles_h3_duration_boundaries_and_resolution() {
        let recipe = RecipeParser::parse(H3_RESOLUTION_RECIPE_YAML).expect("recipe should parse");
        let workflow = WorkflowDocument::parse(
            serde_json::from_str(H3_RESOLUTION_WORKFLOW_JSON).expect("workflow should parse"),
        )
        .expect("workflow should validate");

        for duration in [1, 5, 10, 15] {
            let request = CompileRequest::new(BTreeMap::from([
                ("duration_seconds".to_owned(), InputValue::Integer(duration)),
                ("prompt".to_owned(), InputValue::String("motion".to_owned())),
                (
                    "reference_image".to_owned(),
                    InputValue::Image("ref.png".to_owned()),
                ),
                ("width".to_owned(), InputValue::Integer(1344)),
                ("height".to_owned(), InputValue::Integer(768)),
                ("seed".to_owned(), InputValue::Seed(SeedValue::Fixed(7))),
            ]));
            let result = WorkflowCompiler
                .compile(&workflow, &recipe, &request)
                .expect("H3 duration should compile");
            assert_eq!(result.workflow["22"]["inputs"]["value"], duration);
            assert_eq!(result.workflow["14"]["inputs"]["width"], 1344);
            assert_eq!(result.workflow["14"]["inputs"]["height"], 768);
        }
    }

    #[test]
    fn rejects_input_type_mismatch() {
        let (recipe, workflow) = recipe_and_workflow();
        let mut request = fixed_request();
        request
            .values
            .insert("steps".to_owned(), InputValue::String("hello".to_owned()));

        let error = WorkflowCompiler
            .compile(&workflow, &recipe, &request)
            .expect_err("type mismatch must fail");

        assert!(matches!(error, CompileError::InputTypeMismatch { .. }));
    }
}
