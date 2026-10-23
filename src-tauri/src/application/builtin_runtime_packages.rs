//! Immutable product-owned runtime packages.
//!
//! The package library remains user-data-backed so older packages are never
//! overwritten. Embedded packages are copied only when their exact package
//! directory is absent, then the normal library synchronizer validates and
//! registers them like every other runtime package.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

struct BuiltinPackage {
    directory: &'static str,
    manifest: &'static str,
    recipe: &'static str,
    workflow: &'static str,
}

struct BuiltinPackageIdentity {
    /// Legacy package directory reserved by a product-owned runtime.
    directory: &'static str,
}

const STANDARD_REFERENCE_MANIFEST: &str = r####"schema_version: 1
id: wfl_minimax_h3_reference_standard
name: MiniMax H3 全能参考 · 标准20步（双图）
workflow_version: 1.0.0
recipe_version: 1.0.0
category: video
mode: ref2va
"####;

const STANDARD_REFERENCE_RECIPE: &str = r####"schema_version: 1
id: rcp_minimax_h3_reference_standard_1_0_0
name: MiniMax H3 全能参考 · 标准20步（双图）
workflow:
  file: workflow_api.json
inputs:
  duration_seconds:
    type: integer
    label: Duration (seconds)
    required: true
    default: 4
    min: 4
    max: 15
    step: 1
  prompt:
    type: textarea
    label: Prompt
    required: true
    default: ""
  reference_images:
    type: images
    label: Reference images (Picture 1 / Picture 2)
    required: true
    min_items: 1
    max_items: 2
  width:
    type: integer
    label: Width
    required: true
    default: 864
    min: 32
    max: 2048
    step: 32
  height:
    type: integer
    label: Height
    required: true
    default: 480
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

const STANDARD_REFERENCE_WORKFLOW: &str = r####"{
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
  "115": {
    "inputs": {
      "aspect_ratio": "16:9 (Widescreen)",
      "megapixels": 0.4,
      "multiple": 32
    },
    "class_type": "ResolutionSelector",
    "_meta": {
      "title": "Resolution Selector (Size)"
    }
  },
  "119": {
    "inputs": {
      "vae_name": "minmaxh3\\minimax_h3_video_vae_fp16.safetensors"
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
        "143",
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
        "148",
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
      "noise_seed": 667827762049783
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
      "value": 4
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
      "width": [
        "115",
        0
      ],
      "height": [
        "115",
        1
      ],
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
      ]
    },
    "class_type": "MiniMaxH3ReferenceToVideo",
    "_meta": {
      "title": "MiniMax H3 参考转视频"
    }
  },
  "137": {
    "inputs": {
      "image": "__AI_STUDIO_OPTIONAL__.png"
    },
    "class_type": "LoadImage",
    "_meta": {
      "title": "加载图像"
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
    "inputs": {
      "image": "__AI_STUDIO_OPTIONAL__.png"
    },
    "class_type": "LoadImage",
    "_meta": {
      "title": "加载图像"
    }
  },
  "143": {
    "inputs": {
      "value": 20
    },
    "class_type": "PrimitiveInt",
    "_meta": {
      "title": "Int (Full)"
    }
  },
  "148": {
    "inputs": {
      "sage_attention": "auto",
      "allow_compile": false,
      "model": [
        "127",
        0
      ]
    },
    "class_type": "PathchSageAttentionKJ",
    "_meta": {
      "title": "Patch Sage Attention KJ"
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
      "start_index": 0.0,
      "duration": [
        "132",
        0
      ]
    }
  }
}
"####;

// User-supplied Veda graph: fixed default Turbo branch; examples are never shipped.
// Upstream node contract: https://github.com/veda-sparse/Veda-on-ComfyUI
const VEDA_REFERENCE_MANIFEST: &str = r####"schema_version: 1
id: wfl_minimax_h3_reference_veda_turbo
name: MiniMax H3 Veda 全能参考 · Turbo 4步（1–8图）
workflow_version: 1.0.0
recipe_version: 1.0.0
category: video
mode: ref2va
"####;

const VEDA_REFERENCE_RECIPE: &str = r####"schema_version: 1
id: rcp_minimax_h3_reference_veda_turbo_1_0_0
name: MiniMax H3 Veda 全能参考 · Turbo 4步（1–8图）
workflow:
  file: workflow_api.json
inputs:
  duration_seconds:
    type: integer
    label: Duration (seconds)
    required: true
    default: 5
    min: 4
    max: 15
    step: 1
  prompt:
    type: textarea
    label: Prompt
    required: true
    default: ""
  reference_images:
    type: images
    label: Reference images (Picture 1–8)
    required: true
    min_items: 1
    max_items: 8
  width:
    type: integer
    label: Width
    required: true
    default: 1376
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
      node: "149"
      input: image
    clear_targets:
      - node: "136"
        input: ref_images.ref_image_2
  - source: reference_images
    item: 3
    target:
      node: "150"
      input: image
    clear_targets:
      - node: "136"
        input: ref_images.ref_image_3
  - source: reference_images
    item: 4
    target:
      node: "151"
      input: image
    clear_targets:
      - node: "136"
        input: ref_images.ref_image_4
  - source: reference_images
    item: 5
    target:
      node: "152"
      input: image
    clear_targets:
      - node: "136"
        input: ref_images.ref_image_5
  - source: reference_images
    item: 6
    target:
      node: "153"
      input: image
    clear_targets:
      - node: "136"
        input: ref_images.ref_image_6
  - source: reference_images
    item: 7
    target:
      node: "154"
      input: image
    clear_targets:
      - node: "136"
        input: ref_images.ref_image_7
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

const VEDA_REFERENCE_WORKFLOW: &str = r####"{
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
        "147",
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
      "width": 1376,
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
        "137",
        0
      ],
      "ref_images.ref_image_1": [
        "139",
        0
      ],
      "ref_images.ref_image_2": [
        "149",
        0
      ],
      "ref_images.ref_image_3": [
        "150",
        0
      ],
      "ref_images.ref_image_4": [
        "151",
        0
      ],
      "ref_images.ref_image_5": [
        "152",
        0
      ],
      "ref_images.ref_image_6": [
        "153",
        0
      ],
      "ref_images.ref_image_7": [
        "154",
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
      "value": 4
    },
    "class_type": "PrimitiveInt",
    "_meta": {
      "title": "Int (Lightning LoRA)"
    }
  },
  "145": {
    "inputs": {
      "lora_name": "minmanH3\\minimax_h3_ref2v_turbo_4step_v0.1_comfyui_bf16.safetensors",
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
  "147": {
    "inputs": {
      "predictor": "minimax_h3_r2va_veda_preview_fp8.safetensors",
      "generated_sparsity": "32",
      "reference_sparsity": "32",
      "full_attention_layers": "",
      "full_attention_steps": "",
      "verbose": false,
      "model": [
        "145",
        0
      ]
    },
    "class_type": "VedaSparseAttention",
    "_meta": {
      "title": "⚡ Veda Sparse Attention (MiniMax H3)"
    }
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
  "151": {
    "class_type": "LoadImage",
    "inputs": {
      "image": ""
    }
  },
  "152": {
    "class_type": "LoadImage",
    "inputs": {
      "image": ""
    }
  },
  "153": {
    "class_type": "LoadImage",
    "inputs": {
      "image": ""
    }
  },
  "154": {
    "class_type": "LoadImage",
    "inputs": {
      "image": ""
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
  }
}
"####;

// 1.1.0: same Veda graph plus optional H3 reference video/audio loaders copied
// from minimax_h3_reference_video_quality_2_2_0; empty slots are cleared by the compiler.
const VEDA_REFERENCE_V1_1_MANIFEST: &str = r####"schema_version: 1
id: wfl_minimax_h3_reference_veda_turbo
name: MiniMax H3 Veda 全能参考 · Turbo 4步（1–8图 · 视频/音频可选）
workflow_version: 1.1.0
recipe_version: 1.1.0
category: video
mode: ref2va
"####;

const VEDA_REFERENCE_V1_1_RECIPE: &str = r####"schema_version: 1
id: rcp_minimax_h3_reference_veda_turbo_1_1_0
name: MiniMax H3 Veda 全能参考 · Turbo 4步（1–8图 · 视频/音频可选）
workflow:
  file: workflow_api.json
inputs:
  duration_seconds:
    type: integer
    label: Duration (seconds)
    required: true
    default: 5
    min: 4
    max: 15
    step: 1
  prompt:
    type: textarea
    label: Prompt
    required: true
    default: ""
  reference_images:
    type: images
    label: Reference images (Picture 1–8)
    required: true
    min_items: 1
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
    default: 1376
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
      node: "149"
      input: image
    clear_targets:
      - node: "136"
        input: ref_images.ref_image_2
  - source: reference_images
    item: 3
    target:
      node: "150"
      input: image
    clear_targets:
      - node: "136"
        input: ref_images.ref_image_3
  - source: reference_images
    item: 4
    target:
      node: "151"
      input: image
    clear_targets:
      - node: "136"
        input: ref_images.ref_image_4
  - source: reference_images
    item: 5
    target:
      node: "152"
      input: image
    clear_targets:
      - node: "136"
        input: ref_images.ref_image_5
  - source: reference_images
    item: 6
    target:
      node: "153"
      input: image
    clear_targets:
      - node: "136"
        input: ref_images.ref_image_6
  - source: reference_images
    item: 7
    target:
      node: "154"
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

const VEDA_REFERENCE_V1_1_WORKFLOW: &str = r####"{
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
        "147",
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
      "width": 1376,
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
        "137",
        0
      ],
      "ref_images.ref_image_1": [
        "139",
        0
      ],
      "ref_images.ref_image_2": [
        "149",
        0
      ],
      "ref_images.ref_image_3": [
        "150",
        0
      ],
      "ref_images.ref_image_4": [
        "151",
        0
      ],
      "ref_images.ref_image_5": [
        "152",
        0
      ],
      "ref_images.ref_image_6": [
        "153",
        0
      ],
      "ref_images.ref_image_7": [
        "154",
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
      "value": 4
    },
    "class_type": "PrimitiveInt",
    "_meta": {
      "title": "Int (Lightning LoRA)"
    }
  },
  "145": {
    "inputs": {
      "lora_name": "minmanH3\\minimax_h3_ref2v_turbo_4step_v0.1_comfyui_bf16.safetensors",
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
  "147": {
    "inputs": {
      "predictor": "minimax_h3_r2va_veda_preview_fp8.safetensors",
      "generated_sparsity": "32",
      "reference_sparsity": "32",
      "full_attention_layers": "",
      "full_attention_steps": "",
      "verbose": false,
      "model": [
        "145",
        0
      ]
    },
    "class_type": "VedaSparseAttention",
    "_meta": {
      "title": "⚡ Veda Sparse Attention (MiniMax H3)"
    }
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
  "151": {
    "class_type": "LoadImage",
    "inputs": {
      "image": ""
    }
  },
  "152": {
    "class_type": "LoadImage",
    "inputs": {
      "image": ""
    }
  },
  "153": {
    "class_type": "LoadImage",
    "inputs": {
      "image": ""
    }
  },
  "154": {
    "class_type": "LoadImage",
    "inputs": {
      "image": ""
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
  }
}
"####;

const PACKAGES: &[BuiltinPackage] = &[
    BuiltinPackage {
        directory: "minimax_h3_reference_veda_turbo_1_0_0",
        manifest: VEDA_REFERENCE_MANIFEST,
        recipe: VEDA_REFERENCE_RECIPE,
        workflow: VEDA_REFERENCE_WORKFLOW,
    },
    BuiltinPackage {
        directory: "minimax_h3_reference_veda_turbo_1_1_0",
        manifest: VEDA_REFERENCE_V1_1_MANIFEST,
        recipe: VEDA_REFERENCE_V1_1_RECIPE,
        workflow: VEDA_REFERENCE_V1_1_WORKFLOW,
    },
    BuiltinPackage {
        directory: "minimax_h3_reference_standard_1_0_0",
        manifest: STANDARD_REFERENCE_MANIFEST,
        recipe: STANDARD_REFERENCE_RECIPE,
        workflow: STANDARD_REFERENCE_WORKFLOW,
    },
    #[cfg(test)]
    BuiltinPackage {
        directory: "minimax_h3_fl2va_compatible_1_0_0",
        manifest: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_compatible_1_0_0/manifest.yaml"
        ),
        recipe: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_compatible_1_0_0/recipe.yaml"
        ),
        workflow: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_compatible_1_0_0/workflow_api.json"
        ),
    },
    #[cfg(test)]
    BuiltinPackage {
        directory: "minimax_h3_fl2va_1_0_0",
        manifest: include_str!("../../runtime_packages/minimax_h3_fl2va_1_0_0/manifest.yaml"),
        recipe: include_str!("../../runtime_packages/minimax_h3_fl2va_1_0_0/recipe.yaml"),
        workflow: include_str!("../../runtime_packages/minimax_h3_fl2va_1_0_0/workflow_api.json"),
    },
    #[cfg(test)]
    BuiltinPackage {
        directory: "minimax_h3_reference_video_1_3_0",
        manifest: include_str!(
            "../../runtime_packages/minimax_h3_reference_video_1_3_0/manifest.yaml"
        ),
        recipe: include_str!("../../runtime_packages/minimax_h3_reference_video_1_3_0/recipe.yaml"),
        workflow: include_str!(
            "../../runtime_packages/minimax_h3_reference_video_1_3_0/workflow_api.json"
        ),
    },
    #[cfg(test)]
    BuiltinPackage {
        directory: "minimax_h3_fl2va_t2v_quality_2_0_0",
        manifest: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_t2v_quality_2_0_0/manifest.yaml"
        ),
        recipe: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_t2v_quality_2_0_0/recipe.yaml"
        ),
        workflow: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_t2v_quality_2_0_0/workflow_api.json"
        ),
    },
    BuiltinPackage {
        directory: "minimax_h3_fl2va_t2v_quality_2_1_0",
        manifest: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_t2v_quality_2_1_0/manifest.yaml"
        ),
        recipe: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_t2v_quality_2_1_0/recipe.yaml"
        ),
        workflow: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_t2v_quality_2_1_0/workflow_api.json"
        ),
    },
    BuiltinPackage {
        directory: "minimax_h3_fl2va_t2v_quality_2_2_0",
        manifest: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_t2v_quality_2_2_0/manifest.yaml"
        ),
        recipe: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_t2v_quality_2_2_0/recipe.yaml"
        ),
        workflow: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_t2v_quality_2_2_0/workflow_api.json"
        ),
    },
    #[cfg(test)]
    BuiltinPackage {
        directory: "minimax_h3_fl2va_i2v_quality_2_0_0",
        manifest: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_i2v_quality_2_0_0/manifest.yaml"
        ),
        recipe: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_i2v_quality_2_0_0/recipe.yaml"
        ),
        workflow: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_i2v_quality_2_0_0/workflow_api.json"
        ),
    },
    BuiltinPackage {
        directory: "minimax_h3_fl2va_i2v_quality_2_1_0",
        manifest: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_i2v_quality_2_1_0/manifest.yaml"
        ),
        recipe: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_i2v_quality_2_1_0/recipe.yaml"
        ),
        workflow: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_i2v_quality_2_1_0/workflow_api.json"
        ),
    },
    BuiltinPackage {
        directory: "minimax_h3_fl2va_i2v_quality_2_2_0",
        manifest: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/manifest.yaml"
        ),
        recipe: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml"
        ),
        workflow: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/workflow_api.json"
        ),
    },
    #[cfg(test)]
    BuiltinPackage {
        directory: "minimax_h3_fl2va_first_last_quality_2_0_0",
        manifest: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_first_last_quality_2_0_0/manifest.yaml"
        ),
        recipe: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_first_last_quality_2_0_0/recipe.yaml"
        ),
        workflow: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_first_last_quality_2_0_0/workflow_api.json"
        ),
    },
    BuiltinPackage {
        directory: "minimax_h3_fl2va_first_last_quality_2_1_1",
        manifest: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_first_last_quality_2_1_1/manifest.yaml"
        ),
        recipe: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_first_last_quality_2_1_1/recipe.yaml"
        ),
        workflow: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_first_last_quality_2_1_1/workflow_api.json"
        ),
    },
    BuiltinPackage {
        directory: "minimax_h3_fl2va_first_last_quality_2_2_0",
        manifest: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_first_last_quality_2_2_0/manifest.yaml"
        ),
        recipe: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_first_last_quality_2_2_0/recipe.yaml"
        ),
        workflow: include_str!(
            "../../runtime_packages/minimax_h3_fl2va_first_last_quality_2_2_0/workflow_api.json"
        ),
    },
    #[cfg(test)]
    BuiltinPackage {
        directory: "minimax_h3_reference_video_quality_2_0_0",
        manifest: include_str!(
            "../../runtime_packages/minimax_h3_reference_video_quality_2_0_0/manifest.yaml"
        ),
        recipe: include_str!(
            "../../runtime_packages/minimax_h3_reference_video_quality_2_0_0/recipe.yaml"
        ),
        workflow: include_str!(
            "../../runtime_packages/minimax_h3_reference_video_quality_2_0_0/workflow_api.json"
        ),
    },
    BuiltinPackage {
        directory: "minimax_h3_reference_video_quality_2_1_0",
        manifest: include_str!(
            "../../runtime_packages/minimax_h3_reference_video_quality_2_1_0/manifest.yaml"
        ),
        recipe: include_str!(
            "../../runtime_packages/minimax_h3_reference_video_quality_2_1_0/recipe.yaml"
        ),
        workflow: include_str!(
            "../../runtime_packages/minimax_h3_reference_video_quality_2_1_0/workflow_api.json"
        ),
    },
    BuiltinPackage {
        directory: "minimax_h3_reference_video_quality_2_2_0",
        manifest: include_str!(
            "../../runtime_packages/minimax_h3_reference_video_quality_2_2_0/manifest.yaml"
        ),
        recipe: include_str!(
            "../../runtime_packages/minimax_h3_reference_video_quality_2_2_0/recipe.yaml"
        ),
        workflow: include_str!(
            "../../runtime_packages/minimax_h3_reference_video_quality_2_2_0/workflow_api.json"
        ),
    },
    #[cfg(test)]
    BuiltinPackage {
        directory: "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0",
        manifest: include_str!(
            "../../runtime_packages/aitudou_minimax_h3_lightx2v_8step_fast_1_0_0/manifest.yaml"
        ),
        recipe: include_str!(
            "../../runtime_packages/aitudou_minimax_h3_lightx2v_8step_fast_1_0_0/recipe.yaml"
        ),
        workflow: include_str!(
            "../../runtime_packages/aitudou_minimax_h3_lightx2v_8step_fast_1_0_0/workflow_api.json"
        ),
    },
    BuiltinPackage {
        directory: "kera2_t2i_local_v2_1_1_1_90894e9e",
        manifest: include_str!(
            "../../runtime_packages/kera2_t2i_local_v2_1_1_1_90894e9e/manifest.yaml"
        ),
        recipe: include_str!(
            "../../runtime_packages/kera2_t2i_local_v2_1_1_1_90894e9e/recipe.yaml"
        ),
        workflow: include_str!(
            "../../runtime_packages/kera2_t2i_local_v2_1_1_1_90894e9e/workflow_api.json"
        ),
    },
];

// Retired H3 and older quality/Krea2 identities remain product-owned for
// existing libraries. New installs use only the active immutable packages.
const PRODUCT_PACKAGE_IDENTITIES: &[BuiltinPackageIdentity] = &[
    BuiltinPackageIdentity {
        directory: "minimax_h3_fl2va_compatible_1_0_0",
    },
    BuiltinPackageIdentity {
        directory: "minimax_h3_fl2va_1_0_0",
    },
    BuiltinPackageIdentity {
        directory: "minimax_h3_reference_video_1_3_0",
    },
    BuiltinPackageIdentity {
        directory: "minimax_h3_fl2va_t2v_quality_2_0_0",
    },
    BuiltinPackageIdentity {
        directory: "minimax_h3_fl2va_i2v_quality_2_0_0",
    },
    BuiltinPackageIdentity {
        directory: "minimax_h3_fl2va_first_last_quality_2_0_0",
    },
    BuiltinPackageIdentity {
        directory: "minimax_h3_reference_video_quality_2_0_0",
    },
    BuiltinPackageIdentity {
        directory: "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0",
    },
    BuiltinPackageIdentity {
        directory: "kera2_t2i_local_v2",
    },
    BuiltinPackageIdentity {
        directory: "kera2_t2i_local_v2_1_1_0_1d99a10d",
    },
    BuiltinPackageIdentity {
        directory: "krea2_t2i_local",
    },
];

/// Built-in identity is derived from formal product Runtime Package sources,
/// not from workflow IDs. User-installed packages are never matched here.
pub fn is_builtin_package_name(package_name: &str) -> bool {
    PACKAGES
        .iter()
        .any(|package| package.directory == package_name)
        || PRODUCT_PACKAGE_IDENTITIES
            .iter()
            .any(|package| package.directory == package_name)
}

fn active_package(package: &BuiltinPackage) -> bool {
    crate::application::minimax_video_product_policy::AUTHORIZED_PACKAGES
        .contains(&package.directory)
}

/// Immutable bytes used by the product policy; never inferred from a user name.
pub(crate) fn embedded_package(
    name: &str,
) -> Option<crate::application::ports::WorkflowPackageBytes> {
    let package = PACKAGES.iter().find(|package| package.directory == name)?;
    Some(crate::application::ports::WorkflowPackageBytes::new(
        package.manifest.as_bytes().to_vec(),
        package.recipe.as_bytes().to_vec(),
        package.workflow.as_bytes().to_vec(),
    ))
}

/// The only automatic default advance is this explicitly approved immutable
/// H3 Base upgrade. Generic imports and user-selected/promoted recipes keep
/// their existing default; historical version/recipe identities are untouched.
pub fn h3_base_supersession(
    package: &crate::application::ports::WorkflowPackageRecord,
) -> Option<(&'static str, String, String)> {
    if package.source_kind != "PRODUCT" {
        return None;
    }
    let new = PACKAGES.iter().find(|p| {
        p.directory == package.package_name
            && (p.directory.contains("_quality_2_2_0")
                || p.directory == "minimax_h3_reference_veda_turbo_1_1_0")
    })?;
    let manifest =
        crate::application::workflow_manifest::WorkflowManifest::parse(new.manifest).ok()?;
    let digest = |bytes: &str| format!("{:x}", Sha256::digest(bytes.as_bytes()));
    if package.workflow_id != manifest.id
        || package.workflow_version != manifest.workflow_version
        || package.recipe_version != manifest.recipe_version
        || package.workflow_sha256 != digest(new.workflow)
        || package.recipe_sha256 != digest(new.recipe)
    {
        return None;
    }
    let old_name = match new.directory {
        "minimax_h3_fl2va_t2v_quality_2_2_0" => "minimax_h3_fl2va_t2v_quality_2_1_0",
        "minimax_h3_fl2va_i2v_quality_2_2_0" => "minimax_h3_fl2va_i2v_quality_2_1_0",
        "minimax_h3_fl2va_first_last_quality_2_2_0" => "minimax_h3_fl2va_first_last_quality_2_1_1",
        "minimax_h3_reference_video_quality_2_2_0" => "minimax_h3_reference_video_quality_2_1_0",
        "minimax_h3_reference_veda_turbo_1_1_0" => "minimax_h3_reference_veda_turbo_1_0_0",
        _ => return None,
    };
    let old = PACKAGES.iter().find(|p| p.directory == old_name)?;
    Some((old.directory, digest(old.workflow), digest(old.recipe)))
}

pub fn ensure_installed(root: &Path) -> Result<(), String> {
    for package in PACKAGES.iter().filter(|package| active_package(package)) {
        let directory = root.join(package.directory);
        if directory.exists() {
            if let Some(mismatch) = audit_package(package, &directory) {
                return Err(format!(
                    "BUILTIN_PACKAGE_HASH_MISMATCH: {} expected {} but found {}",
                    mismatch.package_name, mismatch.expected_sha256, mismatch.actual_sha256
                ));
            }
            continue;
        }
        install_package(package, &directory)?;
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltinPackageMismatch {
    pub package_name: String,
    pub code: String,
    pub expected_sha256: String,
    pub actual_sha256: String,
    pub source_path: String,
}

pub fn audit_installed(root: &Path) -> Vec<BuiltinPackageMismatch> {
    PACKAGES
        .iter()
        .filter(|package| active_package(package))
        .filter_map(|package| {
            let directory = root.join(package.directory);
            directory
                .exists()
                .then(|| audit_package(package, &directory))
                .flatten()
        })
        .collect()
}

/// Explicit repair action. The mismatched directory is moved beside the
/// library root before the embedded immutable package is installed, so no
/// user data is silently overwritten and the old bytes remain recoverable.
pub fn repair_package(root: &Path, package_name: &str) -> Result<(), String> {
    let package = PACKAGES
        .iter()
        .find(|package| package.directory == package_name)
        .ok_or_else(|| format!("BUILTIN_PACKAGE_NOT_FOUND: {package_name}"))?;
    let directory = root.join(package.directory);
    if directory.exists() {
        if audit_package(package, &directory).is_none() {
            return Ok(());
        }
        let quarantine_root = quarantine_root(root);
        fs::create_dir_all(&quarantine_root)
            .map_err(|error| format!("create builtin package quarantine: {error}"))?;
        let quarantine = quarantine_root.join(format!("{}-{}", package.directory, unique_suffix()));
        fs::rename(&directory, &quarantine)
            .map_err(|error| format!("quarantine mismatched builtin package: {error}"))?;
    }
    install_package(package, &directory)
}

fn install_package(package: &BuiltinPackage, directory: &Path) -> Result<(), String> {
    fs::create_dir_all(directory)
        .map_err(|error| format!("create builtin runtime package: {error}"))?;
    if let Err(error) = (|| {
        fs::write(directory.join("manifest.yaml"), package.manifest)
            .map_err(|error| format!("write manifest.yaml: {error}"))?;
        fs::write(directory.join("recipe.yaml"), package.recipe)
            .map_err(|error| format!("write recipe.yaml: {error}"))?;
        fs::write(directory.join("workflow_api.json"), package.workflow)
            .map_err(|error| format!("write workflow_api.json: {error}"))?;
        Ok::<(), String>(())
    })() {
        let _ = fs::remove_dir_all(directory);
        return Err(error);
    }
    Ok(())
}

fn audit_package(package: &BuiltinPackage, directory: &Path) -> Option<BuiltinPackageMismatch> {
    let expected = package_hash(package.manifest, package.recipe, package.workflow);
    let actual = match (
        fs::read(directory.join("manifest.yaml")),
        fs::read(directory.join("recipe.yaml")),
        fs::read(directory.join("workflow_api.json")),
    ) {
        (Ok(manifest), Ok(recipe), Ok(workflow)) => {
            package_hash_bytes(&manifest, &recipe, &workflow)
        }
        _ => "missing".to_owned(),
    };
    (actual != expected).then(|| BuiltinPackageMismatch {
        package_name: package.directory.to_owned(),
        code: "BUILTIN_PACKAGE_HASH_MISMATCH".to_owned(),
        expected_sha256: expected,
        actual_sha256: actual,
        source_path: directory.to_string_lossy().to_string(),
    })
}

fn package_hash(manifest: &str, recipe: &str, workflow: &str) -> String {
    package_hash_bytes(manifest.as_bytes(), recipe.as_bytes(), workflow.as_bytes())
}

fn package_hash_bytes(manifest: &[u8], recipe: &[u8], workflow: &[u8]) -> String {
    let mut hasher = Sha256::new();
    for bytes in [manifest, recipe, workflow] {
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    format!("{:x}", hasher.finalize())
}

fn quarantine_root(root: &Path) -> PathBuf {
    let name = root
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "workflow-library".to_owned());
    root.with_file_name(format!("{name}.builtin-quarantine"))
}

fn unique_suffix() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos().to_string())
        .unwrap_or_else(|_| "unknown".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{
        active_package, audit_installed, ensure_installed, is_builtin_package_name, repair_package,
        PACKAGES,
    };
    use crate::application::workflow_manifest::WorkflowManifest;
    use crate::compiler::{BindingValidator, RecipeParser, RecipeValidator, WorkflowValidator};
    use crate::domain::WorkflowDocument;
    use tempfile::tempdir;

    fn veda_reference() -> &'static super::BuiltinPackage {
        PACKAGES
            .iter()
            .find(|p| p.directory == "minimax_h3_reference_veda_turbo_1_0_0")
            .unwrap()
    }

    #[test]
    fn veda_reference_preserves_turbo_chain_and_sanitizes_examples() {
        let p = veda_reference();
        let g: serde_json::Value = serde_json::from_str(p.workflow).unwrap();
        assert!(active_package(p));
        assert_eq!(g["144"]["inputs"]["value"], 4);
        assert_eq!(g["124"]["inputs"]["steps"], serde_json::json!(["144", 0]));
        assert_eq!(g["147"]["class_type"], "VedaSparseAttention");
        assert_eq!(g["147"]["inputs"]["model"], serde_json::json!(["145", 0]));
        assert_eq!(g["145"]["inputs"]["model"], serde_json::json!(["127", 0]));
        assert_eq!(g["126"]["inputs"]["model"], serde_json::json!(["147", 0]));
        assert_eq!(
            g["147"]["inputs"]["predictor"],
            "minimax_h3_r2va_veda_preview_fp8.safetensors"
        );
        assert_eq!(g["147"]["inputs"]["generated_sparsity"], "32");
        assert_eq!(g["147"]["inputs"]["reference_sparsity"], "32");
        assert_eq!(g["138"]["inputs"]["value"], "");
        for id in ["137", "139", "149", "150", "151", "152", "153", "154"] {
            assert_eq!(g[id]["inputs"]["image"], "");
        }
        for id in ["115", "141", "142", "143", "146"] {
            assert!(g.get(id).is_none());
        }
        let standard: serde_json::Value =
            serde_json::from_str(standard_reference().workflow).unwrap();
        assert_eq!(standard["143"]["inputs"]["value"], 20);
        assert!(standard.get("147").is_none());
    }

    #[test]
    fn veda_reference_compiles_one_to_eight_images_in_order_without_empty_slots() {
        use crate::domain::{CompileRequest, InputValue};
        let p = veda_reference();
        let recipe = RecipeParser::parse(p.recipe).unwrap();
        let workflow = WorkflowDocument::parse(serde_json::from_str(p.workflow).unwrap()).unwrap();
        let slot_nodes = ["137", "139", "149", "150", "151", "152", "153", "154"];
        for count in 0..=9 {
            let images = (0..count)
                .map(|i| format!("reference-{i}.png"))
                .collect::<Vec<_>>();
            let request = CompileRequest::new(
                [
                    (
                        "prompt".into(),
                        InputValue::String("Requested action with <Picture 1>".into()),
                    ),
                    (
                        "reference_images".into(),
                        InputValue::Images(images.clone()),
                    ),
                    ("duration_seconds".into(), InputValue::Integer(5)),
                    ("width".into(), InputValue::Integer(1376)),
                    ("height".into(), InputValue::Integer(768)),
                    (
                        "seed".into(),
                        InputValue::Seed(crate::domain::SeedValue::Fixed(42)),
                    ),
                ]
                .into(),
            );
            let result = crate::application::product::h3_resolution::compile_checked(
                &crate::compiler::WorkflowCompiler,
                &workflow,
                &recipe,
                &request,
            );
            if !(1..=8).contains(&count) {
                assert!(result.is_err(), "reference count {count} must be rejected");
                continue;
            }
            let compiled = result.unwrap();
            let g = compiled.workflow;
            for (i, id) in slot_nodes.iter().enumerate() {
                let key = format!("ref_images.ref_image_{i}");
                if i < count {
                    assert_eq!(g["136"]["inputs"][key.as_str()], serde_json::json!([id, 0]));
                    assert_eq!(g[*id]["inputs"]["image"], images[i]);
                } else {
                    assert!(g["136"]["inputs"].get(&key).is_none());
                    assert!(g.get(*id).is_none(), "unused image loader must be removed");
                }
            }
            assert_eq!(
                g["138"]["inputs"]["value"],
                "Requested action with <Picture 1>"
            );
            assert_eq!(g["136"]["inputs"]["width"], 1376);
            assert_eq!(g["136"]["inputs"]["height"], 768);
            assert_eq!(g["129"]["inputs"]["noise_seed"], 42);
            assert_eq!(g["130"]["inputs"]["images"], serde_json::json!(["161", 0]));
            assert_eq!(g["130"]["inputs"]["audio"], serde_json::json!(["162", 0]));
            assert_eq!(recipe.outputs[0].node, "92");
        }
    }

    #[test]
    fn veda_reference_v1_1_adds_optional_media_and_compiles_without_it() {
        use crate::domain::{CompileRequest, InputValue};
        let old = veda_reference();
        let p = PACKAGES
            .iter()
            .find(|p| p.directory == "minimax_h3_reference_veda_turbo_1_1_0")
            .unwrap();
        assert!(active_package(p) && active_package(old));
        let base: serde_json::Value = serde_json::from_str(old.workflow).unwrap();
        let graph: serde_json::Value = serde_json::from_str(p.workflow).unwrap();
        for (id, node) in base.as_object().unwrap() {
            if id != "136" {
                assert_eq!(&graph[id], node, "Veda node {id} must stay byte-equivalent");
            }
        }
        let recipe = RecipeParser::parse(p.recipe).unwrap();
        let classified = crate::application::shot_workflow_compatibility::classify_shot_recipe(
            crate::domain::ShotStage::Video,
            "wfl_minimax_h3_reference_veda_turbo",
            &recipe,
        )
        .unwrap();
        assert!(matches!(
            classified.input_mode,
            crate::application::shot_workflow_compatibility::ShotVideoInputMode::ReferenceMedia
        ));
        let workflow = WorkflowDocument::parse(graph).unwrap();
        let compile = |videos: Vec<String>, audios: Vec<String>| {
            let images = (0..6).map(|i| format!("ref-{i}.png")).collect::<Vec<_>>();
            crate::application::product::h3_resolution::compile_checked(
                &crate::compiler::WorkflowCompiler,
                &workflow,
                &recipe,
                &CompileRequest::new(
                    [
                        ("prompt".into(), InputValue::String("go".into())),
                        ("reference_images".into(), InputValue::Images(images)),
                        ("reference_videos".into(), InputValue::Videos(videos)),
                        ("reference_audios".into(), InputValue::Audios(audios)),
                        ("duration_seconds".into(), InputValue::Integer(6)),
                        ("width".into(), InputValue::Integer(1344)),
                        ("height".into(), InputValue::Integer(768)),
                    ]
                    .into(),
                ),
            )
        };
        let g = compile(vec![], vec![]).unwrap().workflow;
        crate::compiler::FinalCompiledWorkflowValidator::validate(&g, &recipe, &[]).unwrap();
        for id in ["40", "41", "42", "43", "44", "45", "50", "51", "52"] {
            assert!(
                g.get(id).is_none(),
                "empty media loader {id} must be removed"
            );
        }
        let inputs = g["136"]["inputs"].as_object().unwrap();
        assert!(!inputs
            .keys()
            .any(|k| k.starts_with("ref_video") || k.starts_with("ref_audio")));
        assert_eq!(
            g["136"]["inputs"]["ref_images.ref_image_5"],
            serde_json::json!(["152", 0])
        );
        let g = compile(vec!["clip.mp4".into()], vec!["voice.wav".into()])
            .unwrap()
            .workflow;
        assert_eq!(g["40"]["inputs"]["file"], "clip.mp4");
        assert_eq!(g["50"]["inputs"]["audio"], "voice.wav");
        assert_eq!(
            g["136"]["inputs"]["ref_videos.ref_video_0"],
            serde_json::json!(["41", 0])
        );
        assert_eq!(
            g["136"]["inputs"]["ref_audios.ref_audio_0"],
            serde_json::json!(["50", 0])
        );
        assert!(g.get("42").is_none() && g.get("51").is_none());
        assert!(compile(vec!["a.mp4".into(); 4], vec![]).is_err());
    }

    fn standard_reference() -> &'static super::BuiltinPackage {
        PACKAGES
            .iter()
            .find(|p| p.directory == "minimax_h3_reference_standard_1_0_0")
            .unwrap()
    }

    #[test]
    fn standard_reference_preserves_twenty_step_models_and_sage_without_turbo() {
        let p = standard_reference();
        let graph: serde_json::Value = serde_json::from_str(p.workflow).unwrap();
        assert!(active_package(p));
        assert_eq!(graph["143"]["inputs"]["value"], 20);
        assert_eq!(
            graph["124"]["inputs"]["steps"],
            serde_json::json!(["143", 0])
        );
        assert_eq!(graph["123"]["inputs"]["sampler_name"], "res_multistep");
        assert_eq!(graph["124"]["inputs"]["scheduler"], "simple");
        assert_eq!(graph["148"]["class_type"], "PathchSageAttentionKJ");
        assert_eq!(graph["148"]["inputs"]["sage_attention"], "auto");
        assert_eq!(graph["148"]["inputs"]["allow_compile"], false);
        assert_eq!(
            graph["148"]["inputs"]["model"],
            serde_json::json!(["127", 0])
        );
        assert_eq!(
            graph["126"]["inputs"]["model"],
            serde_json::json!(["148", 0])
        );
        assert_eq!(
            graph["127"]["inputs"]["unet_name"],
            r"minmaxh3\minimax_h3_hybrid_fl2va_ref2va_b25-49-int8.safetensors"
        );
        assert_eq!(
            graph["128"]["inputs"]["clip_name"],
            r"minmaxh3\qwen3vl_32b_minimax_h3_nvfp4_awq.safetensors"
        );
        for id in ["141", "142", "144", "145", "146"] {
            assert!(graph.get(id).is_none());
        }
        assert!(!p.workflow.contains("turbo"));
        assert!(!p.workflow.contains("Krea2-202608"));
        assert!(!p.workflow.contains("AnimateDiff_00029"));
        assert_eq!(graph["138"]["inputs"]["value"], "");
        assert!(!p.recipe.contains("reference_videos:"));
        assert!(!p.recipe.contains("reference_audios:"));
    }

    #[test]
    fn standard_reference_compiles_actual_ordered_images_prompt_canvas_and_seed() {
        use crate::domain::{CompileRequest, InputValue};
        let p = standard_reference();
        let recipe = RecipeParser::parse(p.recipe).unwrap();
        let workflow = WorkflowDocument::parse(serde_json::from_str(p.workflow).unwrap()).unwrap();
        let classified = crate::application::shot_workflow_compatibility::classify_shot_recipe(
            crate::domain::ShotStage::Video,
            "wfl_minimax_h3_reference_standard",
            &recipe,
        )
        .unwrap();
        assert!(matches!(
            classified.input_mode,
            crate::application::shot_workflow_compatibility::ShotVideoInputMode::ReferenceImages {
                max_items: 2,
                ..
            }
        ));
        for images in [
            vec!["character.png"],
            vec!["character.png", "scene.png"],
            vec!["scene.png", "character.png"],
        ] {
            let values = [
                (
                    "prompt".into(),
                    InputValue::String("Alice（<Picture 1>）在场景（<Picture 2>）中跳舞".into()),
                ),
                (
                    "reference_images".into(),
                    InputValue::Images(images.iter().map(|s| s.to_string()).collect()),
                ),
                ("duration_seconds".into(), InputValue::Integer(6)),
                ("width".into(), InputValue::Integer(480)),
                ("height".into(), InputValue::Integer(864)),
                (
                    "seed".into(),
                    InputValue::Seed(crate::domain::SeedValue::Fixed(42)),
                ),
            ]
            .into();
            let compiled = crate::application::product::h3_resolution::compile_checked(
                &crate::compiler::WorkflowCompiler,
                &workflow,
                &recipe,
                &CompileRequest::new(values),
            )
            .unwrap();
            let g = compiled.workflow;
            assert_eq!(g["137"]["inputs"]["image"], images[0]);
            if images.len() == 2 {
                assert_eq!(g["139"]["inputs"]["image"], images[1]);
                assert_eq!(
                    g["136"]["inputs"]["ref_images.ref_image_1"],
                    serde_json::json!(["139", 0])
                );
            } else {
                assert!(g["136"]["inputs"].get("ref_images.ref_image_1").is_none());
            }
            assert_eq!(
                g["138"]["inputs"]["value"],
                "Alice（<Picture 1>）在场景（<Picture 2>）中跳舞"
            );
            assert_eq!(g["136"]["inputs"]["prompt"], serde_json::json!(["138", 0]));
            assert_eq!(g["136"]["inputs"]["width"], 480);
            assert_eq!(g["136"]["inputs"]["height"], 864);
            assert_eq!(g["132"]["inputs"]["value"], 6);
            assert_eq!(g["129"]["inputs"]["noise_seed"], 42);
            assert_eq!(g["130"]["inputs"]["images"], serde_json::json!(["161", 0]));
            assert_eq!(g["130"]["inputs"]["audio"], serde_json::json!(["162", 0]));
            assert_eq!(g["161"]["inputs"]["length"], serde_json::json!(["160", 1]));
            assert_eq!(g["160"]["inputs"]["expression"], "round(a * 24)");
            assert_eq!(
                g["162"]["inputs"]["duration"],
                serde_json::json!(["132", 0])
            );
            assert_eq!(recipe.outputs[0].node, "92");
        }
        for (duration, images, ok) in [
            (4, vec![], false),
            (3, vec!["a.png"], false),
            (16, vec!["a.png"], false),
            (4, vec!["a.png", "b.png", "c.png"], false),
            (4, vec!["a.png"], true),
            (15, vec!["a.png", "b.png"], true),
        ] {
            let request = CompileRequest::new(
                [
                    ("prompt".into(), InputValue::String("test".into())),
                    ("duration_seconds".into(), InputValue::Integer(duration)),
                    (
                        "reference_images".into(),
                        InputValue::Images(images.into_iter().map(String::from).collect()),
                    ),
                ]
                .into(),
            );
            assert_eq!(
                crate::application::product::h3_resolution::compile_checked(
                    &crate::compiler::WorkflowCompiler,
                    &workflow,
                    &recipe,
                    &request
                )
                .is_ok(),
                ok
            );
        }
    }

    #[tokio::test]
    async fn standard_reference_syncs_exact_pair_idempotently_without_production_tasks() {
        use crate::application::ports::{
            WorkflowRuntimeArtifactRepository, WorkflowRuntimeRepository,
        };
        use crate::application::workflow_library_service::WorkflowLibraryService;
        use crate::infrastructure::database::{
            initialize, SqliteWorkflowLibraryRepository, SqliteWorkflowRuntimeArtifactRepository,
            SqliteWorkflowRuntimeRepository,
        };
        use crate::infrastructure::filesystem::FileSystemWorkflowLibrarySource;
        use crate::infrastructure::time::SystemClock;
        use std::sync::Arc;
        let dir = tempdir().unwrap();
        let library = dir.path().join("library");
        ensure_installed(&library).unwrap();
        let pool = initialize(&dir.path().join("owned.sqlite")).await.unwrap();
        let service = WorkflowLibraryService::new(
            Arc::new(FileSystemWorkflowLibrarySource::new(library)),
            Arc::new(SqliteWorkflowLibraryRepository::new(pool.clone())),
            Arc::new(SystemClock),
        );
        let first = service.sync().await.unwrap();
        assert_eq!(first.invalid, 0, "{:?}", first.errors);
        assert_eq!(first.inserted, 7);
        let second = service.sync().await.unwrap();
        assert_eq!(second.reused, 7);
        assert_eq!(second.inserted, 0);
        let versions = SqliteWorkflowRuntimeRepository::new(pool.clone())
            .list_versions_for_workflow("wfl_minimax_h3_reference_standard")
            .await
            .unwrap();
        assert_eq!(versions.len(), 1);
        let version = &versions[0];
        assert_eq!(version.recipes.len(), 1);
        let recipe = &version.recipes[0];
        let artifact = SqliteWorkflowRuntimeArtifactRepository::new(pool.clone())
            .find_exact(
                &version.workflow_version_id,
                &recipe.recipe_id,
                standard_reference().directory,
            )
            .await
            .unwrap()
            .unwrap();
        let live = super::embedded_package(standard_reference().directory).unwrap();
        assert!(
            crate::application::minimax_video_product_policy::authorizes(
                version, recipe, &artifact, &live
            )
        );
        for table in [
            "tasks",
            "production_batches",
            "production_batch_items",
            "production_deferred_starts",
        ] {
            let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(
                count, 0,
                "registering a package must not start production: {table}"
            );
        }
        let violations = sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert!(violations.is_empty());
        pool.close().await;
    }

    #[test]
    fn standard_reference_installs_additively_and_never_overwrites_user_package() {
        let dir = tempdir().unwrap();
        ensure_installed(dir.path()).unwrap();
        let package = dir.path().join(standard_reference().directory);
        assert_eq!(
            std::fs::read(package.join("workflow_api.json")).unwrap(),
            standard_reference().workflow.as_bytes()
        );
        std::fs::write(package.join("recipe.yaml"), "owned user edits").unwrap();
        assert!(ensure_installed(dir.path()).is_err());
        assert_eq!(
            std::fs::read_to_string(package.join("recipe.yaml")).unwrap(),
            "owned user edits"
        );
        assert_eq!(audit_installed(dir.path()).len(), 1);
    }

    #[tokio::test]
    async fn h3_default_upgrade_is_exact_preserves_history_and_respects_user_lifecycle() {
        use crate::application::ports::{WorkflowLibraryRepository, WorkflowPackageRecord};
        use crate::infrastructure::database::{initialize, SqliteWorkflowLibraryRepository};
        let record = |package: &super::BuiltinPackage| {
            let manifest = WorkflowManifest::parse(package.manifest).unwrap();
            let recipe = RecipeParser::parse(package.recipe).unwrap();
            let digest = |bytes: &str| {
                format!(
                    "{:x}",
                    <sha2::Sha256 as sha2::Digest>::digest(bytes.as_bytes())
                )
            };
            WorkflowPackageRecord {
                workflow_id: manifest.id,
                source_kind: "PRODUCT".into(),
                package_name: package.directory.into(),
                package_source_path: None,
                name: manifest.name,
                category: manifest.category,
                mode: manifest.mode,
                workflow_version: manifest.workflow_version,
                workflow_json: serde_json::from_str(package.workflow).unwrap(),
                workflow_sha256: digest(package.workflow),
                source_workflow_json: None,
                recognition_metadata_json: None,
                recipe_version: manifest.recipe_version,
                recipe_schema_version: recipe.schema_version,
                recipe_yaml: package.recipe.into(),
                recipe_sha256: digest(package.recipe),
                created_at: chrono::Utc::now(),
            }
        };
        for new in PACKAGES
            .iter()
            .filter(|p| p.directory.contains("_quality_2_2_0"))
        {
            let new_record = record(new);
            let (old_name, _, _) = super::h3_base_supersession(&new_record).unwrap();
            let old = PACKAGES.iter().find(|p| p.directory == old_name).unwrap();
            let mut tampered = new_record.clone();
            tampered.recipe_sha256 = "0".repeat(64);
            assert!(super::h3_base_supersession(&tampered).is_none());
            for lifecycle in ["active", "removed", "disabled", "archived", "promoted"] {
                let directory = tempdir().unwrap();
                let pool = initialize(&directory.path().join("owned.sqlite"))
                    .await
                    .unwrap();
                let repository = SqliteWorkflowLibraryRepository::new(pool.clone());
                repository.register_package(&record(old)).await.unwrap();
                let old_id: String =
                    sqlx::query_scalar("SELECT current_version_id FROM workflows WHERE id=?")
                        .bind(&new_record.workflow_id)
                        .fetch_one(&pool)
                        .await
                        .unwrap();
                if lifecycle == "removed" {
                    sqlx::query("UPDATE workflows SET library_state='REMOVED' WHERE id=?")
                        .bind(&new_record.workflow_id)
                        .execute(&pool)
                        .await
                        .unwrap();
                }
                if lifecycle == "disabled" || lifecycle == "archived" {
                    sqlx::query("INSERT INTO workflow_runtime_states (workflow_version_id,enabled,archived,updated_at) VALUES (?,?,?,?)").bind(&old_id).bind(if lifecycle=="disabled" {0}else{1}).bind(if lifecycle=="archived" {1}else{0}).bind(chrono::Utc::now().to_rfc3339()).execute(&pool).await.unwrap();
                }
                if lifecycle == "promoted" {
                    sqlx::query("INSERT INTO workflow_recipe_promotions (workflow_version_id,recipe_id,promoted_at) SELECT workflow_version_id,id,? FROM recipes WHERE workflow_version_id=?").bind(chrono::Utc::now().to_rfc3339()).bind(&old_id).execute(&pool).await.unwrap();
                }
                repository.register_package(&new_record).await.unwrap();
                repository.register_package(&new_record).await.unwrap();
                let current: String =
                    sqlx::query_scalar("SELECT current_version_id FROM workflows WHERE id=?")
                        .bind(&new_record.workflow_id)
                        .fetch_one(&pool)
                        .await
                        .unwrap();
                assert_eq!(current != old_id, lifecycle == "active");
                let historical: String = sqlx::query_scalar(
                    "SELECT recipe_yaml FROM recipes WHERE workflow_version_id=?",
                )
                .bind(&old_id)
                .fetch_one(&pool)
                .await
                .unwrap();
                assert_eq!(historical, old.recipe);
                assert_eq!(
                    sqlx::query_scalar::<_, i64>("SELECT count(*) FROM workflow_versions")
                        .fetch_one(&pool)
                        .await
                        .unwrap(),
                    2
                );
                assert_eq!(
                    sqlx::query_scalar::<_, i64>("SELECT count(*) FROM recipes")
                        .fetch_one(&pool)
                        .await
                        .unwrap(),
                    2
                );
                pool.close().await;
            }
        }
    }
    #[test]
    fn new_h3_identities_compile_four_to_fifteen_seconds_without_mutating_history() {
        use crate::compiler::WorkflowCompiler;
        use crate::domain::{CompileRequest, InputValue};
        for package in PACKAGES
            .iter()
            .filter(|p| p.directory.contains("_quality_2_2_0"))
        {
            let recipe = RecipeParser::parse(package.recipe).unwrap();
            let workflow =
                WorkflowDocument::parse(serde_json::from_str(package.workflow).unwrap()).unwrap();
            assert!(package.manifest.contains("workflow_version: 2.2.0"));
            for duration in [3, 4, 15, 16] {
                let mut values: std::collections::BTreeMap<String, InputValue> = [
                    ("duration_seconds".into(), InputValue::Integer(duration)),
                    ("prompt".into(), InputValue::String("test".into())),
                ]
                .into();
                for field in ["first_frame", "last_frame"] {
                    if recipe.inputs.contains_key(field) {
                        values.insert(field.into(), InputValue::Image(format!("{field}.png")));
                    }
                }
                // The REF2VA product requires at least one image or video at
                // admission; a missing reference is an editable draft, not a
                // valid production compile fixture.
                if recipe.inputs.contains_key("reference_videos") {
                    values.insert(
                        "reference_videos".into(),
                        InputValue::Videos(vec!["reference.mp4".into()]),
                    );
                }
                let compiled = crate::application::product::h3_resolution::compile_checked(
                    &WorkflowCompiler,
                    &workflow,
                    &recipe,
                    &CompileRequest::new(values),
                );
                assert_eq!(
                    compiled.is_ok(),
                    (4..=15).contains(&duration),
                    "{}: {} {:?}",
                    package.directory,
                    duration,
                    compiled.as_ref().err()
                );
                if let Ok(compiled) = compiled {
                    crate::application::product::h3_resolution::validate_resolved(
                        compiled.workflow.clone(),
                        &recipe,
                        &compiled.resolved_inputs,
                    )
                    .unwrap();
                }
            }
            // Exercise the production compile gate with the shipped REF2VA
            // workflow and Recipe, not only the isolated validator fixture.
            if recipe.id == "rcp_minimax_h3_reference_video_quality_2_2_0" {
                let empty = CompileRequest::new(
                    [
                        ("duration_seconds".into(), InputValue::Integer(5)),
                        ("prompt".into(), InputValue::String("test".into())),
                    ]
                    .into(),
                );
                assert!(matches!(
                    crate::application::product::h3_resolution::compile_checked(
                        &WorkflowCompiler,
                        &workflow,
                        &recipe,
                        &empty,
                    ),
                    Err(crate::compiler::CompileError::InputRequired { input })
                        if input == "reference_images_or_videos"
                ));
            }
        }
        for old in PACKAGES.iter().filter(|p| {
            p.directory.contains("_quality_2_1_0") || p.directory.ends_with("_quality_2_1_1")
        }) {
            assert!(old.recipe.contains("    min: 1\n"));
            assert!(!active_package(old));
            let recipe = RecipeParser::parse(old.recipe).unwrap();
            let workflow =
                WorkflowDocument::parse(serde_json::from_str(old.workflow).unwrap()).unwrap();
            for duration in [3, 4, 15, 16] {
                let mut request = CompileRequest::new(
                    [
                        ("duration_seconds".into(), InputValue::Integer(duration)),
                        ("prompt".into(), InputValue::String("legacy draft".into())),
                        ("width".into(), InputValue::Integer(1344)),
                        ("height".into(), InputValue::Integer(768)),
                    ]
                    .into(),
                );
                for field in ["first_frame", "last_frame"] {
                    if recipe.inputs.contains_key(field) {
                        request
                            .values
                            .insert(field.into(), InputValue::Image(format!("{field}.png")));
                    }
                }
                // Legacy REF recipes are read-only, but this compile fixture
                // must still satisfy the shared production minimum-media gate.
                if recipe.inputs.contains_key("reference_videos") {
                    request.values.insert(
                        "reference_videos".into(),
                        InputValue::Videos(vec!["reference.mp4".into()]),
                    );
                }
                assert_eq!(
                    crate::application::product::h3_resolution::compile_checked(
                        &WorkflowCompiler,
                        &workflow,
                        &recipe,
                        &request
                    )
                    .is_ok(),
                    (4..=15).contains(&duration)
                );
            }
        }
    }
    #[test]
    fn quality_only_installation_trims_video_and_audio_to_requested_seconds() {
        let directory = tempdir().unwrap();
        ensure_installed(directory.path()).unwrap();
        let installed = std::fs::read_dir(directory.path()).unwrap().count();
        assert_eq!(
            installed, 7,
            "four quality modes plus the approved standard and both Veda reference packages install"
        );
        assert!(!directory
            .path()
            .join("kera2_t2i_local_v2_1_1_1_90894e9e")
            .exists());
        for package in PACKAGES
            .iter()
            .filter(|p| p.directory.contains("_quality_2_1_0"))
        {
            let graph: serde_json::Value = serde_json::from_str(package.workflow).unwrap();
            let recipe = RecipeParser::parse(package.recipe).unwrap();
            let workflow = WorkflowDocument::parse(graph.clone()).unwrap();
            RecipeValidator::validate(&recipe).unwrap();
            WorkflowValidator::validate(&workflow).unwrap();
            BindingValidator::validate(&recipe, &workflow).unwrap();
            assert_eq!(graph["60"]["inputs"]["expression"], "round(a * 24)");
            assert_eq!(
                graph["60"]["inputs"]["values.a"],
                serde_json::json!(["22", 0])
            );
            assert_eq!(
                graph["61"]["inputs"]["length"],
                serde_json::json!(["60", 1])
            );
            assert_eq!(graph["61"]["class_type"], "ImageFromBatch");
            assert_eq!(graph["62"]["class_type"], "TrimAudioDuration");
            assert_eq!(
                graph["62"]["inputs"]["duration"],
                serde_json::json!(["22", 0])
            );
            assert_eq!(
                graph["19"]["inputs"]["images"],
                serde_json::json!(["61", 0])
            );
            assert_eq!(graph["19"]["inputs"]["audio"], serde_json::json!(["62", 0]));
            assert_eq!(graph["19"]["inputs"]["fps"], 24);
            assert!(!package.workflow.contains("NBH3HyperStep"));
            for seconds in 1..=15 {
                let frames = seconds * 24;
                let aligned = frames + (5 - frames % 17 + 17) % 17;
                assert!(aligned >= frames);
                assert_eq!(aligned % 17, 5);
                assert_eq!(frames / 24, seconds);
            }
        }
    }

    #[test]
    fn first_last_quality_anchors_tail_at_delivered_last_frame() {
        let package = PACKAGES
            .iter()
            .find(|p| p.directory == "minimax_h3_fl2va_first_last_quality_2_1_1")
            .unwrap();
        let graph: serde_json::Value = serde_json::from_str(package.workflow).unwrap();
        let recipe = RecipeParser::parse(package.recipe).unwrap();
        let workflow = WorkflowDocument::parse(graph.clone()).unwrap();
        RecipeValidator::validate(&recipe).unwrap();
        WorkflowValidator::validate(&workflow).unwrap();
        BindingValidator::validate(&recipe, &workflow).unwrap();
        assert!(graph["14"]["inputs"].get("last_frame").is_none());
        assert_eq!(
            graph["14"]["inputs"]["first_frame"],
            serde_json::json!(["24", 0])
        );
        assert_eq!(graph["63"]["inputs"]["expression"], "a - 1");
        assert_eq!(
            graph["63"]["inputs"]["values.a"],
            serde_json::json!(["60", 1])
        );
        assert_eq!(graph["64"]["class_type"], "MiniMaxH3AddGuide");
        assert_eq!(graph["64"]["inputs"]["image"], serde_json::json!(["28", 0]));
        assert_eq!(
            graph["64"]["inputs"]["frame_idx"],
            serde_json::json!(["63", 1])
        );
        assert_eq!(
            graph["2"]["inputs"]["conditioning"],
            serde_json::json!(["64", 0])
        );
        assert_eq!(
            graph["61"]["inputs"]["length"],
            serde_json::json!(["60", 1])
        );
        assert_eq!(graph["19"]["inputs"]["fps"], 24);
        for seconds in 1..=15 {
            let frames = seconds * 24;
            let aligned = frames + (5 - frames % 17 + 17) % 17;
            assert!(frames - 1 < aligned);
        }
    }

    #[test]
    fn compatible_copy_only_bypasses_hyperstep_and_preserves_original_identity() {
        let original = PACKAGES
            .iter()
            .find(|p| p.directory == "minimax_h3_fl2va_1_0_0")
            .unwrap();
        let compatible = PACKAGES
            .iter()
            .find(|p| p.directory == "minimax_h3_fl2va_compatible_1_0_0")
            .unwrap();
        let mut expected: serde_json::Value = serde_json::from_str(original.workflow).unwrap();
        let actual: serde_json::Value = serde_json::from_str(compatible.workflow).unwrap();
        assert_eq!(expected["26"]["class_type"], "NBH3HyperStepSimple");
        let model = expected["26"]["inputs"]["model"].clone();
        expected["2"]["inputs"]["model"] = model.clone();
        expected["23"]["inputs"]["model"] = model;
        expected.as_object_mut().unwrap().remove("26");
        assert_eq!(actual, expected);
        assert_ne!(
            WorkflowManifest::parse(original.manifest).unwrap().id,
            WorkflowManifest::parse(compatible.manifest).unwrap().id
        );
        assert!(compatible.recipe.contains("duration_seconds:"));
        assert!(compatible.recipe.contains("first_frame:"));
        assert!(compatible.recipe.contains("last_frame:"));
    }

    #[test]
    fn installs_missing_product_packages_without_overwriting_existing_directory() {
        let directory = tempdir().expect("temp directory");
        ensure_installed(directory.path()).expect("builtin packages should install");
        let fl2va = directory.path().join("minimax_h3_fl2va_i2v_quality_2_2_0");
        let ref2va = directory
            .path()
            .join("minimax_h3_reference_video_quality_2_2_0");
        let quality_t2v = directory.path().join("minimax_h3_fl2va_t2v_quality_2_2_0");
        let quality_ref = directory
            .path()
            .join("minimax_h3_reference_video_quality_2_2_0");
        let kera2 = directory.path().join("kera2_t2i_local_v2_1_1_1_90894e9e");
        assert!(fl2va.join("manifest.yaml").is_file());
        assert!(ref2va.join("recipe.yaml").is_file());
        assert!(quality_t2v.join("workflow_api.json").is_file());
        assert!(quality_ref.join("recipe.yaml").is_file());
        assert!(
            !kera2.exists(),
            "new installs must not activate retired image generation"
        );
        // Existing packages remain intact on upgrades; they are not new defaults.
        std::fs::create_dir_all(&kera2).unwrap();
        let historical = super::embedded_package("kera2_t2i_local_v2_1_1_1_90894e9e").unwrap();
        for (name, bytes) in [
            ("manifest.yaml", &historical.manifest_yaml),
            ("recipe.yaml", &historical.recipe_yaml),
            ("workflow_api.json", &historical.workflow_api_json),
        ] {
            std::fs::write(kera2.join(name), bytes).unwrap();
        }
        assert!(kera2.join("manifest.yaml").is_file());
        assert!(kera2.join("recipe.yaml").is_file());
        assert!(kera2.join("workflow_api.json").is_file());
        let sentinel = fl2va.join("sentinel.txt");
        std::fs::write(&sentinel, "keep").expect("sentinel");
        ensure_installed(directory.path()).expect("second install should be a no-op");
        assert_eq!(std::fs::read_to_string(sentinel).unwrap(), "keep");
        assert_eq!(
            std::fs::read(kera2.join("manifest.yaml")).unwrap(),
            historical.manifest_yaml
        );
        assert_eq!(
            std::fs::read(kera2.join("recipe.yaml")).unwrap(),
            historical.recipe_yaml
        );
        assert_eq!(
            std::fs::read(kera2.join("workflow_api.json")).unwrap(),
            historical.workflow_api_json
        );
    }

    #[test]
    fn same_version_hash_mismatch_is_reported_without_overwrite_and_has_explicit_repair() {
        let directory = tempdir().expect("temp directory");
        ensure_installed(directory.path()).expect("builtin packages should install");
        let package = directory.path().join("minimax_h3_fl2va_i2v_quality_2_2_0");
        std::fs::write(package.join("recipe.yaml"), "user change").expect("mutate package");

        let mismatches = audit_installed(directory.path());
        assert_eq!(mismatches.len(), 1);
        assert_eq!(mismatches[0].code, "BUILTIN_PACKAGE_HASH_MISMATCH");
        let error =
            ensure_installed(directory.path()).expect_err("mismatch must not be overwritten");
        assert!(error.contains("BUILTIN_PACKAGE_HASH_MISMATCH"));
        assert_eq!(
            std::fs::read_to_string(package.join("recipe.yaml")).unwrap(),
            "user change"
        );

        repair_package(directory.path(), "minimax_h3_fl2va_i2v_quality_2_2_0")
            .expect("explicit repair should work");
        assert!(audit_installed(directory.path()).is_empty());
        assert!(
            directory
                .path()
                .with_file_name(".builtin-quarantine")
                .exists()
                || directory
                    .path()
                    .with_file_name(format!(
                        "{}.builtin-quarantine",
                        directory.path().file_name().unwrap().to_string_lossy()
                    ))
                    .exists()
        );
    }

    #[test]
    fn embedded_product_packages_pass_the_same_contract_audit_as_user_packages() {
        for package in PACKAGES {
            let manifest = WorkflowManifest::parse(package.manifest).expect("manifest parses");
            manifest.validate().expect("manifest validates");
            let recipe = RecipeParser::parse(package.recipe).expect("recipe parses");
            RecipeValidator::validate(&recipe).expect("recipe validates");
            let workflow_value: serde_json::Value =
                serde_json::from_str(package.workflow).expect("workflow JSON parses");
            let workflow = WorkflowDocument::parse(workflow_value).expect("workflow parses");
            WorkflowValidator::validate(&workflow).expect("workflow validates");
            BindingValidator::validate(&recipe, &workflow).expect("bindings validate");
        }
    }

    #[test]
    fn product_package_identity_marks_kera2_builtin_without_workflow_id_logic() {
        assert!(is_builtin_package_name("kera2_t2i_local_v2"));
        assert!(is_builtin_package_name("kera2_t2i_local_v2_1_1_0_1d99a10d"));
        assert!(is_builtin_package_name("kera2_t2i_local_v2_1_1_1_90894e9e"));
        assert!(is_builtin_package_name("krea2_t2i_local"));
        assert!(!is_builtin_package_name("custom_kera2_copy"));
        assert_eq!(
            PACKAGES
                .iter()
                .filter(|package| package.directory == "kera2_t2i_local_v2_1_1_1_90894e9e")
                .count(),
            1
        );
    }

    #[test]
    fn embedded_kera2_package_has_canonical_scope_and_clean_runtime_defaults() {
        let package = PACKAGES
            .iter()
            .find(|package| package.directory == "kera2_t2i_local_v2_1_1_1_90894e9e")
            .expect("embedded Kera2 image package");
        let manifest = WorkflowManifest::parse(package.manifest).expect("manifest parses");
        assert_eq!(manifest.id, "wfl_kera2_t2i_local_v2");
        assert_eq!(manifest.workflow_version, "1.1.1");
        assert_eq!(manifest.recipe_version, "1.1.1");
        let recipe = RecipeParser::parse(package.recipe).expect("recipe parses");
        RecipeValidator::validate(&recipe).expect("recipe validates");
        assert_eq!(recipe.bindings.len(), 5);
        assert_eq!(recipe.outputs.len(), 1);
        assert!(package.recipe.contains("rcp_kera2_t2i_local_v2_1_1_1"));

        let workflow_value: serde_json::Value =
            serde_json::from_str(package.workflow).expect("workflow JSON parses");
        let workflow = WorkflowDocument::parse(workflow_value).expect("API workflow parses");
        WorkflowValidator::validate(&workflow).expect("workflow validates");
        BindingValidator::validate(&recipe, &workflow).expect("recipe bindings validate");
        let value = workflow.value();
        assert_eq!(value["11"]["class_type"], "SaveImage");
        assert_eq!(value["11"]["inputs"]["filename_prefix"], "Kera2");
        assert_eq!(value["13"]["inputs"]["text"], "");
        assert!(!package.workflow.contains("20260806104502"));
        assert!(!package.workflow.contains("一个美女在跳舞"));
        assert!(!package.workflow.contains("C:\\Users\\ADMIN"));
        assert!(!package.workflow.contains("D:\\"));
    }

    #[test]
    fn aitudou_fast_package_keeps_fixed_eight_step_graph_and_safe_user_inputs() {
        let package = PACKAGES
            .iter()
            .find(|package| package.directory == "aitudou_minimax_h3_lightx2v_8step_fast_1_0_0")
            .expect("AITUDOU package");
        let manifest = WorkflowManifest::parse(package.manifest).expect("manifest parses");
        assert_eq!(manifest.id, "wfl_aitudou_minimax_h3_lightx2v_8step_fast");
        assert_eq!(manifest.category, "video");
        let recipe = RecipeParser::parse(package.recipe).expect("recipe parses");
        assert_eq!(recipe.inputs.len(), 2);
        assert!(recipe.inputs.contains_key("prompt"));
        assert!(recipe.inputs.contains_key("seed"));
        assert!(recipe
            .bindings
            .iter()
            .all(|binding| { binding.target.node == "59" || binding.target.node == "2" }));
        let workflow: serde_json::Value = serde_json::from_str(package.workflow).unwrap();
        assert_eq!(workflow["50"]["inputs"]["steps"], 8);
        assert_eq!(workflow["61"]["inputs"]["megapixels"], 0.9);
        assert_eq!(workflow["62"]["class_type"], "VHS_VideoCombine");
    }

    #[test]
    fn quality_graphs_restore_the_formal_sampling_chain_without_touching_fast_graphs() {
        let fast_fl2va: serde_json::Value = serde_json::from_str(
            PACKAGES
                .iter()
                .find(|package| package.directory == "minimax_h3_fl2va_1_0_0")
                .expect("fast fl2va package")
                .workflow,
        )
        .unwrap();
        let fast_ref2va: serde_json::Value = serde_json::from_str(
            PACKAGES
                .iter()
                .find(|package| package.directory == "minimax_h3_reference_video_1_3_0")
                .expect("fast ref2va package")
                .workflow,
        )
        .unwrap();
        assert_eq!(fast_fl2va["23"]["inputs"]["steps"], 4);
        assert_eq!(fast_ref2va["23"]["inputs"]["steps"], 4);
        assert!(fast_fl2va.get("27").is_some());
        assert!(fast_ref2va.get("27").is_some());

        for directory in [
            "minimax_h3_fl2va_t2v_quality_2_0_0",
            "minimax_h3_fl2va_i2v_quality_2_0_0",
            "minimax_h3_fl2va_first_last_quality_2_0_0",
            "minimax_h3_reference_video_quality_2_0_0",
        ] {
            let package = PACKAGES
                .iter()
                .find(|package| package.directory == directory)
                .expect("quality package");
            let workflow: serde_json::Value = serde_json::from_str(package.workflow).unwrap();
            assert_eq!(workflow["23"]["inputs"]["steps"], 20);
            assert_eq!(
                workflow["13"]["inputs"]["unet_name"]
                    .as_str()
                    .unwrap()
                    .contains("convrot"),
                true
            );
            assert!(
                workflow.get("27").is_none(),
                "Turbo LoRA must be absent in {directory}"
            );
            assert_eq!(
                workflow["16"]["class_type"],
                "MiniMaxH3MemoryEfficientSageAttentionPatch"
            );
        }

        let t2v: serde_json::Value = serde_json::from_str(
            PACKAGES
                .iter()
                .find(|package| package.directory == "minimax_h3_fl2va_t2v_quality_2_0_0")
                .unwrap()
                .workflow,
        )
        .unwrap();
        assert_eq!(t2v["26"]["inputs"]["mode"], "Middle-36");
        assert_eq!(t2v["26"]["inputs"]["manual_bypass_blocks"], 36);

        let i2v: serde_json::Value = serde_json::from_str(
            PACKAGES
                .iter()
                .find(|package| package.directory == "minimax_h3_fl2va_i2v_quality_2_0_0")
                .unwrap()
                .workflow,
        )
        .unwrap();
        assert_eq!(
            i2v["13"]["inputs"]["unet_name"],
            "minmaxh3\\minimax_h3_fl2va_int8_convrot.safetensors"
        );
        assert!(i2v.get("26").is_none());
        assert!(i2v["14"]["inputs"].get("first_frame").is_some());
        assert!(i2v["14"]["inputs"].get("last_frame").is_none());

        let first_last: serde_json::Value = serde_json::from_str(
            PACKAGES
                .iter()
                .find(|package| package.directory == "minimax_h3_fl2va_first_last_quality_2_0_0")
                .unwrap()
                .workflow,
        )
        .unwrap();
        assert!(first_last["14"]["inputs"].get("first_frame").is_some());
        assert!(first_last["14"]["inputs"].get("last_frame").is_some());

        let ref2va: serde_json::Value = serde_json::from_str(
            PACKAGES
                .iter()
                .find(|package| package.directory == "minimax_h3_reference_video_quality_2_0_0")
                .unwrap()
                .workflow,
        )
        .unwrap();
        assert_eq!(
            ref2va["13"]["inputs"]["unet_name"],
            "minmaxh3\\minimax_h3_ref2va_int8_convrot.safetensors"
        );
        assert_eq!(ref2va["26"]["inputs"]["mode"], "Middle-36");
        assert!(ref2va["14"]["inputs"]
            .get("ref_videos.ref_video_0")
            .is_some());
        assert!(ref2va["14"]["inputs"]
            .get("ref_video_audios.ref_video_audio_0")
            .is_some());
    }
}
