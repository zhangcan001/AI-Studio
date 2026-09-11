// @vitest-environment jsdom

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { WorkflowWorkspaceQueryResponse } from "./workflowWorkspaceAdapters";
import { useWorkflowOnboardingStore } from "../../stores/workflowOnboardingStore";
import { useWorkflowWorkspaceStore } from "../../stores/workflowWorkspaceStore";
import { WorkflowWorkspace } from "./WorkflowWorkspace";

const workflowMocks = vi.hoisted(() => ({
  queryWorkflowWorkspace: vi.fn(),
  promoteWorkflowRecipe: vi.fn(),
  clearWorkflowRecipePromotion: vi.fn(),
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
        versions: [version],
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
  workflowMocks.promoteWorkflowRecipe.mockResolvedValue({});
  workflowMocks.clearWorkflowRecipePromotion.mockResolvedValue({});
});

afterEach(() => cleanup());

describe("DEV-090 recipe promotion workspace behavior", () => {
  it("renders the promoted status and keeps the action available for the other exact recipe", async () => {
    renderWorkspace([workspaceResponse("R_B")]);
    const row = await openDetails();

    expect(row.getByText(/已推广/)).toBeTruthy();
    expect(row.getByRole("button", { name: "设为推广配方" })).toBeTruthy();
  });

  it("promotes exact recipe identity, refreshes workspace/catalog, and reflects A → B replacement", async () => {
    const onCatalogChanged = renderWorkspace([workspaceResponse("R_A"), workspaceResponse("R_B")]);
    const row = await openDetails();

    await userEvent.setup().click(row.getByRole("button", { name: "设为推广配方" }));

    await waitFor(() => expect(workflowMocks.promoteWorkflowRecipe).toHaveBeenCalledWith("WV_PROMOTION", "R_B"));
    expect(workflowMocks.queryWorkflowWorkspace).toHaveBeenLastCalledWith("REFRESH");
    expect(onCatalogChanged).toHaveBeenCalledTimes(1);
    expect(await screen.findByText("已将配方 2.0.0 设为该工作流版本的推广配方。")).toBeTruthy();
  });

  it("surfaces promotion failures without optimistic state changes", async () => {
    workflowMocks.promoteWorkflowRecipe.mockRejectedValue(new Error("promotion failed"));
    const onCatalogChanged = renderWorkspace([workspaceResponse("R_A")]);
    const row = await openDetails();

    await userEvent.setup().click(row.getByRole("button", { name: "设为推广配方" }));

    expect((await screen.findByRole("alert")).textContent).toBeTruthy();
    expect(workflowMocks.queryWorkflowWorkspace).toHaveBeenCalledTimes(1);
    expect(onCatalogChanged).not.toHaveBeenCalled();
    expect(row.getByText(/已推广/)).toBeTruthy();
  });

  it("clears the exact promoted recipe, refreshes workspace/catalog, and reflects the authoritative response", async () => {
    const onCatalogChanged = renderWorkspace([workspaceResponse("R_B"), workspaceResponse()]);
    const row = await openDetails();

    await userEvent.setup().click(row.getByRole("button", { name: "取消推广" }));

    await waitFor(() => expect(workflowMocks.clearWorkflowRecipePromotion).toHaveBeenCalledWith("WV_PROMOTION", "R_B"));
    expect(workflowMocks.queryWorkflowWorkspace).toHaveBeenLastCalledWith("REFRESH");
    expect(onCatalogChanged).toHaveBeenCalledTimes(1);
    expect(await screen.findByText("已取消配方 2.0.0 的推广状态。")).toBeTruthy();
    expect(row.queryByText(/已推广/)).toBeNull();
  });

  it("surfaces clear failures without optimistic state changes", async () => {
    workflowMocks.clearWorkflowRecipePromotion.mockRejectedValue(new Error("clear failed"));
    const onCatalogChanged = renderWorkspace([workspaceResponse("R_B")]);
    const row = await openDetails();

    await userEvent.setup().click(row.getByRole("button", { name: "取消推广" }));

    expect((await screen.findByRole("alert")).textContent).toBeTruthy();
    expect(workflowMocks.queryWorkflowWorkspace).toHaveBeenCalledTimes(1);
    expect(onCatalogChanged).not.toHaveBeenCalled();
    expect(row.getByText(/已推广/)).toBeTruthy();
  });

  it("does not expose promotion controls for archived versions", async () => {
    const response = workspaceResponse("R_B");
    response.items[0].registry.currentVersion!.archived = true;
    response.items[0].registry.versions[0].archived = true;
    response.items[0].runtime[0].archived = true;
    response.items[0].registry.libraryState = "REMOVED";
    const rowResponse = response;
    renderWorkspace([rowResponse]);
    const row = await openDetails("archived");

    expect(row.queryByRole("button", { name: "设为推广配方" })).toBeNull();
    expect(row.getByRole("button", { name: "取消推广" })).toBeTruthy();
  });
});
