//! User-provided API graph, with its LCM/ManualSigmas six-step path preserved.
//! Source SHA256: ff31d299c4b5b7323c9284c1dec8e92ccc720b91a4f9fbbfbc5115c1da036791
//! Sampler contract: https://github.com/Comfy-Org/docs/blob/0260c1f7f49381c074b3943c66f55508ecce7394/tutorials/video/pika/pika-h3-flex.mdx
//! Retains the attachment’s rank-55 adapter; no model weights or example media are bundled.
//! Reuses existing H3 bindings, optional media loaders and exact output trimming.

pub(super) const MANIFEST: &str = r####"schema_version: 1
id: wfl_minimax_h3_reference_pika_flex_6step
name: MiniMax H3 Pika 全能参考 · Flex 6步
workflow_version: 1.0.0
recipe_version: 1.0.0
category: video
mode: ref2va
"####;

pub(super) const RECIPE: &str = r####"schema_version: 1
id: rcp_minimax_h3_reference_pika_flex_6step_1_0_0
name: MiniMax H3 Pika 全能参考 · Flex 6步
workflow:
  file: workflow_api.json
inputs:
  duration_seconds:
    type: integer
    label: Duration (seconds)
    required: true
    default: 5
    min: 5
    max: 11
    step: 1
  prompt:
    type: textarea
    label: Prompt
    required: true
    default: ""
  reference_images:
    type: images
    label: Reference images (Picture 1–9)
    required: false
    min_items: 0
    max_items: 9
  reference_videos:
    type: videos
    label: Reference videos
    required: false
    min_items: 0
    max_items: 3
  reference_audios:
    type: audios
    label: Reference audios
    required: false
    min_items: 0
    max_items: 3
  width:
    type: integer
    label: Width
    required: true
    default: 1344
    min: 32
    max: 2048
    step: 32
  height:
    type: integer
    label: Height
    required: true
    default: 768
    min: 32
    max: 2048
    step: 32
  seed:
    type: seed
    label: Seed
    default: random
    min: 0
    max: 1125899906842624
bindings:
  - source: duration_seconds
    target:
      node: "132"
      input: value
  - source: prompt
    target:
      node: "138"
      input: value
  - source: reference_images
    item: 0
    target:
      node: "149"
      input: image
    clear_targets:
      - node: "136"
        input: ref_images.ref_image_0
  - source: reference_images
    item: 1
    target:
      node: "150"
      input: image
    clear_targets:
      - node: "136"
        input: ref_images.ref_image_1
  - source: reference_images
    item: 2
    target:
      node: "201"
      input: image
    clear_targets:
      - node: "136"
        input: ref_images.ref_image_2
  - source: reference_images
    item: 3
    target:
      node: "202"
      input: image
    clear_targets:
      - node: "136"
        input: ref_images.ref_image_3
  - source: reference_images
    item: 4
    target:
      node: "203"
      input: image
    clear_targets:
      - node: "136"
        input: ref_images.ref_image_4
  - source: reference_images
    item: 5
    target:
      node: "204"
      input: image
    clear_targets:
      - node: "136"
        input: ref_images.ref_image_5
  - source: reference_images
    item: 6
    target:
      node: "205"
      input: image
    clear_targets:
      - node: "136"
        input: ref_images.ref_image_6
  - source: reference_images
    item: 7
    target:
      node: "206"
      input: image
    clear_targets:
      - node: "136"
        input: ref_images.ref_image_7
  - source: reference_images
    item: 8
    target:
      node: "207"
      input: image
    clear_targets:
      - node: "136"
        input: ref_images.ref_image_8
  - source: reference_videos
    item: 0
    target:
      node: "40"
      input: file
    clear_targets:
      - node: "136"
        input: ref_videos.ref_video_0
      - node: "136"
        input: ref_video_audios.ref_video_audio_0
  - source: reference_videos
    item: 1
    target:
      node: "42"
      input: file
    clear_targets:
      - node: "136"
        input: ref_videos.ref_video_1
      - node: "136"
        input: ref_video_audios.ref_video_audio_1
  - source: reference_videos
    item: 2
    target:
      node: "44"
      input: file
    clear_targets:
      - node: "136"
        input: ref_videos.ref_video_2
      - node: "136"
        input: ref_video_audios.ref_video_audio_2
  - source: reference_audios
    item: 0
    target:
      node: "50"
      input: audio
    clear_targets:
      - node: "136"
        input: ref_audios.ref_audio_0
  - source: reference_audios
    item: 1
    target:
      node: "51"
      input: audio
    clear_targets:
      - node: "136"
        input: ref_audios.ref_audio_1
  - source: reference_audios
    item: 2
    target:
      node: "52"
      input: audio
    clear_targets:
      - node: "136"
        input: ref_audios.ref_audio_2
  - source: width
    target:
      node: "136"
      input: width
  - source: height
    target:
      node: "136"
      input: height
  - source: seed
    target:
      node: "129"
      input: noise_seed
outputs:
  - id: generated_video
    type: video
    node: "92"
    required: true
"####;

pub(super) const WORKFLOW: &str = r####"{
  "40": {
    "inputs": {
      "file": "__AI_STUDIO_OPTIONAL__.mp4"
    },
    "class_type": "LoadVideo"
  },
  "41": {
    "inputs": {
      "video": [
        "40",
        0
      ]
    },
    "class_type": "GetVideoComponents"
  },
  "42": {
    "inputs": {
      "file": "__AI_STUDIO_OPTIONAL__.mp4"
    },
    "class_type": "LoadVideo"
  },
  "43": {
    "inputs": {
      "video": [
        "42",
        0
      ]
    },
    "class_type": "GetVideoComponents"
  },
  "44": {
    "inputs": {
      "file": "__AI_STUDIO_OPTIONAL__.mp4"
    },
    "class_type": "LoadVideo"
  },
  "45": {
    "inputs": {
      "video": [
        "44",
        0
      ]
    },
    "class_type": "GetVideoComponents"
  },
  "50": {
    "inputs": {
      "audio": "__AI_STUDIO_OPTIONAL__.wav"
    },
    "class_type": "LoadAudio"
  },
  "51": {
    "inputs": {
      "audio": "__AI_STUDIO_OPTIONAL__.wav"
    },
    "class_type": "LoadAudio"
  },
  "52": {
    "inputs": {
      "audio": "__AI_STUDIO_OPTIONAL__.wav"
    },
    "class_type": "LoadAudio"
  },
  "92": {
    "inputs": {
      "filename_prefix": "video/Pika_H3_Flex_REF2V",
      "format": "auto",
      "format.codec": "auto",
      "codec": "auto",
      "video": [
        "130",
        0
      ]
    },
    "class_type": "SaveVideo"
  },
  "119": {
    "inputs": {
      "vae_name": "minmaxh3\\minimax_h3_video_vae_int8_convrot.safetensors"
    },
    "class_type": "VAELoader"
  },
  "120": {
    "inputs": {
      "vae_name": "minmaxh3\\minimax_h3_audio_vae_fp32.safetensors"
    },
    "class_type": "VAELoader"
  },
  "121": {
    "inputs": {
      "samples": [
        "125",
        0
      ],
      "vae": [
        "120",
        0
      ]
    },
    "class_type": "VAEDecodeAudio"
  },
  "122": {
    "inputs": {
      "samples": [
        "125",
        0
      ],
      "vae": [
        "119",
        0
      ]
    },
    "class_type": "VAEDecode"
  },
  "123": {
    "inputs": {
      "sampler_name": "lcm"
    },
    "class_type": "KSamplerSelect"
  },
  "125": {
    "inputs": {
      "noise": [
        "129",
        0
      ],
      "guider": [
        "126",
        0
      ],
      "sampler": [
        "123",
        0
      ],
      "sigmas": [
        "147",
        0
      ],
      "latent_image": [
        "136",
        1
      ]
    },
    "class_type": "SamplerCustomAdvanced"
  },
  "126": {
    "inputs": {
      "model": [
        "156",
        0
      ],
      "conditioning": [
        "136",
        0
      ]
    },
    "class_type": "BasicGuider"
  },
  "127": {
    "inputs": {
      "unet_name": "minmaxh3\\minimax_h3_hybrid_fl2va_ref2va_b25-49-int8.safetensors",
      "weight_dtype": "default"
    },
    "class_type": "UNETLoader"
  },
  "128": {
    "inputs": {
      "clip_name": "minmaxh3\\qwen3vl_32b_minimax_h3_nvfp4_awq.safetensors",
      "type": "minimax",
      "device": "default"
    },
    "class_type": "CLIPLoader"
  },
  "129": {
    "inputs": {
      "noise_seed": 0
    },
    "class_type": "RandomNoise"
  },
  "130": {
    "inputs": {
      "fps": 24,
      "bit_depth": 8,
      "color_space": "sRGB",
      "chroma_subsampling": "none",
      "codec": "none",
      "images": [
        "161",
        0
      ],
      "audio": [
        "162",
        0
      ]
    },
    "class_type": "CreateVideo"
  },
  "131": {
    "inputs": {
      "expression": "max(5, round(a * 24)) + (5 - (max(5, round(a * 24)) % 17)) % 17",
      "values.a": [
        "132",
        0
      ]
    },
    "class_type": "ComfyMathExpression"
  },
  "132": {
    "inputs": {
      "value": 5
    },
    "class_type": "PrimitiveFloat"
  },
  "136": {
    "inputs": {
      "prompt": [
        "138",
        0
      ],
      "width": 1344,
      "height": 768,
      "length": [
        "131",
        1
      ],
      "ref_image_size": "match",
      "clip": [
        "128",
        0
      ],
      "vae": [
        "119",
        0
      ],
      "audio_vae": [
        "120",
        0
      ],
      "ref_images.ref_image_0": [
        "149",
        0
      ],
      "ref_images.ref_image_1": [
        "150",
        0
      ],
      "ref_images.ref_image_2": [
        "201",
        0
      ],
      "ref_images.ref_image_3": [
        "202",
        0
      ],
      "ref_images.ref_image_4": [
        "203",
        0
      ],
      "ref_images.ref_image_5": [
        "204",
        0
      ],
      "ref_images.ref_image_6": [
        "205",
        0
      ],
      "ref_images.ref_image_7": [
        "206",
        0
      ],
      "ref_images.ref_image_8": [
        "207",
        0
      ],
      "ref_videos.ref_video_0": [
        "41",
        0
      ],
      "ref_video_audios.ref_video_audio_0": [
        "41",
        1
      ],
      "ref_videos.ref_video_1": [
        "43",
        0
      ],
      "ref_video_audios.ref_video_audio_1": [
        "43",
        1
      ],
      "ref_videos.ref_video_2": [
        "45",
        0
      ],
      "ref_video_audios.ref_video_audio_2": [
        "45",
        1
      ],
      "ref_audios.ref_audio_0": [
        "50",
        0
      ],
      "ref_audios.ref_audio_1": [
        "51",
        0
      ],
      "ref_audios.ref_audio_2": [
        "52",
        0
      ]
    },
    "class_type": "MiniMaxH3ReferenceToVideo"
  },
  "138": {
    "inputs": {
      "value": ""
    },
    "class_type": "PrimitiveStringMultiline"
  },
  "145": {
    "inputs": {
      "lora_name": "minmanH3\\pika_h3_flex_ref2va_v1_avg_rank_55_bf16.safetensors",
      "strength_model": 1,
      "model": [
        "127",
        0
      ]
    },
    "class_type": "LoraLoaderModelOnly"
  },
  "147": {
    "inputs": {
      "sigmas": "1.0, 0.972973, 0.923077, 0.8, 0.571429, 0.333333, 0.0"
    },
    "class_type": "ManualSigmas"
  },
  "149": {
    "class_type": "LoadImage",
    "inputs": {
      "image": ""
    }
  },
  "150": {
    "class_type": "LoadImage",
    "inputs": {
      "image": ""
    }
  },
  "156": {
    "inputs": {
      "selection": "sol-attn",
      "selection.tau": 1.2,
      "start_percent": 0.25,
      "end_percent": 1,
      "dense_blocks": "",
      "min_tokens": 12288,
      "extra_tokens": 256,
      "sink_conditioning": "exact_kv_and_rows",
      "verbose": false,
      "model": [
        "145",
        0
      ]
    },
    "class_type": "BlockSparseAttention"
  },
  "160": {
    "class_type": "ComfyMathExpression",
    "inputs": {
      "expression": "round(a * 24)",
      "values.a": [
        "132",
        0
      ]
    }
  },
  "161": {
    "class_type": "ImageFromBatch",
    "inputs": {
      "image": [
        "122",
        0
      ],
      "batch_index": 0,
      "length": [
        "160",
        1
      ]
    }
  },
  "162": {
    "class_type": "TrimAudioDuration",
    "inputs": {
      "audio": [
        "121",
        0
      ],
      "start_index": 0,
      "duration": [
        "132",
        0
      ]
    }
  },
  "201": {
    "class_type": "LoadImage",
    "inputs": {
      "image": ""
    }
  },
  "202": {
    "class_type": "LoadImage",
    "inputs": {
      "image": ""
    }
  },
  "203": {
    "class_type": "LoadImage",
    "inputs": {
      "image": ""
    }
  },
  "204": {
    "class_type": "LoadImage",
    "inputs": {
      "image": ""
    }
  },
  "205": {
    "class_type": "LoadImage",
    "inputs": {
      "image": ""
    }
  },
  "206": {
    "class_type": "LoadImage",
    "inputs": {
      "image": ""
    }
  },
  "207": {
    "class_type": "LoadImage",
    "inputs": {
      "image": ""
    }
  }
}
"####;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::product::h3_resolution::{compile_checked, presets_for};
    use crate::compiler::{
        CompileError, FinalCompiledWorkflowValidator, RecipeParser, WorkflowCompiler,
    };
    use crate::domain::{CompileRequest, InputValue, WorkflowDocument};
    use serde_json::{json, Value};
    use std::collections::BTreeMap;

    const IMAGE_NODES: [&str; 9] = [
        "149", "150", "201", "202", "203", "204", "205", "206", "207",
    ];

    fn compile(
        settings: Option<(i64, i64, i64)>,
        (images, videos, audios): (usize, usize, usize),
    ) -> Result<Value, CompileError> {
        let recipe = RecipeParser::parse(RECIPE).unwrap();
        let workflow = WorkflowDocument::parse(serde_json::from_str(WORKFLOW).unwrap()).unwrap();
        let files = |count, ext| (0..count).map(|i| format!("reference-{i}.{ext}")).collect();
        let mut values = BTreeMap::from([
            (
                "prompt".into(),
                InputValue::String("Use <Picture 1> as the subject.".into()),
            ),
            (
                "reference_images".into(),
                InputValue::Images(files(images, "png")),
            ),
            (
                "reference_videos".into(),
                InputValue::Videos(files(videos, "mp4")),
            ),
            (
                "reference_audios".into(),
                InputValue::Audios(files(audios, "wav")),
            ),
        ]);
        if let Some((width, height, duration)) = settings {
            values.extend([
                ("width".into(), InputValue::Integer(width)),
                ("height".into(), InputValue::Integer(height)),
                ("duration_seconds".into(), InputValue::Integer(duration)),
            ]);
        }
        let result = compile_checked(
            &WorkflowCompiler,
            &workflow,
            &recipe,
            &CompileRequest::new(values),
        )?;
        FinalCompiledWorkflowValidator::validate(&result.workflow, &recipe, &[]).unwrap();
        Ok(result.workflow)
    }

    #[test]
    fn pika_flex_preserves_six_step_sampler_and_attachment_model_chain() {
        let source: Value = serde_json::from_str(WORKFLOW).unwrap();
        let graph = compile(None, (1, 0, 0)).unwrap();
        assert_eq!(source["138"]["inputs"]["value"], "");
        assert!(!WORKFLOW.contains("ref_pilot_profile.png"));
        assert!(!WORKFLOW.contains("ref_subject_station.png"));
        assert_eq!(
            graph["138"]["inputs"]["value"],
            "Use <Picture 1> as the subject."
        );
        assert_eq!(graph["123"]["inputs"]["sampler_name"], "lcm");
        assert_eq!(graph["147"]["class_type"], "ManualSigmas");
        assert_eq!(
            graph["147"]["inputs"]["sigmas"],
            "1.0, 0.972973, 0.923077, 0.8, 0.571429, 0.333333, 0.0"
        );
        assert_eq!(graph["125"]["inputs"]["sigmas"], json!(["147", 0]));
        assert_eq!(graph["126"]["inputs"]["model"], json!(["156", 0]));
        assert_eq!(graph["156"]["class_type"], "BlockSparseAttention");
        assert_eq!(graph["156"]["inputs"]["selection.tau"], 1.2);
        assert_eq!(graph["156"]["inputs"]["model"], json!(["145", 0]));
        assert_eq!(graph["145"]["inputs"]["model"], json!(["127", 0]));
        assert_eq!(graph["145"]["inputs"]["strength_model"], 1);
        assert_eq!(
            graph["145"]["inputs"]["lora_name"],
            r"minmanH3\pika_h3_flex_ref2va_v1_avg_rank_55_bf16.safetensors"
        );
        assert_eq!(
            graph["127"]["inputs"]["unet_name"],
            r"minmaxh3\minimax_h3_hybrid_fl2va_ref2va_b25-49-int8.safetensors"
        );
    }

    #[test]
    fn pika_flex_size_and_duration_reach_canvas_and_audio_video_trim() {
        let recipe = RecipeParser::parse(RECIPE).unwrap();
        let presets = presets_for(serde_json::from_str(WORKFLOW).unwrap(), &recipe);
        let default = compile(None, (1, 0, 0)).unwrap();
        assert_eq!(default["136"]["inputs"]["width"], 1344);
        assert_eq!(default["136"]["inputs"]["height"], 768);
        assert_eq!(default["132"]["inputs"]["value"], 5);
        for (width, height, duration) in [(960, 544, 5), (736, 1280, 8), (1344, 768, 11)] {
            assert!(presets
                .iter()
                .any(|p| p.width == width && p.height == height));
            let g = compile(Some((width, height, duration)), (1, 0, 0)).unwrap();
            assert_eq!(g["136"]["inputs"]["width"], width);
            assert_eq!(g["136"]["inputs"]["height"], height);
            assert_eq!(g["132"]["inputs"]["value"], duration);
            assert_eq!(g["131"]["inputs"]["values.a"], json!(["132", 0]));
            assert_eq!(g["136"]["inputs"]["length"], json!(["131", 1]));
            assert_eq!(g["160"]["inputs"]["values.a"], json!(["132", 0]));
            assert_eq!(g["161"]["inputs"]["length"], json!(["160", 1]));
            assert_eq!(g["162"]["inputs"]["duration"], json!(["132", 0]));
            assert_eq!(g["130"]["inputs"]["images"], json!(["161", 0]));
            assert_eq!(g["130"]["inputs"]["audio"], json!(["162", 0]));
            assert!(g.get("115").is_none());
        }
        for settings in [(1344, 768, 4), (1344, 768, 12), (32, 32, 5)] {
            assert!(compile(Some(settings), (1, 0, 0)).is_err());
        }
    }

    #[test]
    fn pika_flex_compiles_variable_ordered_references_and_prunes_unused_loaders() {
        let cases = (1..=9)
            .map(|n| (n, 0, 0))
            .chain([(0, 1, 0), (0, 2, 1), (6, 3, 3), (9, 1, 2)]);
        for (images, videos, audios) in cases {
            let g = compile(None, (images, videos, audios)).unwrap();
            let refs = &g["136"]["inputs"];
            for (i, id) in IMAGE_NODES.into_iter().enumerate() {
                let key = format!("ref_images.ref_image_{i}");
                if i < images {
                    assert_eq!(g[id]["inputs"]["image"], format!("reference-{i}.png"));
                    assert_eq!(refs[key], json!([id, 0]));
                } else {
                    assert!(g.get(id).is_none() && refs.get(&key).is_none());
                }
            }
            for (i, (loader, components)) in [("40", "41"), ("42", "43"), ("44", "45")]
                .into_iter()
                .enumerate()
            {
                let video = format!("ref_videos.ref_video_{i}");
                let soundtrack = format!("ref_video_audios.ref_video_audio_{i}");
                if i < videos {
                    assert_eq!(g[loader]["inputs"]["file"], format!("reference-{i}.mp4"));
                    assert_eq!(refs[video], json!([components, 0]));
                    assert_eq!(refs[soundtrack], json!([components, 1]));
                } else {
                    assert!(g.get(loader).is_none() && g.get(components).is_none());
                    assert!(refs.get(&video).is_none() && refs.get(&soundtrack).is_none());
                }
            }
            for (i, id) in ["50", "51", "52"].into_iter().enumerate() {
                let key = format!("ref_audios.ref_audio_{i}");
                if i < audios {
                    assert_eq!(g[id]["inputs"]["audio"], format!("reference-{i}.wav"));
                    assert_eq!(refs[key], json!([id, 0]));
                } else {
                    assert!(g.get(id).is_none() && refs.get(&key).is_none());
                }
            }
        }
    }

    #[test]
    fn pika_flex_requires_an_image_or_video_and_checks_individual_and_total_caps() {
        for refs in [
            (0, 0, 0),
            (0, 0, 1),
            (10, 0, 0),
            (1, 4, 0),
            (1, 0, 4),
            (9, 3, 1),
        ] {
            assert!(
                compile(None, refs).is_err(),
                "unexpectedly admitted {refs:?}"
            );
        }
    }
}
