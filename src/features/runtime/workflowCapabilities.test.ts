import { describe, expect, it } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import {
  MINIMAX_H3_FL2VA_I2V_QUALITY_WORKFLOW_ID,
  MINIMAX_H3_FL2VA_T2V_QUALITY_WORKFLOW_ID,
} from "./productRuntimeScope";
import {
  filterImageRecipes,
  imageRecipeCapability,
  migrateGenerationValues,
  resolveProjectFolderRecipes,
  resolveVideoRecipe,
  videoRecipeCapability,
} from "./workflowCapabilities";

function recipe(overrides: Partial<RecipeViewModel>): RecipeViewModel {
  return {
    workflowId: "custom-workflow",
    workflowVersionId: "version-1",
    recipeId: "recipe-1",
    name: "Custom workflow",
    category: "custom",
    mode: "custom",
    fields: [{ key: "prompt", type: "textarea", label: "Prompt", required: true, default: "" }],
    outputTypes: ["image"],
    ...overrides,
  };
}

function h3Recipe(overrides: Partial<RecipeViewModel>): RecipeViewModel {
  return recipe({
    workflowId: MINIMAX_H3_FL2VA_T2V_QUALITY_WORKFLOW_ID,
    outputTypes: ["video"],
    fields: [
      { key: "prompt", type: "textarea", label: "Prompt", required: true, default: "" },
      { key: "width", type: "integer", label: "Width", required: true, default: 1376 },
      { key: "height", type: "integer", label: "Height", required: true, default: 768 },
      { key: "duration_seconds", type: "integer", label: "Duration", required: true, default: 5, min: 1, max: 15, step: 1 },
      { key: "seed", type: "seed", label: "Seed", defaultMode: "random" },
    ],
    ...overrides,
  });
}

describe("workflow capability resolution", () => {
  it("filters image output by outputTypes and keeps promptless images generic-only", () => {
    const promptless = recipe({ recipeId: "promptless", fields: [], outputTypes: ["image"] });
    const video = recipe({ recipeId: "video", outputTypes: ["video"] });

    expect(filterImageRecipes([promptless, video])).toEqual([promptless]);
    expect(imageRecipeCapability(promptless)).toMatchObject({
      outputImage: true,
      batchPromptCompatible: false,
      genericGenerationCompatible: true,
    });
  });

  it("derives video mode compatibility from semantic field keys", () => {
    const i2v = recipe({
      outputTypes: ["video"],
      fields: [
        { key: "prompt", type: "textarea", label: "Prompt", required: true, default: "" },
        { key: "first_frame", type: "image", label: "首帧", required: true },
      ],
    });
    const capability = videoRecipeCapability(i2v);
    expect(capability.supportedModes).toContain("FL2VA_IMAGE_TO_VIDEO");
    expect(capability.supportedModes).toContain("CUSTOM_VIDEO");
    expect(capability.supportedModes).not.toContain("FL2VA_FIRST_LAST");
    expect(capability.projectFolderModes).toEqual([]);
  });

  it("prefers a compatible manual selection and falls back to recommendation when stale", () => {
    const recommended = recipe({ recipeId: "recommended", name: "Recommended", outputTypes: ["video"] });
    const manual = recipe({ recipeId: "manual", name: "Manual", outputTypes: ["video"] });
    const resolved = resolveVideoRecipe(
      [recommended, manual],
      "FL2VA_TEXT_TO_VIDEO",
      { workflowVersionId: "version-1", recipeId: "manual" },
      recommended,
    );
    expect(resolved.recipe?.recipeId).toBe("manual");
    expect(resolved.source).toBe("manual");

    const stale = resolveVideoRecipe(
      [recommended],
      "FL2VA_TEXT_TO_VIDEO",
      { workflowVersionId: "missing", recipeId: "missing" },
      recommended,
    );
    expect(stale.recipe?.recipeId).toBe("recommended");
    expect(stale.source).toBe("recommended");
    expect(stale.staleManualSelection).toBe(true);
  });

  it("resolves project-folder workflows per mode without changing automatic recommendations", () => {
    const t2v = h3Recipe({ recipeId: "t2v" });
    const i2v = h3Recipe({
      workflowId: MINIMAX_H3_FL2VA_I2V_QUALITY_WORKFLOW_ID,
      recipeId: "i2v",
      fields: [
        { key: "prompt", type: "textarea", label: "Prompt", required: true, default: "" },
        { key: "first_frame", type: "image", label: "首帧", required: true },
        { key: "width", type: "integer", label: "Width", required: true, default: 1376 },
        { key: "height", type: "integer", label: "Height", required: true, default: 768 },
        { key: "duration_seconds", type: "integer", label: "Duration", required: true, default: 5, min: 1, max: 15, step: 1 },
        { key: "seed", type: "seed", label: "Seed", defaultMode: "random" },
      ],
    });
    const resolved = resolveProjectFolderRecipes(
      [t2v, i2v],
      ["FL2VA_TEXT_TO_VIDEO", "FL2VA_IMAGE_TO_VIDEO"],
      {
        FL2VA_TEXT_TO_VIDEO: { workflowVersionId: "version-1", recipeId: "t2v" },
        FL2VA_IMAGE_TO_VIDEO: { workflowVersionId: "version-1", recipeId: "i2v" },
      },
      { FL2VA_TEXT_TO_VIDEO: { workflowVersionId: "version-1", recipeId: "i2v" } },
    );
    expect(resolved.map((item) => [item.mode, item.recipe?.recipeId])).toEqual([
      ["FL2VA_TEXT_TO_VIDEO", "t2v"],
      ["FL2VA_IMAGE_TO_VIDEO", "i2v"],
    ]);
  });

  it("migrates only same-key, same-type draft values when switching workflows", () => {
    const previous = recipe({
      fields: [
        { key: "prompt", type: "textarea", label: "Prompt", required: true, default: "" },
        { key: "reference_image", type: "image", label: "参考图", required: false },
      ],
    });
    const next = recipe({
      fields: [
        { key: "prompt", type: "textarea", label: "Prompt", required: true, default: "new" },
        { key: "reference_image", type: "images", label: "参考图", required: false, minItems: 0, maxItems: 2 },
      ],
    });
    const values = migrateGenerationValues(previous, next, {
      prompt: { type: "string", value: "keep me" },
      reference_image: { type: "image_asset", assetId: "ast_old" },
    });
    expect(values.prompt).toEqual({ type: "string", value: "keep me" });
    expect(values.reference_image).toEqual({ type: "image_assets", assetIds: [] });
  });
});
