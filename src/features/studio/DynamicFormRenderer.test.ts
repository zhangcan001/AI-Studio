import { describe, expect, it } from "vitest";
import { validateRecipeValues } from "./DynamicFormRenderer";
import type { RecipeViewModel } from "../../types/generation";

const recipe: RecipeViewModel = {
  workflowId: "wfl_seed",
  workflowVersionId: "wfv_seed",
  recipeId: "rcp_seed",
  name: "Seed Recipe",
  category: "image",
  mode: "text_to_image",
  fields: [
    {
      key: "seed",
      type: "seed",
      label: "Seed",
      defaultMode: "random",
      defaultValue: null,
      minValue: "10",
      maxValue: "20",
    },
  ],
};

describe("validateRecipeValues seed ranges", () => {
  it("accepts fixed values at both recipe boundaries", () => {
    expect(
      validateRecipeValues(recipe, { seed: { type: "seed_fixed", value: "10" } }),
    ).toEqual({});
    expect(
      validateRecipeValues(recipe, { seed: { type: "seed_fixed", value: "20" } }),
    ).toEqual({});
  });

  it("rejects fixed values outside the recipe range", () => {
    const errors = validateRecipeValues(recipe, {
      seed: { type: "seed_fixed", value: "21" },
    });

    expect(errors.seed).toContain("随机种子必须在 10 到 20 之间");
  });

  it("uses BigInt for the full u64 range when Recipe has no bounds", () => {
    const unbounded = {
      ...recipe,
      fields: [
        {
          key: "seed",
          type: "seed" as const,
          label: "Seed",
          defaultMode: "random" as const,
          defaultValue: null,
          minValue: null,
          maxValue: null,
        },
      ],
    } satisfies RecipeViewModel;

    expect(
      validateRecipeValues(unbounded, {
        seed: { type: "seed_fixed", value: "18446744073709551615" },
      }),
    ).toEqual({});
  });
});

describe("validateRecipeValues integer steps", () => {
  const integerRecipe: RecipeViewModel = {
    ...recipe,
    fields: [{ key: "width", type: "integer", label: "宽度", required: true, min: 16, max: 2048, step: 16, default: 1024 }],
  };

  it("accepts a valid step and rejects an invalid step without rounding", () => {
    expect(validateRecipeValues(integerRecipe, { width: { type: "integer", value: 1280 } })).toEqual({});
    expect(validateRecipeValues(integerRecipe, { width: { type: "integer", value: 1270 } }).width).toContain("16 的倍数");
  });
});

describe("validateRecipeValues number steps", () => {
  const numberRecipe: RecipeViewModel = {
    ...recipe,
    fields: [{ key: "strength", type: "number", label: "强度", required: true, min: 0, max: 1, step: 0.1, default: 0.3 }],
  };

  it("accepts 0.3 with a 0.1 step and rejects misaligned values", () => {
    expect(validateRecipeValues(numberRecipe, { strength: { type: "number", value: 0.3 } })).toEqual({});
    expect(validateRecipeValues(numberRecipe, { strength: { type: "number", value: 0.35 } }).strength).toContain("0.1");
  });
});

describe("validateRecipeValues image inputs", () => {
  const imageRecipe: RecipeViewModel = {
    ...recipe,
    fields: [{ key: "reference", type: "image", label: "Reference", required: true }],
  };

  it("requires an image asset for a required image field", () => {
    expect(validateRecipeValues(imageRecipe, {})).toEqual({ reference: "请选择图片。" });
    expect(
      validateRecipeValues(imageRecipe, {
        reference: { type: "image_asset", assetId: "ast_reference" },
      }),
    ).toEqual({});
  });
});

describe("validateRecipeValues multi-image inputs", () => {
  const multiRecipe: RecipeViewModel = {
    ...recipe,
    fields: [{
      key: "references",
      type: "images",
      label: "References",
      required: true,
      minItems: 2,
      maxItems: 3,
    }],
  };

  it("requires the recipe minimum and keeps ordered ids as a valid value", () => {
    expect(validateRecipeValues(multiRecipe, {
      references: { type: "image_assets", assetIds: ["ast_first"] },
    }).references).toContain("请选择 2 到 3 张图片。");
    expect(validateRecipeValues(multiRecipe, {
      references: { type: "image_assets", assetIds: ["ast_first", "ast_second"] },
    })).toEqual({});
  });
});

describe("validateRecipeValues media inputs", () => {
  it("requires a compatible single video or audio asset", () => {
    const mediaRecipe: RecipeViewModel = {
      ...recipe,
      fields: [
        { key: "video", type: "video", label: "Source video", required: true },
        { key: "audio", type: "audio", label: "Source audio", required: true },
      ],
    };

    expect(validateRecipeValues(mediaRecipe, {})).toEqual({
      video: "请选择视频。",
      audio: "请选择音频文件。",
    });
    expect(validateRecipeValues(mediaRecipe, {
      video: { type: "video_asset", assetId: "ast_video" },
      audio: { type: "audio_asset", assetId: "ast_audio" },
    })).toEqual({});
  });

  it("validates ordered plural media slots", () => {
    const mediaRecipe: RecipeViewModel = {
      ...recipe,
      fields: [{ key: "clips", type: "videos", label: "Clips", required: true, minItems: 2, maxItems: 3 }],
    };
    expect(validateRecipeValues(mediaRecipe, {
      clips: { type: "video_assets", assetIds: ["ast_one"] },
    }).clips).toContain("请选择 2 到 3 个视频。");
    expect(validateRecipeValues(mediaRecipe, {
      clips: { type: "video_assets", assetIds: ["ast_one", "ast_two"] },
    })).toEqual({});
  });
});
