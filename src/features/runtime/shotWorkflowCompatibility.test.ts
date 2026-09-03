import { describe, expect, it } from "vitest";
import type { RecipeField, RecipeViewModel } from "../../types/generation";
import {
  resolveShotStageRecipe,
  shotStageRecipeCompatibility,
  validateShotReferenceImages,
} from "./shotWorkflowCompatibility";

function recipe(
  id: string,
  outputTypes: RecipeViewModel["outputTypes"],
  fields: RecipeField[] = [],
): RecipeViewModel {
  return {
    workflowId: `custom-${id}`,
    workflowVersionId: `version-${id}`,
    recipeId: `recipe-${id}`,
    name: id,
    category: "test",
    mode: "test",
    fields,
    outputTypes,
  };
}

const prompt: RecipeField = { key: "prompt", type: "textarea", label: "Prompt", required: true, default: "" };

describe("formal Shot workflow compatibility", () => {
  it("accepts custom image output without an internal workflow ID", () => {
    expect(shotStageRecipeCompatibility(recipe("image", ["image"], [prompt]), "image")).toEqual({
      compatible: true,
      stage: "image",
    });
  });

  it("classifies video text, single-image, and plural-reference recipes", () => {
    expect(shotStageRecipeCompatibility(recipe("text", ["video"], [prompt]), "video").videoInputMode).toBe("TEXT_ONLY");
    expect(shotStageRecipeCompatibility(recipe("single", ["video"], [prompt, { key: "image", type: "image", label: "Image", required: true }]), "video").videoInputMode).toBe("SINGLE_IMAGE");
    expect(shotStageRecipeCompatibility(recipe("plural", ["video"], [prompt, { key: "images", type: "images", label: "Images", required: true, minItems: 1, maxItems: 4 }]), "video").videoInputMode).toBe("REFERENCE_IMAGES");
  });

  it("fails closed for unsupported media and multiple image roles", () => {
    const audio = recipe("audio", ["video"], [prompt, { key: "audio", type: "audio", label: "Audio", required: true }]);
    const firstLast = recipe("first-last", ["video"], [
      prompt,
      { key: "first", type: "image", label: "First", required: true },
      { key: "last", type: "image", label: "Last", required: true },
    ]);
    expect(shotStageRecipeCompatibility(audio, "video")).toMatchObject({ compatible: false, videoInputMode: "UNSUPPORTED" });
    expect(shotStageRecipeCompatibility(audio, "video").reason).toContain("SHOT_WORKFLOW_UNSUPPORTED_MEDIA_INPUT");
    expect(shotStageRecipeCompatibility(firstLast, "video")).toMatchObject({ compatible: false, videoInputMode: "UNSUPPORTED" });
  });

  it("validates a custom plural reference bound with its own min/max", () => {
    const field = { key: "images", type: "images" as const, label: "Images", required: true, minItems: 1, maxItems: 4 };
    expect(validateShotReferenceImages(field, [])).toBe("参考图至少需要 1 张。");
    expect(validateShotReferenceImages(field, ["a", "a"])).toBe("参考图不能重复。");
    expect(validateShotReferenceImages(field, ["a", "b", "c", "d", "e"])).toBe("参考图最多允许 4 张。");
    expect(validateShotReferenceImages(field, ["a"])).toBeUndefined();
  });
});

describe("strict Shot workflow resolution", () => {
  const imageA = recipe("image-a", ["image"], [prompt]);
  const imageB = recipe("image-b", ["image"], [prompt]);

  const projectDefault = {
    workflowVersionId: imageB.workflowVersionId,
    recipeId: imageB.recipeId,
    available: true,
  };

  it("uses a project default only when its exact pair is compatible", () => {
    expect(resolveShotStageRecipe([imageA, imageB], "image", undefined, projectDefault, imageA)).toMatchObject({
      recipe: imageB,
      source: "project_default",
      blocked: false,
    });
  });

  it("blocks unavailable, missing, and formally incompatible project bindings without legacy fallback", () => {
    expect(resolveShotStageRecipe([imageA, imageB], "image", undefined, { ...projectDefault, available: false }, imageA).blocked).toBe(true);
    expect(resolveShotStageRecipe([imageA], "image", undefined, projectDefault, imageA)).toMatchObject({ blocked: true, source: "project_default" });
    expect(resolveShotStageRecipe([recipe("video", ["video"], [prompt])], "image", undefined, {
      workflowVersionId: "version-video",
      recipeId: "recipe-video",
      available: true,
    }, imageA)).toMatchObject({ blocked: true, source: "project_default" });
  });

  it("uses legacy fallback only when the stage has no project binding, while Shot config wins", () => {
    expect(resolveShotStageRecipe([imageA], "image", undefined, undefined, imageA)).toMatchObject({ source: "legacy", recipe: imageA, blocked: false });
    expect(resolveShotStageRecipe([imageA, imageB], "image", {
      workflowVersionId: imageA.workflowVersionId,
      recipeId: imageA.recipeId,
    }, projectDefault, imageB)).toMatchObject({ source: "stage_config", recipe: imageA, blocked: false });
  });
});
