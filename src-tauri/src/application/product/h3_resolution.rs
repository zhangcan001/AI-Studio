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

pub fn presets_for(workflow: Value, recipe: &Recipe) -> Vec<VideoResolutionPreset> {
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
    // Use active node evidence and the actual dimension bindings, never names
    // or guessed workflow IDs. Imported/renamed H3 generators work as well.
    let h3 = active.active_nodes.iter().any(|node| {
        matches!(
            document.class_type(node),
            Some("MiniMaxH3ImageToVideo" | "MiniMaxH3ReferenceToVideo")
        ) && ["width", "height"].iter().all(|key| {
            recipe.bindings.iter().any(|binding| {
                binding.source == *key
                    && binding.target.node == *node
                    && binding.target.input == *key
            })
        })
    });
    if !h3 {
        return Vec::new();
    }
    // 16:9 follows ComfyUI's documented native 1344x768 canvas. The other
    // common ratios derive from MiniMax's default short edge (768), aligned32.
    [
        ("landscape", "16:9 横屏", 1344, 768),
        ("portrait", "9:16 竖屏", 768, 1344),
        ("square", "1:1 方形", 768, 768),
        ("landscape-4-3", "4:3 横屏", 1024, 768),
        ("portrait-3-4", "3:4 竖屏", 768, 1024),
    ]
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
