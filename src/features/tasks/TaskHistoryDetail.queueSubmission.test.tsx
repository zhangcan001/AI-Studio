// @vitest-environment jsdom

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ProductionBatchDetail } from "../../types/productionQueue";
import type { ReusableGenerationDraft, TaskDetail } from "../../types/history";
import { TaskHistoryDetail } from "./TaskHistoryDetail";

const mocks = vi.hoisted(() => ({
  getReusableDraft: vi.fn(),
  listGenerationToolUsages: vi.fn(),
  listGenerationAssetVersionLinks: vi.fn(),
  submitGeneration: vi.fn(),
  startProductionQueue: vi.fn(),
}));

vi.mock("../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../services/tauriClient")>("../../services/tauriClient");
  return {
    ...actual,
    getReusableDraft: mocks.getReusableDraft,
    listGenerationToolUsages: mocks.listGenerationToolUsages,
    listGenerationAssetVersionLinks: mocks.listGenerationAssetVersionLinks,
    submitGeneration: mocks.submitGeneration,
    startProductionQueue: mocks.startProductionQueue,
  };
});

const draft: ReusableGenerationDraft = {
  projectId: "project-1",
  workflowVersionId: "workflow-version-1",
  recipeId: "recipe-1",
  modelVersionId: "model-version-1",
  promptVersionId: "prompt-version-1",
  workflowName: "Workflow",
  createdAt: "2026-09-17T00:00:00Z",
  values: { prompt: { type: "string", value: "the original prompt" } },
  missingAssetIds: [],
};

const detail: TaskDetail = {
  id: "task-failed",
  projectId: "project-1",
  workflowId: "workflow-1",
  workflowVersionId: "workflow-version-1",
  recipeId: "recipe-1",
  workflowName: "Workflow",
  status: "FAILED",
  createdAt: "2026-09-16T00:00:00Z",
  errorCode: "COMFY_TIMEOUT",
  errorMessage: "temporary timeout",
  outputAssets: [],
  reusableDraft: { available: true, missingAssetIds: [] },
};

const queuedBatch: ProductionBatchDetail = {
  id: "batch-retry",
  projectId: "project-1",
  name: "Generation",
  status: "READY",
  continueOnFailure: true,
  createdAt: "2026-09-17T00:00:00Z",
  updatedAt: "2026-09-17T00:00:00Z",
  total: 1,
  pending: 1,
  running: 0,
  succeeded: 0,
  failed: 0,
  cancelled: 0,
  skipped: 0,
  items: [{ id: "item-retry", ordinal: 0, workflowVersionId: "workflow-version-1", recipeId: "recipe-1", status: "PENDING" }],
};

describe("Task History retry queue authority", () => {
  beforeEach(() => {
    mocks.getReusableDraft.mockReset().mockResolvedValue(draft);
    mocks.listGenerationToolUsages.mockReset().mockResolvedValue([{
      id: "usage-1",
      generationId: "generation-1",
      toolInstanceId: "tool-instance-1",
      toolVersionId: "tool-version-1",
      metadata: {},
      createdAt: "2026-09-16T00:00:00Z",
    }]);
    mocks.listGenerationAssetVersionLinks.mockReset().mockResolvedValue([]);
    mocks.submitGeneration.mockReset().mockResolvedValue(queuedBatch);
    mocks.startProductionQueue.mockReset().mockResolvedValue(queuedBatch);
  });

  afterEach(() => vi.restoreAllMocks());

  it("enqueues the frozen retry with provenance and parent before official Queue Start", async () => {
    const order: string[] = [];
    mocks.submitGeneration.mockImplementation(async () => {
      order.push("enqueue");
      return queuedBatch;
    });
    mocks.startProductionQueue.mockImplementation(async () => {
      order.push("start");
      return queuedBatch;
    });
    render(
      <TaskHistoryDetail
        projectId="project-1"
        detail={detail}
        loadingDraft={false}
        comfyConnected
        productionBusy={false}
        onBack={vi.fn()}
        onLoadInputs={vi.fn()}
        onOpenAsset={vi.fn()}
      />,
    );

    const retry = await screen.findByRole("button", { name: "重试一次" });
    await waitFor(() => expect((retry as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(retry);

    await waitFor(() => expect(mocks.startProductionQueue).toHaveBeenCalledWith("project-1", "batch-retry"));
    expect(order).toEqual(["enqueue", "start"]);
    expect(mocks.submitGeneration).toHaveBeenCalledWith(expect.objectContaining({
      projectId: "project-1",
      workflowVersionId: "workflow-version-1",
      recipeId: "recipe-1",
      values: draft.values,
      modelVersionId: "model-version-1",
      promptVersionId: "prompt-version-1",
      toolInstanceId: "tool-instance-1",
      toolVersionId: "tool-version-1",
      parentTaskId: "task-failed",
      submissionIdempotencyKey: expect.stringMatching(/^task-retry:task-failed:.+/),
    }));
    expect(await screen.findByText(/重试已加入并开始处理：batch-retry/)).toBeTruthy();
  });

  it("reuses one idempotency key after an uncertain enqueue failure", async () => {
    mocks.submitGeneration
      .mockRejectedValueOnce(new Error("submission response lost"))
      .mockResolvedValueOnce(queuedBatch);
    render(
      <TaskHistoryDetail
        projectId="project-1"
        detail={detail}
        loadingDraft={false}
        comfyConnected
        productionBusy={false}
        onBack={vi.fn()}
        onLoadInputs={vi.fn()}
        onOpenAsset={vi.fn()}
      />,
    );

    const retry = await screen.findByRole("button", { name: "重试一次" });
    await waitFor(() => expect((retry as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(retry);
    await waitFor(() => expect(mocks.submitGeneration).toHaveBeenCalledTimes(1));
    fireEvent.click(retry);

    await waitFor(() => expect(mocks.startProductionQueue).toHaveBeenCalledWith("project-1", "batch-retry"));
    expect(mocks.submitGeneration).toHaveBeenCalledTimes(2);
    const firstKey = mocks.submitGeneration.mock.calls[0][0].submissionIdempotencyKey;
    const secondKey = mocks.submitGeneration.mock.calls[1][0].submissionIdempotencyKey;
    expect(firstKey).toEqual(expect.stringMatching(/^task-retry:task-failed:.+/));
    expect(secondKey).toBe(firstKey);
  });
});
