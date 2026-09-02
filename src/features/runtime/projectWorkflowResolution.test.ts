import { describe, expect, it } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import { resolveProjectWorkflow } from "./projectWorkflowResolution";

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
});
