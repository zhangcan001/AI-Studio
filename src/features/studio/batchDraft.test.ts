import { describe, expect, it } from "vitest";
import type { GenerationValues } from "../../types/generation";
import {
  cloneGenerationValues,
  copyBatchDraftItem,
  moveBatchDraftItem,
  removeBatchDraftItem,
  retainFailedBatchItems,
  type BatchDraftItem,
} from "./batchDraft";

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

  it("supports copy, remove, and reorder while preserving shared parameters", () => {
    const makeItem = (id: string, prompt: string): BatchDraftItem => ({
      id,
      workflowName: "Krea2",
      workflowVersionId: "wfv_krea2",
      recipeId: "rcp_krea2",
      values: {
        prompt: { type: "string", value: prompt },
        steps: { type: "integer", value: 4 },
        seed: { type: "seed_random" },
      },
    });
    const items = [makeItem("one", "one"), makeItem("two", "two"), makeItem("three", "three")];
    const copied = copyBatchDraftItem(items, "one", "one-copy");
    expect(copied.map((item) => item.id)).toEqual(["one", "one-copy", "two", "three"]);
    expect(copied[1].values).toEqual(copied[0].values);
    expect(moveBatchDraftItem(copied, "three", -1).map((item) => item.id)).toEqual(["one", "one-copy", "three", "two"]);
    expect(removeBatchDraftItem(copied, "one-copy").map((item) => item.id)).toEqual(["one", "two", "three"]);
  });
});
