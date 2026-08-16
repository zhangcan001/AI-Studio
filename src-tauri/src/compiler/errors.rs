use crate::domain::{RecipeError, WorkflowError};
use std::{error::Error, fmt};

#[derive(Clone, Debug, PartialEq)]
pub enum CompileError {
    Recipe(RecipeError),
    Workflow(WorkflowError),
    BindingInvalid {
        source: String,
        node: String,
        input: String,
        message: String,
    },
    OutputInvalid {
        output_id: String,
        node: String,
        message: String,
    },
    CompiledDanglingNodeReference {
        source_node: String,
        input_name: String,
        referenced_node: String,
        output_index: i64,
    },
    CompiledInternalPlaceholder {
        path: String,
        marker: String,
    },
    CompiledMediaBindingIncomplete {
        asset_id: String,
        media_kind: String,
        reference_index: Option<usize>,
        input_key: String,
        expected_target: String,
        actual_target: String,
    },
    CompiledOutputNodeMissing {
        output_id: String,
        node: String,
    },
    CompiledGraphInvalid {
        message: String,
    },
    UnknownInput {
        input: String,
    },
    InputRequired {
        input: String,
    },
    InputTypeMismatch {
        input: String,
        expected: String,
        actual: String,
    },
    InputOutOfRange {
        input: String,
        value: i64,
        min: Option<i64>,
        max: Option<i64>,
    },
    InputStepMismatch {
        input: String,
        value: i64,
        step: i64,
    },
    InputNumberOutOfRange {
        input: String,
        value: f64,
        min: Option<f64>,
        max: Option<f64>,
    },
    InputNumberStepMismatch {
        input: String,
        value: f64,
        step: f64,
    },
    InputCountOutOfRange {
        input: String,
        count: usize,
        min: usize,
        max: usize,
    },
    SeedOutOfRange {
        input: String,
        value: u64,
        min: Option<u64>,
        max: Option<u64>,
    },
    Internal {
        message: String,
    },
}

impl CompileError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Recipe(error) => error.code(),
            Self::Workflow(error) => error.code(),
            Self::BindingInvalid { .. } => "BINDING_INVALID",
            Self::OutputInvalid { .. } => "OUTPUT_INVALID",
            Self::CompiledDanglingNodeReference { .. } => "COMPILED_DANGLING_NODE_REFERENCE",
            Self::CompiledInternalPlaceholder { .. } => "COMPILED_INTERNAL_PLACEHOLDER",
            Self::CompiledMediaBindingIncomplete { .. } => "COMPILED_MEDIA_BINDING_INCOMPLETE",
            Self::CompiledOutputNodeMissing { .. } => "COMPILED_OUTPUT_NODE_MISSING",
            Self::CompiledGraphInvalid { .. } => "COMPILED_GRAPH_INVALID",
            Self::UnknownInput { .. } => "UNKNOWN_INPUT",
            Self::InputRequired { .. } => "INPUT_REQUIRED",
            Self::InputTypeMismatch { .. } => "INPUT_TYPE_MISMATCH",
            Self::InputOutOfRange { .. } => "INPUT_OUT_OF_RANGE",
            Self::InputStepMismatch { .. } => "INPUT_STEP_MISMATCH",
            Self::InputNumberOutOfRange { .. } => "INPUT_NUMBER_OUT_OF_RANGE",
            Self::InputNumberStepMismatch { .. } => "INPUT_NUMBER_STEP_MISMATCH",
            Self::InputCountOutOfRange { .. } => "INPUT_COUNT_OUT_OF_RANGE",
            Self::SeedOutOfRange { .. } => "SEED_OUT_OF_RANGE",
            Self::Internal { .. } => "COMPILE_INTERNAL",
        }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Recipe(error) => write!(formatter, "{error}"),
            Self::Workflow(error) => write!(formatter, "{error}"),
            Self::BindingInvalid {
                source,
                node,
                input,
                message,
            } => write!(
                formatter,
                "{}: binding \"{source}\" targets node \"{node}\" input \"{input}\": {message}",
                self.code()
            ),
            Self::OutputInvalid {
                output_id,
                node,
                message,
            } => write!(
                formatter,
                "{}: output \"{output_id}\" references node \"{node}\": {message}",
                self.code()
            ),
            Self::UnknownInput { input } => {
                write!(
                    formatter,
                    "{}: input \"{input}\" is not declared by the recipe",
                    self.code()
                )
            }
            Self::InputRequired { input } => {
                write!(
                    formatter,
                    "{}: required input \"{input}\" has no value",
                    self.code()
                )
            }
            Self::InputTypeMismatch {
                input,
                expected,
                actual,
            } => write!(
                formatter,
                "{}: input \"{input}\" expects {expected}, received {actual}",
                self.code()
            ),
            Self::InputOutOfRange {
                input,
                value,
                min,
                max,
            } => write!(
                formatter,
                "{}: input \"{input}\" value {value} is outside range [{}, {}]",
                self.code(),
                min.map_or_else(|| "-∞".to_owned(), |value| value.to_string()),
                max.map_or_else(|| "∞".to_owned(), |value| value.to_string())
            ),
            Self::InputStepMismatch { input, value, step } => write!(
                formatter,
                "{}: input \"{input}\" value {value} must be a multiple of {step}",
                self.code()
            ),
            Self::CompiledDanglingNodeReference {
                source_node,
                input_name,
                referenced_node,
                output_index,
            } => write!(
                formatter,
                "{}: node {source_node} input {input_name} references missing node {referenced_node} output {output_index}",
                self.code()
            ),
            Self::CompiledInternalPlaceholder { path, marker } => write!(
                formatter,
                "{}: unresolved internal placeholder {marker} at {path}",
                self.code()
            ),
            Self::CompiledMediaBindingIncomplete {
                asset_id,
                media_kind,
                reference_index,
                input_key,
                expected_target,
                actual_target,
            } => write!(
                formatter,
                "{}: {media_kind} asset {asset_id} reference {:?} for input {input_key} expected {expected_target}, received {actual_target}",
                self.code(),
                reference_index
            ),
            Self::CompiledOutputNodeMissing { output_id, node } => write!(
                formatter,
                "{}: output {output_id} references missing node {node}",
                self.code()
            ),
            Self::CompiledGraphInvalid { message } => {
                write!(formatter, "{}: {message}", self.code())
            }
            Self::InputNumberOutOfRange {
                input,
                value,
                min,
                max,
            } => write!(
                formatter,
                "{}: input \"{input}\" value {value} is outside range [{}, {}]",
                self.code(),
                min.map_or_else(|| "-∞".to_owned(), |value| value.to_string()),
                max.map_or_else(|| "∞".to_owned(), |value| value.to_string())
            ),
            Self::InputNumberStepMismatch { input, value, step } => write!(
                formatter,
                "{}: input \"{input}\" value {value} must align to step {step}",
                self.code()
            ),
            Self::InputCountOutOfRange {
                input,
                count,
                min,
                max,
            } => write!(
                formatter,
                "{}: input \"{input}\" contains {count} images; expected {min}..{max}",
                self.code()
            ),
            Self::SeedOutOfRange {
                input,
                value,
                min,
                max,
            } => write!(
                formatter,
                "{}: seed input \"{input}\" value {value} is outside range [{}, {}]",
                self.code(),
                min.map_or_else(|| "0".to_owned(), |value| value.to_string()),
                max.map_or_else(|| u64::MAX.to_string(), |value| value.to_string())
            ),
            Self::Internal { message } => write!(formatter, "{}: {message}", self.code()),
        }
    }
}

impl Error for CompileError {}

impl From<RecipeError> for CompileError {
    fn from(error: RecipeError) -> Self {
        Self::Recipe(error)
    }
}

impl From<WorkflowError> for CompileError {
    fn from(error: WorkflowError) -> Self {
        Self::Workflow(error)
    }
}
