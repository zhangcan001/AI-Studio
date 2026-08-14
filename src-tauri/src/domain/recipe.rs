use std::{collections::BTreeMap, error::Error, fmt, path::Component, path::Path};

#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub workflow: WorkflowRef,
    pub inputs: BTreeMap<String, InputDefinition>,
    pub bindings: Vec<Binding>,
    pub outputs: Vec<OutputDefinition>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowRef {
    pub file: String,
}

impl WorkflowRef {
    pub fn is_safe_relative_path(&self) -> bool {
        if self.file.trim().is_empty() {
            return false;
        }

        let path = Path::new(&self.file);
        if path.is_absolute() {
            return false;
        }

        if self.file.starts_with('/') || self.file.starts_with('\\') {
            return false;
        }

        !path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum InputDefinition {
    TextArea {
        label: String,
        required: bool,
        default: Option<String>,
    },
    Integer {
        label: String,
        required: bool,
        default: Option<i64>,
        min: Option<i64>,
        max: Option<i64>,
        step: Option<i64>,
    },
    Number {
        label: String,
        required: bool,
        default: Option<f64>,
        min: Option<f64>,
        max: Option<f64>,
        step: Option<f64>,
    },
    Seed {
        label: String,
        default: SeedDefault,
        min: Option<u64>,
        max: Option<u64>,
    },
    Image {
        label: String,
        required: bool,
    },
    Images {
        label: String,
        required: bool,
        min_items: usize,
        max_items: usize,
    },
    Video {
        label: String,
        required: bool,
    },
    Audio {
        label: String,
        required: bool,
    },
    Videos {
        label: String,
        required: bool,
        min_items: usize,
        max_items: usize,
    },
    Audios {
        label: String,
        required: bool,
        min_items: usize,
        max_items: usize,
    },
}

impl InputDefinition {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::TextArea { .. } => "textarea",
            Self::Integer { .. } => "integer",
            Self::Number { .. } => "number",
            Self::Seed { .. } => "seed",
            Self::Image { .. } => "image",
            Self::Images { .. } => "images",
            Self::Video { .. } => "video",
            Self::Audio { .. } => "audio",
            Self::Videos { .. } => "videos",
            Self::Audios { .. } => "audios",
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::TextArea { label, .. }
            | Self::Integer { label, .. }
            | Self::Number { label, .. }
            | Self::Seed { label, .. } => label,
            Self::Image { label, .. }
            | Self::Images { label, .. }
            | Self::Video { label, .. }
            | Self::Audio { label, .. }
            | Self::Videos { label, .. }
            | Self::Audios { label, .. } => label,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeedDefault {
    Random,
    Fixed(u64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeedValue {
    Random,
    Fixed(u64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub source: String,
    pub item_index: Option<usize>,
    pub target: BindingTarget,
    /// Optional workflow inputs to remove when the source value is absent (or
    /// when an optional plural slot is not populated). This is intentionally
    /// explicit: runtime packages can keep a real ComfyUI loader link in the
    /// graph while allowing the product to select a verified mode at compile
    /// time without guessing node fields.
    pub clear_targets: Vec<BindingTarget>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindingTarget {
    pub node: String,
    pub input: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputType {
    Image,
    Video,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputDefinition {
    pub id: String,
    pub output_type: OutputType,
    pub node: String,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompileRequest {
    pub values: BTreeMap<String, InputValue>,
}

impl CompileRequest {
    pub fn new(values: BTreeMap<String, InputValue>) -> Self {
        Self { values }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum InputValue {
    String(String),
    Integer(i64),
    Number(f64),
    Seed(SeedValue),
    Image(String),
    Images(Vec<String>),
    Video(String),
    Audio(String),
    Videos(Vec<String>),
    Audios(Vec<String>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedInputValue {
    String(String),
    Integer(i64),
    Number(f64),
    Seed(u64),
    Image(String),
    Images(Vec<String>),
    Video(String),
    Audio(String),
    Videos(Vec<String>),
    Audios(Vec<String>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecipeError {
    Parse { message: String },
    UnsupportedSchema { found: u32 },
    Invalid { message: String },
}

impl RecipeError {
    pub fn parse(message: impl Into<String>) -> Self {
        Self::Parse {
            message: message.into(),
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid {
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Parse { .. } => "RECIPE_PARSE_ERROR",
            Self::UnsupportedSchema { .. } => "UNSUPPORTED_RECIPE_SCHEMA",
            Self::Invalid { .. } => "RECIPE_INVALID",
        }
    }
}

impl fmt::Display for RecipeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse { message } => write!(formatter, "{}: {message}", self.code()),
            Self::UnsupportedSchema { found } => write!(
                formatter,
                "{}: schema_version {found} is unsupported; only version 1 is supported",
                self.code()
            ),
            Self::Invalid { message } => write!(formatter, "{}: {message}", self.code()),
        }
    }
}

impl Error for RecipeError {}
