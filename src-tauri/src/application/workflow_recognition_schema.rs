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
    Image,
    Mask,
    Latent,
    Video,
    Audio,
    Enum,
    Unknown,
}

impl RecognitionDeclaredType {
    pub fn from_schema_name(value: &str) -> Self {
        match value.trim().to_ascii_uppercase().as_str() {
            "STRING" | "TEXT" => Self::String,
            "INT" | "INTEGER" => Self::Integer,
            "FLOAT" | "NUMBER" => Self::Float,
            "BOOL" | "BOOLEAN" => Self::Boolean,
            "IMAGE" => Self::Image,
            "MASK" => Self::Mask,
            "LATENT" => Self::Latent,
            "VIDEO" => Self::Video,
            "AUDIO" => Self::Audio,
            "ENUM" | "COMBO" => Self::Enum,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionInputSchema {
    pub name: String,
    pub declared_type: RecognitionDeclaredType,
    pub upload_media_kind: Option<MediaKind>,
    pub required: bool,
    pub enum_options: Vec<String>,
    pub numeric_min: Option<f64>,
    pub numeric_max: Option<f64>,
    pub numeric_step: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionNodeSchema {
    pub class_type: String,
    pub output_node: bool,
    pub declared_output_types: Vec<RecognitionDeclaredType>,
    pub inputs: BTreeMap<String, RecognitionInputSchema>,
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
            if let Some(input_metadata) = metadata.get("input").and_then(Value::as_object) {
                for group in ["optional", "hidden", "required"] {
                    let Some(group_inputs) = input_metadata.get(group).and_then(Value::as_object)
                    else {
                        continue;
                    };
                    for (name, spec) in group_inputs {
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
                    }
                }
            }

            let declared_output_types = metadata
                .get("output")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(RecognitionDeclaredType::from_schema_name)
                .collect();
            context.nodes.insert(
                class_type.clone(),
                RecognitionNodeSchema {
                    class_type: class_type.clone(),
                    output_node: metadata
                        .get("output_node")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    declared_output_types,
                    inputs,
                },
            );
        }
        context
    }

    pub fn node(&self, class_type: &str) -> Option<&RecognitionNodeSchema> {
        self.nodes.get(class_type)
    }
}

fn parse_input_schema(name: &str, spec: &Value) -> Option<RecognitionInputSchema> {
    let values = spec.as_array()?;
    let declared = values.first()?;
    let (declared_type, enum_options) = match declared {
        Value::String(value) => (RecognitionDeclaredType::from_schema_name(value), Vec::new()),
        Value::Array(options) => (
            RecognitionDeclaredType::Enum,
            options
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
        ),
        _ => (RecognitionDeclaredType::Unknown, Vec::new()),
    };
    let constraints = values.get(1).and_then(Value::as_object);
    Some(RecognitionInputSchema {
        name: name.to_owned(),
        declared_type,
        upload_media_kind: constraints.and_then(parse_upload_media_kind),
        required: false,
        enum_options,
        numeric_min: constraints
            .and_then(|value| value.get("min"))
            .and_then(number_value),
        numeric_max: constraints
            .and_then(|value| value.get("max"))
            .and_then(number_value),
        numeric_step: constraints
            .and_then(|value| value.get("step"))
            .and_then(number_value),
    })
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
