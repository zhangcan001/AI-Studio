import { describe, expect, it } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import {
  batchPromptField,
  materializeWorkflowExecutionBatch,
  parseBatchPrompts,
} from "./workflowExecutionBatch";

const recipe: RecipeViewModel = {
  workflowId: "wfl_test",
  workflowVersionId: "wfv_test",
  recipeId: "rcp_test",
  name: "Batch test",
  category: "image",
  mode: "text_to_image",
  fields: [
    { key: "prompt", type: "textarea", label: "Prompt", required: true, default: "" },
    { key: "seed", type: "seed", label: "Seed", defaultMode: "fixed", defaultValue: "42" },
  ],
};

describe("workflow execution batch materialization", () => {
  it("creates the prompt by seed cartesian product with deterministic fixed seeds", () => {
    const batch = materializeWorkflowExecutionBatch(
      recipe,
      { prompt: { type: "string", value: "base" }, seed: { type: "seed_fixed", value: "40" } },
      "first\nsecond",
      2,
      [],
    );

    expect(batch.expectedCount).toBe(4);
    expect(batch.errors).toEqual([]);
    expect(batch.items.map(({ values }) => values.prompt)).toEqual([
      { type: "string", value: "first" },
      { type: "string", value: "first" },
      { type: "string", value: "second" },
      { type: "string", value: "second" },
    ]);
    expect(batch.items.map(({ values }) => values.seed)).toEqual([
      { type: "seed_fixed", value: "40" },
      { type: "seed_fixed", value: "41" },
      { type: "seed_fixed", value: "40" },
      { type: "seed_fixed", value: "41" },
    ]);
  });

  it("leaves random seeds for the repository to freeze when batch items are persisted", () => {
    const batch = materializeWorkflowExecutionBatch(
      recipe,
      { prompt: { type: "string", value: "base" }, seed: { type: "seed_random" } },
      "",
      3,
      [],
    );
    expect(batch.items.map(({ values }) => values.seed)).toEqual([
      { type: "seed_random" },
      { type: "seed_random" },
      { type: "seed_random" },
    ]);
  });

  it("keeps image lists to one prompt and seed instead of making an implicit cartesian product", () => {
    const imageRecipe: RecipeViewModel = {
      ...recipe,
      fields: [
        ...recipe.fields,
        { key: "image", type: "image", label: "Image", required: true },
      ],
    };
    const batch = materializeWorkflowExecutionBatch(
      imageRecipe,
      { prompt: { type: "string", value: "base" }, seed: { type: "seed_random" } },
      "one\ntwo",
      1,
      ["ast_a", "ast_b"],
    );
    expect(batch.expectedCount).toBe(4);
    expect(batch.items).toHaveLength(0);
    expect(batch.errors).toContain("IMAGE_BATCH_REQUIRES_ONE_PROMPT_AND_ONE_SEED");
  });

  it("guards expansion before materializing more than 100 items", () => {
    const batch = materializeWorkflowExecutionBatch(
      recipe,
      { prompt: { type: "string", value: "base" }, seed: { type: "seed_random" } },
      Array.from({ length: 11 }, (_, index) => `prompt ${index}`).join("\n"),
      10,
      [],
    );
    expect(batch.expectedCount).toBe(110);
    expect(batch.items).toEqual([]);
    expect(batch.errors).toContain("BATCH_TOO_LARGE");
  });

  it("parses non-empty prompt lines in order", () => {
    expect(parseBatchPrompts(" first \n\n second\n  ")).toEqual(["first", "second"]);
  });

  it("W-06: batch prompt lines never overwrite the negative prompt", () => {
    const negativeFirst: RecipeViewModel = {
      ...recipe,
      fields: [
        { key: "negative_prompt", type: "textarea", label: "Negative", required: false, default: "blurry" },
        { key: "system_prompt", type: "textarea", label: "System", required: false, default: "sys" },
        ...recipe.fields,
      ],
    };
    expect(batchPromptField(negativeFirst)?.key).toBe("prompt");
    const batch = materializeWorkflowExecutionBatch(
      negativeFirst,
      {
        negative_prompt: { type: "string", value: "blurry" },
        prompt: { type: "string", value: "base" },
        seed: { type: "seed_fixed", value: "1" },
      },
      "first\nsecond",
      1,
      [],
    );
    expect(batch.errors).toEqual([]);
    expect(batch.items.map(({ values }) => values.prompt)).toEqual([
      { type: "string", value: "first" },
      { type: "string", value: "second" },
    ]);
    expect(batch.items.every(({ values }) => (
      values.negative_prompt?.type === "string" && values.negative_prompt.value === "blurry"
    ))).toBe(true);
  });

  it("W-06: a recipe with only a negative prompt has no batch prompt field", () => {
    const onlyNegative: RecipeViewModel = {
      ...recipe,
      fields: [{ key: "negative_prompt", type: "textarea", label: "Negative", required: false, default: "" }],
    };
    expect(batchPromptField(onlyNegative)).toBeUndefined();
  });
});
