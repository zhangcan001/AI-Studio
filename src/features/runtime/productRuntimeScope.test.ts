import { describe, expect, it } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import {
  filterProductionRuntimeCatalog,
  isProductionRuntimeForStage,
  kera2PromptField,
  productionRuntimeForStage,
  productionRuntimeForWorkflowId,
  PRODUCTION_WORKFLOW_IDS,
  KERA2_WORKFLOW_ID,
  MINIMAX_H3_FL2VA_WORKFLOW_ID,
  MINIMAX_H3_WORKFLOW_ID,
} from "./productRuntimeScope";

function recipe(workflowId: string, name: string): RecipeViewModel {
  return {
    workflowId,
    workflowVersionId: `${workflowId}:version`,
    recipeId: `${workflowId}:recipe`,
    name,
    category: "image",
    mode: "text_to_image",
    fields: [],
  };
}

describe("0.3.0 product runtime scope", () => {
  it("accepts only the two exact workflow IDs", () => {
    expect(PRODUCTION_WORKFLOW_IDS).toEqual([KERA2_WORKFLOW_ID, MINIMAX_H3_WORKFLOW_ID, MINIMAX_H3_FL2VA_WORKFLOW_ID]);
    expect(productionRuntimeForWorkflowId(KERA2_WORKFLOW_ID)).toBe("kera2Image");
    expect(productionRuntimeForWorkflowId(MINIMAX_H3_WORKFLOW_ID)).toBe("minimaxH3Video");
    expect(productionRuntimeForWorkflowId(MINIMAX_H3_FL2VA_WORKFLOW_ID)).toBe("minimaxH3Video");
    expect(productionRuntimeForWorkflowId("wfl_other")).toBeUndefined();
  });

  it("does not trust a fake ID's Kera2 or H3 display name", () => {
    expect(productionRuntimeForWorkflowId("wfl_other")).toBeUndefined();
    expect(productionRuntimeForWorkflowId("wfl_fake")).toBeUndefined();
    expect(filterProductionRuntimeCatalog([
      recipe("wfl_other", "Kera2 Test Fake"),
      recipe("wfl_fake", "MiniMax H3 Reference Video Clone"),
      recipe(KERA2_WORKFLOW_ID, "Kera2 renamed"),
      recipe(MINIMAX_H3_WORKFLOW_ID, "H3 renamed"),
      recipe(MINIMAX_H3_FL2VA_WORKFLOW_ID, "H3 FL2VA"),
    ]).map((item) => item.workflowId)).toEqual([KERA2_WORKFLOW_ID, MINIMAX_H3_WORKFLOW_ID, MINIMAX_H3_FL2VA_WORKFLOW_ID]);
  });

  it("matches each runtime only to its frozen production stage", () => {
    expect(productionRuntimeForStage("image", KERA2_WORKFLOW_ID)).toBe("kera2Image");
    expect(productionRuntimeForStage("video", MINIMAX_H3_WORKFLOW_ID)).toBe("minimaxH3Video");
    expect(productionRuntimeForStage("video", MINIMAX_H3_FL2VA_WORKFLOW_ID)).toBe("minimaxH3Video");
    expect(isProductionRuntimeForStage("video", KERA2_WORKFLOW_ID)).toBe(false);
    expect(isProductionRuntimeForStage("image", MINIMAX_H3_WORKFLOW_ID)).toBe(false);
  });

  it("requires Krea2's exact prompt textarea key instead of the first textarea", () => {
    const valid = recipe(KERA2_WORKFLOW_ID, "Krea2");
    valid.fields = [
      { key: "negative_prompt", type: "textarea", label: "Negative", required: false, default: "" },
      { key: "prompt", type: "textarea", label: "Prompt", required: true, default: "" },
    ];
    expect(kera2PromptField(valid)?.key).toBe("prompt");
    const missing = {
      ...valid,
      fields: [{ key: "text", type: "textarea" as const, label: "Text", required: true, default: "" }],
    };
    expect(kera2PromptField(missing)).toBeUndefined();
  });
});
