//! Local H3-Base presets, not the hosted H3-Regenerate-2K pipeline.
//! Sources: https://huggingface.co/MiniMaxAI/MiniMax-H3
//! https://docs.comfy.org/tutorials/video/minimax/minimax-h3-native
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

pub const H3_BASE_RESOLUTIONS: [(&str, &str, i64, i64); 5] = [
    ("landscape", "16:9 横屏", 1344, 768),
    ("portrait", "9:16 竖屏", 768, 1344),
    ("square", "1:1 方形", 768, 768),
    ("landscape-4-3", "4:3 横屏", 1024, 768),
    ("portrait-3-4", "3:4 竖屏", 768, 1024),
];

pub fn is_supported_h3_base_resolution(width: i64, height: i64) -> bool {
    H3_BASE_RESOLUTIONS
        .iter()
        .any(|(_, _, w, h)| (*w, *h) == (width, height))
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
    // 16:9 follows ComfyUI's documented native 1344x768 canvas. The other
    // common ratios derive from MiniMax's default short edge (768), aligned32.
    H3_BASE_RESOLUTIONS
        .into_iter()
        .filter(|(_, _, width, height)| {
            allows(recipe, "width", *width) && allows(recipe, "height", *height)
        })
        .map(|(id, label, width, height)| VideoResolutionPreset {
            id: id.into(),
            label: label.into(),
            width,
            height,
        })
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
        if audios > 0 && images + videos == 0 {
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
    #[test]
    fn duration_and_resolution_are_product_admission_not_ui_constraints() {
        for class in ["MiniMaxH3ImageToVideo", "MiniMaxH3ReferenceToVideo"] {
            for duration in [3, 4, 15, 16] {
                assert_eq!(
                    validate_resolved(workflow(class), &recipe(), &resolved(duration, 1344, 768))
                        .is_ok(),
                    (4..=15).contains(&duration)
                );
            }
            for (_, _, width, height) in H3_BASE_RESOLUTIONS {
                assert!(
                    validate_resolved(workflow(class), &recipe(), &resolved(4, width, height))
                        .is_ok()
                );
            }
            for (width, height) in [(1280, 720), (1920, 1080), (2048, 2048), (32, 32)] {
                assert!(
                    validate_resolved(workflow(class), &recipe(), &resolved(4, width, height))
                        .is_err()
                );
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
    fn mixed_references_keep_individual_and_combined_limits() {
        use crate::domain::ResolvedInputValue::{Audios, Images, Videos};
        for (images, videos, audios, ok) in [
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
                validate_resolved(workflow("MiniMaxH3ReferenceToVideo"), &recipe(), &values)
                    .is_ok(),
                ok
            );
        }
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
        assert_eq!(presets.len(), 5);
        assert_eq!((presets[0].width, presets[0].height), (1344, 768));
        assert!(presets
            .iter()
            .all(|p| p.width % 32 == 0 && p.height % 32 == 0));
        assert!(presets_for(workflow("UnrelatedGenerator"), &recipe).is_empty());
        if let Some(InputDefinition::Integer { max, .. }) = recipe.inputs.get_mut("width") {
            *max = Some(1024);
        }
        assert_eq!(
            presets_for(workflow("MiniMaxH3ReferenceToVideo"), &recipe).len(),
            4
        );
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
