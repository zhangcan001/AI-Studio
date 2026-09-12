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

  it("routes collection filters to the existing project-scoped list surfaces", () => {
    expect(resolveProjectCommandCenterNavigation({
      destination: "shots",
      projectId: "project-a",
      section: "production",
      collectionFilter: { kind: "shots", status: "READY" },
    })).toEqual({
      workspace: "shots",
      section: "production",
      projectId: "project-a",
      shotId: undefined,
      batchId: undefined,
      collectionFilter: { kind: "shots", status: "READY" },
    });

    expect(resolveProjectCommandCenterNavigation({
      destination: "tasks",
      projectId: "project-a",
      collectionFilter: { kind: "tasks", status: "FAILED" },
    })).toEqual({
      workspace: "tasks",
      section: "review",
      projectId: "project-a",
      shotId: undefined,
      batchId: undefined,
      collectionFilter: { kind: "tasks", status: "FAILED" },
    });

    expect(resolveProjectCommandCenterNavigation({
      destination: "shots",
      projectId: "project-a",
      collectionFilter: { kind: "review", state: "PENDING" },
    })).toEqual({
      workspace: "shots",
      section: "review",
      projectId: "project-a",
      shotId: undefined,
      batchId: undefined,
      collectionFilter: { kind: "review", state: "PENDING" },
    });
  });

  it("keeps an exact item target authoritative while retaining its collection context", () => {
    expect(resolveProjectCommandCenterNavigation({
      destination: "tasks",
      projectId: "project-a",
      taskId: "task-1",
      collectionFilter: { kind: "tasks", status: "FAILED" },
    })).toEqual({
      workspace: "tasks",
      section: "review",
      projectId: "project-a",
      shotId: undefined,
      batchId: undefined,
      taskId: "task-1",
      collectionFilter: { kind: "tasks", status: "FAILED" },
    });
  });
});
