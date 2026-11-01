//! User-supplied Pika graph, normalized to its enabled 8-step LoRA branch.
//! Source SHA256: 6f0fbd42e6b97f9fb446a591ea1f48096571a38d3414b6d6fe7005fecdaab669
//! Optional media, precise input binding and output trimming reuse the built-in H3 recipe.
//! Sparse attention contract: https://github.com/Comfy-Org/embedded-docs/blob/main/comfyui_embedded_docs/docs/BlockSparseAttention/en.md

pub(super) const MANIFEST: &str = r####"schema_version: 1
id: wfl_minimax_h3_reference_pika_8step
name: MiniMax H3 Pika 全能参考 · 8步（最多8图 · 视频/音频可选）
workflow_version: 1.0.0
recipe_version: 1.0.0
category: video
mode: ref2va
"####;

pub(super) const RECIPE: &str = r####"schema_version: 1
id: rcp_minimax_h3_reference_pika_8step_1_0_0
name: MiniMax H3 Pika 全能参考 · 8步（最多8图 · 视频/音频可选）
workflow:
  file: workflow_api.json
inputs:
  duration_seconds:
    type: integer
    label: Duration (seconds)
    required: true
    default: 5
    min: 4
    max: 11
    step: 1
  prompt:
    type: textarea
    label: Prompt
    required: true
    default: ""
  reference_images:
    type: images
    label: Reference images (Picture 1–8)
    required: false
    min_items: 0
    max_items: 8
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
    default: 1280
    min: 32
    max: 2048
    step: 32
  height:
    type: integer
    label: Height
    required: true
    default: 736
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
      node: "137"
      input: image
    clear_targets:
      - node: "136"
        input: ref_images.ref_image_0
  - source: reference_images
    item: 1
    target:
      node: "139"
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
      "filename_prefix": "video/MiniMax_H3",
      "format": "auto",
      "format.codec": "auto",
      "codec": "auto",
      "video": [
        "130",
        0
      ]
    },
    "class_type": "SaveVideo",
    "_meta": {
      "title": "保存视频"
    }
  },
  "119": {
    "inputs": {
      "vae_name": "minmaxh3\\minimax_h3_video_vae_int8_convrot.safetensors"
    },
    "class_type": "VAELoader",
    "_meta": {
      "title": "加载VAE"
    }
  },
  "120": {
    "inputs": {
      "vae_name": "minmaxh3\\minimax_h3_audio_vae_fp32.safetensors"
    },
    "class_type": "VAELoader",
    "_meta": {
      "title": "加载VAE"
    }
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
    "class_type": "VAEDecodeAudio",
    "_meta": {
      "title": "VAE解码（音频）"
    }
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
    "class_type": "VAEDecode",
    "_meta": {
      "title": "VAE解码"
    }
  },
  "123": {
    "inputs": {
      "sampler_name": "res_multistep"
    },
    "class_type": "KSamplerSelect",
    "_meta": {
      "title": "K采样器选择"
    }
  },
  "124": {
    "inputs": {
      "scheduler": "simple",
      "steps": [
        "144",
        0
      ],
      "denoise": 1,
      "model": [
        "127",
        0
      ]
    },
    "class_type": "BasicScheduler",
    "_meta": {
      "title": "基本调度器"
    }
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
        "124",
        0
      ],
      "latent_image": [
        "136",
        1
      ]
    },
    "class_type": "SamplerCustomAdvanced",
    "_meta": {
      "title": "自定义采样器（高级）"
    }
  },
  "126": {
    "inputs": {
      "model": [
        "149",
        0
      ],
      "conditioning": [
        "136",
        0
      ]
    },
    "class_type": "BasicGuider",
    "_meta": {
      "title": "基本引导器"
    }
  },
  "127": {
    "inputs": {
      "unet_name": "minmaxh3\\minimax_h3_hybrid_fl2va_ref2va_b25-49-int8.safetensors",
      "weight_dtype": "default"
    },
    "class_type": "UNETLoader",
    "_meta": {
      "title": "UNet加载器"
    }
  },
  "128": {
    "inputs": {
      "clip_name": "minmaxh3\\qwen3vl_32b_minimax_h3_nvfp4_awq.safetensors",
      "type": "minimax",
      "device": "default"
    },
    "class_type": "CLIPLoader",
    "_meta": {
      "title": "加载CLIP"
    }
  },
  "129": {
    "inputs": {
      "noise_seed": 0
    },
    "class_type": "RandomNoise",
    "_meta": {
      "title": "随机噪波"
    }
  },
  "130": {
    "inputs": {
      "fps": 24,
      "bit_depth": 8,
      "color_space": "sRGB",
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
    "class_type": "CreateVideo",
    "_meta": {
      "title": "创建视频"
    }
  },
  "131": {
    "inputs": {
      "expression": "max(5, round(a * 24)) + (5 - (max(5, round(a * 24)) % 17)) % 17",
      "values.a": [
        "132",
        0
      ]
    },
    "class_type": "ComfyMathExpression",
    "_meta": {
      "title": "数学表达式"
    }
  },
  "132": {
    "inputs": {
      "value": 5
    },
    "class_type": "PrimitiveFloat",
    "_meta": {
      "title": "Float (Duration)"
    }
  },
  "136": {
    "inputs": {
      "prompt": [
        "138",
        0
      ],
      "width": 1280,
      "height": 736,
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
        "137",
        0
      ],
      "ref_images.ref_image_1": [
        "139",
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
    "class_type": "MiniMaxH3ReferenceToVideo",
    "_meta": {
      "title": "MiniMax H3 参考转视频"
    }
  },
  "137": {
    "class_type": "LoadImage",
    "inputs": {
      "image": ""
    }
  },
  "138": {
    "inputs": {
      "value": ""
    },
    "class_type": "PrimitiveStringMultiline",
    "_meta": {
      "title": "Input Text (Prompt)"
    }
  },
  "139": {
    "class_type": "LoadImage",
    "inputs": {
      "image": ""
    }
  },
  "144": {
    "inputs": {
      "value": 8
    },
    "class_type": "PrimitiveInt",
    "_meta": {
      "title": "Int (Lightning LoRA)"
    }
  },
  "145": {
    "inputs": {
      "lora_name": "minmanH3\\pika_h3_flex_turbo_8step_hd_ref2va.safetensors",
      "strength_model": 1,
      "model": [
        "127",
        0
      ]
    },
    "class_type": "LoraLoaderModelOnly",
    "_meta": {
      "title": "LoRA加载器（仅模型）"
    }
  },
  "148": {
    "inputs": {
      "attention": "comfy kitchen attention",
      "model": [
        "145",
        0
      ]
    },
    "class_type": "ModelAttentionBackend",
    "_meta": {
      "title": "模型注意力后端"
    }
  },
  "149": {
    "inputs": {
      "selection": "sol-attn",
      "selection.tau": 1.3,
      "start_percent": 0.2,
      "end_percent": 1,
      "dense_blocks": "",
      "min_tokens": 12288,
      "extra_tokens": 256,
      "sink_conditioning": "exact_kv_and_rows",
      "verbose": true,
      "model": [
        "148",
        0
      ]
    },
    "class_type": "BlockSparseAttention",
    "_meta": {
      "title": "模型稀疏注意力"
    }
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
  }
}
"####;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::product::h3_resolution::compile_checked;
    use crate::compiler::{
        BindingValidator, FinalCompiledWorkflowValidator, RecipeParser, RecipeValidator,
        WorkflowCompiler, WorkflowValidator,
    };
    use crate::domain::{CompileRequest, InputValue, WorkflowDocument};

    fn compile(
        images: usize,
        videos: usize,
        audios: usize,
    ) -> Result<crate::compiler::CompileResult, crate::compiler::CompileError> {
        let recipe = RecipeParser::parse(RECIPE).unwrap();
        let workflow = WorkflowDocument::parse(serde_json::from_str(WORKFLOW).unwrap()).unwrap();
        let request = CompileRequest::new(
            [
                (
                    "prompt".into(),
                    InputValue::String(
                        "Keep <Picture 1> appearance and <Picture 2> scene layout".into(),
                    ),
                ),
                (
                    "reference_images".into(),
                    InputValue::Images((0..images).map(|i| format!("image-{i}.png")).collect()),
                ),
                (
                    "reference_videos".into(),
                    InputValue::Videos((0..videos).map(|i| format!("video-{i}.mp4")).collect()),
                ),
                (
                    "reference_audios".into(),
                    InputValue::Audios((0..audios).map(|i| format!("audio-{i}.wav")).collect()),
                ),
                (
                    "seed".into(),
                    InputValue::Seed(crate::domain::SeedValue::Fixed(42)),
                ),
            ]
            .into(),
        );
        compile_checked(&WorkflowCompiler, &workflow, &recipe, &request)
    }

    #[test]
    fn pika_preserves_enabled_eight_step_model_and_attention_chain() {
        let g: serde_json::Value = serde_json::from_str(WORKFLOW).unwrap();
        let r = RecipeParser::parse(RECIPE).unwrap();
        let w = WorkflowDocument::parse(g.clone()).unwrap();
        RecipeValidator::validate(&r).unwrap();
        WorkflowValidator::validate(&w).unwrap();
        BindingValidator::validate(&r, &w).unwrap();
        assert_eq!(g["144"]["inputs"]["value"], 8);
        assert_eq!(g["124"]["inputs"]["steps"], serde_json::json!(["144", 0]));
        assert_eq!(
            g["145"]["inputs"]["lora_name"],
            r"minmanH3\pika_h3_flex_turbo_8step_hd_ref2va.safetensors"
        );
        assert_eq!(g["145"]["inputs"]["strength_model"], 1);
        assert_eq!(
            g["127"]["inputs"]["unet_name"],
            r"minmaxh3\minimax_h3_hybrid_fl2va_ref2va_b25-49-int8.safetensors"
        );
        assert_eq!(
            g["119"]["inputs"]["vae_name"],
            r"minmaxh3\minimax_h3_video_vae_int8_convrot.safetensors"
        );
        assert_eq!(g["148"]["inputs"]["model"], serde_json::json!(["145", 0]));
        assert_eq!(g["148"]["inputs"]["attention"], "comfy kitchen attention");
        assert_eq!(g["149"]["class_type"], "BlockSparseAttention");
        assert_eq!(g["149"]["inputs"]["model"], serde_json::json!(["148", 0]));
        assert_eq!(g["149"]["inputs"]["selection"], "sol-attn");
        assert_eq!(g["149"]["inputs"]["selection.tau"], 1.3);
        assert_eq!(g["149"]["inputs"]["sink_conditioning"], "exact_kv_and_rows");
        assert_eq!(g["126"]["inputs"]["model"], serde_json::json!(["149", 0]));
        for id in ["115", "141", "142", "143", "146"] {
            assert!(g.get(id).is_none());
        }
        assert_eq!(g["138"]["inputs"]["value"], "");
        assert!(
            !WORKFLOW.contains("red_superboy")
                && !WORKFLOW.contains("mecha_dragon")
                && !WORKFLOW.contains("GET READY")
        );
    }

    #[test]
    fn pika_compiles_one_to_eight_ordered_images_and_prunes_unused_media() {
        let slots = ["137", "139", "201", "202", "203", "204", "205", "206"];
        let recipe = RecipeParser::parse(RECIPE).unwrap();
        for count in 1..=8 {
            let result = compile(count, 0, 0).unwrap();
            let g = result.workflow;
            FinalCompiledWorkflowValidator::validate(&g, &recipe, &[]).unwrap();
            for (i, node) in slots.iter().enumerate() {
                let key = format!("ref_images.ref_image_{i}");
                if i < count {
                    assert_eq!(g["136"]["inputs"][&key], serde_json::json!([node, 0]));
                    assert_eq!(g[*node]["inputs"]["image"], format!("image-{i}.png"));
                } else {
                    assert!(g.get(*node).is_none() && g["136"]["inputs"].get(&key).is_none());
                }
            }
            for node in ["40", "41", "42", "43", "44", "45", "50", "51", "52"] {
                assert!(g.get(node).is_none());
            }
            assert_eq!(
                g["138"]["inputs"]["value"],
                "Keep <Picture 1> appearance and <Picture 2> scene layout"
            );
            assert_eq!(g["129"]["inputs"]["noise_seed"], 42);
            assert_eq!(g["136"]["inputs"]["width"], 1280);
            assert_eq!(g["136"]["inputs"]["height"], 736);
            assert_eq!(g["130"]["inputs"]["images"], serde_json::json!(["161", 0]));
            assert_eq!(g["130"]["inputs"]["audio"], serde_json::json!(["162", 0]));
        }
    }

    #[test]
    fn pika_video_only_and_mixed_references_compile_with_exact_media_order() {
        for (images, videos, audios) in [(0, 1, 0), (0, 1, 1), (1, 3, 3), (8, 2, 2)] {
            let g = compile(images, videos, audios).unwrap().workflow;
            for (i, node) in ["40", "42", "44"].iter().enumerate() {
                if i < videos {
                    assert_eq!(g[*node]["inputs"]["file"], format!("video-{i}.mp4"));
                    assert_eq!(
                        g["136"]["inputs"][format!("ref_videos.ref_video_{i}")],
                        serde_json::json!([format!("{}", 41 + i * 2), 0])
                    );
                    assert_eq!(
                        g["136"]["inputs"][format!("ref_video_audios.ref_video_audio_{i}")],
                        serde_json::json!([format!("{}", 41 + i * 2), 1])
                    );
                } else {
                    assert!(g.get(*node).is_none());
                }
            }
            for (i, node) in ["50", "51", "52"].iter().enumerate() {
                if i < audios {
                    assert_eq!(g[*node]["inputs"]["audio"], format!("audio-{i}.wav"));
                } else {
                    assert!(g.get(*node).is_none());
                }
            }
            assert_eq!(
                g["149"]["class_type"], "BlockSparseAttention",
                "image slots must not overwrite sparse attention"
            );
        }
    }

    #[test]
    fn pika_rejects_empty_audio_only_and_reference_overflow() {
        for (images, videos, audios) in [
            (0, 0, 0),
            (0, 0, 1),
            (9, 0, 0),
            (1, 4, 0),
            (1, 0, 4),
            (8, 3, 2),
        ] {
            assert!(
                compile(images, videos, audios).is_err(),
                "must reject {images}/{videos}/{audios}"
            );
        }
    }

    #[test]
    fn pika_installs_immutably_without_superseding_existing_defaults() {
        let root = tempfile::tempdir().unwrap();
        super::super::ensure_installed(root.path()).unwrap();
        let name = "minimax_h3_reference_pika_8step_1_0_0";
        let dir = root.path().join(name);
        assert_eq!(
            std::fs::read_to_string(dir.join("workflow_api.json")).unwrap(),
            WORKFLOW
        );
        assert!(super::super::is_builtin_package_name(name));
        assert!(
            crate::application::minimax_video_product_policy::AUTHORIZED_PACKAGES.contains(&name)
        );
        assert!(super::super::audit_installed(root.path()).is_empty());
        std::fs::write(dir.join("workflow_api.json"), "tampered").unwrap();
        assert!(super::super::ensure_installed(root.path())
            .unwrap_err()
            .contains("BUILTIN_PACKAGE_HASH_MISMATCH"));
        assert_eq!(
            std::fs::read_to_string(dir.join("workflow_api.json")).unwrap(),
            "tampered"
        );
    }
}
