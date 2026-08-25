import { describe, expect, it } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import {
  addOrderedReference,
  buildShotContextPath,
  ensurePrimaryReference,
  isRef2vaRecipe,
  moveOrderedReference,
  removeOrderedReference,
  referenceImagesField,
  shotContextSurface,
  toggleOrderedReference,
  validateRef2vaReferences,
} from "./ShotWorkspace";
import {
  MINIMAX_H3_FL2VA_I2V_QUALITY_WORKFLOW_ID,
  MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID,
} from "../runtime/productRuntimeScope";
import type { ProductionStructureTree } from "../../types/productionStructure";
import type { WorkspaceSelection } from "../../types/workspaceSelection";

const structure: ProductionStructureTree = {
  projectId: "project-1",
  series: [{
    id: "series-1",
    projectId: "project-1",
    ordinal: 0,
    name: "第一季",
    description: "",
    createdAt: "",
    updatedAt: "",
    episodes: [{
      id: "episode-1",
      seriesId: "series-1",
      ordinal: 0,
      name: "第一集",
      description: "",
      createdAt: "",
      updatedAt: "",
      scenes: [{
        id: "scene-1",
        episodeId: "episode-1",
        ordinal: 0,
        name: "入口",
        description: "",
        shotIds: ["shot-1"],
        createdAt: "",
        updatedAt: "",
      }],
    }],
  }],
  unassignedShotIds: [],
};

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

describe("Shot workspace context path", () => {
  it("derives project hierarchy and selected shot breadcrumbs", () => {
    const selections: WorkspaceSelection[] = [
      { type: "project", projectId: "project-1" },
      { type: "series", seriesId: "series-1" },
      { type: "episode", episodeId: "episode-1" },
      { type: "scene", sceneId: "scene-1" },
      { type: "shot", shotId: "shot-1" },
    ];

    expect(buildShotContextPath(structure, selections[0])).toEqual([]);
    expect(buildShotContextPath(structure, selections[1]).map((item) => item.id)).toEqual(["series-1"]);
    expect(buildShotContextPath(structure, selections[2]).map((item) => item.id)).toEqual(["series-1", "episode-1"]);
    expect(buildShotContextPath(structure, selections[3]).map((item) => item.id)).toEqual(["series-1", "episode-1", "scene-1"]);
    expect(buildShotContextPath(structure, selections[4]).map((item) => item.id)).toEqual(["series-1", "episode-1", "scene-1", "shot-1"]);
  });

  it("keeps creation contexts exclusive and reserves production/review for their official surfaces", () => {
    expect(shotContextSurface("creation", "project")).toBe("project");
    expect(shotContextSurface("creation", "series")).toBe("series");
    expect(shotContextSurface("creation", "episode")).toBe("episode");
    expect(shotContextSurface("creation", "scene")).toBe("scene");
    expect(shotContextSurface("creation", "shot")).toBe("shot");
    expect(shotContextSurface("production", "shot")).toBe("production");
    expect(shotContextSurface("review", "series")).toBe("review");
  });
});
