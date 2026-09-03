import { describe, expect, it } from "vitest";
import type { RecipeField, RecipeViewModel } from "../../types/generation";
import {
  MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID,
} from "./productRuntimeScope";
import {
  resolveProjectFolderWorkflow,
  resolveProjectVideoWorkflow,
  resolveProjectWorkflow,
} from "./projectWorkflowResolution";
import { recipeRef } from "./workflowCapabilities";

function recipe(id: string): RecipeViewModel {
  return {
    workflowId: `workflow-${id}`,
    workflowVersionId: `version-${id}`,
    recipeId: `recipe-${id}`,
    name: id,
    category: "test",
    mode: "test",
    fields: [],
    outputTypes: ["image"],
  };
}

function videoRecipe(
  id: string,
  mediaKey?: "reference_audio",
  workflowId = `workflow-${id}`,
): RecipeViewModel {
  const fields: RecipeField[] = [
    { key: "prompt", type: "textarea", label: "Prompt", required: true, default: "" },
  ];
  if (mediaKey) fields.push({ key: mediaKey, type: "audio", label: mediaKey, required: false });
  return {
    ...recipe(id),
    workflowId,
    category: "video",
    mode: "video",
    fields,
    outputTypes: ["video"],
  };
}

const refs = {
  explicit: { workflowVersionId: "version-explicit", recipeId: "recipe-explicit" },
  projectMode: { workflowVersionId: "version-mode", recipeId: "recipe-mode" },
  projectDefault: { workflowVersionId: "version-default", recipeId: "recipe-default" },
};

describe("project workflow resolution", () => {
  const candidates = [recipe("explicit"), recipe("mode"), recipe("default"), recipe("compatible")];

  it("uses the explicit selection before every project or recommendation fallback", () => {
    const result = resolveProjectWorkflow({
      candidates,
      explicit: refs.explicit,
      projectMode: refs.projectMode,
      projectDefault: refs.projectDefault,
      recommended: candidates[3],
    });
    expect(result.recipe?.recipeId).toBe("recipe-explicit");
    expect(result.source).toBe("explicit");
  });

  it("resolves project mode, then project default, then recommended, then compatible", () => {
    expect(resolveProjectWorkflow({ candidates, projectMode: refs.projectMode }).source).toBe("project_mode");
    expect(resolveProjectWorkflow({ candidates, projectDefault: refs.projectDefault }).source).toBe("project_default");
    expect(resolveProjectWorkflow({ candidates, recommended: candidates[3] }).source).toBe("recommended");
    expect(resolveProjectWorkflow({ candidates }).source).toBe("compatible");
  });

  it("marks stale project references without mutating or hiding the fallback", () => {
    const result = resolveProjectWorkflow({
      candidates,
      projectMode: { workflowVersionId: "missing", recipeId: "missing" },
      projectDefault: refs.projectDefault,
    });
    expect(result.recipe?.recipeId).toBe("recipe-default");
    expect(result.source).toBe("project_default");
    expect(result.staleProjectBinding).toBe(true);
  });

  it("blocks a specific H3 mode when only a generic video recipe exists", () => {
    const genericVideo: RecipeViewModel = {
      ...recipe("generic-video"),
      outputTypes: ["video"],
      fields: [],
    };

    expect(resolveProjectVideoWorkflow(
      [genericVideo],
      "FL2VA_IMAGE_TO_VIDEO",
      undefined,
      undefined,
      undefined,
      undefined,
      { allowGenericFallback: false },
    ).recipe).toBeUndefined();
  });

  it("preserves generic CUSTOM_VIDEO fallback when explicitly allowed", () => {
    const genericVideo: RecipeViewModel = {
      ...recipe("generic-video"),
      outputTypes: ["video"],
      fields: [],
    };

    const resolved = resolveProjectVideoWorkflow(
      [genericVideo],
      "FL2VA_IMAGE_TO_VIDEO",
      undefined,
      undefined,
      undefined,
      undefined,
      { allowGenericFallback: true },
    );
    expect(resolved).toMatchObject({
      recipe: genericVideo,
      source: "compatible",
      staleProjectBinding: false,
    });
    expect(resolveProjectVideoWorkflow([genericVideo], "CUSTOM_VIDEO", undefined, undefined, undefined, undefined, {
      allowGenericFallback: false,
    }).recipe).toBe(genericVideo);
  });

  it("does not mark an available but mode-incompatible video default stale", () => {
    const videoDefault = videoRecipe("default");
    const compatible = videoRecipe("compatible", "reference_audio");
    const recommended = videoRecipe(
      "recommended",
      "reference_audio",
      MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID,
    );
    expect(resolveProjectVideoWorkflow(
      [videoDefault, compatible, recommended],
      "REF2VA_AUDIO",
      undefined,
      undefined,
      recipeRef(videoDefault),
      recommended,
      { allowGenericFallback: false },
    )).toMatchObject({
      recipe: recommended,
      source: "recommended",
      staleProjectBinding: false,
    });
  });

  it("keeps project-folder resolution strict instead of using generic video recipes", () => {
    const genericVideo = videoRecipe("generic-video");
    expect(resolveProjectFolderWorkflow([genericVideo], "FL2VA_IMAGE_TO_VIDEO").recipe).toBeUndefined();
  });
});
