import { describe, expect, it } from "vitest";
import type { AssetView } from "../../types/asset";
import type { RecipeViewModel } from "../../types/generation";
import { buildH3BatchValues, isImageAssetForVideo, splitPromptBlocks } from "./assetVideoBatch";

const recipe: RecipeViewModel = {
  workflowId: "wfl_minimax_h3_reference_video",
  workflowVersionId: "wfv_h3",
  recipeId: "rcp_h3",
  name: "MiniMax H3",
  category: "video",
  mode: "reference",
  fields: [
    { key: "duration_seconds", type: "integer", label: "时长", required: true, default: 5, min: 1, max: 5 },
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
  });

  it("freezes the H3 safe profile and binds one reference image", () => {
    const values = buildH3BatchValues(recipe, "ast-image", "  move slowly  ");
    expect(values.duration_seconds).toEqual({ type: "integer", value: 1 });
    expect(values.prompt).toEqual({ type: "string", value: "move slowly" });
    expect(values.reference_image).toEqual({ type: "image_asset", assetId: "ast-image" });
    expect(values.seed).toEqual({ type: "seed_random" });
  });
});
