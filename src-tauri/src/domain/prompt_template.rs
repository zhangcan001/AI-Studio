use std::collections::BTreeMap;

/// Project-level values available to a prompt template.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PromptProjectContext {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

/// A Series, Episode, or Scene value available to a prompt template.
///
/// `number` is deliberately the one-based value exposed to templates. The
/// production structure stores the corresponding ordinal as zero-based.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PromptStructureContext {
    pub id: String,
    pub name: String,
    pub description: String,
    pub number: u32,
}

/// Shot-level values available to a prompt template.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PromptShotContext {
    pub id: String,
    pub name: String,
    pub number: u32,
    pub base_prompt: String,
}

/// Metadata used by an anchor context. Asset ids and storage paths are
/// intentionally absent: templates may only see anchor metadata.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PromptAnchor {
    pub id: String,
    pub name: String,
    pub description: String,
}

/// The selected anchors, retaining the caller's selection order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PromptAnchorContext {
    pub all: Vec<PromptAnchor>,
    pub character: Vec<PromptAnchor>,
    pub scene: Vec<PromptAnchor>,
    pub prop: Vec<PromptAnchor>,
    pub style: Vec<PromptAnchor>,
}

impl PromptAnchorContext {
    /// Builds the categorized view from anchors in the user's selection order.
    pub fn from_selected<I>(anchors: I) -> Self
    where
        I: IntoIterator<Item = (PromptAnchorKind, PromptAnchor)>,
    {
        let mut context = Self::default();
        for (kind, anchor) in anchors {
            context.all.push(anchor.clone());
            match kind {
                PromptAnchorKind::Character => context.character.push(anchor),
                PromptAnchorKind::Scene => context.scene.push(anchor),
                PromptAnchorKind::Prop => context.prop.push(anchor),
                PromptAnchorKind::Style => context.style.push(anchor),
            }
        }
        context
    }

    pub fn from_categories(
        character: Vec<PromptAnchor>,
        scene: Vec<PromptAnchor>,
        prop: Vec<PromptAnchor>,
        style: Vec<PromptAnchor>,
    ) -> Self {
        let all = character
            .iter()
            .chain(scene.iter())
            .chain(prop.iter())
            .chain(style.iter())
            .cloned()
            .collect();
        Self {
            all,
            character,
            scene,
            prop,
            style,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptAnchorKind {
    Character,
    Scene,
    Prop,
    Style,
}

/// All values needed to render one shot. Structure nodes are optional because
/// an existing shot may legitimately be unassigned.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PromptTemplateContext {
    pub project: PromptProjectContext,
    pub series: Option<PromptStructureContext>,
    pub episode: Option<PromptStructureContext>,
    pub scene: Option<PromptStructureContext>,
    pub shot: PromptShotContext,
    pub anchors: PromptAnchorContext,
    pub custom_values: BTreeMap<String, String>,
}

impl PromptTemplateContext {
    pub fn new(project: PromptProjectContext, shot: PromptShotContext) -> Self {
        Self {
            project,
            shot,
            ..Self::default()
        }
    }

    pub fn with_series(mut self, value: PromptStructureContext) -> Self {
        self.series = Some(value);
        self
    }

    pub fn with_episode(mut self, value: PromptStructureContext) -> Self {
        self.episode = Some(value);
        self
    }

    pub fn with_scene(mut self, value: PromptStructureContext) -> Self {
        self.scene = Some(value);
        self
    }

    pub fn with_anchors(mut self, value: PromptAnchorContext) -> Self {
        self.anchors = value;
        self
    }

    pub fn with_custom_values(mut self, value: BTreeMap<String, String>) -> Self {
        self.custom_values = value;
        self
    }
}

/// A parsed template is a sequence of literal and variable segments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PromptTemplateSegment {
    Literal(String),
    Variable(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedPromptTemplate {
    pub segments: Vec<PromptTemplateSegment>,
    /// Unique variables in first-seen order.
    pub variables: Vec<String>,
}

pub type PromptTemplateParse = ParsedPromptTemplate;

/// Output of the read-only template analyzer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PromptTemplateAnalysis {
    pub is_template: bool,
    pub variables: Vec<String>,
    pub builtin_variables: Vec<String>,
    pub custom_variables: Vec<String>,
    pub requires_structure: bool,
}

pub type ProjectTemplateContext = PromptProjectContext;
pub type StructureTemplateContext = PromptStructureContext;
pub type ShotTemplateContext = PromptShotContext;
