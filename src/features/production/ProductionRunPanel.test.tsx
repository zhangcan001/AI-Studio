import { describe, expect, it } from "vitest";
import type { ProductionRunStage } from "../../types/productionRun";
import type { RecipeViewModel } from "../../types/generation";
import {
  h3ReferenceImageMax,
  moveProductionRunAsset,
  productionRunSelectionBounds,
  productionRunSelectionError,
  productionRunSelectionIds,
} from "./ProductionRunPanel";

const ref2vaRecipe: RecipeViewModel = {
  workflowId: "wfl_minimax_h3_reference_video",
  workflowVersionId: "workflow-version-ref2va",
  recipeId: "recipe-ref2va",
  name: "REF2VA",
  category: "video",
  mode: "REF2VA_IMAGE",
  outputTypes: ["video"],
  fields: [{
    key: "reference_images",
    type: "images",
    label: "Reference images",
    required: false,
    minItems: 0,
    maxItems: 9,
  }],
};

describe("ProductionRun REF2VA selection contract", () => {
  it("requires two references for REF2VA while keeping I2V single-image", () => {
    expect(h3ReferenceImageMax(ref2vaRecipe)).toBe(9);
    expect(productionRunSelectionBounds("REF2VA", ref2vaRecipe)).toEqual({ min: 2, max: 9 });
    expect(productionRunSelectionBounds("I2V", ref2vaRecipe)).toEqual({ min: 1, max: 1 });
    expect(productionRunSelectionError("REF2VA", 1, 9)).toContain("至少需要选择 2 张");
    expect(productionRunSelectionError("I2V", 2, 1)).toContain("需要选择 1 张");
    expect(productionRunSelectionError("REF2VA", 3, 9)).toBeUndefined();
  });

  it("keeps explicit user order for move controls", () => {
    expect(moveProductionRunAsset(["A", "B", "C"], "B", -1)).toEqual(["B", "A", "C"]);
    expect(moveProductionRunAsset(["B", "A", "C"], "B", 1)).toEqual(["A", "B", "C"]);
    expect(moveProductionRunAsset(["A", "B", "C"], "A", -1)).toEqual(["A", "B", "C"]);
  });

  it("rejects duplicate persisted references instead of silently reordering them", () => {
    expect(productionRunSelectionError("REF2VA", 3, 9, 0, ["A", "A", "B"]))
      .toContain("不能重复");
  });

  it("hydrates persisted selection by reference_index rather than asset id", () => {
    const stage: ProductionRunStage = {
      id: "selection-stage",
      ordinal: 1,
      stageType: "ASSET_SELECTION",
      status: "SUCCEEDED",
      frozenConfig: {},
      items: [
        {
          id: "selection-a",
          stageId: "selection-stage",
          ordinal: 1,
          status: "SUCCEEDED",
          assetId: "A",
          sourceAssetId: "A",
          referenceIndex: 1,
          attempt: 1,
          frozenValues: {},
        },
        {
          id: "selection-b",
          stageId: "selection-stage",
          ordinal: 0,
          status: "SUCCEEDED",
          assetId: "B",
          sourceAssetId: "B",
          referenceIndex: 0,
          attempt: 1,
          frozenValues: {},
        },
        {
          id: "selection-c",
          stageId: "selection-stage",
          ordinal: 2,
          status: "SUCCEEDED",
          assetId: "C",
          sourceAssetId: "C",
          referenceIndex: 2,
          attempt: 1,
          frozenValues: {},
        },
      ],
    };
    expect(productionRunSelectionIds(stage)).toEqual(["B", "A", "C"]);
  });
});
