import { describe, expect, it } from "vitest";
import { resolveProjectCommandCenterNavigation, workflowUseProjectDestination } from "./App";

describe("project workflow navigation", () => {
  const catalog = [
    { workflowId: "workflow-1", recipeId: "recipe-1" },
    { workflowId: "workflow-1", recipeId: "recipe-2" },
  ];

  it("uses the projects workspace only for the exact workflow and recipe", () => {
    expect(workflowUseProjectDestination(catalog, "workflow-1", "recipe-2")).toBe("projects");
    expect(workflowUseProjectDestination(catalog, "workflow-1", "missing-recipe")).toBeUndefined();
  });

  it("keeps Continue Work shot and batch targets when resolving the App route", () => {
    expect(resolveProjectCommandCenterNavigation({
      destination: "shots",
      section: "review",
      shotId: "shot-42",
      actionKind: "VIDEO_REVIEW",
    })).toEqual({ workspace: "shots", section: "review", shotId: "shot-42", batchId: undefined });

    expect(resolveProjectCommandCenterNavigation({
      destination: "shots",
      section: "production",
      batchId: "batch-8",
      actionKind: "ACTIVE_PRODUCTION",
    })).toEqual({ workspace: "shots", section: "production", shotId: undefined, batchId: "batch-8" });
  });
});
