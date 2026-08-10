import { describe, expect, it } from "vitest";
import type { GenerationValues, RecipeViewModel } from "../../types/generation";
import {
  buildExperimentPlan,
  freezeSeedVariants,
  freezeRandomSeeds,
  removeExperimentPlanItem,
  snapshotDiff,
} from "./experimentPlanner";

const recipe: RecipeViewModel = {
  workflowId: "workflow-image",
  workflowVersionId: "version-image",
  recipeId: "recipe-image",
  name: "通用图片实验",
  category: "image",
  mode: "text_to_image",
  fields: [
    { key: "prompt", type: "textarea", label: "提示词", required: true, default: "基础提示" },
    { key: "steps", type: "integer", label: "步数", required: true, min: 1, max: 20, default: 8 },
    { key: "seed", type: "seed", label: "Seed", defaultMode: "random", minValue: "10", maxValue: "1000" },
    { key: "reference", type: "image", label: "参考图", required: true },
  ],
};

const baseValues: GenerationValues = {
  prompt: { type: "string", value: "基础提示" },
  steps: { type: "integer", value: 8 },
  seed: { type: "seed_random" },
  reference: { type: "image_asset", assetId: "asset-1" },
};

describe("experiment planner", () => {
  it("builds a two-dimension cartesian plan with frozen explicit seeds", () => {
    const result = buildExperimentPlan({
      recipe,
      baseValues,
      dimensions: [
        { fieldKey: "prompt", values: [{ type: "string", value: "A" }, { type: "string", value: "B" }] },
        { fieldKey: "seed", values: [{ type: "seed_fixed", value: "42" }, { type: "seed_fixed", value: "43" }] },
      ],
      seedSource: () => "44",
      now: "2026-08-10T00:00:00.000Z",
    });
    expect(result.issues).toEqual([]);
    expect(result.plan?.items).toHaveLength(4);
    expect(result.plan?.items[0].values.reference).toEqual({ type: "image_asset", assetId: "asset-1" });
    expect(result.plan?.items.every((item) => item.seed === "42" || item.seed === "43")).toBe(true);
    expect(result.plan?.baseValues.seed).toEqual({ type: "seed_fixed", value: "44" });
  });

  it("rejects media fields, invalid ranges, and more than 24 items", () => {
    const tooMany = buildExperimentPlan({
      recipe,
      baseValues: { ...baseValues, seed: { type: "seed_fixed", value: "20" } },
      dimensions: [
        { fieldKey: "prompt", values: Array.from({ length: 8 }, (_, index) => ({ type: "string", value: String(index) })) },
        { fieldKey: "steps", values: Array.from({ length: 4 }, (_, index) => ({ type: "integer", value: index + 1 })) },
      ],
    });
    expect(tooMany.issues).toContain("本次实验将生成 32 个任务，限制为 1–24 个。");
    const invalidMedia = buildExperimentPlan({
      recipe,
      baseValues,
      dimensions: [{ fieldKey: "reference", values: [{ type: "image_asset", assetId: "asset-2" }] as never }],
    });
    expect(invalidMedia.issues).toContain("字段“reference”不支持组合实验。");
  });

  it("freezes random seeds within the recipe-owned range and rejects an invalid range", () => {
    const frozen = freezeRandomSeeds(recipe, baseValues, () => "999");
    expect(frozen.issues).toEqual([]);
    expect(frozen.values.seed).toEqual({ type: "seed_fixed", value: "999" });
    const invalid = freezeSeedVariants({ key: "seed", minValue: "100", maxValue: "10" }, 1, () => "50");
    expect(invalid.issues).toEqual(["当前 Recipe 没有可用的 Seed 合法范围。"]);
  });

  it("removes a frozen item without changing the remaining values", () => {
    const result = buildExperimentPlan({
      recipe,
      baseValues: { ...baseValues, seed: { type: "seed_fixed", value: "20" } },
      dimensions: [{ fieldKey: "steps", values: [{ type: "integer", value: 4 }, { type: "integer", value: 8 }] }],
    });
    expect(result.plan).toBeDefined();
    const next = removeExperimentPlanItem(result.plan!, result.plan!.items[0].id);
    expect(next.items).toHaveLength(1);
    expect(next.items[0].ordinal).toBe(0);
    expect(next.items[0].values.steps).toEqual({ type: "integer", value: 8 });
  });

  it("returns a redacted generic snapshot diff", () => {
    expect(snapshotDiff(
      { prompt: { type: "string", value: "A" }, reference: { type: "image_asset", assetId: "private-path-like-id" } },
      { prompt: { type: "string", value: "B" }, reference: { type: "image_asset", assetId: "other-id" } },
      { prompt: "提示词", reference: "参考素材" },
    )).toEqual([
      { fieldKey: "提示词", before: "A", after: "B" },
      { fieldKey: "参考素材", before: "素材已绑定", after: "素材已绑定" },
    ]);
  });
});
