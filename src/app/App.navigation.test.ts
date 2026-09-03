import { describe, expect, it } from "vitest";
import { workflowUseProjectDestination } from "./App";

describe("project workflow navigation", () => {
  const catalog = [
    { workflowId: "workflow-1", recipeId: "recipe-1" },
    { workflowId: "workflow-1", recipeId: "recipe-2" },
  ];

  it("uses the projects workspace only for the exact workflow and recipe", () => {
    expect(workflowUseProjectDestination(catalog, "workflow-1", "recipe-2")).toBe("projects");
    expect(workflowUseProjectDestination(catalog, "workflow-1", "missing-recipe")).toBeUndefined();
  });
});
