// @vitest-environment jsdom

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { WorkflowWorkspaceQueryResponse } from "./workflowWorkspaceAdapters";
import { useWorkflowOnboardingStore } from "../../stores/workflowOnboardingStore";
import { useWorkflowWorkspaceStore } from "../../stores/workflowWorkspaceStore";
import { WorkflowLabControlPlaneHarness as WorkflowWorkspace } from "../workflow-lab/testing/WorkflowLabControlPlaneHarness";

const workflowMocks = vi.hoisted(() => ({
  queryWorkflowWorkspace: vi.fn(),
  deleteWorkflowVersionOf: vi.fn(),
}));

vi.mock("../../services/workflowClient", async () => {
  const actual = await vi.importActual<typeof import("../../services/workflowClient")>("../../services/workflowClient");
  return { ...actual, ...workflowMocks };
});

function recipe(recipeId: string, version: string, isPromoted = false) {
  return {
    workflowVersionId: "WV_PROMOTION",
    recipeId,
    version,
    recipeSha256: `${recipeId}-sha`,
    inputCount: 1,
    outputCount: 1,
    isPromoted,
  };
}

function workspaceResponse(promotedRecipeId?: string): WorkflowWorkspaceQueryResponse {
  const recipes = [recipe("R_A", "1.0.0", promotedRecipeId === "R_A"), recipe("R_B", "2.0.0", promotedRecipeId === "R_B")];
  const version = {
    workflowVersionId: "WV_PROMOTION",
    workflowId: "WF_PROMOTION",
    version: "1.0.0",
    workflowSha256: "workflow-sha",
    isCurrent: true,
    enabled: true,
    archived: false,
    recipes,
  };
  const olderVersion = { ...version, workflowVersionId: "WV_OLDER", version: "0.9.0", isCurrent: false, recipes: [] };
  const currentRecipe = recipes.find((entry) => entry.isPromoted) ?? recipes[1];
  return {
    items: [{
      registry: {
        workflowId: "WF_PROMOTION",
        name: "Promotion Workflow",
        sourceKind: "USER",
        libraryState: "ACTIVE",
        currentVersionId: "WV_PROMOTION",
        currentVersion: version,
        currentRecipe,
        versions: [version, olderVersion],
        recipes,
        projectUsageCount: 0,
        historyCount: 0,
      },
      runtime: [{
        workflowId: "WF_PROMOTION",
        workflowVersionId: "WV_PROMOTION",
        recipeId: currentRecipe.recipeId,
        name: "Promotion Workflow",
        category: "image",
        mode: "text_to_image",
        workflowVersion: "1.0.0",
        recipeVersion: currentRecipe.version,
        workflowSha256: "workflow-sha",
        recipeSha256: `${currentRecipe.recipeId}-sha`,
        artifactId: "ART_PROMOTION",
        artifactSourceKind: "USER",
        packageName: "promotion-workflow-package",
        packageSourcePath: null,
        artifactStatus: "VALID",
        packageStatus: "VALID",
        libraryState: "ACTIVE",
        enabled: true,
        archived: false,
        capability: "READY",
        capabilityIssues: [],
        readiness: "READY",
        readinessReasons: [],
        diagnostics: [],
        nodeCount: 1,
        hasSuccessfulRun: false,
        activeTasks: 0,
        totalTasks: 0,
      }],
    }],
    staging: [],
  };
}

function renderWorkspace(responses: WorkflowWorkspaceQueryResponse[]) {
  workflowMocks.queryWorkflowWorkspace.mockImplementation((mode: string) => {
    if (mode === "REFRESH") return Promise.resolve(responses[1] ?? responses[0]);
    return Promise.resolve(responses[0]);
  });
  const onCatalogChanged = vi.fn().mockResolvedValue(undefined);
  render(
    <WorkflowWorkspace
      catalog={[]}
      comfyConnected={false}
      onCatalogChanged={onCatalogChanged}
      onOpenStudio={vi.fn().mockResolvedValue(undefined)}
      onUseInProject={vi.fn().mockResolvedValue(undefined)}
    />,
  );
  return onCatalogChanged;
}

async function openDetails(filter?: "archived") {
  await waitFor(() => expect(workflowMocks.queryWorkflowWorkspace).toHaveBeenCalledWith("FAST"));
  if (filter) await userEvent.setup().selectOptions(screen.getByLabelText("工作流筛选"), filter);
  const row = within((await screen.findByText("Promotion Workflow")).closest("article")!);
  await userEvent.setup().click(row.getByText("查看详情"));
  return row;
}

beforeEach(() => {
  vi.resetAllMocks();
  useWorkflowWorkspaceStore.getState().reset();
  useWorkflowOnboardingStore.getState().reset();
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("W-20 版本级删除", () => {
  it("offers deletion only for non-current versions and calls the version command", async () => {
    workflowMocks.deleteWorkflowVersionOf.mockResolvedValue({});
    vi.spyOn(window, "confirm").mockReturnValue(true);
    const onCatalogChanged = renderWorkspace([workspaceResponse()]);
    const row = await openDetails();

    const buttons = row.getAllByRole("button", { name: "删除此版本" });
    expect(buttons).toHaveLength(1);
    await userEvent.setup().click(buttons[0]);

    await waitFor(() => expect(workflowMocks.deleteWorkflowVersionOf).toHaveBeenCalledWith("WF_PROMOTION", "WV_OLDER"));
    await waitFor(() => expect(onCatalogChanged).toHaveBeenCalled());
    expect((await screen.findByRole("status")).textContent).toContain("已删除版本 0.9.0");
  });

  it("surfaces backend rejections instead of silently falling back", async () => {
    workflowMocks.deleteWorkflowVersionOf.mockRejectedValue({ code: "WORKFLOW_VERSION_IN_USE", message: "in use" });
    vi.spyOn(window, "confirm").mockReturnValue(true);
    renderWorkspace([workspaceResponse()]);
    const row = await openDetails();

    await userEvent.setup().click(row.getByRole("button", { name: "删除此版本" }));

    expect(await screen.findByText(/仍被生产批次或队列任务引用/)).toBeTruthy();
  });

  it("does nothing when the confirmation is cancelled", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(false);
    renderWorkspace([workspaceResponse()]);
    const row = await openDetails();

    await userEvent.setup().click(row.getByRole("button", { name: "删除此版本" }));

    expect(workflowMocks.deleteWorkflowVersionOf).not.toHaveBeenCalled();
  });
});
