// @vitest-environment jsdom

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import type { ProductionBatchDetail } from "../../types/productionQueue";
import type { WorkflowWorkspaceQueryResponse } from "./workflowWorkspaceAdapters";
import { useWorkflowOnboardingStore } from "../../stores/workflowOnboardingStore";
import { useWorkflowWorkspaceStore } from "../../stores/workflowWorkspaceStore";
import { WorkflowWorkspace } from "./WorkflowWorkspace";

const mocks = vi.hoisted(() => ({
  queryWorkflowWorkspace: vi.fn(),
  getProjectWorkflowConfig: vi.fn(),
  listModels: vi.fn(),
  listModelVersions: vi.fn(),
  listRuntimeProfiles: vi.fn(),
  submitGeneration: vi.fn(),
  startProductionQueue: vi.fn(),
}));

vi.mock("../../services/workflowClient", async () => {
  const actual = await vi.importActual<typeof import("../../services/workflowClient")>("../../services/workflowClient");
  return { ...actual, queryWorkflowWorkspace: mocks.queryWorkflowWorkspace };
});

vi.mock("../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../services/tauriClient")>("../../services/tauriClient");
  return { ...actual, ...mocks };
});

const recipe: RecipeViewModel = {
  workflowId: "WF_QUICK_TEST",
  workflowVersionId: "WV_QUICK_TEST",
  recipeId: "R_QUICK_TEST",
  recipeVersion: "1.0.0",
  name: "Quick Test Workflow",
  category: "image",
  mode: "text_to_image",
  fields: [{ key: "prompt", type: "textarea", label: "提示词", required: true, default: "test prompt" }],
  outputTypes: ["image"],
};

const queueDetail: ProductionBatchDetail = {
  id: "B_QUICK_TEST",
  projectId: "P_QUICK_TEST",
  name: "Generation",
  status: "READY",
  continueOnFailure: true,
  total: 1,
  pending: 1,
  running: 0,
  succeeded: 0,
  failed: 0,
  cancelled: 0,
  skipped: 0,
  items: [{ id: "I_QUICK_TEST", ordinal: 0, workflowVersionId: recipe.workflowVersionId, recipeId: recipe.recipeId, status: "PENDING" }],
  createdAt: "2026-09-17T00:00:00Z",
  updatedAt: "2026-09-17T00:00:00Z",
};

function workspaceResponse(): WorkflowWorkspaceQueryResponse {
  const registryRecipe = {
    workflowVersionId: recipe.workflowVersionId,
    recipeId: recipe.recipeId,
    version: "1.0.0",
    recipeSha256: "recipe-sha",
    inputCount: 1,
    outputCount: 1,
  };
  const version = {
    workflowVersionId: recipe.workflowVersionId,
    workflowId: recipe.workflowId,
    version: "1.0.0",
    workflowSha256: "workflow-sha",
    isCurrent: true,
    enabled: true,
    archived: false,
    recipes: [registryRecipe],
  };
  return {
    items: [{
      registry: {
        workflowId: recipe.workflowId,
        name: recipe.name,
        sourceKind: "USER",
        libraryState: "ACTIVE",
        currentVersionId: recipe.workflowVersionId,
        currentVersion: version,
        currentRecipe: registryRecipe,
        versions: [version],
        recipes: [registryRecipe],
        projectUsageCount: 0,
        historyCount: 0,
      },
      runtime: [{
        workflowId: recipe.workflowId,
        workflowVersionId: recipe.workflowVersionId,
        recipeId: recipe.recipeId,
        name: recipe.name,
        category: recipe.category,
        mode: recipe.mode,
        workflowVersion: "1.0.0",
        recipeVersion: "1.0.0",
        workflowSha256: "workflow-sha",
        recipeSha256: "recipe-sha",
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
  } as unknown as WorkflowWorkspaceQueryResponse;
}

beforeEach(() => {
  vi.resetAllMocks();
  useWorkflowWorkspaceStore.getState().reset();
  useWorkflowOnboardingStore.getState().reset();
  mocks.queryWorkflowWorkspace.mockResolvedValue(workspaceResponse());
  mocks.getProjectWorkflowConfig.mockResolvedValue({ projectId: "P_QUICK_TEST", videoModeOverrides: [] });
  mocks.listModels.mockResolvedValue([]);
  mocks.listModelVersions.mockResolvedValue([]);
  mocks.listRuntimeProfiles.mockResolvedValue([]);
  mocks.submitGeneration.mockResolvedValue(queueDetail);
  mocks.startProductionQueue.mockResolvedValue(undefined);
});

afterEach(() => cleanup());

describe("Workflow quick test Production Queue submission", () => {
  it("persists the exact workflow/recipe submission before the official Queue Start", async () => {
    const user = userEvent.setup();
    const onOpenTask = vi.fn();
    render(
      <WorkflowWorkspace
        projectId="P_QUICK_TEST"
        catalog={[recipe]}
        comfyConnected
        onCatalogChanged={vi.fn().mockResolvedValue(undefined)}
        onOpenStudio={vi.fn().mockResolvedValue(undefined)}
        onUseInProject={vi.fn().mockResolvedValue(undefined)}
        onOpenTask={onOpenTask}
      />,
    );

    await user.click(await screen.findByRole("button", { name: "测试" }));

    await waitFor(() => expect(mocks.startProductionQueue).toHaveBeenCalledWith("P_QUICK_TEST", queueDetail.id));
    expect(mocks.submitGeneration).toHaveBeenCalledWith(expect.objectContaining({
      projectId: "P_QUICK_TEST",
      workflowVersionId: recipe.workflowVersionId,
      recipeId: recipe.recipeId,
      values: { prompt: { type: "string", value: "test prompt" } },
    }));
    expect(mocks.submitGeneration.mock.invocationCallOrder[0]).toBeLessThan(mocks.startProductionQueue.mock.invocationCallOrder[0]);
    expect(onOpenTask).not.toHaveBeenCalled();
  });

  it("retries Queue Start on the same pending submission after a start failure", async () => {
    const user = userEvent.setup();
    mocks.startProductionQueue.mockRejectedValueOnce(new Error("queue start unavailable"));
    render(
      <WorkflowWorkspace
        projectId="P_QUICK_TEST"
        catalog={[recipe]}
        comfyConnected
        onCatalogChanged={vi.fn().mockResolvedValue(undefined)}
        onOpenStudio={vi.fn().mockResolvedValue(undefined)}
        onUseInProject={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    await user.click(await screen.findByRole("button", { name: "测试" }));
    await waitFor(() => expect(mocks.startProductionQueue).toHaveBeenCalledTimes(1));
    await user.click(screen.getByRole("button", { name: "测试" }));

    await waitFor(() => expect(mocks.startProductionQueue).toHaveBeenCalledTimes(2));
    expect(mocks.submitGeneration).toHaveBeenCalledTimes(1);
    expect(mocks.startProductionQueue).toHaveBeenNthCalledWith(1, "P_QUICK_TEST", queueDetail.id);
    expect(mocks.startProductionQueue).toHaveBeenNthCalledWith(2, "P_QUICK_TEST", queueDetail.id);
  });
});
