import { describe, expect, it } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import type { ProjectWorkflowBindingView, ProjectWorkflowConfigView } from "../../types/projectWorkflow";
import type { RuntimeParameterProfile } from "../../types/settings";
import type { WorkflowWorkspaceItem } from "./workflowWorkspaceAdapters";
import {
  buildProductionProfiles,
  buildWorkflowCenterSummary,
  runtimeProfilesForRecipe,
} from "./workflowCenterModel";

const recipeA: RecipeViewModel = {
  workflowId: "workflow-a",
  workflowVersionId: "version-a",
  recipeId: "recipe-a",
  name: "图片生产",
  category: "image",
  mode: "text_to_image",
  fields: [],
  outputTypes: ["image"],
};

const recipeSameName: RecipeViewModel = {
  ...recipeA,
  workflowVersionId: "version-b",
  recipeId: "recipe-b",
};

function binding(
  stage: "IMAGE" | "VIDEO",
  workflowVersionId: string,
  recipeId: string,
  available = true,
): ProjectWorkflowBindingView {
  return {
    stage,
    mode: "DEFAULT",
    workflowVersionId,
    recipeId,
    available,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
  };
}

function config(patch: Partial<ProjectWorkflowConfigView> = {}): ProjectWorkflowConfigView {
  return { projectId: "project-1", videoModeOverrides: [], ...patch };
}

function workspaceItem(patch: Partial<WorkflowWorkspaceItem> = {}): WorkflowWorkspaceItem {
  return {
    packageName: "package",
    builtin: false,
    source: "USER",
    archived: false,
    packageStatus: "VALID",
    workflowId: "workflow-a",
    workflowVersionId: "version-a",
    name: "图片生产",
    workflowVersion: "1.0.0",
    enabled: true,
    capability: "READY",
    readiness: "READY",
    readinessReasons: [],
    capabilityIssues: [],
    nodeCount: 1,
    recipes: [{ recipeId: "recipe-a", version: "1.0.0", inputCount: 0, outputCount: 1 }],
    activeTasks: 0,
    totalTasks: 0,
    hasSuccessfulRun: false,
    diagnostics: [],
    registryBacked: true,
    sourceKind: "USER",
    libraryState: "ACTIVE",
    currentVersionId: "version-a",
    currentRecipe: { recipeId: "recipe-a", workflowVersionId: "version-a" },
    versions: [],
    registryRecipes: [],
    projectUsageCount: 0,
    historyCount: 0,
    ...patch,
  };
}

describe("workflow center projection", () => {
  it("matches runtime profiles by exact workflow version and recipe IDs", () => {
    const profiles: RuntimeParameterProfile[] = [
      { id: "profile-a", workflowVersionId: "version-a", recipeId: "recipe-a", name: "A", values: {}, updatedAt: "" },
      { id: "profile-b", workflowVersionId: "version-b", recipeId: "recipe-b", name: "B", values: {}, updatedAt: "" },
    ];

    expect(runtimeProfilesForRecipe(profiles, recipeA).map((profile) => profile.id)).toEqual(["profile-a"]);
    expect(runtimeProfilesForRecipe(profiles, recipeSameName).map((profile) => profile.id)).toEqual(["profile-b"]);
  });

  it("keeps stale project bindings visible and never guesses by name", () => {
    const result = buildProductionProfiles(
      config({ videoDefault: binding("VIDEO", "missing-version", "missing-recipe", false) }),
      [recipeA, recipeSameName],
      [],
      [],
    );
    const videoDefault = result.find((profile) => profile.key === "VIDEO_DEFAULT");

    expect(videoDefault?.status).toBe("ATTENTION");
    expect(videoDefault?.staleConfiguredBinding).toBe(true);
    expect(videoDefault?.recipe).toBeUndefined();
  });

  it("derives summary counts without adding a persisted state source", () => {
    const productionProfiles = buildProductionProfiles(
      config({ imageDefault: binding("IMAGE", "version-a", "recipe-a") }),
      [recipeA],
      [workspaceItem()],
      [],
    );
    const summary = buildWorkflowCenterSummary(
      [workspaceItem(), workspaceItem({ workflowId: "workflow-b", readiness: "BLOCKED", currentRecipe: undefined })],
      [],
      productionProfiles,
    );

    expect(summary.availableWorkflowCount).toBe(1);
    expect(summary.issueWorkflowCount).toBe(1);
    expect(summary.projectUsedWorkflowCount).toBe(1);
  });
});
