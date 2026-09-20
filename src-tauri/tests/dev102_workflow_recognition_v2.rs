use ai_studio_lib::application::workflow_analysis_service::WorkflowAnalysisService;
use ai_studio_lib::compiler::RecipeParser;
use ai_studio_lib::domain::{Binding, OutputDefinition, Recipe, WorkflowDocument};
use serde_json::Value;
use std::{collections::BTreeSet, fs, path::PathBuf};

const CORPUS_PACKAGE_NAMES: &[&str] = &[
    "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0",
    "kera2_t2i_local_v2_1_1_1_90894e9e",
    "minimax_h3_fl2va_1_0_0",
    "minimax_h3_fl2va_compatible_1_0_0",
    "minimax_h3_fl2va_first_last_quality_2_0_0",
    "minimax_h3_fl2va_i2v_quality_2_0_0",
    "minimax_h3_fl2va_t2v_quality_2_0_0",
    "minimax_h3_reference_video_1_3_0",
    "minimax_h3_reference_video_quality_2_0_0",
];

const CORE_FIELDS: &[&str] = &[
    "prompt",
    "negative_prompt",
    "seed",
    "width",
    "height",
    "duration_seconds",
    "fps",
    "steps",
    "cfg",
    "denoise",
    "first_frame",
    "last_frame",
    "reference_image",
    "reference_images",
    "reference_video",
    "reference_videos",
    "reference_audio",
    "reference_audios",
];

#[derive(Debug)]
struct CorpusCase {
    name: String,
    workflow: WorkflowDocument,
    workflow_bytes: Vec<u8>,
    recipe: Recipe,
}

#[derive(Default)]
struct BindingMetrics {
    expected: usize,
    correct: usize,
    missing: usize,
    wrong: usize,
    ambiguous: usize,
    wrong_high_confidence: usize,
}

#[derive(Default)]
struct OutputMetrics {
    expected: usize,
    correct: usize,
    wrong: usize,
    ambiguous: usize,
    wrong_high_confidence: usize,
}

#[test]
fn v1_corpus_benchmark() {
    let cases = load_corpus();
    assert_eq!(cases.len(), CORPUS_PACKAGE_NAMES.len());

    let actual_names = cases
        .iter()
        .map(|case| case.name.as_str())
        .collect::<BTreeSet<_>>();
    let expected_names = CORPUS_PACKAGE_NAMES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_names, expected_names);

    let mut bindings = BindingMetrics::default();
    let mut outputs = OutputMetrics::default();
    for case in &cases {
        let analysis =
            WorkflowAnalysisService::analyze_workflow(&case.workflow, &case.workflow_bytes);
        for binding in case
            .recipe
            .bindings
            .iter()
            .filter(|binding| CORE_FIELDS.contains(&binding.source.as_str()))
        {
            bindings.expected += 1;
            match classify_binding(&analysis, binding) {
                BindingResult::Correct => bindings.correct += 1,
                BindingResult::Missing => bindings.missing += 1,
                BindingResult::Wrong => bindings.wrong += 1,
                BindingResult::Ambiguous => bindings.ambiguous += 1,
            }
        }
        for output in &case.recipe.outputs {
            outputs.expected += 1;
            match classify_output(&analysis, output) {
                OutputResult::Correct => outputs.correct += 1,
                OutputResult::Missing => outputs.wrong += 1,
                OutputResult::Wrong => outputs.wrong += 1,
                OutputResult::Ambiguous => outputs.ambiguous += 1,
            }
        }
    }

    println!(
        "V1_TOTAL_EXPECTED_CORE_BINDINGS={} \
V1_CORRECT_BINDINGS={} \
V1_MISSING_BINDINGS={} \
V1_WRONG_BINDINGS={} \
V1_AMBIGUOUS_BINDINGS={} \
V1_EXPECTED_OUTPUTS={} \
V1_CORRECT_OUTPUTS={} \
V1_WRONG_OUTPUTS={} \
V1_AMBIGUOUS_OUTPUTS={}",
        bindings.expected,
        bindings.correct,
        bindings.missing,
        bindings.wrong,
        bindings.ambiguous,
        outputs.expected,
        outputs.correct,
        outputs.wrong,
        outputs.ambiguous,
    );

    assert_eq!(bindings.expected, 79);
    assert_eq!(bindings.correct, 73);
    assert_eq!(bindings.missing, 6);
    assert_eq!(bindings.wrong, 0);
    assert_eq!(bindings.ambiguous, 0);
    assert_eq!(outputs.expected, 9);
    assert_eq!(outputs.correct, 9);
    assert_eq!(outputs.wrong, 0);
    assert_eq!(outputs.ambiguous, 0);
}

#[test]
fn v2_corpus_benchmark_preserves_frozen_v1_floor_with_static_fallback() {
    let cases = load_corpus();
    let mut bindings = BindingMetrics::default();
    let mut outputs = OutputMetrics::default();
    for case in &cases {
        // The corpus intentionally has no checked-in object_info snapshot.
        // Passing None verifies the offline/static V2 path without deriving a
        // schema from recipe expectations.
        let analysis = WorkflowAnalysisService::analyze_workflow_with_schema(
            &case.workflow,
            &case.workflow_bytes,
            None,
        );
        for binding in case
            .recipe
            .bindings
            .iter()
            .filter(|binding| CORE_FIELDS.contains(&binding.source.as_str()))
        {
            bindings.expected += 1;
            match classify_binding(&analysis, binding) {
                BindingResult::Correct => bindings.correct += 1,
                BindingResult::Missing => bindings.missing += 1,
                BindingResult::Wrong => {
                    bindings.wrong += 1;
                    if analysis.inputs.iter().any(|input| {
                        input.semantic_key == binding.source
                            && input.confidence
                                == ai_studio_lib::application::workflow_recognition_service::RecognitionConfidence::High
                            && (input.node_id != binding.target.node
                                || input.input_name != binding.target.input
                                || input.item_index != binding.item_index)
                    }) {
                        bindings.wrong_high_confidence += 1;
                    }
                }
                BindingResult::Ambiguous => bindings.ambiguous += 1,
            }
        }
        for output in &case.recipe.outputs {
            outputs.expected += 1;
            match classify_output(&analysis, output) {
                OutputResult::Correct => outputs.correct += 1,
                OutputResult::Missing => outputs.wrong += 1,
                OutputResult::Wrong => {
                    outputs.wrong += 1;
                    if analysis.outputs.iter().any(|candidate| {
                        candidate.confidence
                            == ai_studio_lib::application::workflow_recognition_service::RecognitionConfidence::High
                            && candidate.node_id != output.node
                    }) {
                        outputs.wrong_high_confidence += 1;
                    }
                }
                OutputResult::Ambiguous => outputs.ambiguous += 1,
            }
        }
    }

    println!(
        "V2_TOTAL_EXPECTED_CORE_BINDINGS={} \
V2_CORRECT_BINDINGS={} \
V2_MISSING_BINDINGS={} \
V2_WRONG_BINDINGS={} \
V2_AMBIGUOUS_BINDINGS={} \
V2_WRONG_HIGH_CONFIDENCE_BINDINGS={} \
V2_EXPECTED_OUTPUTS={} \
V2_CORRECT_OUTPUTS={} \
V2_WRONG_OUTPUTS={} \
V2_AMBIGUOUS_OUTPUTS={} \
V2_WRONG_HIGH_CONFIDENCE_OUTPUTS={} \
SCHEMA_SOURCE=STATIC_FALLBACK",
        bindings.expected,
        bindings.correct,
        bindings.missing,
        bindings.wrong,
        bindings.ambiguous,
        bindings.wrong_high_confidence,
        outputs.expected,
        outputs.correct,
        outputs.wrong,
        outputs.ambiguous,
        outputs.wrong_high_confidence,
    );

    assert_eq!(bindings.wrong_high_confidence, 0);
    assert_eq!(outputs.wrong_high_confidence, 0);
    assert!(bindings.correct >= 73);
    assert!(outputs.correct >= 9);
}

#[derive(Clone, Copy)]
enum BindingResult {
    Correct,
    Missing,
    Wrong,
    Ambiguous,
}

fn classify_binding(
    analysis: &ai_studio_lib::application::workflow_analysis_service::WorkflowAnalysisReport,
    binding: &Binding,
) -> BindingResult {
    let selected = analysis.inputs.iter().find(|input| {
        input.semantic_key == binding.source
            && input.item_index == binding.item_index
            && input.node_id == binding.target.node
            && input.input_name == binding.target.input
    });
    if selected.is_some() {
        return BindingResult::Correct;
    }

    let ambiguous = analysis.issues.iter().any(|issue| {
        issue.code == "AMBIGUOUS_INPUT"
            && issue.field.as_deref() == Some(binding.source.as_str())
            && issue.candidates.iter().any(|candidate| {
                candidate.node_id.as_deref() == Some(binding.target.node.as_str())
                    && candidate.input_name.as_deref() == Some(binding.target.input.as_str())
            })
    });
    if ambiguous {
        return BindingResult::Ambiguous;
    }

    let same_field = analysis
        .inputs
        .iter()
        .any(|input| input.semantic_key == binding.source);
    if same_field {
        return BindingResult::Wrong;
    }

    if analysis.issues.iter().any(|issue| {
        issue.field.as_deref() == Some(binding.source.as_str()) && issue.code.contains("INPUT")
    }) {
        BindingResult::Ambiguous
    } else {
        BindingResult::Missing
    }
}

#[derive(Clone, Copy)]
enum OutputResult {
    Correct,
    Missing,
    Wrong,
    Ambiguous,
}

fn classify_output(
    analysis: &ai_studio_lib::application::workflow_analysis_service::WorkflowAnalysisReport,
    expected: &OutputDefinition,
) -> OutputResult {
    let expected_type = match expected.output_type {
        ai_studio_lib::domain::OutputType::Image => "image",
        ai_studio_lib::domain::OutputType::Video => "video",
    };
    let correct = analysis.outputs.iter().any(|output| {
        output.node_id == expected.node
            && output.output_type == expected_type
            && !analysis.issues.iter().any(|issue| {
                issue.code == "AMBIGUOUS_OUTPUT"
                    && issue.candidates.iter().any(|candidate| {
                        candidate.node_id.as_deref() == Some(expected.node.as_str())
                    })
            })
    });
    if correct {
        return OutputResult::Correct;
    }

    if analysis.issues.iter().any(|issue| {
        issue.code == "AMBIGUOUS_OUTPUT"
            && issue
                .candidates
                .iter()
                .any(|candidate| candidate.node_id.as_deref() == Some(expected.node.as_str()))
    }) {
        return OutputResult::Ambiguous;
    }

    if analysis.outputs.is_empty() {
        OutputResult::Missing
    } else {
        OutputResult::Wrong
    }
}

fn load_corpus() -> Vec<CorpusCase> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime_packages");
    CORPUS_PACKAGE_NAMES
        .iter()
        .map(|name| {
            let directory = root.join(name);
            let workflow_path = directory.join("workflow_api.json");
            let recipe_path = directory.join("recipe.yaml");
            let workflow_bytes = fs::read(&workflow_path).expect("workflow fixture should exist");
            let workflow = WorkflowDocument::parse(
                serde_json::from_slice::<Value>(&workflow_bytes)
                    .expect("workflow fixture should be valid JSON"),
            )
            .expect("workflow fixture should be an API workflow");
            let recipe = RecipeParser::parse(
                &fs::read_to_string(&recipe_path).expect("recipe fixture should exist"),
            )
            .expect("recipe fixture should parse");
            CorpusCase {
                name: (*name).to_owned(),
                workflow,
                workflow_bytes,
                recipe,
            }
        })
        .collect()
}
