use crate::application::ordered_reference_binding::ref2va_image_bounds;
use crate::domain::{InputDefinition, OutputType, Recipe, ShotStage};

pub const SHOT_WORKFLOW_UNSUPPORTED_MEDIA_INPUT: &str = "SHOT_WORKFLOW_UNSUPPORTED_MEDIA_INPUT";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShotVideoInputMode {
    TextOnly,
    SingleImage {
        key: String,
    },
    ReferenceImages {
        key: String,
        min_items: usize,
        max_items: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShotWorkflowCompatibility {
    pub input_mode: ShotVideoInputMode,
    pub ref2va_bounds: Option<(usize, usize)>,
}

pub fn classify_shot_recipe(
    stage: ShotStage,
    workflow_id: &str,
    recipe: &Recipe,
) -> Result<ShotWorkflowCompatibility, String> {
    let expected_output = match stage {
        ShotStage::Image => OutputType::Image,
        ShotStage::Video => OutputType::Video,
    };
    if !recipe
        .outputs
        .iter()
        .any(|output| output.output_type == expected_output)
    {
        return Err(format!(
            "{} 阶段需要兼容的 {} 输出",
            stage.as_str(),
            match expected_output {
                OutputType::Image => "image",
                OutputType::Video => "video",
            }
        ));
    }

    let mut image_input = None;
    for (key, input) in &recipe.inputs {
        match input {
            InputDefinition::Image { .. } | InputDefinition::Images { .. } => {
                if image_input.replace((key, input)).is_some() {
                    return Err(unsupported_media(
                        "Shot 只能表达一个 image 或 images 输入；双帧输入不受支持",
                    ));
                }
            }
            InputDefinition::Video { .. }
            | InputDefinition::Videos { .. }
            | InputDefinition::Audio { .. }
            | InputDefinition::Audios { .. } => {
                return Err(unsupported_media(&format!(
                    "输入 {key} 需要 Shot 当前不支持的音频或视频素材",
                )));
            }
            InputDefinition::TextArea { .. }
            | InputDefinition::Integer { .. }
            | InputDefinition::Number { .. }
            | InputDefinition::Seed { .. } => {}
        }
    }

    let ref2va_bounds = if stage == ShotStage::Video {
        ref2va_image_bounds(workflow_id, recipe)?
    } else {
        None
    };
    let input_mode = match image_input {
        None => {
            if ref2va_bounds.is_some() {
                return Err("REF2VA Recipe 必须声明 plural reference_images 输入".to_owned());
            }
            ShotVideoInputMode::TextOnly
        }
        Some((key, InputDefinition::Image { .. })) => {
            if ref2va_bounds.is_some() {
                return Err("REF2VA Recipe 必须声明 plural reference_images 输入".to_owned());
            }
            ShotVideoInputMode::SingleImage { key: key.clone() }
        }
        Some((
            key,
            InputDefinition::Images {
                min_items,
                max_items,
                ..
            },
        )) => {
            if min_items > max_items {
                return Err(format!(
                    "SHOT_WORKFLOW_INVALID_REFERENCE_BOUNDS: 输入 {key} 的 min_items 不能大于 max_items"
                ));
            }
            let (min_items, max_items) = ref2va_bounds.unwrap_or((*min_items, *max_items));
            ShotVideoInputMode::ReferenceImages {
                key: key.clone(),
                min_items,
                max_items,
            }
        }
        Some((_key, _)) => unreachable!("image_input only contains image definitions"),
    };

    Ok(ShotWorkflowCompatibility {
        input_mode,
        ref2va_bounds,
    })
}

fn unsupported_media(reason: &str) -> String {
    format!("{SHOT_WORKFLOW_UNSUPPORTED_MEDIA_INPUT}: {reason}")
}

#[cfg(test)]
mod tests {
    use super::{classify_shot_recipe, ShotVideoInputMode};
    use crate::domain::{
        InputDefinition, OutputDefinition, OutputType, Recipe, ShotStage, WorkflowRef,
    };
    use std::collections::BTreeMap;

    fn recipe(inputs: BTreeMap<String, InputDefinition>, output: OutputType) -> Recipe {
        Recipe {
            schema_version: 1,
            id: "recipe".to_owned(),
            name: "Recipe".to_owned(),
            workflow: WorkflowRef {
                file: "workflow_api.json".to_owned(),
            },
            inputs,
            bindings: Vec::new(),
            outputs: vec![OutputDefinition {
                id: "output".to_owned(),
                output_type: output,
                node: "1".to_owned(),
                required: true,
            }],
        }
    }

    fn text() -> InputDefinition {
        InputDefinition::TextArea {
            label: "Prompt".to_owned(),
            required: true,
            default: None,
        }
    }

    #[test]
    fn custom_video_modes_are_classified_without_workflow_id_whitelist() {
        let text_only = classify_shot_recipe(
            ShotStage::Video,
            "wfl_custom_t2v",
            &recipe(
                BTreeMap::from([(String::from("prompt"), text())]),
                OutputType::Video,
            ),
        )
        .expect("custom T2V should be compatible");
        assert_eq!(text_only.input_mode, ShotVideoInputMode::TextOnly);

        let single = classify_shot_recipe(
            ShotStage::Video,
            "wfl_custom_i2v",
            &recipe(
                BTreeMap::from([(
                    String::from("image"),
                    InputDefinition::Image {
                        label: "Image".to_owned(),
                        required: true,
                    },
                )]),
                OutputType::Video,
            ),
        )
        .expect("custom I2V should be compatible");
        assert_eq!(
            single.input_mode,
            ShotVideoInputMode::SingleImage {
                key: "image".to_owned()
            }
        );

        let references = classify_shot_recipe(
            ShotStage::Video,
            "wfl_custom_reference",
            &recipe(
                BTreeMap::from([(
                    String::from("images"),
                    InputDefinition::Images {
                        label: "Images".to_owned(),
                        required: true,
                        min_items: 1,
                        max_items: 4,
                    },
                )]),
                OutputType::Video,
            ),
        )
        .expect("custom reference video should be compatible");
        assert_eq!(
            references.input_mode,
            ShotVideoInputMode::ReferenceImages {
                key: "images".to_owned(),
                min_items: 1,
                max_items: 4,
            }
        );
    }

    #[test]
    fn unsupported_media_and_dual_image_inputs_fail_closed() {
        let audio = classify_shot_recipe(
            ShotStage::Video,
            "wfl_custom_audio",
            &recipe(
                BTreeMap::from([(
                    String::from("audio"),
                    InputDefinition::Audio {
                        label: "Audio".to_owned(),
                        required: true,
                    },
                )]),
                OutputType::Video,
            ),
        )
        .expect_err("audio should not be expressible by Shot");
        assert!(audio.starts_with("SHOT_WORKFLOW_UNSUPPORTED_MEDIA_INPUT:"));

        let dual_frame = classify_shot_recipe(
            ShotStage::Video,
            "wfl_custom_dual_frame",
            &recipe(
                BTreeMap::from([
                    (
                        String::from("first_frame"),
                        InputDefinition::Image {
                            label: "First".to_owned(),
                            required: true,
                        },
                    ),
                    (
                        String::from("last_frame"),
                        InputDefinition::Image {
                            label: "Last".to_owned(),
                            required: true,
                        },
                    ),
                ]),
                OutputType::Video,
            ),
        )
        .expect_err("dual frame should not be expressible by Shot");
        assert!(dual_frame.starts_with("SHOT_WORKFLOW_UNSUPPORTED_MEDIA_INPUT:"));
    }
}
