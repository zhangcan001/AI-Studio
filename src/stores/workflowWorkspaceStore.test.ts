import { describe, expect, it, beforeEach } from "vitest";
import { useWorkflowWorkspaceStore } from "./workflowWorkspaceStore";

describe("workflow workspace cache", () => {
  beforeEach(() => {
    useWorkflowWorkspaceStore.getState().reset();
  });

  it("keeps the last successful workspace across component remounts", () => {
    const workspace = {
      items: [],
      staging: [],
    };

    useWorkflowWorkspaceStore.getState().setWorkspace(workspace);

    expect(useWorkflowWorkspaceStore.getState().workspace).toEqual(workspace);
    expect(useWorkflowWorkspaceStore.getState().loadedAt).toEqual(expect.any(Number));
  });

  it("keeps cached data available when an invalidation is requested", () => {
    const workspace = { items: [], staging: [] };
    useWorkflowWorkspaceStore.getState().setWorkspace(workspace);

    useWorkflowWorkspaceStore.getState().invalidate();

    expect(useWorkflowWorkspaceStore.getState().workspace).toEqual(workspace);
    expect(useWorkflowWorkspaceStore.getState().loadedAt).toBeUndefined();
  });
});
