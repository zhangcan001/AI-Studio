use crate::application::prompt_library_service::canonical_prompt_text;
use crate::domain::{
    ParsedPromptTemplate, PromptAnchor, PromptStructureContext, PromptTemplateAnalysis,
    PromptTemplateContext, PromptTemplateSegment,
};
use std::{error::Error, fmt};

pub const PROMPT_TEMPLATE_SYNTAX_ERROR: &str = "PROMPT_TEMPLATE_SYNTAX_ERROR";
pub const PROMPT_TEMPLATE_UNKNOWN_VARIABLE: &str = "PROMPT_TEMPLATE_UNKNOWN_VARIABLE";
pub const PROMPT_TEMPLATE_CONTEXT_MISSING: &str = "PROMPT_TEMPLATE_CONTEXT_MISSING";
pub const PROMPT_TEMPLATE_CUSTOM_VALUE_MISSING: &str = "PROMPT_TEMPLATE_CUSTOM_VALUE_MISSING";
pub const PROMPT_TEMPLATE_RESULT_TOO_LARGE: &str = "PROMPT_TEMPLATE_RESULT_TOO_LARGE";
pub const PROMPT_TEMPLATE_CUSTOM_VALUES_INVALID: &str = "PROMPT_TEMPLATE_CUSTOM_VALUES_INVALID";

const MAX_CUSTOM_VARIABLES: usize = 50;
const MAX_CUSTOM_KEY_CHARS: usize = 64;
const MAX_CUSTOM_VALUE_BYTES: usize = 4096;
const MAX_CUSTOM_VALUES_BYTES: usize = 32 * 1024;

const BUILTIN_VARIABLES: &[&str] = &[
    "project.id",
    "project.name",
    "project.description",
    "series.id",
    "series.name",
    "series.description",
    "series.number",
    "episode.id",
    "episode.name",
    "episode.description",
    "episode.number",
    "scene.id",
    "scene.name",
    "scene.description",
    "scene.number",
    "shot.id",
    "shot.name",
    "shot.number",
    "shot.basePrompt",
    "anchors.character.names",
    "anchors.character.context",
    "anchors.scene.names",
    "anchors.scene.context",
    "anchors.prop.names",
    "anchors.prop.context",
    "anchors.style.names",
    "anchors.style.context",
    "anchors.all.names",
    "anchors.all.context",
];

#[derive(Clone, Copy, Debug, Default)]
pub struct PromptTemplateService;

impl PromptTemplateService {
    pub fn new() -> Self {
        Self
    }

    pub fn parse(&self, text: &str) -> Result<ParsedPromptTemplate, PromptTemplateError> {
        parse_prompt_template(text)
    }

    pub fn analyze(&self, text: &str) -> Result<PromptTemplateAnalysis, PromptTemplateError> {
        analyze_prompt_template(text)
    }

    pub fn render(
        &self,
        text: &str,
        context: &PromptTemplateContext,
    ) -> Result<String, PromptTemplateError> {
        let parsed = parse_prompt_template(text)?;
        self.render_parsed(&parsed, context)
    }

    pub fn render_parsed(
        &self,
        parsed: &ParsedPromptTemplate,
        context: &PromptTemplateContext,
    ) -> Result<String, PromptTemplateError> {
        validate_custom_values(&context.custom_values)?;
        let mut rendered = String::new();
        for segment in &parsed.segments {
            match segment {
                PromptTemplateSegment::Literal(value) => rendered.push_str(value),
                PromptTemplateSegment::Variable(variable) => {
                    rendered.push_str(&resolve_variable(variable, context)?)
                }
            }
        }
        canonical_prompt_text(&rendered).map_err(|_| PromptTemplateError::result_too_large())
    }
}

pub fn parse_prompt_template(text: &str) -> Result<ParsedPromptTemplate, PromptTemplateError> {
    let mut segments = Vec::new();
    let mut variables = Vec::new();
    let mut cursor = 0;

    while cursor < text.len() {
        let remaining = &text[cursor..];
        let open = remaining.find("{{");
        let close = remaining.find("}}");
        match (open, close) {
            (None, Some(_)) => return Err(PromptTemplateError::syntax("unmatched closing braces")),
            (None, None) => {
                push_literal(&mut segments, remaining);
                cursor = text.len();
            }
            (Some(open), Some(close)) if close < open => {
                return Err(PromptTemplateError::syntax("unmatched closing braces"));
            }
            (Some(open), None) => {
                push_literal(&mut segments, &remaining[..open]);
                return Err(PromptTemplateError::syntax("unclosed variable"));
            }
            (Some(open), Some(close)) => {
                push_literal(&mut segments, &remaining[..open]);
                let inner_start = cursor + open + 2;
                let inner_end = cursor + close;
                let raw_variable = &text[inner_start..inner_end];
                if raw_variable.contains("{{") || raw_variable.contains("}}") {
                    return Err(PromptTemplateError::syntax("nested variable braces"));
                }
                let variable = raw_variable.trim();
                if variable.is_empty() || !is_variable_name(variable) {
                    return Err(PromptTemplateError::syntax(format!(
                        "invalid variable `{raw_variable}`"
                    )));
                }
                if !is_supported_variable(variable) {
                    return Err(PromptTemplateError::unknown_variable(variable));
                }
                segments.push(PromptTemplateSegment::Variable(variable.to_owned()));
                if !variables.iter().any(|current| current == variable) {
                    variables.push(variable.to_owned());
                }
                cursor = inner_end + 2;
            }
        }
    }

    if cursor == 0 && text.is_empty() {
        segments.push(PromptTemplateSegment::Literal(String::new()));
    }
    Ok(ParsedPromptTemplate {
        segments,
        variables,
    })
}

pub fn analyze_prompt_template(text: &str) -> Result<PromptTemplateAnalysis, PromptTemplateError> {
    let parsed = parse_prompt_template(text)?;
    let mut builtin_variables = Vec::new();
    let mut custom_variables = Vec::new();
    let mut requires_structure = false;
    for variable in &parsed.variables {
        if let Some(custom) = variable.strip_prefix("custom.") {
            custom_variables.push(custom.to_owned());
        } else {
            builtin_variables.push(variable.clone());
            if variable.starts_with("series.")
                || variable.starts_with("episode.")
                || variable.starts_with("scene.")
            {
                requires_structure = true;
            }
        }
    }
    Ok(PromptTemplateAnalysis {
        is_template: !parsed.variables.is_empty(),
        variables: parsed.variables,
        builtin_variables,
        custom_variables,
        requires_structure,
    })
}

pub fn render_prompt_template(
    text: &str,
    context: &PromptTemplateContext,
) -> Result<String, PromptTemplateError> {
    PromptTemplateService::new().render(text, context)
}

fn push_literal(segments: &mut Vec<PromptTemplateSegment>, text: &str) {
    if !text.is_empty() {
        segments.push(PromptTemplateSegment::Literal(text.to_owned()));
    }
}

fn is_variable_name(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn is_supported_variable(value: &str) -> bool {
    BUILTIN_VARIABLES.contains(&value)
        || value
            .strip_prefix("custom.")
            .is_some_and(|key| !key.is_empty())
}

fn validate_custom_values(
    values: &std::collections::BTreeMap<String, String>,
) -> Result<(), PromptTemplateError> {
    if values.len() > MAX_CUSTOM_VARIABLES {
        return Err(PromptTemplateError::custom_values_invalid(format!(
            "at most {MAX_CUSTOM_VARIABLES} custom variables are allowed"
        )));
    }
    let mut total_bytes = 0;
    for (key, value) in values {
        if key.is_empty() || key.chars().count() > MAX_CUSTOM_KEY_CHARS || !is_variable_name(key) {
            return Err(PromptTemplateError::custom_values_invalid(format!(
                "custom key `{key}` must be 1–{MAX_CUSTOM_KEY_CHARS} ASCII variable characters"
            )));
        }
        if value.as_bytes().len() > MAX_CUSTOM_VALUE_BYTES {
            return Err(PromptTemplateError::custom_values_invalid(format!(
                "custom value `{key}` must be at most {MAX_CUSTOM_VALUE_BYTES} bytes"
            )));
        }
        total_bytes += value.as_bytes().len();
    }
    if total_bytes > MAX_CUSTOM_VALUES_BYTES {
        return Err(PromptTemplateError::custom_values_invalid(format!(
            "custom values must total at most {MAX_CUSTOM_VALUES_BYTES} bytes"
        )));
    }
    Ok(())
}

fn resolve_variable(
    variable: &str,
    context: &PromptTemplateContext,
) -> Result<String, PromptTemplateError> {
    let result = match variable {
        "project.id" => context.project.id.clone(),
        "project.name" => context.project.name.clone(),
        "project.description" => context.project.description.clone().unwrap_or_default(),
        "series.id" => {
            structure_value(&context.series, variable, context, |value| value.id.clone())?
        }
        "series.name" => structure_value(&context.series, variable, context, |value| {
            value.name.clone()
        })?,
        "series.description" => structure_value(&context.series, variable, context, |value| {
            value.description.clone()
        })?,
        "series.number" => structure_value(&context.series, variable, context, |value| {
            value.number.to_string()
        })?,
        "episode.id" => structure_value(&context.episode, variable, context, |value| {
            value.id.clone()
        })?,
        "episode.name" => structure_value(&context.episode, variable, context, |value| {
            value.name.clone()
        })?,
        "episode.description" => structure_value(&context.episode, variable, context, |value| {
            value.description.clone()
        })?,
        "episode.number" => structure_value(&context.episode, variable, context, |value| {
            value.number.to_string()
        })?,
        "scene.id" => structure_value(&context.scene, variable, context, |value| value.id.clone())?,
        "scene.name" => structure_value(&context.scene, variable, context, |value| {
            value.name.clone()
        })?,
        "scene.description" => structure_value(&context.scene, variable, context, |value| {
            value.description.clone()
        })?,
        "scene.number" => structure_value(&context.scene, variable, context, |value| {
            value.number.to_string()
        })?,
        "shot.id" => context.shot.id.clone(),
        "shot.name" => context.shot.name.clone(),
        "shot.number" => context.shot.number.to_string(),
        "shot.basePrompt" => context.shot.base_prompt.clone(),
        "anchors.character.names" => anchor_names(&context.anchors.character),
        "anchors.character.context" => anchor_context(&context.anchors.character),
        "anchors.scene.names" => anchor_names(&context.anchors.scene),
        "anchors.scene.context" => anchor_context(&context.anchors.scene),
        "anchors.prop.names" => anchor_names(&context.anchors.prop),
        "anchors.prop.context" => anchor_context(&context.anchors.prop),
        "anchors.style.names" => anchor_names(&context.anchors.style),
        "anchors.style.context" => anchor_context(&context.anchors.style),
        "anchors.all.names" => anchor_names(&context.anchors.all),
        "anchors.all.context" => anchor_context(&context.anchors.all),
        custom if custom.starts_with("custom.") => {
            let key = &custom["custom.".len()..];
            context
                .custom_values
                .get(key)
                .cloned()
                .ok_or_else(|| PromptTemplateError::custom_value_missing(key))?
        }
        _ => return Err(PromptTemplateError::unknown_variable(variable)),
    };
    Ok(result)
}

fn structure_value<T>(
    value: &Option<PromptStructureContext>,
    variable: &str,
    context: &PromptTemplateContext,
    map: impl FnOnce(&PromptStructureContext) -> T,
) -> Result<T, PromptTemplateError> {
    value
        .as_ref()
        .map(map)
        .ok_or_else(|| PromptTemplateError::context_missing(variable, &context.shot))
}

fn anchor_names(anchors: &[PromptAnchor]) -> String {
    anchors
        .iter()
        .map(|anchor| anchor.name.as_str())
        .collect::<Vec<_>>()
        .join("、")
}

fn anchor_context(anchors: &[PromptAnchor]) -> String {
    anchors
        .iter()
        .map(|anchor| {
            if anchor.description.is_empty() {
                anchor.name.clone()
            } else {
                format!("{}：{}", anchor.name, anchor.description)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PromptTemplateError {
    Syntax {
        message: String,
    },
    UnknownVariable {
        variable: String,
    },
    ContextMissing {
        variable: String,
        shot_id: String,
        shot_name: String,
    },
    CustomValueMissing {
        key: String,
    },
    CustomValuesInvalid {
        message: String,
    },
    ResultTooLarge,
}

impl PromptTemplateError {
    fn syntax(message: impl Into<String>) -> Self {
        Self::Syntax {
            message: message.into(),
        }
    }

    fn unknown_variable(variable: impl Into<String>) -> Self {
        Self::UnknownVariable {
            variable: variable.into(),
        }
    }

    fn context_missing(
        variable: impl Into<String>,
        shot: &crate::domain::PromptShotContext,
    ) -> Self {
        Self::ContextMissing {
            variable: variable.into(),
            shot_id: shot.id.clone(),
            shot_name: shot.name.clone(),
        }
    }

    fn custom_value_missing(key: impl Into<String>) -> Self {
        Self::CustomValueMissing { key: key.into() }
    }

    fn custom_values_invalid(message: impl Into<String>) -> Self {
        Self::CustomValuesInvalid {
            message: message.into(),
        }
    }

    fn result_too_large() -> Self {
        Self::ResultTooLarge
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Syntax { .. } => PROMPT_TEMPLATE_SYNTAX_ERROR,
            Self::UnknownVariable { .. } => PROMPT_TEMPLATE_UNKNOWN_VARIABLE,
            Self::ContextMissing { .. } => PROMPT_TEMPLATE_CONTEXT_MISSING,
            Self::CustomValueMissing { .. } => PROMPT_TEMPLATE_CUSTOM_VALUE_MISSING,
            Self::CustomValuesInvalid { .. } => PROMPT_TEMPLATE_CUSTOM_VALUES_INVALID,
            Self::ResultTooLarge => PROMPT_TEMPLATE_RESULT_TOO_LARGE,
        }
    }
}

impl fmt::Display for PromptTemplateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax { message } => {
                write!(formatter, "{PROMPT_TEMPLATE_SYNTAX_ERROR}: {message}")
            }
            Self::UnknownVariable { variable } => write!(
                formatter,
                "{PROMPT_TEMPLATE_UNKNOWN_VARIABLE}: variable `{variable}` is not supported"
            ),
            Self::ContextMissing {
                variable,
                shot_id,
                shot_name,
            } => write!(
                formatter,
                "{PROMPT_TEMPLATE_CONTEXT_MISSING}: shot `{shot_name}` ({shot_id}) is missing context for `{{{{{variable}}}}}`"
            ),
            Self::CustomValueMissing { key } => write!(
                formatter,
                "{PROMPT_TEMPLATE_CUSTOM_VALUE_MISSING}: custom value `{key}` is missing"
            ),
            Self::CustomValuesInvalid { message } => {
                write!(
                    formatter,
                    "{PROMPT_TEMPLATE_CUSTOM_VALUES_INVALID}: {message}"
                )
            }
            Self::ResultTooLarge => write!(
                formatter,
                "{PROMPT_TEMPLATE_RESULT_TOO_LARGE}: rendered prompt exceeds 64 KiB"
            ),
        }
    }
}

impl Error for PromptTemplateError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        PromptAnchorContext, PromptAnchorKind, PromptProjectContext, PromptShotContext,
    };
    use std::collections::BTreeMap;

    fn context() -> PromptTemplateContext {
        let anchors = PromptAnchorContext::from_selected([
            (
                PromptAnchorKind::Character,
                PromptAnchor {
                    id: "anc-character".to_owned(),
                    name: "释迦牟尼佛".to_owned(),
                    description: "成熟庄严佛相，金色袈裟".to_owned(),
                },
            ),
            (
                PromptAnchorKind::Style,
                PromptAnchor {
                    id: "anc-style".to_owned(),
                    name: "电影写实".to_owned(),
                    description: "庄严神圣，体积光".to_owned(),
                },
            ),
        ]);
        PromptTemplateContext::new(
            PromptProjectContext {
                id: "prj-1".to_owned(),
                name: "地藏经".to_owned(),
                description: None,
            },
            PromptShotContext {
                id: "shot-1".to_owned(),
                name: "佛陀端坐".to_owned(),
                number: 1,
                base_prompt: "庄严构图".to_owned(),
            },
        )
        .with_series(PromptStructureContext {
            id: "ser-1".to_owned(),
            name: "第一季".to_owned(),
            description: String::new(),
            number: 1,
        })
        .with_episode(PromptStructureContext {
            id: "epi-1".to_owned(),
            name: "第一集".to_owned(),
            description: String::new(),
            number: 1,
        })
        .with_scene(PromptStructureContext {
            id: "scn-1".to_owned(),
            name: "忉利天宫".to_owned(),
            description: "佛陀于忉利天为母说法".to_owned(),
            number: 1,
        })
        .with_anchors(anchors)
    }

    #[test]
    fn parser_is_deterministic_and_normalizes_variable_whitespace() {
        let parsed =
            parse_prompt_template("项目 {{ project.name }} / {{shot.name}} / {{ project.name }}")
                .unwrap();
        assert_eq!(
            parsed.variables,
            vec!["project.name".to_owned(), "shot.name".to_owned()]
        );
        assert_eq!(
            parsed.segments,
            vec![
                PromptTemplateSegment::Literal("项目 ".to_owned()),
                PromptTemplateSegment::Variable("project.name".to_owned()),
                PromptTemplateSegment::Literal(" / ".to_owned()),
                PromptTemplateSegment::Variable("shot.name".to_owned()),
                PromptTemplateSegment::Literal(" / ".to_owned()),
                PromptTemplateSegment::Variable("project.name".to_owned()),
            ]
        );
    }

    #[test]
    fn parser_rejects_syntax_and_unknown_variables_with_codes() {
        for (text, code) in [
            ("{{scene.name", PROMPT_TEMPLATE_SYNTAX_ERROR),
            ("{{ }}", PROMPT_TEMPLATE_SYNTAX_ERROR),
            ("{{scene/name}}", PROMPT_TEMPLATE_SYNTAX_ERROR),
            ("{{scene.location}}", PROMPT_TEMPLATE_UNKNOWN_VARIABLE),
        ] {
            assert_eq!(parse_prompt_template(text).unwrap_err().code(), code);
        }
    }

    #[test]
    fn analysis_separates_builtin_custom_and_structure_requirements() {
        let analysis = analyze_prompt_template(
            "{{project.name}} {{scene.name}} {{custom.camera}} {{custom.mood}}",
        )
        .unwrap();
        assert!(analysis.is_template);
        assert!(analysis.requires_structure);
        assert_eq!(
            analysis.builtin_variables,
            vec!["project.name", "scene.name"]
        );
        assert_eq!(analysis.custom_variables, vec!["camera", "mood"]);
    }

    #[test]
    fn renderer_resolves_all_context_categories_and_one_based_numbers() {
        let mut values = BTreeMap::new();
        values.insert("camera".to_owned(), "中景缓慢推进".to_owned());
        let rendered = render_prompt_template(
            "{{project.name}}/{{series.number}}/{{episode.number}}/{{scene.number}}/{{shot.number}}\n{{scene.description}}\n{{anchors.character.context}}\n{{anchors.style.context}}\n{{custom.camera}}",
            &context().with_custom_values(values),
        )
        .unwrap();
        assert!(rendered.contains("地藏经/1/1/1/1"));
        assert!(rendered.contains("佛陀于忉利天为母说法"));
        assert!(rendered.contains("释迦牟尼佛：成熟庄严佛相，金色袈裟"));
        assert!(rendered.contains("电影写实：庄严神圣，体积光"));
        assert!(rendered.contains("中景缓慢推进"));
    }

    #[test]
    fn renderer_keeps_optional_descriptions_empty_and_anchors_optional() {
        let rendered = render_prompt_template(
            "{{project.description}}|{{series.description}}|{{anchors.prop.context}}",
            &context(),
        )
        .unwrap();
        assert_eq!(rendered, "||");
    }

    #[test]
    fn renderer_reports_missing_structure_and_custom_values() {
        let missing_scene =
            render_prompt_template("镜头 {{scene.name}}", &context().with_scene_removed());
        assert_eq!(
            missing_scene.unwrap_err().code(),
            PROMPT_TEMPLATE_CONTEXT_MISSING
        );
        let missing_custom = render_prompt_template("{{custom.camera}}", &context());
        assert_eq!(
            missing_custom.unwrap_err().code(),
            PROMPT_TEMPLATE_CUSTOM_VALUE_MISSING
        );
    }

    #[test]
    fn renderer_canonicalizes_line_endings_and_rejects_large_values() {
        assert_eq!(
            render_prompt_template("  a\r\nb\r  ", &context()).unwrap(),
            "a\nb"
        );
        let huge = render_prompt_template(
            &"{{custom.value}}".to_owned(),
            &context().with_custom_values(
                [("value".to_owned(), "x".repeat(4096))]
                    .into_iter()
                    .collect(),
            ),
        );
        assert!(huge.is_ok());
        let too_many = (0..=MAX_CUSTOM_VARIABLES)
            .map(|index| (format!("key{index}"), String::new()))
            .collect();
        let error =
            render_prompt_template("plain", &context().with_custom_values(too_many)).unwrap_err();
        assert_eq!(error.code(), PROMPT_TEMPLATE_CUSTOM_VALUES_INVALID);
    }

    #[test]
    fn renderer_is_plain_text_only() {
        let rendered = render_prompt_template(
            "{{custom.value}}",
            &context().with_custom_values(
                [("value".to_owned(), "{{shot.name}}".to_owned())]
                    .into_iter()
                    .collect(),
            ),
        )
        .unwrap();
        assert_eq!(rendered, "{{shot.name}}");
    }

    trait RemoveScene {
        fn with_scene_removed(self) -> Self;
    }

    impl RemoveScene for PromptTemplateContext {
        fn with_scene_removed(mut self) -> Self {
            self.scene = None;
            self
        }
    }

    #[test]
    fn anchor_context_preserves_selection_order() {
        let context = PromptAnchorContext::from_selected([
            (
                PromptAnchorKind::Character,
                PromptAnchor {
                    name: "二".to_owned(),
                    ..PromptAnchor::default()
                },
            ),
            (
                PromptAnchorKind::Character,
                PromptAnchor {
                    name: "一".to_owned(),
                    ..PromptAnchor::default()
                },
            ),
        ]);
        assert_eq!(anchor_names(&context.character), "二、一");
        assert_eq!(anchor_context(&context.all), "二\n一");
    }

    #[test]
    fn analysis_of_literal_text_is_not_a_template() {
        let analysis = analyze_prompt_template("普通提示词").unwrap();
        assert!(!analysis.is_template);
        assert!(analysis.variables.is_empty());
    }

    #[test]
    fn final_output_limit_is_checked_after_expansion() {
        let value = "x".repeat(4096);
        let values = [("x".to_owned(), value)].into_iter().collect();
        let template = "{{custom.x}}".repeat(17);
        let error =
            render_prompt_template(&template, &context().with_custom_values(values)).unwrap_err();
        assert_eq!(error.code(), PROMPT_TEMPLATE_RESULT_TOO_LARGE);
    }
}
