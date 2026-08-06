import { describe, expect, it } from "vitest";
import { mergeTaskHistoryItems } from "./taskHistoryState";

const item = (id: string) => ({
  id,
  workflowId: "workflow",
  workflowVersionId: "version",
  recipeId: "recipe",
  workflowName: "Demo",
  status: "SUCCEEDED" as const,
  createdAt: "2026-01-01T00:00:00Z",
  outputCount: 1,
});

describe("task history pagination state", () => {
  it("appends a keyset page without duplicating an item", () => {
    const result = mergeTaskHistoryItems([item("tsk_1")], [item("tsk_1"), item("tsk_2")], false);
    expect(result.map((task) => task.id)).toEqual(["tsk_1", "tsk_2"]);
  });

  it("replaces items when a filter starts a fresh page", () => {
    expect(mergeTaskHistoryItems([item("tsk_old")], [item("tsk_new")], true).map((task) => task.id)).toEqual([
      "tsk_new",
    ]);
  });
});
