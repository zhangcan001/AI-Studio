use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MediaKind {
    Image,
    Video,
    Audio,
}

impl MediaKind {
    fn from_upload_value(value: &Value) -> Option<Self> {
        match value.as_str()?.trim().to_ascii_lowercase().as_str() {
            "image" | "images" => Some(Self::Image),
            "video" | "videos" => Some(Self::Video),
            "audio" | "audios" => Some(Self::Audio),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecognitionDeclaredType {
    String,
    Integer,
    Float,
    Boolean,
    Standard,
    Conditioning,
    Image,
    Mask,
    Latent,
    Video,
    Audio,
    Model,
    VideoModel,
    Enum,
    DynamicCombo,
    Unknown,
}

impl RecognitionDeclaredType {
    pub fn from_schema_name(value: &str) -> Self {
        match value.trim().to_ascii_uppercase().as_str() {
            "STRING" | "TEXT" => Self::String,
            "INT" | "INTEGER" => Self::Integer,
            "FLOAT" | "NUMBER" => Self::Float,
            "BOOL" | "BOOLEAN" => Self::Boolean,
            "CLIP" | "VAE" | "SIGMAS" | "NOISE" | "GUIDER" | "SAMPLER" | "CONTROL_NET"
            | "CONTROLNET" | "STYLE_MODEL" | "UPSCALE_MODEL" => Self::Standard,
            "CONDITIONING" => Self::Conditioning,
            "IMAGE" => Self::Image,
            "MASK" => Self::Mask,
            "LATENT" => Self::Latent,
            "VIDEO" => Self::Video,
            "AUDIO" => Self::Audio,
            "MODEL" => Self::Model,
            "VIDEO_MODEL" | "VIDEOMODEL" => Self::VideoModel,
            "ENUM" | "COMBO" => Self::Enum,
            "COMFY_DYNAMICCOMBO_V3" | "COMFY_AUTOGROW_V3" => Self::DynamicCombo,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionMatchTypeTemplate {
    pub template_id: String,
    pub allowed_types: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionInputSchema {
    pub name: String,
    pub raw_type: String,
    pub declared_type: RecognitionDeclaredType,
    pub match_template: Option<RecognitionMatchTypeTemplate>,
    pub upload_media_kind: Option<MediaKind>,
    pub required: bool,
    pub enum_options: Vec<String>,
    pub enum_values: Vec<Value>,
    pub numeric_min: Option<f64>,
    pub numeric_max: Option<f64>,
    pub numeric_step: Option<f64>,
    pub multiline: bool,
    pub default_value: Option<Value>,
    pub socketless: bool,
    pub dynamic_prefix: Option<String>,
    pub dynamic_names: Vec<String>,
    pub dynamic_value_type: Option<RecognitionDeclaredType>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionNodeSchema {
    pub class_type: String,
    pub display_name: Option<String>,
    pub output_node: bool,
    pub raw_output_types: Vec<String>,
    pub declared_output_types: Vec<RecognitionDeclaredType>,
    pub output_match_types: Vec<Option<String>>,
    pub output_names: Vec<String>,
    pub output_is_list: Vec<bool>,
    pub ordered_inputs: Vec<String>,
    pub hidden_inputs: Vec<String>,
    pub inputs: BTreeMap<String, RecognitionInputSchema>,
    pub conditional_inputs:
        BTreeMap<String, BTreeMap<String, BTreeMap<String, RecognitionInputSchema>>>,
}

impl RecognitionNodeSchema {
    pub fn input(&self, name: &str) -> Option<&RecognitionInputSchema> {
        self.inputs.get(name)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionSchemaContext {
    pub nodes: BTreeMap<String, RecognitionNodeSchema>,
}

impl RecognitionSchemaContext {
    pub fn parse(object_info: &Value) -> Self {
        let mut context = Self::default();
        let Some(classes) = object_info.as_object() else {
            return context;
        };

        for (class_type, metadata) in classes {
            let Some(metadata) = metadata.as_object() else {
                continue;
            };
            let mut inputs = BTreeMap::new();
            let mut conditional_inputs = BTreeMap::new();
            let mut ordered_inputs = Vec::new();
            let mut hidden_inputs = Vec::new();
            if let Some(input_order) = metadata.get("input_order").and_then(Value::as_object) {
                for group in ["required", "optional"] {
                    if let Some(names) = input_order.get(group).and_then(Value::as_array) {
                        ordered_inputs
                            .extend(names.iter().filter_map(Value::as_str).map(str::to_owned));
                    }
                }
                if let Some(names) = input_order.get("hidden").and_then(Value::as_array) {
                    hidden_inputs.extend(names.iter().filter_map(Value::as_str).map(str::to_owned));
                }
            }
            let has_explicit_input_order = !ordered_inputs.is_empty() || !hidden_inputs.is_empty();
            if let Some(input_metadata) = metadata.get("input").and_then(Value::as_object) {
                for group in ["required", "optional", "hidden"] {
                    let Some(group_inputs) = input_metadata.get(group).and_then(Value::as_object)
                    else {
                        continue;
                    };
                    for (name, spec) in group_inputs {
                        if !has_explicit_input_order && group == "hidden" {
                            hidden_inputs.push(name.clone());
                        } else if !has_explicit_input_order && !ordered_inputs.contains(name) {
                            ordered_inputs.push(name.clone());
                        }
                        if let Some(mut input) = parse_input_schema(name, spec) {
                            input.required = group == "required";
                            inputs
                                .entry(name.clone())
                                .and_modify(|existing: &mut RecognitionInputSchema| {
                                    if input.required {
                                        *existing = input.clone();
                                    }
                                })
                                .or_insert(input);
                        }
                        collect_conditional_inputs(name, spec, &mut conditional_inputs);
                    }
                }
            }

            let raw_output_types = metadata
                .get("output")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::trim)
                .map(str::to_owned)
                .collect::<Vec<_>>();
            let declared_output_types = raw_output_types
                .iter()
                .map(|raw| RecognitionDeclaredType::from_schema_name(raw))
                .collect();
            let output_match_types = metadata
                .get("output_matchtypes")
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .map(|value| value.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            let output_names = metadata
                .get("output_name")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
            let output_is_list = metadata
                .get("output_is_list")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_bool)
                .collect();
            context.nodes.insert(
                class_type.clone(),
                RecognitionNodeSchema {
                    class_type: class_type.clone(),
                    display_name: metadata
                        .get("display_name")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    output_node: metadata
                        .get("output_node")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    raw_output_types,
                    declared_output_types,
                    output_match_types,
                    output_names,
                    output_is_list,
                    ordered_inputs,
                    hidden_inputs,
                    inputs,
                    conditional_inputs,
                },
            );
        }
        context
    }

    pub fn node(&self, class_type: &str) -> Option<&RecognitionNodeSchema> {
        self.nodes.get(class_type)
    }
}

fn collect_conditional_inputs(
    selector_name: &str,
    spec: &Value,
    conditional_inputs: &mut BTreeMap<
        String,
        BTreeMap<String, BTreeMap<String, RecognitionInputSchema>>,
    >,
) {
    let Some(constraints) = spec
        .as_array()
        .and_then(|values| values.get(1))
        .and_then(Value::as_object)
    else {
        return;
    };

    if let Some(options) = constraints.get("options").and_then(Value::as_array) {
        for option in options {
            let Some(selected_value) = option.get("key").and_then(Value::as_str) else {
                continue;
            };
            let Some(groups) = option.get("inputs").and_then(Value::as_object) else {
                continue;
            };
            collect_conditional_input_groups(
                selector_name,
                selected_value,
                groups,
                conditional_inputs,
            );
        }
    }

    if let Some(formats) = constraints.get("formats").and_then(Value::as_object) {
        for (selected_value, entries) in formats {
            let Some(entries) = entries.as_array() else {
                continue;
            };
            for entry in entries {
                let Some(entry) = entry.as_array() else {
                    continue;
                };
                let Some(input_name) = entry.first().and_then(Value::as_str) else {
                    continue;
                };
                let input_spec = Value::Array(entry.iter().skip(1).cloned().collect());
                if let Some(input) = parse_input_schema(input_name, &input_spec) {
                    conditional_inputs
                        .entry(selector_name.to_owned())
                        .or_default()
                        .entry(selected_value.clone())
                        .or_default()
                        .insert(input_name.to_owned(), input);
                }
            }
        }
    }
}

fn collect_conditional_input_groups(
    selector_name: &str,
    selected_value: &str,
    groups: &serde_json::Map<String, Value>,
    conditional_inputs: &mut BTreeMap<
        String,
        BTreeMap<String, BTreeMap<String, RecognitionInputSchema>>,
    >,
) {
    for group in ["required", "optional", "hidden"] {
        let Some(group_inputs) = groups.get(group).and_then(Value::as_object) else {
            continue;
        };
        for (input_name, input_spec) in group_inputs {
            if let Some(mut input) = parse_input_schema(input_name, input_spec) {
                input.required = group == "required";
                conditional_inputs
                    .entry(selector_name.to_owned())
                    .or_default()
                    .entry(selected_value.to_owned())
                    .or_default()
                    .insert(input_name.to_owned(), input);
            }
        }
    }
}

fn parse_input_schema(name: &str, spec: &Value) -> Option<RecognitionInputSchema> {
    let values = spec.as_array()?;
    let declared = values.first()?;
    let raw_type = raw_schema_type(declared);
    let (declared_type, enum_options, enum_values) = match declared {
        Value::String(value) => (
            RecognitionDeclaredType::from_schema_name(value),
            Vec::new(),
            Vec::new(),
        ),
        Value::Array(options) => (
            RecognitionDeclaredType::Enum,
            options
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
            options.clone(),
        ),
        _ => (RecognitionDeclaredType::Unknown, Vec::new(), Vec::new()),
    };
    let constraints = values.get(1).and_then(Value::as_object);
    let mut enum_options = enum_options;
    let mut enum_values = enum_values;
    if declared_type == RecognitionDeclaredType::Enum && enum_values.is_empty() {
        if let Some(options) = constraints
            .and_then(|value| value.get("options"))
            .and_then(Value::as_array)
        {
            enum_options = options
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
            enum_values = options.clone();
        }
    }
    let match_template = constraints.and_then(parse_match_template);
    let (dynamic_prefix, dynamic_names, dynamic_value_type) =
        constraints.map(parse_dynamic_metadata).unwrap_or_default();
    Some(RecognitionInputSchema {
        name: name.to_owned(),
        raw_type,
        declared_type,
        match_template,
        upload_media_kind: constraints.and_then(parse_upload_media_kind),
        required: false,
        enum_options,
        enum_values,
        numeric_min: constraints
            .and_then(|value| value.get("min"))
            .and_then(number_value),
        numeric_max: constraints
            .and_then(|value| value.get("max"))
            .and_then(number_value),
        numeric_step: constraints
            .and_then(|value| value.get("step"))
            .and_then(number_value),
        multiline: constraints
            .and_then(|value| value.get("multiline"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        default_value: constraints.and_then(|value| value.get("default")).cloned(),
        socketless: constraints
            .and_then(|value| value.get("socketless"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        dynamic_prefix,
        dynamic_names,
        dynamic_value_type,
    })
}

fn raw_schema_type(declared: &Value) -> String {
    match declared {
        Value::String(value) => value.trim().to_owned(),
        Value::Array(_) => "COMBO".to_owned(),
        _ => String::new(),
    }
}

fn parse_match_template(
    constraints: &serde_json::Map<String, Value>,
) -> Option<RecognitionMatchTypeTemplate> {
    let template = constraints.get("template")?.as_object()?;
    let template_id = template.get("template_id")?.as_str()?.trim();
    if template_id.is_empty() {
        return None;
    }
    let allowed_types = match template.get("allowed_types") {
        Some(Value::String(value)) => value
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_ascii_uppercase)
            .collect(),
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_ascii_uppercase)
            .collect(),
        _ => vec!["*".to_owned()],
    };
    Some(RecognitionMatchTypeTemplate {
        template_id: template_id.to_owned(),
        allowed_types,
    })
}

fn parse_dynamic_metadata(
    constraints: &serde_json::Map<String, Value>,
) -> (Option<String>, Vec<String>, Option<RecognitionDeclaredType>) {
    let template = constraints.get("template").and_then(Value::as_object);
    let prefix = template
        .and_then(|template| template.get("prefix"))
        .or_else(|| constraints.get("prefix"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let names = template
        .and_then(|template| template.get("names"))
        .or_else(|| constraints.get("names"))
        .and_then(Value::as_array)
        .map(|names| {
            names
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let dynamic_value_type = template
        .and_then(|template| template.get("input"))
        .and_then(Value::as_object)
        .and_then(|input| input.values().next())
        .and_then(Value::as_array)
        .and_then(|spec| spec.first())
        .and_then(Value::as_str)
        .and_then(|raw| raw.split(',').next())
        .map(RecognitionDeclaredType::from_schema_name);
    (prefix, names, dynamic_value_type)
}

fn parse_upload_media_kind(constraints: &serde_json::Map<String, Value>) -> Option<MediaKind> {
    let explicit_kinds = [
        ("image_upload", MediaKind::Image),
        ("video_upload", MediaKind::Video),
        ("audio_upload", MediaKind::Audio),
    ]
    .into_iter()
    .filter_map(|(key, kind)| {
        constraints
            .get(key)
            .and_then(Value::as_bool)
            .filter(|enabled| *enabled)
            .map(|_| kind)
    })
    .collect::<Vec<_>>();

    match explicit_kinds.as_slice() {
        [kind] => Some(*kind),
        [] => constraints
            .get("upload")
            .and_then(MediaKind::from_upload_value),
        _ => None,
    }
}

fn number_value(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
}

#[cfg(test)]
mod tests {
    use super::{RecognitionDeclaredType, RecognitionSchemaContext};
    use serde_json::json;

    #[test]
    fn parses_declared_types_options_ranges_and_outputs() {
        let context = RecognitionSchemaContext::parse(&json!({
            "CustomNode": {
                "input": {
                    "required": {
                        "text": ["STRING", {}],
                        "count": ["INT", {"min": 0, "max": 9, "step": 1}],
                        "strength": ["FLOAT", {"min": 0.0, "max": 1.0}],
                        "enabled": ["BOOLEAN", {}],
                        "image": ["IMAGE", {}],
                        "mask": ["MASK", {}],
                        "latent": ["LATENT", {}],
                        "conditioning": ["CONDITIONING", {}],
                        "model": ["MODEL", {}],
                        "video_model": ["VIDEO_MODEL", {}],
                        "video": ["VIDEO", {}],
                        "audio": ["AUDIO", {}],
                        "sampler": [["euler", "ddim"], {}]
                    },
                    "optional": {
                        "caption": ["STRING", {}]
                    }
                },
                "output": ["IMAGE", "VIDEO"],
                "output_node": true
            }
        }));

        let node = context
            .node("CustomNode")
            .expect("node schema should exist");
        assert!(node.output_node);
        assert_eq!(
            node.declared_output_types,
            vec![
                RecognitionDeclaredType::Image,
                RecognitionDeclaredType::Video
            ]
        );
        assert_eq!(
            node.input("text").unwrap().declared_type,
            RecognitionDeclaredType::String
        );
        assert_eq!(
            node.input("image").unwrap().declared_type,
            RecognitionDeclaredType::Image
        );
        assert_eq!(
            node.input("conditioning").unwrap().declared_type,
            RecognitionDeclaredType::Conditioning
        );
        assert_eq!(
            node.input("model").unwrap().declared_type,
            RecognitionDeclaredType::Model
        );
        assert_eq!(
            node.input("video_model").unwrap().declared_type,
            RecognitionDeclaredType::VideoModel
        );
        assert_eq!(node.input("count").unwrap().numeric_min, Some(0.0));
        assert_eq!(node.input("count").unwrap().numeric_max, Some(9.0));
        assert_eq!(node.input("count").unwrap().numeric_step, Some(1.0));
        assert_eq!(
            node.input("sampler").unwrap().declared_type,
            RecognitionDeclaredType::Enum
        );
        assert_eq!(
            node.input("sampler").unwrap().enum_options,
            vec!["euler".to_owned(), "ddim".to_owned()]
        );
        assert!(node.input("text").unwrap().required);
        assert!(!node.input("caption").unwrap().required);
    }

    #[test]
    fn unknown_declared_type_is_safe_and_distinct_from_json_value_kind() {
        let context = RecognitionSchemaContext::parse(&json!({
            "CustomNode": {
                "input": {
                    "required": {
                        "mystery": ["CUSTOM_SOCKET", {}]
                    }
                }
            }
        }));

        assert_eq!(
            context
                .node("CustomNode")
                .unwrap()
                .input("mystery")
                .unwrap()
                .declared_type,
            RecognitionDeclaredType::Unknown
        );
    }

    #[test]
    fn parses_upload_media_kind_separately_from_combo_enum_type() {
        let context = RecognitionSchemaContext::parse(&json!({
            "LoadVideo": {
                "input": {"required": {
                    "file": ["COMBO", {"video_upload": true}]
                }},
                "output": ["VIDEO"]
            },
            "OrdinaryCombo": {
                "input": {"required": {
                    "value": ["COMBO", {"options": ["one", "two"]}]
                }}
            }
        }));

        let video = context.node("LoadVideo").unwrap().input("file").unwrap();
        assert_eq!(video.declared_type, RecognitionDeclaredType::Enum);
        assert_eq!(video.upload_media_kind, Some(super::MediaKind::Video));
        assert_eq!(
            context
                .node("OrdinaryCombo")
                .unwrap()
                .input("value")
                .unwrap()
                .upload_media_kind,
            None
        );
    }

    #[test]
    fn ignores_ambiguous_upload_media_flags() {
        let context = RecognitionSchemaContext::parse(&json!({
            "AmbiguousUploader": {
                "input": {"required": {
                    "file": ["COMBO", {
                        "image_upload": true,
                        "video_upload": true,
                        "upload": "audio"
                    }]
                }}
            }
        }));

        assert_eq!(
            context
                .node("AmbiguousUploader")
                .unwrap()
                .input("file")
                .unwrap()
                .upload_media_kind,
            None
        );
    }
}
