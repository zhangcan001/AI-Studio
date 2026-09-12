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

  it("preserves exact task and deliverable targets across existing routes", () => {
    expect(resolveProjectCommandCenterNavigation({
      destination: "tasks",
      taskId: "task-9",
      shotId: "shot-9",
      actionKind: "ACTIVE_PRODUCTION",
    })).toEqual({ workspace: "tasks", section: "review", shotId: "shot-9", batchId: undefined, taskId: "task-9" });

    expect(resolveProjectCommandCenterNavigation({
      destination: "assets",
      section: "assets",
      assetId: "asset-9",
      actionKind: "COMPLETE",
    })).toEqual({ workspace: "assets", section: "assets", shotId: undefined, batchId: undefined, assetId: "asset-9" });

    expect(resolveProjectCommandCenterNavigation({
      destination: "shots",
      section: "review",
      batchId: "batch-9",
      itemId: "item-9",
      shotId: "shot-9",
      taskId: "task-9",
    })).toEqual({ workspace: "shots", section: "review", shotId: "shot-9", batchId: "batch-9", itemId: "item-9", reviewId: "item-9", taskId: "task-9" });
  });

  it("applies exact target precedence without dropping context IDs", () => {
    expect(resolveProjectCommandCenterNavigation({
      destination: "tasks",
      section: "production",
      projectId: "project-a",
      taskId: "task-1",
      batchId: "batch-1",
      shotId: "shot-1",
    })).toEqual({ workspace: "tasks", section: "review", projectId: "project-a", shotId: "shot-1", batchId: "batch-1", taskId: "task-1" });

    expect(resolveProjectCommandCenterNavigation({
      destination: "tasks",
      section: "production",
      projectId: "project-a",
      reviewId: "review-1",
      itemId: "item-1",
      taskId: "task-1",
      batchId: "batch-1",
      shotId: "shot-1",
      assetId: "asset-1",
      stage: "VIDEO",
    })).toEqual({ workspace: "shots", section: "review", projectId: "project-a", shotId: "shot-1", batchId: "batch-1", itemId: "item-1", reviewId: "review-1", taskId: "task-1", assetId: "asset-1", stage: "VIDEO" });

    expect(resolveProjectCommandCenterNavigation({ destination: "shots", section: "creation", batchId: "batch-1" })).toEqual({ workspace: "shots", section: "production", batchId: "batch-1" });
    expect(resolveProjectCommandCenterNavigation({ destination: "shots", section: "creation", assetId: "asset-1" })).toEqual({ workspace: "assets", section: "assets", assetId: "asset-1" });
    expect(resolveProjectCommandCenterNavigation({ destination: "tasks", shotId: "shot-1" })).toEqual({ workspace: "shots", section: "creation", shotId: "shot-1" });
  });
});
