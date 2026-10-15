//! Local H3-Base presets, not the hosted H3-Regenerate-2K pipeline.
//! Sources: https://huggingface.co/MiniMaxAI/MiniMax-H3
//! https://docs.comfy.org/tutorials/video/minimax/minimax-h3-native
//!
//! Resolution ladder is a megapixel series. 16:9 sizes are the product table
//! below (each side a multiple of 32). 9:16 swaps width/height. For 1:1, 4:3,
//! and 3:4 the same megapixel target is used with:
//!   w = align32(sqrt(mp * 1_000_000 * rw / rh))
//!   h = align32(w * rh / rw)
//! where align32 rounds to the nearest multiple of 32 (minimum 32).
use crate::application::workflow_graph_analysis::WorkflowGraph;
use crate::application::workflow_semantic_graph::ActiveDependencyGraph;
use crate::domain::{InputDefinition, Recipe, WorkflowDocument};
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoResolutionPreset {
    pub id: String,
    pub label: String,
    pub width: i64,
    pub height: i64,
}

fn active_h3_nodes(workflow: Value, recipe: &Recipe) -> Vec<(String, String)> {
    let Ok(document) = WorkflowDocument::parse(workflow) else {
        return Vec::new();
    };
    let Ok(graph) = WorkflowGraph::from_document(&document) else {
        return Vec::new();
    };
    let roots = recipe
        .outputs
        .iter()
        .map(|output| output.node.clone())
        .collect::<Vec<_>>();
    if roots.is_empty() {
        return Vec::new();
    }
    let Ok(active) = ActiveDependencyGraph::from_graph(&graph, &roots) else {
        return Vec::new();
    };
    active
        .active_nodes
        .iter()
        .filter_map(|node| match document.class_type(node) {
            Some(class @ ("MiniMaxH3ImageToVideo" | "MiniMaxH3ReferenceToVideo")) => {
                Some((node.clone(), class.to_owned()))
            }
            _ => None,
        })
        .collect()
}

pub fn active_h3_kind(workflow: Value, recipe: &Recipe) -> Option<String> {
    let nodes = active_h3_nodes(workflow, recipe);
    if nodes
        .iter()
        .any(|(_, class)| class == "MiniMaxH3ReferenceToVideo")
    {
        Some("MiniMaxH3ReferenceToVideo".into())
    } else {
        nodes.into_iter().next().map(|(_, class)| class)
    }
}

/// 16:9 megapixel ladder (aligned to 32). Labels use the MP tag as shown in UI.
const H3_MP_16X9: [(&str, f64, i64, i64); 14] = [
    ("0.2", 0.2, 608, 352),
    ("0.3", 0.3, 736, 416),
    ("0.4", 0.4, 864, 480),
    ("0.5", 0.5, 960, 544),
    ("0.6", 0.6, 1056, 608),
    ("0.7", 0.7, 1152, 640),
    ("0.8", 0.8, 1216, 672),
    ("0.9", 0.9, 1280, 736),
    ("0.98", 0.98, 1344, 768),
    ("1.0", 1.0, 1376, 768),
    ("1.2", 1.2, 1504, 832),
    ("1.5", 1.5, 1664, 928),
    ("1.8", 1.8, 1824, 1024),
    ("2.0", 2.0, 1920, 1088),
];

const H3_ASPECTS: [(&str, i64, i64); 5] = [
    ("16:9", 16, 9),
    ("9:16", 9, 16),
    ("1:1", 1, 1),
    ("4:3", 4, 3),
    ("3:4", 3, 4),
];

fn align32(v: f64) -> i64 {
    let rounded = (v / 32.0).round() as i64 * 32;
    rounded.max(32)
}

/// Same megapixel target for non-16:9 aspects; see module docs for the formula.
fn dims_for_mp(mp: f64, rw: i64, rh: i64) -> (i64, i64) {
    if rw == 16 && rh == 9 {
        return H3_MP_16X9
            .iter()
            .find(|(_, m, _, _)| (*m - mp).abs() < 1e-9)
            .map(|(_, _, w, h)| (*w, *h))
            .unwrap_or((1344, 768));
    }
    if rw == 9 && rh == 16 {
        let (w, h) = dims_for_mp(mp, 16, 9);
        return (h, w);
    }
    let target = mp * 1_000_000.0;
    let w = align32((target * rw as f64 / rh as f64).sqrt());
    let h = align32(w as f64 * rh as f64 / rw as f64);
    (w, h)
}

fn mp_id_tag(tag: &str) -> String {
    tag.replace('.', "_")
}

/// Owned preset rows used by product UI and admission.
pub fn h3_base_presets() -> Vec<VideoResolutionPreset> {
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut push = |aspect: &str, tag: &str, w: i64, h: i64| {
        if !(w % 32 == 0 && h % 32 == 0 && w > 0 && h > 0) {
            return;
        }
        if !seen.insert((w, h)) {
            return;
        }
        out.push(VideoResolutionPreset {
            id: format!("mp-{}-{}", mp_id_tag(tag), aspect.replace(':', "x")),
            label: format!("{tag}MP · {w}×{h}"),
            width: w,
            height: h,
        });
    };
    push("16:9", "0.98", 1344, 768);
    for (aspect, rw, rh) in H3_ASPECTS {
        for (tag, mp, _, _) in H3_MP_16X9 {
            if aspect == "16:9" && tag == "0.98" {
                continue;
            }
            let (w, h) = dims_for_mp(mp, rw, rh);
            push(aspect, tag, w, h);
        }
    }
    out
}

/// Legacy name kept for tests that iterate approved (w,h) pairs.
pub fn h3_base_resolutions() -> Vec<(i64, i64)> {
    h3_base_presets()
        .into_iter()
        .map(|p| (p.width, p.height))
        .collect()
}

pub fn is_supported_h3_base_resolution(width: i64, height: i64) -> bool {
    // Multiple-of-32 is mandatory for the H3 canvas; membership in the MP table
    // is the product allow-list (2K regenerate stays a separate pipeline).
    width % 32 == 0
        && height % 32 == 0
        && width > 0
        && height > 0
        && h3_base_presets()
            .iter()
            .any(|p| p.width == width && p.height == height)
}

pub fn presets_for(workflow: Value, recipe: &Recipe) -> Vec<VideoResolutionPreset> {
    let nodes = active_h3_nodes(workflow, recipe);
    if !nodes.iter().any(|(node, _)| {
        ["width", "height"].iter().all(|key| {
            recipe.bindings.iter().any(|binding| {
                binding.source == *key
                    && binding.target.node == *node
                    && binding.target.input == *key
            })
        })
    }) {
        return Vec::new();
    }
    h3_base_presets()
        .into_iter()
        .filter(|p| allows(recipe, "width", p.width) && allows(recipe, "height", p.height))
        .collect()
}

/// Existing compiler plus product admission; no alternate executor or queue.
pub fn compile_checked(
    compiler: &crate::compiler::WorkflowCompiler,
    workflow: &WorkflowDocument,
    recipe: &Recipe,
    request: &crate::domain::CompileRequest,
) -> Result<crate::compiler::CompileResult, crate::compiler::CompileError> {
    validate_product_request(recipe, request)?;
    let compiled = compiler.compile(workflow, recipe, request)?;
    validate_resolved(compiled.workflow.clone(), recipe, &compiled.resolved_inputs)?;
    Ok(compiled)
}

/// A stricter product requirement over the unchanged, accepted I2V recipe.
pub fn validate_product_request(
    recipe: &Recipe,
    request: &crate::domain::CompileRequest,
) -> Result<(), crate::compiler::CompileError> {
    if recipe.id == "rcp_minimax_h3_fl2va_i2v_quality_2_2_0"
        && !matches!(request.values.get("first_frame"), Some(crate::domain::InputValue::Image(path)) if !path.trim().is_empty())
    {
        return Err(crate::compiler::CompileError::InputRequired {
            input: "first_frame".into(),
        });
    }
    Ok(())
}

/// Product admission uses compiler-resolved values, including recipe defaults.
/// This same policy is called by readiness and by actual runtime execution.
pub fn validate_resolved(
    workflow: Value,
    recipe: &Recipe,
    values: &std::collections::BTreeMap<String, crate::domain::ResolvedInputValue>,
) -> Result<(), crate::compiler::CompileError> {
    use crate::compiler::CompileError;
    use crate::domain::ResolvedInputValue;
    let Some(kind) = active_h3_kind(workflow.clone(), recipe) else {
        return Ok(());
    };
    let integer = |key: &str| match values.get(key) {
        Some(ResolvedInputValue::Integer(value)) => Ok(*value),
        _ => Err(CompileError::InputRequired { input: key.into() }),
    };
    let duration = integer("duration_seconds")?;
    if !(4..=15).contains(&duration) {
        return Err(CompileError::InputOutOfRange {
            input: "duration_seconds".into(),
            value: duration,
            min: Some(4),
            max: Some(15),
        });
    }
    let (width, height) = (integer("width")?, integer("height")?);
    if !is_supported_h3_base_resolution(width, height) {
        return Err(CompileError::BindingInvalid {
            source: "width/height".into(),
            node: "H3-Base".into(),
            input: "resolution".into(),
            message: "unsupported H3 Base resolution; 2K requires a separate pipeline".into(),
        });
    }
    // Validate the compiled H3 node dimensions too: a forged/misbound recipe
    // cannot use approved form values to submit a different canvas.
    for (node, _) in active_h3_nodes(workflow.clone(), recipe) {
        let inputs = &workflow[&node]["inputs"];
        let (w, h) = (inputs["width"].as_i64(), inputs["height"].as_i64());
        if !w
            .zip(h)
            .is_some_and(|(w, h)| is_supported_h3_base_resolution(w, h))
        {
            return Err(CompileError::BindingInvalid {
                source: "width/height".into(),
                node: "H3-Base".into(),
                input: "resolution".into(),
                message: "compiled H3 canvas must use an approved Base resolution".into(),
            });
        }
    }
    if kind == "MiniMaxH3ReferenceToVideo" {
        let count = |key: &str| match values.get(key) {
            Some(
                ResolvedInputValue::Images(v)
                | ResolvedInputValue::Videos(v)
                | ResolvedInputValue::Audios(v),
            ) => v.len(),
            _ => 0,
        };
        let (images, videos, audios) = (
            count("reference_images"),
            count("reference_videos"),
            count("reference_audios"),
        );
        for (key, n, max) in [
            ("reference_images", images, 9),
            ("reference_videos", videos, 3),
            ("reference_audios", audios, 3),
            ("references", images + videos + audios, 12),
        ] {
            if n > max {
                return Err(CompileError::InputCountOutOfRange {
                    input: key.into(),
                    count: n,
                    min: 0,
                    max,
                });
            }
        }
        // The generic H3 reference node also powers legacy text-only T2V
        // workflows. Only the exact approved REF2VA recipe requires reference
        // media; retaining the audio-only denial for other reference graphs.
        // Empty input drafts may save, but production admission must fail.
        if images + videos == 0
            && (audios > 0 || recipe.id == "rcp_minimax_h3_reference_video_quality_2_2_0")
        {
            return Err(CompileError::InputRequired {
                input: "reference_images_or_videos".into(),
            });
        }
    }
    Ok(())
}

/// Asset duration comes from the existing media probe; unavailable metadata is
/// explicitly blocked rather than estimated from size, frame count or filenames.
pub fn validate_reference_durations(key: &str, durations: &[Option<u64>]) -> Result<(), String> {
    let mut total = 0_u64;
    for duration in durations {
        let duration = duration.ok_or_else(|| {
            format!("BLOCKED_BY_METADATA: {key}.duration_ms unavailable; probe/reimport this asset")
        })?;
        if !(2000..=15000).contains(&duration) {
            return Err(format!("{key}: each reference must be 2–15 seconds"));
        }
        total += duration; // Each term is bounded and reference counts are already checked.
    }
    if total > 15000 {
        return Err(format!(
            "{key}: combined reference duration exceeds 15 seconds"
        ));
    }
    Ok(())
}

fn allows(recipe: &Recipe, key: &str, value: i64) -> bool {
    let Some(InputDefinition::Integer { min, max, step, .. }) = recipe.inputs.get(key) else {
        return false;
    };
    !min.is_some_and(|min| value < min)
        && !max.is_some_and(|max| value > max)
        && !step.is_some_and(|step| {
            step <= 0 || (i128::from(value) - i128::from(min.unwrap_or(0))) % i128::from(step) != 0
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn recipe() -> Recipe {
        crate::compiler::RecipeParser::parse(
            r#"
schema_version: 1
id: rcp_resolution_test
name: Renamed generator
workflow: {file: workflow_api.json}
inputs:
  width: {type: integer, label: Width, required: true, min: 32, max: 2048, step: 32}
  height: {type: integer, label: Height, required: true, min: 32, max: 2048, step: 32}
bindings:
  - {source: width, target: {node: '1', input: width}}
  - {source: height, target: {node: '1', input: height}}
outputs:
  - {id: video, node: '2', type: video, required: true}
"#,
        )
        .unwrap()
    }
    fn workflow(class: &str) -> Value {
        json!({"1":{"class_type":class,"inputs":{"width":1344,"height":768}},
            "2":{"class_type":"SaveVideo","inputs":{"video":["1",0]}}})
    }
    fn resolved(
        duration: i64,
        width: i64,
        height: i64,
    ) -> std::collections::BTreeMap<String, crate::domain::ResolvedInputValue> {
        use crate::domain::ResolvedInputValue::Integer;
        [
            ("duration_seconds".into(), Integer(duration)),
            ("width".into(), Integer(width)),
            ("height".into(), Integer(height)),
        ]
        .into()
    }
    fn reference_recipe() -> Recipe {
        let mut reference = recipe();
        reference.id = "rcp_minimax_h3_reference_video_quality_2_2_0".into();
        reference
    }
    fn production_values(
        class: &str,
        duration: i64,
        width: i64,
        height: i64,
    ) -> std::collections::BTreeMap<String, crate::domain::ResolvedInputValue> {
        let mut values = resolved(duration, width, height);
        if class == "MiniMaxH3ReferenceToVideo" {
            values.insert(
                "reference_videos".into(),
                crate::domain::ResolvedInputValue::Videos(vec!["reference.mp4".into()]),
            );
        }
        values
    }
    #[test]
    fn duration_and_resolution_are_product_admission_not_ui_constraints() {
        for class in ["MiniMaxH3ImageToVideo", "MiniMaxH3ReferenceToVideo"] {
            for duration in [3, 4, 15, 16] {
                assert_eq!(
                    validate_resolved(
                        workflow(class),
                        &recipe(),
                        &production_values(class, duration, 1344, 768)
                    )
                    .is_ok(),
                    (4..=15).contains(&duration)
                );
            }
            for (width, height) in h3_base_resolutions() {
                assert!(validate_resolved(
                    workflow(class),
                    &recipe(),
                    &production_values(class, 4, width, height)
                )
                .is_ok());
            }
            for (width, height) in [(1280, 720), (1920, 1080), (2048, 2048), (32, 32)] {
                assert!(validate_resolved(
                    workflow(class),
                    &recipe(),
                    &production_values(class, 4, width, height)
                )
                .is_err());
            }
        }
        let mut forged = workflow("MiniMaxH3ImageToVideo");
        forged["1"]["inputs"]["width"] = json!(2048);
        forged["1"]["inputs"]["height"] = json!(2048);
        assert!(validate_resolved(forged, &recipe(), &resolved(4, 1344, 768)).is_err());
        assert!(validate_resolved(
            workflow("UnrelatedGenerator"),
            &recipe(),
            &resolved(3, 32, 32)
        )
        .is_ok());
    }
    #[test]
    fn megapixel_ladder_is_multiple_of_32_and_defaults_to_0_98() {
        let presets = h3_base_presets();
        assert_eq!((presets[0].width, presets[0].height), (1344, 768));
        assert!(presets[0].label.starts_with("0.98MP"));
        assert!(presets
            .iter()
            .all(|p| p.width % 32 == 0 && p.height % 32 == 0));
        assert!(is_supported_h3_base_resolution(1344, 768));
        assert!(is_supported_h3_base_resolution(1920, 1088));
        assert!(is_supported_h3_base_resolution(768, 1344));
        assert!(!is_supported_h3_base_resolution(1920, 1080));
        assert!(!is_supported_h3_base_resolution(1280, 720));
        // 33 is not a multiple of 32.
        assert!(!is_supported_h3_base_resolution(33, 33));
    }
    #[test]
    fn mixed_references_keep_individual_and_combined_limits() {
        use crate::domain::ResolvedInputValue::{Audios, Images, Videos};
        for (images, videos, audios, ok) in [
            (0, 0, 0, false),
            (0, 1, 0, true),
            (1, 0, 0, true),
            (9, 3, 0, true),
            (9, 0, 3, true),
            (9, 3, 1, false),
            (8, 2, 2, true),
            (0, 0, 1, false),
            (10, 0, 0, false),
            (1, 4, 0, false),
            (1, 0, 4, false),
        ] {
            let mut values = resolved(4, 1344, 768);
            values.insert(
                "reference_images".into(),
                Images(vec!["image".into(); images]),
            );
            values.insert(
                "reference_videos".into(),
                Videos(vec!["video".into(); videos]),
            );
            values.insert(
                "reference_audios".into(),
                Audios(vec!["audio".into(); audios]),
            );
            assert_eq!(
                validate_resolved(
                    workflow("MiniMaxH3ReferenceToVideo"),
                    &reference_recipe(),
                    &values
                )
                .is_ok(),
                ok
            );
        }
    }
    #[test]
    fn empty_ref2va_draft_is_savable_but_never_production_ready() {
        use crate::compiler::CompileError;
        use crate::domain::ResolvedInputValue::{Audios, Images, Videos};
        let missing = resolved(4, 1344, 768);
        let check = |values: &std::collections::BTreeMap<_, _>| {
            assert!(matches!(
                validate_resolved(workflow("MiniMaxH3ReferenceToVideo"), &reference_recipe(), values),
                Err(CompileError::InputRequired { input }) if input == "reference_images_or_videos"
            ));
        };
        check(&missing);
        let mut empty = missing.clone();
        empty.insert("reference_images".into(), Images(vec![]));
        empty.insert("reference_videos".into(), Videos(vec![]));
        empty.insert("reference_audios".into(), Audios(vec![]));
        check(&empty);
        empty.insert("reference_audios".into(), Audios(vec!["audio.wav".into()]));
        check(&empty);
        // Zero-media T2V stays valid even when it reuses the H3 Reference node.
        assert!(validate_resolved(workflow("MiniMaxH3ImageToVideo"), &recipe(), &missing).is_ok());
        assert!(
            validate_resolved(workflow("MiniMaxH3ReferenceToVideo"), &recipe(), &missing).is_ok()
        );
        empty.insert("reference_audios".into(), Audios(vec![]));
        empty.insert(
            "reference_videos".into(),
            Videos(vec!["reference.mp4".into()]),
        );
        assert!(validate_resolved(
            workflow("MiniMaxH3ReferenceToVideo"),
            &reference_recipe(),
            &empty
        )
        .is_ok());
    }
    #[test]
    fn reference_duration_checks_real_milliseconds_and_blocks_missing_metadata() {
        for key in ["reference_videos", "reference_audios"] {
            for (duration, ok) in [(1900, false), (2000, true), (15000, true), (15001, false)] {
                assert_eq!(
                    validate_reference_durations(key, &[Some(duration)]).is_ok(),
                    ok
                );
            }
            assert!(validate_reference_durations(key, &[Some(8000), Some(8000)]).is_err());
            assert!(validate_reference_durations(key, &[Some(7500), Some(7500)]).is_ok());
            assert!(validate_reference_durations(key, &[None])
                .unwrap_err()
                .starts_with("BLOCKED_BY_METADATA"));
        }
    }
    #[test]
    fn h3_native_presets_follow_node_evidence_and_recipe_constraints() {
        let mut recipe = recipe();
        let presets = presets_for(workflow("MiniMaxH3ImageToVideo"), &recipe);
        assert!(presets.len() >= 14);
        assert_eq!((presets[0].width, presets[0].height), (1344, 768));
        assert!(presets
            .iter()
            .all(|p| p.width % 32 == 0 && p.height % 32 == 0));
        assert!(presets_for(workflow("UnrelatedGenerator"), &recipe).is_empty());
        if let Some(InputDefinition::Integer { max, .. }) = recipe.inputs.get_mut("width") {
            *max = Some(1024);
        }
        let capped = presets_for(workflow("MiniMaxH3ReferenceToVideo"), &recipe);
        assert!(!capped.is_empty());
        assert!(capped.iter().all(|p| p.width <= 1024));
        if let Some(InputDefinition::Integer { min, .. }) = recipe.inputs.get_mut("width") {
            *min = Some(33);
        }
        assert!(presets_for(workflow("MiniMaxH3ImageToVideo"), &recipe).is_empty());
    }
    #[test]
    fn inactive_h3_or_wrong_dimension_bindings_do_not_enable_presets() {
        let mut document = workflow("MiniMaxH3ImageToVideo");
        document["2"]["inputs"]["video"] = json!(["3", 0]);
        document["3"] = json!({"class_type":"OtherVideoGenerator","inputs":{}});
        assert!(presets_for(document, &recipe()).is_empty());
        let mut recipe = recipe();
        recipe.bindings[0].target.input = "unrelated".into();
        assert!(presets_for(workflow("MiniMaxH3ImageToVideo"), &recipe).is_empty());
    }
}
