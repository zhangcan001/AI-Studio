use crate::domain::{RecipeError, WorkflowError};
use std::{error::Error, fmt};

#[derive(Clone, Debug, PartialEq, Eq)]
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
            Self::UnknownInput { .. } => "UNKNOWN_INPUT",
            Self::InputRequired { .. } => "INPUT_REQUIRED",
            Self::InputTypeMismatch { .. } => "INPUT_TYPE_MISMATCH",
            Self::InputOutOfRange { .. } => "INPUT_OUT_OF_RANGE",
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
