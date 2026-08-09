import { describe, expect, it } from "vitest";
import type { GenerationValues } from "../../types/generation";
import { cloneGenerationValues, retainFailedBatchItems, type BatchDraftItem } from "./batchDraft";

describe("batch draft helpers", () => {
  it("freezes nested generation values independently from the current form", () => {
    const values: GenerationValues = {
      prompt: { type: "string", value: "first prompt" },
      references: { type: "image_assets", assetIds: ["ast_a", "ast_b"] },
    };

    const frozen = cloneGenerationValues(values);
    const prompt = values.prompt;
    const references = values.references;
    if (prompt.type !== "string" || references.type !== "image_assets") {
      throw new Error("unexpected test fixture type");
    }
    prompt.value = "changed prompt";
    references.assetIds.push("ast_c");

    expect(frozen).toEqual({
      prompt: { type: "string", value: "first prompt" },
      references: { type: "image_assets", assetIds: ["ast_a", "ast_b"] },
    });
  });

  it("keeps only failed items after a partial batch response", () => {
    const makeItem = (id: string): BatchDraftItem => ({
      id,
      workflowName: id,
      workflowVersionId: `wfv_${id}`,
      recipeId: `rcp_${id}`,
      values: {},
    });
    const items = [makeItem("a"), makeItem("b"), makeItem("c")];

    expect(retainFailedBatchItems(items, [1])).toEqual([items[1]]);
    expect(retainFailedBatchItems(items, [])).toEqual([]);
  });
});
