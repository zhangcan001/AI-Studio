import { describe, expect, it } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import {
  addOrderedReference,
  ensurePrimaryReference,
  isRef2vaRecipe,
  moveOrderedReference,
  removeOrderedReference,
  referenceImagesField,
  toggleOrderedReference,
  validateRef2vaReferences,
} from "./ShotWorkspace";
import {
  MINIMAX_H3_FL2VA_I2V_QUALITY_WORKFLOW_ID,
  MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID,
} from "../runtime/productRuntimeScope";

function recipe(workflowId: string = MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID): RecipeViewModel {
  return {
    workflowId,
    workflowVersionId: `${workflowId}-version`,
    recipeId: `${workflowId}-recipe`,
    name: "H3",
    category: "video",
    mode: "reference_to_video",
    outputTypes: ["video"],
    fields: [{ key: "reference_images", type: "images", label: "参考图", required: false, minItems: 2, maxItems: 3 }],
  };
}

describe("Shot ordered REF2VA references", () => {
  it("puts the selected keyframe at @图片1 without changing I2V selection state", () => {
    const selectedImageAssetId = "ast-a";
    expect(ensurePrimaryReference(["ast-b", "ast-c"], selectedImageAssetId)).toEqual(["ast-a", "ast-b", "ast-c"]);
    expect(selectedImageAssetId).toBe("ast-a");
    expect(isRef2vaRecipe(recipe())).toBe(true);
    expect(isRef2vaRecipe(recipe(MINIMAX_H3_FL2VA_I2V_QUALITY_WORKFLOW_ID))).toBe(false);
  });

  it("supports ordered add, remove, and move while enforcing max items", () => {
    const refs = ["ast-b", "ast-a"];
    expect(addOrderedReference(refs, "ast-c", 3)).toEqual(["ast-b", "ast-a", "ast-c"]);
    expect(addOrderedReference(["ast-b", "ast-a", "ast-c"], "ast-d", 3)).toEqual(["ast-b", "ast-a", "ast-c"]);
    expect(moveOrderedReference(["ast-b", "ast-a", "ast-c"], 2, -1)).toEqual(["ast-b", "ast-c", "ast-a"]);
    expect(removeOrderedReference(["ast-b", "ast-c", "ast-a"], "ast-c")).toEqual(["ast-b", "ast-a"]);
    expect(toggleOrderedReference(["ast-b", "ast-a"], "ast-a")).toEqual(["ast-b"]);
  });

  it("uses the recipe image range for the Generate gate", () => {
    const field = referenceImagesField(recipe());
    expect(validateRef2vaReferences(field, ["ast-a"])).toBe("REF2VA 至少需要 2 张参考图");
    expect(validateRef2vaReferences(field, ["ast-a", "ast-b", "ast-c", "ast-d"])).toBe("REF2VA 最多允许 3 张参考图");
    expect(validateRef2vaReferences(field, ["ast-b", "ast-a"])).toBeUndefined();
    expect(validateRef2vaReferences({ ...field!, minItems: 0 }, [])).toBe("REF2VA 至少需要 2 张参考图");
    expect(validateRef2vaReferences(field, ["ast-a", "ast-a"])).toBe("REF2VA 参考图不能重复");
  });
});
