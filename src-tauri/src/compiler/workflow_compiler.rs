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
                    ..
                } => {
                    let value =
                        resolve_integer_input(input_key, *required, *default, *min, *max, request)?;
                    if let Some(value) = value {
                        resolved_inputs
                            .insert(input_key.clone(), ResolvedInputValue::Integer(value));
                    }
                }
                InputDefinition::Seed { default, .. } => {
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
                    let seed = seed_resolver.resolve(&seed_value);
                    resolved_seed.get_or_insert(seed);
                    resolved_inputs.insert(input_key.clone(), ResolvedInputValue::Seed(seed));
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

    Ok(Some(value))
}

fn type_mismatch(input_key: &str, expected: &str, value: &InputValue) -> CompileError {
    CompileError::InputTypeMismatch {
        input: input_key.to_owned(),
        expected: expected.to_owned(),
        actual: match value {
            InputValue::String(_) => "string",
            InputValue::Integer(_) => "integer",
            InputValue::Seed(_) => "seed",
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

        inputs.insert(binding.target.input.clone(), resolved_value_to_json(value));
    }

    Ok(())
}

fn resolved_value_to_json(value: &ResolvedInputValue) -> Value {
    match value {
        ResolvedInputValue::String(value) => Value::String(value.clone()),
        ResolvedInputValue::Integer(value) => Value::Number(Number::from(*value)),
        ResolvedInputValue::Seed(value) => Value::Number(Number::from(*value)),
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
