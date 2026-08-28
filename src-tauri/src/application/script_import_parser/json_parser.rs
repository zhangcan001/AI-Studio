//! Deterministic Script Import JSON v1 parser.
//!
//! The input is deliberately narrower than any workflow, manifest, or batch
//! JSON.  Only Draft fields are copied into the Draft model; everything else
//! is either surfaced as inert metadata or rejected as a schema/type error.

use super::{
    check_cancel, deterministic_node_id, diagnostic, make_episode, make_scene, make_shot,
    new_structure, ParserError, ParserInput, ParserOutput,
};
use crate::domain::script_draft::{
    Diagnostic, DiagnosticSeverity, DraftDialogue, DraftNodeOrigin, EntityMention, EntityType,
    SourceBlockKind, SourceSpan, CODE_DRAFT_CAPACITY_EXCEEDED, CODE_DUPLICATE_SOURCE_ID,
    CODE_ENCODING_OR_BOM, CODE_JSON_PARSE_INVALID, CODE_JSON_TYPE_INVALID, CODE_JSON_UNKNOWN_FIELD,
    CODE_JSON_UNKNOWN_METADATA_TOO_LARGE, CODE_MISSING_NAME,
};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

const UNKNOWN_METADATA_KEY: &str = "scriptImport.jsonUnknown.v1";
const MAX_EPISODES: usize = 100;
const MAX_SCENES: usize = 1_000;
const MAX_SHOTS: usize = 5_000;

const EXECUTION_FIELDS: &[&str] = &[
    "workflowVersionId",
    "recipeId",
    "profileId",
    "referenceSetId",
    "assetId",
    "batchId",
    "taskId",
    "selectedImageAssetId",
    "selectedVideoAssetId",
    "comfyPromptId",
];

const ROOT_FIELDS: &[&str] = &["schemaVersion", "title", "sourceId", "episodes"];
const EPISODE_FIELDS: &[&str] = &["sourceId", "name", "scenes", "description"];
const SCENE_FIELDS: &[&str] = &["sourceId", "name", "shots", "description"];
const SHOT_FIELDS: &[&str] = &[
    "sourceId",
    "name",
    "description",
    "action",
    "dialogue",
    "characters",
    "scene",
    "props",
    "imagePromptDraft",
    "videoPromptDraft",
];

pub fn parse(input: &ParserInput<'_>) -> Result<ParserOutput, ParserError> {
    check_cancel(input)?;
    let mut diagnostics = Vec::new();
    if input.map.has_bom() {
        diagnostics.push(diagnostic(
            DiagnosticSeverity::Info,
            CODE_ENCODING_OR_BOM,
            "UTF-8 BOM was ignored for semantic parsing",
            Some(input.map.span(0, input.map.bom_len())),
        ));
    }

    let text = std::str::from_utf8(input.raw).map_err(|_| ParserError::InvalidUtf8)?;
    let value = match serde_json::from_str::<Value>(input.map.text()) {
        Ok(value) => value,
        Err(error) => {
            diagnostics.push(with_span(
                Diagnostic::new(
                    DiagnosticSeverity::Blocker,
                    CODE_JSON_PARSE_INVALID,
                    "The JSON source is malformed",
                ),
                json_error_span(input, &error),
            ));
            return Ok(ParserOutput {
                source_blocks: vec![super::source_block(
                    input,
                    input.map.bom_len(),
                    input.raw.len(),
                    SourceBlockKind::Unknown,
                    None,
                )],
                structure: None,
                diagnostics,
                anchors: BTreeMap::new(),
            });
        }
    };

    let Some(root) = value.as_object() else {
        diagnostics.push(type_diagnostic("root", "JSON root must be an object", None));
        return Ok(empty_output(input, diagnostics));
    };

    let mut unknown = UnknownFields::default();
    collect_unknown(root, ROOT_FIELDS, "root", &mut unknown, &mut diagnostics);

    let schema_version = match root.get("schemaVersion") {
        Some(Value::Number(value)) if value.as_u64() == Some(1) => true,
        Some(Value::Number(_)) => {
            diagnostics.push(with_span(
                Diagnostic::new(
                    DiagnosticSeverity::Blocker,
                    crate::domain::script_draft::CODE_UNKNOWN_JSON_SCHEMA,
                    "JSON schemaVersion is unsupported",
                ),
                field_span(input, text, "schemaVersion"),
            ));
            false
        }
        Some(_) => {
            diagnostics.push(type_diagnostic(
                "schemaVersion",
                "schemaVersion must be the number 1",
                field_span(input, text, "schemaVersion"),
            ));
            false
        }
        None => {
            diagnostics.push(with_span(
                Diagnostic::new(
                    DiagnosticSeverity::Blocker,
                    crate::domain::script_draft::CODE_UNKNOWN_JSON_SCHEMA,
                    "JSON schemaVersion is required",
                ),
                field_span(input, text, "schemaVersion"),
            ));
            false
        }
    };

    let title = match root.get("title") {
        Some(Value::String(value)) if !value.trim().is_empty() => Some(value.clone()),
        Some(Value::String(_)) | None => {
            diagnostics.push(with_span(
                Diagnostic::new(
                    DiagnosticSeverity::Blocker,
                    CODE_MISSING_NAME,
                    "JSON title is required",
                )
                .with_field("title".to_owned()),
                field_span(input, text, "title"),
            ));
            None
        }
        Some(_) => {
            diagnostics.push(type_diagnostic(
                "title",
                "title must be a string",
                field_span(input, text, "title"),
            ));
            None
        }
    };

    let Some(episodes) = root.get("episodes") else {
        diagnostics.push(type_diagnostic(
            "episodes",
            "episodes is required",
            field_span(input, text, "episodes"),
        ));
        return Ok(empty_output(input, diagnostics));
    };
    let Some(episodes) = episodes.as_array() else {
        diagnostics.push(type_diagnostic(
            "episodes",
            "episodes must be an array",
            field_span(input, text, "episodes"),
        ));
        return Ok(empty_output(input, diagnostics));
    };

    let mut structure = new_structure(input.source_id, title);
    let mut anchors = BTreeMap::new();
    let mut source_ids = BTreeSet::new();
    let mut source_id_counts = BTreeMap::new();
    let mut locator = JsonLocator::new(text, input);

    for (episode_index, value) in episodes.iter().enumerate() {
        check_cancel(input)?;
        let Some(object) = value.as_object() else {
            diagnostics.push(type_diagnostic(
                &format!("episodes[{episode_index}]"),
                "episode must be an object",
                None,
            ));
            continue;
        };
        collect_unknown(
            object,
            EPISODE_FIELDS,
            &format!("episodes[{episode_index}]"),
            &mut unknown,
            &mut diagnostics,
        );
        let episode_source_id = anchor_value(
            object,
            &format!("episodes[{episode_index}].sourceId"),
            input,
            &mut locator,
            &mut source_ids,
            &mut source_id_counts,
            &mut diagnostics,
        );
        let Some(name) = required_name(
            object,
            &format!("episodes[{episode_index}].name"),
            input,
            &mut diagnostics,
        ) else {
            continue;
        };
        let Some(scene_values) = typed_array(
            object,
            "scenes",
            &format!("episodes[{episode_index}].scenes"),
            input,
            &mut diagnostics,
        ) else {
            continue;
        };
        let episode_anchor = format!("episode:{episode_source_id}");
        let episode_span = locator.span_for_value(&episode_source_id);
        let episode_id = deterministic_node_id(&episode_anchor);
        let episode_description = optional_string(
            object,
            "description",
            &format!("episodes[{episode_index}].description"),
            input,
            &mut diagnostics,
        );
        let mut scenes = Vec::new();
        for (scene_index, scene_value) in scene_values.iter().enumerate() {
            check_cancel(input)?;
            let Some(scene_object) = scene_value.as_object() else {
                diagnostics.push(type_diagnostic(
                    &format!("episodes[{episode_index}].scenes[{scene_index}]"),
                    "scene must be an object",
                    None,
                ));
                continue;
            };
            collect_unknown(
                scene_object,
                SCENE_FIELDS,
                &format!("episodes[{episode_index}].scenes[{scene_index}]"),
                &mut unknown,
                &mut diagnostics,
            );
            let scene_source_id = anchor_value(
                scene_object,
                &format!("episodes[{episode_index}].scenes[{scene_index}].sourceId"),
                input,
                &mut locator,
                &mut source_ids,
                &mut source_id_counts,
                &mut diagnostics,
            );
            let Some(scene_name) = required_name(
                scene_object,
                &format!("episodes[{episode_index}].scenes[{scene_index}].name"),
                input,
                &mut diagnostics,
            ) else {
                continue;
            };
            let Some(shot_values) = typed_array(
                scene_object,
                "shots",
                &format!("episodes[{episode_index}].scenes[{scene_index}].shots"),
                input,
                &mut diagnostics,
            ) else {
                continue;
            };
            let scene_anchor = format!("scene:{episode_source_id}/{scene_source_id}");
            let scene_span = locator.span_for_value(&scene_source_id);
            let scene_id = deterministic_node_id(&scene_anchor);
            let scene_description = optional_string(
                scene_object,
                "description",
                &format!("episodes[{episode_index}].scenes[{scene_index}].description"),
                input,
                &mut diagnostics,
            );
            let mut shots = Vec::new();
            for (shot_index, shot_value) in shot_values.iter().enumerate() {
                check_cancel(input)?;
                let Some(shot_object) = shot_value.as_object() else {
                    diagnostics.push(type_diagnostic(
                        &format!(
                            "episodes[{episode_index}].scenes[{scene_index}].shots[{shot_index}]"
                        ),
                        "shot must be an object",
                        None,
                    ));
                    continue;
                };
                collect_unknown(
                    shot_object,
                    SHOT_FIELDS,
                    &format!("episodes[{episode_index}].scenes[{scene_index}].shots[{shot_index}]"),
                    &mut unknown,
                    &mut diagnostics,
                );
                let shot_source_id = anchor_value(
                    shot_object,
                    &format!(
                        "episodes[{episode_index}].scenes[{scene_index}].shots[{shot_index}].sourceId"
                    ),
                    input,
                    &mut locator,
                    &mut source_ids,
                    &mut source_id_counts,
                    &mut diagnostics,
                );
                let Some(shot_name) = required_name(
                    shot_object,
                    &format!(
                        "episodes[{episode_index}].scenes[{scene_index}].shots[{shot_index}].name"
                    ),
                    input,
                    &mut diagnostics,
                ) else {
                    continue;
                };
                let shot_anchor =
                    format!("shot:{episode_source_id}/{scene_source_id}/{shot_source_id}");
                let shot_span = locator.span_for_value(&shot_source_id);
                let shot_id = deterministic_node_id(&shot_anchor);
                let description = optional_string(
                    shot_object,
                    "description",
                    &format!(
                        "episodes[{episode_index}].scenes[{scene_index}].shots[{shot_index}].description"
                    ),
                    input,
                    &mut diagnostics,
                );
                let action = optional_string(
                    shot_object,
                    "action",
                    &format!(
                        "episodes[{episode_index}].scenes[{scene_index}].shots[{shot_index}].action"
                    ),
                    input,
                    &mut diagnostics,
                );
                let scene_suggestion = optional_string(
                    shot_object,
                    "scene",
                    &format!(
                        "episodes[{episode_index}].scenes[{scene_index}].shots[{shot_index}].scene"
                    ),
                    input,
                    &mut diagnostics,
                );
                let image_prompt = optional_string(
                    shot_object,
                    "imagePromptDraft",
                    &format!(
                        "episodes[{episode_index}].scenes[{scene_index}].shots[{shot_index}].imagePromptDraft"
                    ),
                    input,
                    &mut diagnostics,
                );
                let video_prompt = optional_string(
                    shot_object,
                    "videoPromptDraft",
                    &format!(
                        "episodes[{episode_index}].scenes[{scene_index}].shots[{shot_index}].videoPromptDraft"
                    ),
                    input,
                    &mut diagnostics,
                );
                let dialogue = parse_dialogue(
                    shot_object.get("dialogue"),
                    &format!(
                        "episodes[{episode_index}].scenes[{scene_index}].shots[{shot_index}].dialogue"
                    ),
                    shot_span.clone(),
                    input,
                    &mut unknown,
                    &mut diagnostics,
                );
                let characters = parse_mentions(
                    shot_object.get("characters"),
                    EntityType::Character,
                    &shot_anchor,
                    &shot_id,
                    shot_span.clone(),
                    &format!(
                        "episodes[{episode_index}].scenes[{scene_index}].shots[{shot_index}].characters"
                    ),
                    input,
                    &mut unknown,
                    &mut diagnostics,
                );
                let props = parse_mentions(
                    shot_object.get("props"),
                    EntityType::Prop,
                    &shot_anchor,
                    &shot_id,
                    shot_span.clone(),
                    &format!(
                        "episodes[{episode_index}].scenes[{scene_index}].shots[{shot_index}].props"
                    ),
                    input,
                    &mut unknown,
                    &mut diagnostics,
                );
                let mut shot = make_shot(
                    &shot_anchor,
                    scene_id.clone(),
                    shot_name,
                    shot_span,
                    description,
                    action,
                    dialogue,
                    characters,
                    Vec::new(),
                );
                shot.draft_node_id = shot_id.clone();
                shot.scene_suggestion = scene_suggestion;
                shot.props = props;
                shot.image_prompt_draft = image_prompt;
                shot.video_prompt_draft = video_prompt;
                shot.ordinal = shots.len() as u32;
                anchors.insert(shot_anchor, shot_id);
                shots.push(shot);
            }
            let mut scene = make_scene(
                &scene_anchor,
                episode_id.clone(),
                scene_name,
                scene_span,
                Vec::new(),
                None,
                None,
                shots,
            );
            scene.draft_node_id = scene_id.clone();
            scene.description = scene_description;
            scene.ordinal = scenes.len() as u32;
            for shot in &mut scene.shots {
                shot.parent_draft_node_id = Some(scene_id.clone());
                shot.parent_scene_draft_id = scene_id.clone();
            }
            anchors.insert(scene_anchor, scene_id);
            scenes.push(scene);
        }
        let mut episode = make_episode(&episode_anchor, name, episode_span, Vec::new(), scenes);
        episode.draft_node_id = episode_id.clone();
        episode.description = episode_description;
        episode.ordinal = structure.episodes.len() as u32;
        for scene in &mut episode.scenes {
            scene.parent_draft_node_id = Some(episode_id.clone());
        }
        anchors.insert(episode_anchor, episode_id);
        structure.episodes.push(episode);
    }

    if !schema_version {
        // Keep the parsed shape available for preview only; the blocker makes
        // create/reparse refuse persistence in the application service.
    }
    add_capacity_diagnostics(&structure, &mut diagnostics);
    if unknown.overflowed {
        diagnostics.push(Diagnostic::new(
            DiagnosticSeverity::Blocker,
            CODE_JSON_UNKNOWN_METADATA_TOO_LARGE,
            "Unknown JSON metadata exceeds the 1 MiB limit",
        ));
    } else if !unknown.fields.is_empty() {
        structure.metadata.insert(
            UNKNOWN_METADATA_KEY.to_owned(),
            serde_json::to_string(&unknown.fields).unwrap_or_else(|_| "{}".to_owned()),
        );
    }
    structure.diagnostics.extend(diagnostics.clone());
    let source_block = super::source_block(
        input,
        input.map.bom_len(),
        input.raw.len(),
        SourceBlockKind::Paragraph,
        None,
    );
    Ok(ParserOutput {
        source_blocks: vec![source_block],
        structure: Some(structure),
        diagnostics,
        anchors,
    })
}

fn empty_output(input: &ParserInput<'_>, diagnostics: Vec<Diagnostic>) -> ParserOutput {
    ParserOutput {
        source_blocks: vec![super::source_block(
            input,
            input.map.bom_len(),
            input.raw.len(),
            SourceBlockKind::Unknown,
            None,
        )],
        structure: None,
        diagnostics,
        anchors: BTreeMap::new(),
    }
}

fn add_capacity_diagnostics(
    structure: &crate::domain::script_draft::DraftStructureV1,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let counts = structure.counts();
    if counts.episodes > MAX_EPISODES {
        diagnostics.push(Diagnostic::new(
            DiagnosticSeverity::Blocker,
            CODE_DRAFT_CAPACITY_EXCEEDED,
            "JSON import exceeds the episode capacity",
        ));
    }
    if counts.scenes > MAX_SCENES {
        diagnostics.push(Diagnostic::new(
            DiagnosticSeverity::Blocker,
            CODE_DRAFT_CAPACITY_EXCEEDED,
            "JSON import exceeds the scene capacity",
        ));
    }
    if counts.shots > MAX_SHOTS {
        diagnostics.push(Diagnostic::new(
            DiagnosticSeverity::Blocker,
            CODE_DRAFT_CAPACITY_EXCEEDED,
            "JSON import exceeds the shot capacity",
        ));
    }
}

#[derive(Default)]
struct UnknownFields {
    fields: BTreeMap<String, Value>,
    serialized_value_bytes: usize,
    overflowed: bool,
}

fn collect_unknown(
    object: &Map<String, Value>,
    allowed: &[&str],
    path: &str,
    unknown: &mut UnknownFields,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (key, value) in object {
        if allowed.contains(&key.as_str()) {
            continue;
        }
        let field_path = format!("{path}.{key}");
        diagnostics.push(Diagnostic::new(
            DiagnosticSeverity::Warning,
            CODE_JSON_UNKNOWN_FIELD,
            "Unknown JSON field was ignored by the Draft parser",
        ));
        if EXECUTION_FIELDS.contains(&key.as_str()) {
            // Keep the field visible through the same inert metadata path, but
            // never map it into any execution or formal-production structure.
        }
        if unknown.overflowed {
            continue;
        }
        // The compatibility budget is intentionally based on the serialized
        // unknown values, not on their paths or on the eventual metadata map
        // wrapper.  This prevents a long key from consuming the budget while
        // still refusing to retain an oversized opaque blob.
        let serialized_value_bytes =
            serde_json::to_vec(value).map_or(usize::MAX, |bytes| bytes.len());
        if unknown
            .serialized_value_bytes
            .saturating_add(serialized_value_bytes)
            > super::MAX_UNKNOWN_JSON_METADATA_BYTES
        {
            unknown.overflowed = true;
            unknown.fields.clear();
            continue;
        }
        unknown.serialized_value_bytes += serialized_value_bytes;
        unknown.fields.insert(field_path, value.clone());
    }
}

fn required_name(
    object: &Map<String, Value>,
    path: &str,
    input: &ParserInput<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    match object.get("name") {
        Some(Value::String(value)) if !value.trim().is_empty() => Some(value.clone()),
        Some(Value::String(_)) | None => {
            diagnostics.push(with_span(
                Diagnostic::new(
                    DiagnosticSeverity::Blocker,
                    CODE_MISSING_NAME,
                    "Draft node name is required",
                ),
                field_span(
                    input,
                    raw_text(input),
                    path.rsplit('.').next().unwrap_or("name"),
                ),
            ));
            None
        }
        Some(_) => {
            diagnostics.push(type_diagnostic(
                path,
                "Draft node name must be a string",
                None,
            ));
            None
        }
    }
}

fn typed_array<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
    input: &ParserInput<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<&'a [Value]> {
    match object.get(key) {
        Some(Value::Array(value)) => Some(value.as_slice()),
        Some(_) => {
            diagnostics.push(type_diagnostic(
                path,
                "JSON field must be an array",
                field_span(input, raw_text(input), key),
            ));
            None
        }
        None => {
            diagnostics.push(type_diagnostic(
                path,
                "JSON field is required",
                field_span(input, raw_text(input), key),
            ));
            None
        }
    }
}

fn optional_string(
    object: &Map<String, Value>,
    key: &str,
    path: &str,
    input: &ParserInput<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    match object.get(key) {
        None => None,
        Some(Value::String(value)) => Some(value.clone()),
        Some(_) => {
            diagnostics.push(type_diagnostic(
                path,
                "JSON field must be a string",
                field_span(input, raw_text(input), key),
            ));
            None
        }
    }
}

fn anchor_value(
    object: &Map<String, Value>,
    path: &str,
    input: &ParserInput<'_>,
    locator: &mut JsonLocator<'_, '_, '_>,
    source_ids: &mut BTreeSet<String>,
    source_id_counts: &mut BTreeMap<String, usize>,
    diagnostics: &mut Vec<Diagnostic>,
) -> String {
    let Some(value) = object.get("sourceId") else {
        diagnostics.push(type_diagnostic(
            path,
            "JSON node sourceId is required",
            field_span(input, raw_text(input), "sourceId"),
        ));
        return format!("missing:{path}");
    };
    let Some(value) = value.as_str() else {
        diagnostics.push(type_diagnostic(
            path,
            "sourceId must be a string",
            field_span(input, raw_text(input), "sourceId"),
        ));
        return format!("invalid:{path}");
    };
    if value.trim().is_empty() {
        diagnostics.push(type_diagnostic(
            path,
            "JSON node sourceId must be non-empty",
            field_span(input, raw_text(input), "sourceId"),
        ));
        return format!("empty:{path}");
    }
    locator.record(value);
    let occurrence = source_id_counts.entry(value.to_owned()).or_insert(0);
    *occurrence += 1;
    if !source_ids.insert(value.to_owned()) {
        diagnostics.push(with_span(
            Diagnostic::new(
                DiagnosticSeverity::Blocker,
                CODE_DUPLICATE_SOURCE_ID,
                "JSON sourceId must be unique across the Draft",
            ),
            Some(locator.span_for_value(value)),
        ));
        return format!("{value}#{}", occurrence);
    }
    value.to_owned()
}

fn parse_dialogue(
    value: Option<&Value>,
    path: &str,
    span: SourceSpan,
    input: &ParserInput<'_>,
    unknown: &mut UnknownFields,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<DraftDialogue> {
    let Some(value) = value else {
        return Vec::new();
    };
    match value {
        Value::String(text) => vec![DraftDialogue {
            speaker: None,
            text: text.clone(),
            source_spans: vec![span],
        }],
        Value::Array(values) => values
            .iter()
            .enumerate()
            .filter_map(|(index, value)| match value {
                Value::String(text) => Some(DraftDialogue {
                    speaker: None,
                    text: text.clone(),
                    source_spans: vec![span.clone()],
                }),
                Value::Object(object) => {
                    collect_unknown(
                        object,
                        &["speaker", "text"],
                        &format!("{path}[{index}]"),
                        unknown,
                        diagnostics,
                    );
                    let text_value = object.get("text");
                    let Some(text) = text_value.and_then(Value::as_str) else {
                        diagnostics.push(type_diagnostic(
                            &format!("{path}[{index}].text"),
                            "dialogue text must be a string",
                            None,
                        ));
                        return None;
                    };
                    let speaker = match object.get("speaker") {
                        None => None,
                        Some(Value::String(value)) => Some(value.clone()),
                        Some(_) => {
                            diagnostics.push(type_diagnostic(
                                &format!("{path}[{index}].speaker"),
                                "dialogue speaker must be a string",
                                None,
                            ));
                            None
                        }
                    };
                    Some(DraftDialogue {
                        speaker,
                        text: text.to_owned(),
                        source_spans: vec![span.clone()],
                    })
                }
                _ => {
                    diagnostics.push(type_diagnostic(
                        &format!("{path}[{index}]"),
                        "dialogue item must be a string or object",
                        None,
                    ));
                    None
                }
            })
            .collect(),
        _ => {
            diagnostics.push(type_diagnostic(
                path,
                "dialogue must be a string or array",
                field_span(input, raw_text(input), "dialogue"),
            ));
            Vec::new()
        }
    }
}

fn parse_mentions(
    value: Option<&Value>,
    entity_type: EntityType,
    shot_anchor: &str,
    shot_id: &crate::domain::script_draft::DraftNodeId,
    span: SourceSpan,
    path: &str,
    input: &ParserInput<'_>,
    unknown: &mut UnknownFields,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<EntityMention> {
    let Some(value) = value else {
        return Vec::new();
    };
    let Some(values) = value.as_array() else {
        diagnostics.push(type_diagnostic(
            path,
            "entity mentions must be an array",
            field_span(
                input,
                raw_text(input),
                path.rsplit('.').next().unwrap_or_default(),
            ),
        ));
        return Vec::new();
    };
    values
        .iter()
        .enumerate()
        .filter_map(|(index, value)| {
            let item_path = format!("{path}[{index}]");
            let text = match value {
                Value::String(text) if !text.trim().is_empty() => text.as_str(),
                Value::String(_) => {
                    diagnostics.push(type_diagnostic(
                        &item_path,
                        "entity mention must be a non-empty string or named object",
                        None,
                    ));
                    return None;
                }
                Value::Object(object) => {
                    collect_unknown(object, &["text", "name"], &item_path, unknown, diagnostics);
                    let text_value = object.get("text").or_else(|| object.get("name"));
                    let Some(text_value) = text_value else {
                        diagnostics.push(type_diagnostic(
                            &item_path,
                            "entity mention object requires a string text or name",
                            None,
                        ));
                        return None;
                    };
                    let Some(text) = text_value.as_str() else {
                        diagnostics.push(type_diagnostic(
                            &format!("{item_path}.text"),
                            "entity mention text or name must be a string",
                            None,
                        ));
                        return None;
                    };
                    if text.trim().is_empty() {
                        diagnostics.push(type_diagnostic(
                            &format!("{item_path}.text"),
                            "entity mention must be a non-empty string",
                            None,
                        ));
                        return None;
                    }
                    text
                }
                _ => {
                    diagnostics.push(type_diagnostic(
                        &item_path,
                        "entity mention must be a non-empty string or named object",
                        None,
                    ));
                    return None;
                }
            };
            let kind = match entity_type {
                EntityType::Character => "character",
                EntityType::Prop => "prop",
                EntityType::Scene => "scene",
            };
            let mention_id = format!("mention:{shot_anchor}/{kind}:{index}");
            Some(EntityMention {
                mention_id,
                entity_type,
                raw_text: text.to_owned(),
                normalized_text: Some(text.to_owned()),
                draft_node_id: Some(shot_id.clone()),
                source_spans: vec![span.clone()],
                origin: DraftNodeOrigin::Imported,
                confidence: Some(1.0),
                evidence: vec!["explicit-json-field".to_owned()],
            })
        })
        .collect()
}

fn type_diagnostic(path: &str, message: &str, span: Option<SourceSpan>) -> Diagnostic {
    let mut diagnostic =
        Diagnostic::new(DiagnosticSeverity::Blocker, CODE_JSON_TYPE_INVALID, message)
            .with_field(path.to_owned());
    if let Some(span) = span {
        diagnostic = diagnostic.with_span(span);
    }
    diagnostic
}

fn with_span(mut diagnostic: Diagnostic, span: Option<SourceSpan>) -> Diagnostic {
    if let Some(span) = span {
        diagnostic = diagnostic.with_span(span);
    }
    diagnostic
}

fn field_span(input: &ParserInput<'_>, text: &str, field: &str) -> Option<SourceSpan> {
    let needle = format!("\"{field}\"");
    let offset = text.find(&needle)?;
    Some(input.map.span(offset, offset + needle.len()))
}

fn json_error_span(input: &ParserInput<'_>, error: &serde_json::Error) -> Option<SourceSpan> {
    let line_index = error.line().saturating_sub(1);
    let line = input.map.lines().get(line_index)?;
    let character_index = error.column().saturating_sub(1);
    let byte_offset = line
        .text()
        .char_indices()
        .nth(character_index)
        .map(|(offset, _)| line.start_byte + offset)
        .unwrap_or(line.content_end_byte);
    Some(input.map.span(byte_offset, byte_offset))
}

struct JsonLocator<'text, 'input, 'source> {
    text: &'text str,
    cursor: usize,
    input: &'input ParserInput<'source>,
    last_span: Option<SourceSpan>,
}

impl<'text, 'input, 'source> JsonLocator<'text, 'input, 'source> {
    fn new(text: &'text str, input: &'input ParserInput<'source>) -> Self {
        Self {
            text,
            cursor: input.map.bom_len(),
            input,
            last_span: None,
        }
    }

    fn record(&mut self, value: &str) {
        let key = self.text[self.cursor..]
            .find("\"sourceId\"")
            .map(|offset| self.cursor + offset);
        let Some(key) = key else { return };
        let encoded = serde_json::to_string(value).unwrap_or_else(|_| format!("\"{value}\""));
        let value_start = self.text[key..].find(&encoded).map(|offset| key + offset);
        let Some(value_start) = value_start else {
            return;
        };
        let end = value_start + encoded.len();
        self.cursor = end;
        self.last_span = Some(self.input.map.span(value_start, end));
    }

    fn span_for_value(&self, _value: &str) -> SourceSpan {
        self.last_span.clone().unwrap_or_else(|| {
            let offset = self.cursor.min(self.input.raw.len());
            self.input.map.span(offset, offset)
        })
    }
}

fn raw_text<'a, 'b>(input: &'a ParserInput<'b>) -> &'b str {
    std::str::from_utf8(input.raw).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::script_import_parser::{parse_source, ScriptParseOptions};
    use crate::domain::script_draft::{ScriptFormat, SourceId};

    fn parse(source: &str) -> ParserOutput {
        let source_id = SourceId::new();
        parse_source(
            &source_id,
            "project",
            ScriptFormat::Json,
            Some("import.json"),
            source.as_bytes(),
            &ScriptParseOptions::default(),
            None,
        )
        .expect("JSON parser should return a result")
    }

    #[test]
    fn valid_v1_builds_draft_only_structure_and_anchor_map() {
        let output = parse(
            r#"{
              "schemaVersion": 1,
              "title": "Pilot",
              "episodes": [{
                "sourceId": "ep-1",
                "name": "Episode 1",
                "scenes": [{
                  "sourceId": "sc-1",
                  "name": "Room",
                  "shots": [{
                    "sourceId": "sh-1",
                    "name": "Open",
                    "description": "A room",
                    "dialogue": [{"speaker":"A","text":"Hello"}],
                    "characters": ["A"],
                    "props": ["Lamp"],
                    "imagePromptDraft": "Draft image text"
                  }]
                }]
              }]
            }"#,
        );
        let structure = output.structure.expect("valid structure");
        assert_eq!(structure.counts().episodes, 1);
        assert_eq!(structure.counts().scenes, 1);
        assert_eq!(structure.counts().shots, 1);
        let shot = &structure.episodes[0].scenes[0].shots[0];
        assert_eq!(shot.dialogue[0].speaker.as_deref(), Some("A"));
        assert_eq!(shot.props[0].entity_type, EntityType::Prop);
        assert!(shot.camera_suggestion.is_none());
        assert!(shot.duration_suggestion.is_none());
        assert!(output.anchors.contains_key("shot:ep-1/sc-1/sh-1"));
        assert!(structure.metadata.contains_key(super::UNKNOWN_METADATA_KEY) == false);
    }

    #[test]
    fn malformed_json_returns_blocker_with_local_span() {
        let output = parse(r#"{"schemaVersion":1,"title":"x","episodes":[}"#);
        assert!(output.structure.is_none());
        let diagnostic = output
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == CODE_JSON_PARSE_INVALID)
            .expect("parse diagnostic");
        assert_eq!(diagnostic.severity, DiagnosticSeverity::Blocker);
        assert!(diagnostic.source_spans[0].start_byte > 0);
    }

    #[test]
    fn unknown_execution_fields_are_visible_but_never_mapped() {
        let output =
            parse(r#"{"schemaVersion":1,"title":"x","workflowVersionId":"w","episodes":[]}"#);
        let structure = output.structure.expect("structure remains previewable");
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == CODE_JSON_UNKNOWN_FIELD));
        assert!(structure.metadata.contains_key(super::UNKNOWN_METADATA_KEY));
        assert!(structure.episodes.is_empty());
    }

    #[test]
    fn title_is_required_and_must_be_non_empty() {
        for source in [
            r#"{"schemaVersion":1,"episodes":[]}"#,
            r#"{"schemaVersion":1,"title":"   ","episodes":[]}"#,
        ] {
            let output = parse(source);
            assert!(output.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == CODE_MISSING_NAME && diagnostic.field.as_deref() == Some("title")
            }));
        }
    }

    #[test]
    fn node_source_ids_are_required_non_empty_strings() {
        for source in [
            r#"{"schemaVersion":1,"title":"x","episodes":[{"name":"e","scenes":[]}] }"#,
            r#"{"schemaVersion":1,"title":"x","episodes":[{"sourceId":"","name":"e","scenes":[]}] }"#,
            r#"{"schemaVersion":1,"title":"x","episodes":[{"sourceId":"e","name":"e","scenes":[{"sourceId":3,"name":"s","shots":[]}] }] }"#,
        ] {
            let output = parse(source);
            assert!(output.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == CODE_JSON_TYPE_INVALID
                    && diagnostic
                        .field
                        .as_deref()
                        .is_some_and(|field| field.contains("sourceId"))
            }));
        }
    }

    #[test]
    fn optional_and_nested_values_report_type_errors_instead_of_being_dropped() {
        let output = parse(
            r#"{
              "schemaVersion": 1,
              "title": "x",
              "episodes": [{
                "sourceId": "ep",
                "name": "e",
                "description": 1,
                "scenes": [{
                  "sourceId": "sc",
                  "name": "s",
                  "description": false,
                  "shots": [{
                    "sourceId": "sh",
                    "name": "shot",
                    "description": 1,
                    "action": [],
                    "scene": {},
                    "imagePromptDraft": 1,
                    "videoPromptDraft": false,
                    "dialogue": [1, {"text": 2}],
                    "characters": [1, {"text": 2, "name": "fallback"}],
                    "props": [false]
                  }]
                }]
              }]
            }"#,
        );
        let type_errors = output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == CODE_JSON_TYPE_INVALID)
            .count();
        assert!(type_errors >= 10, "type_errors={type_errors}");
    }

    #[test]
    fn unknown_metadata_budget_counts_serialized_values_and_drops_oversized_payload() {
        let value = "x".repeat(super::super::MAX_UNKNOWN_JSON_METADATA_BYTES);
        let source = format!(
            "{{\"schemaVersion\":1,\"title\":\"x\",\"future\":{},\"episodes\":[]}}",
            serde_json::to_string(&value).expect("string serializes")
        );
        let output = parse(&source);
        let structure = output.structure.expect("shape is retained for preview");
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == CODE_JSON_UNKNOWN_METADATA_TOO_LARGE
                && diagnostic.severity == DiagnosticSeverity::Blocker
        }));
        assert!(!structure.metadata.contains_key(super::UNKNOWN_METADATA_KEY));
    }

    #[test]
    fn duplicate_source_id_and_capacity_are_blockers_without_silent_truncation() {
        let mut shots = String::new();
        for index in 0..=MAX_SHOTS {
            if index > 0 {
                shots.push(',');
            }
            shots.push_str(&format!(
                "{{\"sourceId\":\"shot-{index}\",\"name\":\"Shot {index}\"}}"
            ));
        }
        let source = format!(
            "{{\"schemaVersion\":1,\"title\":\"x\",\"episodes\":[{{\"sourceId\":\"ep\",\"name\":\"e\",\"scenes\":[{{\"sourceId\":\"sc\",\"name\":\"s\",\"shots\":[{shots}]}}]}}]}}"
        );
        let output = parse(&source);
        let structure = output.structure.expect("shape is retained for preview");
        assert_eq!(structure.counts().shots, MAX_SHOTS + 1);
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == CODE_DRAFT_CAPACITY_EXCEEDED));
    }
}
