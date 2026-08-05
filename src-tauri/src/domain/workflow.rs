use serde_json::{Map, Value};
use std::{error::Error, fmt};

#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowDocument {
    value: Value,
}

impl WorkflowDocument {
    pub fn parse(value: Value) -> Result<Self, WorkflowError> {
        if !value.is_object() {
            return Err(WorkflowError::invalid(
                "workflow root must be a JSON object",
            ));
        }

        Ok(Self { value })
    }

    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn node(&self, node_id: &str) -> Option<&Value> {
        self.value.as_object()?.get(node_id)
    }

    pub fn node_mut(&mut self, node_id: &str) -> Option<&mut Value> {
        self.value.as_object_mut()?.get_mut(node_id)
    }

    pub fn class_type(&self, node_id: &str) -> Option<&str> {
        self.node(node_id)?.as_object()?.get("class_type")?.as_str()
    }

    pub fn inputs(&self, node_id: &str) -> Option<&Map<String, Value>> {
        self.node(node_id)?.as_object()?.get("inputs")?.as_object()
    }

    pub fn inputs_mut(&mut self, node_id: &str) -> Option<&mut Map<String, Value>> {
        self.node_mut(node_id)?
            .as_object_mut()?
            .get_mut("inputs")?
            .as_object_mut()
    }

    pub fn into_value(self) -> Value {
        self.value
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowError {
    Invalid { message: String },
}

impl WorkflowError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid {
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        "WORKFLOW_INVALID"
    }
}

impl fmt::Display for WorkflowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid { message } => write!(formatter, "{}: {message}", self.code()),
        }
    }
}

impl Error for WorkflowError {}
