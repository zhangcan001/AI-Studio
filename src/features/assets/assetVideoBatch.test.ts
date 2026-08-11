import { describe, expect, it } from "vitest";
import type { AssetView } from "../../types/asset";
import type { RecipeViewModel } from "../../types/generation";
import {
  buildH3BatchValues,
  canCreateH3Batch,
  h3AssetQualification,
  h3RecipeContract,
  isImageAssetForVideo,
  splitPromptBlocks,
} from "./assetVideoBatch";
import { validateResolution } from "../runtime/resolution";

const recipe: RecipeViewModel = {
  workflowId: "wfl_minimax_h3_reference_video",
  workflowVersionId: "wfv_h3",
  recipeId: "rcp_h3",
  name: "MiniMax H3",
  category: "video",
  mode: "reference",
  fields: [
    { key: "duration_seconds", type: "integer", label: "时长", required: true, default: 5, min: 1, max: 15, step: 1 },
    { key: "width", type: "integer", label: "宽度", required: true, default: 1344, min: 32, max: 2048, step: 32 },
    { key: "height", type: "integer", label: "高度", required: true, default: 768, min: 32, max: 2048, step: 32 },
    { key: "prompt", type: "textarea", label: "提示词", required: true, default: "" },
    { key: "reference_image", type: "image", label: "参考图片", required: true },
    { key: "seed", type: "seed", label: "种子", defaultMode: "random" },
  ],
  outputTypes: ["video"],
};

function asset(overrides: Partial<AssetView> = {}): AssetView {
  return {
    id: "ast-image",
    assetType: "image",
    category: "source_image",
    name: "image.png",
    originalName: "image.png",
    mimeType: "image/png",
    fileSize: 1,
    createdAt: "2026-08-10T00:00:00Z",
    isFavorite: false,
    tags: [],
    ...overrides,
  };
}

describe("独立视频批次输入", () => {
  it("only accepts image assets and splits pasted blocks on blank lines", () => {
    expect(isImageAssetForVideo(asset())).toBe(true);
    expect(isImageAssetForVideo(asset({ assetType: "video", category: "source_video" }))).toBe(false);
    expect(splitPromptBlocks(" first \n\n second\nline \n\n\n third ")).toEqual([
      "first",
      "second\nline",
      "third",
    ]);
    expect(splitPromptBlocks("one\n\ntwo\n\nthree\n\nfour\n\nfive")).toHaveLength(5);
    expect(splitPromptBlocks("  \n\n\t ")).toEqual([]);
  });

  it("uses the Recipe duration default and freezes each public duration selection", () => {
    const values = buildH3BatchValues(recipe, "ast-image", "  move slowly  ");
    expect(values.duration_seconds).toEqual({ type: "integer", value: 5 });
    expect(values.width).toEqual({ type: "integer", value: 1344 });
    expect(values.height).toEqual({ type: "integer", value: 768 });
    expect(values.prompt).toEqual({ type: "string", value: "move slowly" });
    expect(values.reference_image).toEqual({ type: "image_asset", assetId: "ast-image" });
    expect(values.seed).toEqual({ type: "seed_random" });
    expect(buildH3BatchValues(recipe, "ast-image", "one", 1).duration_seconds).toEqual({ type: "integer", value: 1 });
    expect(buildH3BatchValues(recipe, "ast-image", "three", 3).duration_seconds).toEqual({ type: "integer", value: 3 });
    expect(buildH3BatchValues(recipe, "ast-image", "five", 5).duration_seconds).toEqual({ type: "integer", value: 5 });
    expect(buildH3BatchValues(recipe, "ast-image", "ten", 10).duration_seconds).toEqual({ type: "integer", value: 10 });
    expect(buildH3BatchValues(recipe, "ast-image", "fifteen", 15).duration_seconds).toEqual({ type: "integer", value: 15 });
    expect(() => buildH3BatchValues(recipe, "ast-image", "invalid", 0)).toThrow("1–15");
    expect(() => buildH3BatchValues(recipe, "ast-image", "invalid", 16)).toThrow("1–15");
    expect(buildH3BatchValues(recipe, "ast-image", "portrait", 15, 1152, 2048)).toMatchObject({
      width: { type: "integer", value: 1152 },
      height: { type: "integer", value: 2048 },
      duration_seconds: { type: "integer", value: 15 },
    });
  });

  it("builds an independent H3 item from either source or generated image assets", () => {
    const source = asset({ id: "ast-source", category: "source_image" });
    const generated = asset({ id: "ast-generated", category: "generated_image" });
    expect(isImageAssetForVideo(source)).toBe(true);
    expect(isImageAssetForVideo(generated)).toBe(true);
    expect(buildH3BatchValues(recipe, source.id, "source motion").reference_image).toEqual({
      type: "image_asset",
      assetId: "ast-source",
    });
    expect(buildH3BatchValues(recipe, generated.id, "generated motion").reference_image).toEqual({
      type: "image_asset",
      assetId: "ast-generated",
    });
  });

  it("requires the exact H3 semantic keys and types", () => {
    expect(h3RecipeContract(recipe)).toMatchObject({ ok: true });
    for (const [key, type, message] of [
      ["prompt", "integer", "prompt"],
      ["reference_image", "textarea", "reference_image"],
      ["duration_seconds", "textarea", "duration_seconds"],
      ["width", "textarea", "width"],
      ["height", "textarea", "height"],
      ["seed", "textarea", "seed"],
    ] as const) {
      const invalid = {
        ...recipe,
        fields: recipe.fields.map((field) => field.key === key ? { ...field, type } : field),
      } as RecipeViewModel;
      const result = h3RecipeContract(invalid);
      expect(result.ok).toBe(false);
      if (!result.ok) expect(result.reason).toContain(message);
    }
  });

  it("rejects a Recipe range outside the public 1–15 second contract", () => {
    const invalid = {
      ...recipe,
      fields: recipe.fields.map((field) => field.key === "duration_seconds" ? { ...field, min: 0, max: 16 } : field),
    };
    const result = h3RecipeContract(invalid);
    expect(result).toEqual({ ok: false, reason: "H3 Recipe 的 duration_seconds 必须是 1–15 秒、步长 1 且包含默认值。" });
  });

  it("blocks asset qualification and creation eligibility for invalid custom resolution", () => {
    const invalid = validateResolution(recipe, 1270, 768);
    const invalidQualification = h3AssetQualification({
      isImage: true,
      promptReady: true,
      promptTooLong: false,
      h3RuntimeReady: true,
      comfyConnected: true,
      taskEventsReady: true,
      durationReady: true,
      resolutionReady: invalid.ok,
      resolutionError: invalid.errors.width,
    });
    expect(invalidQualification).not.toBe("符合条件");
    expect(invalidQualification).toContain("32");
    expect(canCreateH3Batch({
      runtimeReady: invalid.ok,
      admissionBusy: false,
      imageCount: 1,
      missingPromptCount: 0,
      oversizedPromptCount: 0,
    })).toBe(false);

    const valid = validateResolution(recipe, 1280, 768);
    expect(h3AssetQualification({
      isImage: true,
      promptReady: true,
      promptTooLong: false,
      h3RuntimeReady: true,
      comfyConnected: true,
      taskEventsReady: true,
      durationReady: true,
      resolutionReady: valid.ok,
      resolutionError: valid.errors.width,
    })).toBe("符合条件");
    expect(canCreateH3Batch({
      runtimeReady: valid.ok,
      admissionBusy: false,
      imageCount: 1,
      missingPromptCount: 0,
      oversizedPromptCount: 0,
    })).toBe(true);
  });
});
