import { describe, expect, it } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import { assignAssetToField, compatibleAssetFields } from "./assetIntent";

const recipe: RecipeViewModel = {
  workflowId: "workflow",
  workflowVersionId: "version",
  recipeId: "recipe",
  name: "测试工作流",
  category: "image",
  mode: "image_to_video",
  fields: [
    { key: "reference", type: "image", label: "参考图", required: false },
    { key: "references", type: "images", label: "参考图组", required: false, minItems: 0, maxItems: 2 },
    { key: "prompt", type: "textarea", label: "提示词", required: true, default: "" },
  ],
};

describe("素材快速加入创作", () => {
  it("单个兼容输入会自动填入", () => {
    const target = { key: "reference", type: "image", label: "参考图", required: false } as const;
    expect(assignAssetToField(target, {}, "asset-a")).toEqual({
      kind: "applied",
      values: { reference: { type: "image_asset", assetId: "asset-a" } },
    });
  });

  it("多个兼容输入不会猜测目标", () => {
    expect(compatibleAssetFields(recipe, "image").map((field) => field.key)).toEqual([
      "reference",
      "references",
    ]);
  });

  it("多选输入追加并保持顺序，达到上限后阻止", () => {
    const target = recipe.fields[1];
    expect(assignAssetToField(target, { references: { type: "image_assets", assetIds: ["a"] } }, "b")).toEqual({
      kind: "applied",
      values: { references: { type: "image_assets", assetIds: ["a", "b"] } },
    });
    expect(assignAssetToField(target, { references: { type: "image_assets", assetIds: ["a", "b"] } }, "c")).toEqual({ kind: "max_items" });
  });

  it("单选已有素材必须确认替换", () => {
    const target = recipe.fields[0];
    const values = { reference: { type: "image_asset", assetId: "old" } } as const;
    expect(assignAssetToField(target, values, "new")).toEqual({ kind: "requires_confirmation" });
    expect(assignAssetToField(target, values, "new", true)).toEqual({
      kind: "applied",
      values: { reference: { type: "image_asset", assetId: "new" } },
    });
  });
});
