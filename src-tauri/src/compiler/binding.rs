use crate::compiler::CompileError;
use crate::domain::{Recipe, WorkflowDocument};

pub struct BindingValidator;

impl BindingValidator {
    pub fn validate(recipe: &Recipe, workflow: &WorkflowDocument) -> Result<(), CompileError> {
        for binding in &recipe.bindings {
            if !recipe.inputs.contains_key(&binding.source) {
                return Err(CompileError::BindingInvalid {
                    source: binding.source.clone(),
                    node: binding.target.node.clone(),
                    input: binding.target.input.clone(),
                    message: "source is not declared in recipe inputs".to_owned(),
                });
            }

            if workflow.node(&binding.target.node).is_none() {
                return Err(CompileError::BindingInvalid {
                    source: binding.source.clone(),
                    node: binding.target.node.clone(),
                    input: binding.target.input.clone(),
                    message: "target node does not exist in workflow".to_owned(),
                });
            }

            let Some(inputs) = workflow.inputs(&binding.target.node) else {
                return Err(CompileError::BindingInvalid {
                    source: binding.source.clone(),
                    node: binding.target.node.clone(),
                    input: binding.target.input.clone(),
                    message: "target node has no inputs object".to_owned(),
                });
            };

            if !inputs.contains_key(&binding.target.input) {
                return Err(CompileError::BindingInvalid {
                    source: binding.source.clone(),
                    node: binding.target.node.clone(),
                    input: binding.target.input.clone(),
                    message: "target input does not exist in workflow".to_owned(),
                });
            }
        }

        for output in recipe.outputs.iter().filter(|output| output.required) {
            if workflow.node(&output.node).is_none() {
                return Err(CompileError::OutputInvalid {
                    output_id: output.id.clone(),
                    node: output.node.clone(),
                    message: "required output node does not exist in workflow".to_owned(),
                });
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::BindingValidator;
    use crate::compiler::{RecipeParser, WorkflowValidator};
    use crate::domain::WorkflowDocument;
    use serde_json::json;

    const RECIPE: &str = r#"
schema_version: 1
id: binding_test
name: Binding Test
workflow:
  file: workflow.json
inputs:
  prompt:
    type: textarea
    label: Prompt
    required: true
bindings:
  - source: prompt
    target:
      node: "3"
      input: text
outputs:
  - id: image
    type: image
    node: "9"
    required: true
"#;

    fn valid_workflow() -> WorkflowDocument {
        let workflow = WorkflowDocument::parse(json!({
            "3": {"inputs": {"text": "original"}, "class_type": "Text"},
            "9": {"inputs": {"images": ["3", 0]}, "class_type": "SaveImage"}
        }))
        .expect("workflow should parse");
        WorkflowValidator::validate(&workflow).expect("workflow should validate");
        workflow
    }

    #[test]
    fn rejects_missing_binding_node() {
        let recipe = RecipeParser::parse(&RECIPE.replace("node: \"3\"", "node: \"999\""))
            .expect("recipe should parse");

        let error = BindingValidator::validate(&recipe, &valid_workflow())
            .expect_err("missing node must fail");

        assert!(error.to_string().contains("node \"999\""));
    }

    #[test]
    fn rejects_missing_binding_input() {
        let recipe = RecipeParser::parse(&RECIPE.replace("input: text", "input: seeeed"))
            .expect("recipe should parse");

        let error = BindingValidator::validate(&recipe, &valid_workflow())
            .expect_err("missing input must fail");

        assert!(error.to_string().contains("seeeed"));
    }

    #[test]
    fn rejects_missing_required_output_node() {
        let recipe = RecipeParser::parse(&RECIPE.replace("node: \"9\"", "node: \"999\""))
            .expect("recipe should parse");

        let error = BindingValidator::validate(&recipe, &valid_workflow())
            .expect_err("missing output node must fail");

        assert!(error.to_string().contains("required output node"));
    }

    #[test]
    fn binding_validator_does_not_need_user_input_values() {
        let recipe = RecipeParser::parse(RECIPE).expect("recipe should parse");

        assert!(BindingValidator::validate(&recipe, &valid_workflow()).is_ok());
    }
}
