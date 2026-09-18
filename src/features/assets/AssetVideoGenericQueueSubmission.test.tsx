// @vitest-environment jsdom

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import type { ProductionBatchDetail } from "../../types/productionQueue";
import { AssetVideoBatchWorkspace } from "./AssetVideoBatchWorkspace";

const mocks = vi.hoisted(() => ({
  getProjectWorkflowConfig: vi.fn(),
  assetLibraryPage: vi.fn(),
  listAssetVideoPrompts: vi.fn(),
  listPresets: vi.fn(),
  getPreferredPreset: vi.fn(),
  listModels: vi.fn(),
  listModelVersions: vi.fn(),
  submitGeneration: vi.fn(),
  startProductionQueue: vi.fn(),
}));

vi.mock("../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../services/tauriClient")>("../../services/tauriClient");
  return { ...actual, ...mocks };
});

const recipe: RecipeViewModel = {
  workflowId: "WF_CUSTOM_VIDEO",
  workflowVersionId: "WV_CUSTOM_VIDEO",
  recipeId: "R_CUSTOM_VIDEO",
  recipeVersion: "1.0.0",
  name: "Custom Text Video",
  category: "video",
  mode: "text_to_video",
  fields: [{ key: "prompt", type: "textarea", label: "提示词", required: true, default: "A test video" }],
  outputTypes: ["video"],
};

const queueDetail: ProductionBatchDetail = {
  id: "B_CUSTOM_VIDEO",
  projectId: "P_VIDEO",
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
  items: [{ id: "I_CUSTOM_VIDEO", ordinal: 0, workflowVersionId: recipe.workflowVersionId, recipeId: recipe.recipeId, status: "PENDING" }],
  createdAt: "2026-09-17T00:00:00Z",
  updatedAt: "2026-09-17T00:00:00Z",
};

beforeEach(() => {
  vi.resetAllMocks();
  mocks.getProjectWorkflowConfig.mockResolvedValue({ projectId: "P_VIDEO", videoModeOverrides: [] });
  mocks.assetLibraryPage.mockResolvedValue({ items: [], nextCursor: undefined });
  mocks.listAssetVideoPrompts.mockResolvedValue([]);
  mocks.listPresets.mockResolvedValue([]);
  mocks.getPreferredPreset.mockResolvedValue(undefined);
  mocks.listModels.mockResolvedValue([]);
  mocks.listModelVersions.mockResolvedValue([]);
  mocks.submitGeneration.mockResolvedValue(queueDetail);
  mocks.startProductionQueue.mockResolvedValue(undefined);
});

afterEach(() => cleanup());

describe("generic Asset Video Production Queue submission", () => {
  it("submits once and retries official Queue Start against the persisted batch", async () => {
    const user = userEvent.setup();
    const onOpenTask = vi.fn();
    const onOpenProductionQueue = vi.fn();
    mocks.startProductionQueue.mockRejectedValueOnce(new Error("queue start unavailable"));

    render(
      <AssetVideoBatchWorkspace
        projectId="P_VIDEO"
        catalog={[recipe]}
        initialAssets={[]}
        comfyConnected
        taskEventsReady
        productionAdmission={{ busy: false }}
        onAdmissionChanged={vi.fn().mockResolvedValue(undefined)}
        onProductionBatchFocused={vi.fn()}
        onOpenTask={onOpenTask}
        onOpenProductionQueue={onOpenProductionQueue}
        onBackToAssets={vi.fn()}
      />,
    );

    const submitButton = await screen.findByRole("button", { name: "加入队列并开始" });
    await waitFor(() => expect((submitButton as HTMLButtonElement).disabled).toBe(false));
    await user.click(submitButton);
    await waitFor(() => expect(mocks.startProductionQueue).toHaveBeenCalledTimes(1));

    expect(mocks.submitGeneration).toHaveBeenCalledWith(expect.objectContaining({
      projectId: "P_VIDEO",
      workflowVersionId: recipe.workflowVersionId,
      recipeId: recipe.recipeId,
      values: { prompt: { type: "string", value: "A test video" } },
    }));
    expect(mocks.submitGeneration.mock.invocationCallOrder[0]).toBeLessThan(mocks.startProductionQueue.mock.invocationCallOrder[0]);
    expect(await screen.findByRole("button", { name: "打开生产队列" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "打开生产队列" }));
    expect(onOpenProductionQueue).toHaveBeenCalledWith(queueDetail.id);
    await user.click(submitButton);

    await waitFor(() => expect(mocks.startProductionQueue).toHaveBeenCalledTimes(2));
    expect(mocks.submitGeneration).toHaveBeenCalledTimes(1);
    expect(mocks.startProductionQueue).toHaveBeenNthCalledWith(1, "P_VIDEO", queueDetail.id);
    expect(mocks.startProductionQueue).toHaveBeenNthCalledWith(2, "P_VIDEO", queueDetail.id);
    expect(onOpenTask).not.toHaveBeenCalled();
  });
});
