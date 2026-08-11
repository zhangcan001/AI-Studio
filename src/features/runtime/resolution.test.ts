import { describe, expect, it } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import { isResolutionAllowedByRecipe, validateResolution } from "./resolution";
import {
  KREA2_RESOLUTION_PRESETS,
  isMinimaxH3OutputResolution,
  MINIMAX_H3_RESOLUTION_PRESETS,
  resolutionPresetsForRecipe,
} from "./resolutionPresets";

function recipe(overrides: Partial<RecipeViewModel> = {}): RecipeViewModel {
  return {
    workflowId: "wfl_resolution",
    workflowVersionId: "wfv_resolution",
    recipeId: "rcp_resolution",
    name: "Resolution",
    category: "image",
    mode: "text_to_image",
    fields: [
      { key: "width", type: "integer", label: "宽度", required: true, default: 1024, min: 16, max: 2048, step: 16 },
      { key: "height", type: "integer", label: "高度", required: true, default: 1024, min: 16, max: 2048, step: 16 },
    ],
    ...overrides,
  };
}

describe("Recipe-bound resolution", () => {
  it("keeps the eight Krea2 official aspect ratio presets", () => {
    expect(KREA2_RESOLUTION_PRESETS.filter((preset) => preset.tier === "1k")).toHaveLength(8);
    expect(KREA2_RESOLUTION_PRESETS.find((preset) => preset.width === 1280 && preset.height === 720)).toMatchObject({
      label: "16:9",
      ratio: "16:9",
    });
  });

  it("accepts a valid custom size and rejects non-integer, range, and step violations", () => {
    const valid = validateResolution(recipe(), 1280, 720);
    expect(valid).toEqual({ ok: true, errors: {} });
    expect(validateResolution(recipe(), 1270, 720).errors.width).toContain("16 的倍数");
    expect(validateResolution(recipe(), 1024.5, 1024).errors.width).toContain("整数");
    expect(validateResolution(recipe(), 0, 1024).errors.width).toContain("大于 0");
    expect(validateResolution(recipe(), 4096, 1024).errors.width).toContain("不能大于 2048");
    expect(isResolutionAllowedByRecipe(recipe(), 1024, 1024)).toBe(true);
  });

  it("hides 2K presets when the active Recipe maximum cannot support them", () => {
    const limited = recipe({
      fields: recipe().fields.map((field) => ({ ...field, max: 1536 })),
    });
    const presets = resolutionPresetsForRecipe(limited, KREA2_RESOLUTION_PRESETS);
    expect(presets.some((preset) => preset.tier === "2k")).toBe(false);
    expect(resolutionPresetsForRecipe({
      ...limited,
      fields: limited.fields.map((field) => ({ ...field, max: 4096 })),
    }, KREA2_RESOLUTION_PRESETS).some((preset) => preset.tier === "2k")).toBe(true);
  });

  it("exposes the exact H3 16:9 megapixel ladder through the Recipe contract", () => {
    const h3 = recipe({
      fields: [
        { key: "width", type: "integer", label: "宽度", required: true, default: 1344, min: 32, max: 2048, step: 32 },
        { key: "height", type: "integer", label: "高度", required: true, default: 768, min: 32, max: 2048, step: 32 },
      ],
    });
    const presets = resolutionPresetsForRecipe(h3, MINIMAX_H3_RESOLUTION_PRESETS);
    expect(presets).toHaveLength(14);
    expect(presets.map(({ width, height }) => [width, height])).toEqual([
      [608, 352],
      [736, 416],
      [864, 480],
      [960, 544],
      [1056, 608],
      [1152, 640],
      [1216, 672],
      [1280, 736],
      [1344, 768],
      [1376, 768],
      [1504, 832],
      [1664, 928],
      [1824, 1024],
      [1920, 1088],
    ]);
    expect(new Set(presets.map((preset) => preset.ratio))).toEqual(new Set(["16:9"]));
    expect(isMinimaxH3OutputResolution(960, 544)).toBe(true);
    expect(isMinimaxH3OutputResolution(1024, 576)).toBe(false);
    expect(isMinimaxH3OutputResolution(544, 960)).toBe(false);
  });
});
