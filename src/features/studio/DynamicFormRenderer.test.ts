import { describe, expect, it } from "vitest";
import { validateRecipeValues } from "./DynamicFormRenderer";
import type { RecipeViewModel } from "../../types/generation";

const recipe: RecipeViewModel = {
  workflowId: "wfl_seed",
  workflowVersionId: "wfv_seed",
  recipeId: "rcp_seed",
  name: "Seed Recipe",
  category: "image",
  mode: "text_to_image",
  fields: [
    {
      key: "seed",
      type: "seed",
      label: "Seed",
      defaultMode: "random",
      defaultValue: null,
      minValue: "10",
      maxValue: "20",
    },
  ],
};

describe("validateRecipeValues seed ranges", () => {
  it("accepts fixed values at both recipe boundaries", () => {
    expect(
      validateRecipeValues(recipe, { seed: { type: "seed_fixed", value: "10" } }),
    ).toEqual({});
    expect(
      validateRecipeValues(recipe, { seed: { type: "seed_fixed", value: "20" } }),
    ).toEqual({});
  });

  it("rejects fixed values outside the recipe range", () => {
    const errors = validateRecipeValues(recipe, {
      seed: { type: "seed_fixed", value: "21" },
    });

    expect(errors.seed).toContain("between 10 and 20");
  });

  it("uses BigInt for the full u64 range when Recipe has no bounds", () => {
    const unbounded = {
      ...recipe,
      fields: [
        {
          key: "seed",
          type: "seed" as const,
          label: "Seed",
          defaultMode: "random" as const,
          defaultValue: null,
          minValue: null,
          maxValue: null,
        },
      ],
    } satisfies RecipeViewModel;

    expect(
      validateRecipeValues(unbounded, {
        seed: { type: "seed_fixed", value: "18446744073709551615" },
      }),
    ).toEqual({});
  });
});
