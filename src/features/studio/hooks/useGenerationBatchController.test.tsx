// @vitest-environment jsdom

import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { GenerationValues, RecipeViewModel } from "../../../types/generation";
import type { ProductionBatchDetail } from "../../../types/productionQueue";
import { useGenerationBatchController } from "./useGenerationBatchController";

const mocks = vi.hoisted(() => ({
  createProductionQueue: vi.fn(),
  startProductionQueue: vi.fn(),
}));

vi.mock("../../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../../services/tauriClient")>("../../../services/tauriClient");
  return {
    ...actual,
    createProductionQueue: mocks.createProductionQueue,
    startProductionQueue: mocks.startProductionQueue,
  };
});

const recipeA: RecipeViewModel = {
  workflowId: "workflow-a",
  workflowVersionId: "workflow-version-a",
  recipeId: "recipe-shared",
  name: "Workflow A",
  category: "image",
  mode: "text-to-image",
  outputTypes: ["image"],
  fields: [{ key: "prompt", type: "textarea", label: "Prompt", required: true, default: "" }],
};

const recipeB: RecipeViewModel = {
  ...recipeA,
  workflowId: "workflow-b",
  workflowVersionId: "workflow-version-b",
  name: "Workflow B",
  fields: [{ key: "positive_prompt", type: "textarea", label: "Positive prompt", required: true, default: "" }],
};

const values: GenerationValues = {
  prompt: { type: "string", value: "current prompt" },
};

function batchDetail(id = "batch-created", total = 1): ProductionBatchDetail {
  return {
    id,
    projectId: "project-a",
    name: "Batch",
    status: "READY",
    continueOnFailure: true,
    createdAt: "2026-09-10T00:00:00Z",
    updatedAt: "2026-09-10T00:00:00Z",
    total,
    pending: total,
    running: 0,
    succeeded: 0,
    failed: 0,
    cancelled: 0,
    skipped: 0,
    items: [],
  };
}

function options(overrides: Partial<Parameters<typeof useGenerationBatchController>[0]> = {}) {
  return {
    projectId: "project-a",
    productCatalog: [recipeA, recipeB],
    selectedWorkflow: recipeA,
    values,
    hasUnsupportedField: false,
    missingAsset: false,
    canSubmitLocalBatch: true,
    comfyConnected: true,
    taskEventsReady: true,
    onValidationErrors: vi.fn(),
    onBatchCreated: vi.fn(),
    onProductionAdmissionChanged: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

function taskList(items: Array<{ workflowVersionId: string; recipeId: string; values: GenerationValues }>) {
  return {
    schemaVersion: 1,
    items,
  };
}

function taskListFile(items: Array<{ workflowVersionId: string; recipeId: string; values: GenerationValues }>) {
  const text = JSON.stringify(taskList(items));
  return new File([text], "batch.json", { type: "application/json" });
}

async function importItems(
  result: { current: ReturnType<typeof useGenerationBatchController> },
  items: Array<{ workflowVersionId: string; recipeId: string; values: GenerationValues }>,
) {
  await act(async () => {
    await result.current.importBatchTaskList(taskListFile(items));
  });
  await waitFor(() => expect(result.current.batchItems).toHaveLength(items.length));
}

describe("useGenerationBatchController", () => {
  beforeEach(() => {
    mocks.createProductionQueue.mockReset();
    mocks.startProductionQueue.mockReset();
    mocks.createProductionQueue.mockResolvedValue(batchDetail());
    mocks.startProductionQueue.mockResolvedValue(batchDetail());
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("adds the current draft with the exact workflowVersionId and recipeId", () => {
    const onValidationErrors = vi.fn();
    const { result } = renderHook(() => useGenerationBatchController(options({ onValidationErrors })));

    act(() => result.current.addCurrentToBatch());

    expect(result.current.batchItems[0]).toMatchObject({
      workflowVersionId: "workflow-version-a",
      recipeId: "recipe-shared",
      values,
    });
    expect(onValidationErrors).toHaveBeenCalledWith({});
  });

  it("blocks a missing workflow and reports validation errors for an invalid draft", () => {
    const noWorkflow = renderHook(() => useGenerationBatchController(options({ selectedWorkflow: undefined })));
    act(() => noWorkflow.result.current.addCurrentToBatch());
    expect(noWorkflow.result.current.batchItems).toHaveLength(0);

    const onValidationErrors = vi.fn();
    const invalid = renderHook(() => useGenerationBatchController(options({ values: {}, onValidationErrors })));
    act(() => invalid.result.current.addCurrentToBatch());
    expect(invalid.result.current.batchItems).toHaveLength(0);
    expect(onValidationErrors).toHaveBeenCalledWith(expect.objectContaining({ prompt: expect.any(String) }));
    expect(invalid.result.current.batchNotice).toContain("还未准备好");
  });

  it("resolves imported prompt fields by exact workflow identity", async () => {
    const { result } = renderHook(() => useGenerationBatchController(options()));
    await importItems(result, [{
      workflowVersionId: recipeB.workflowVersionId,
      recipeId: recipeB.recipeId,
      values: { positive_prompt: { type: "string", value: "workflow B prompt" } },
    }]);

    expect(result.current.batchPrompt(result.current.batchItems[0])).toBe("workflow B prompt");
    expect(result.current.batchItems[0]).toMatchObject({
      workflowVersionId: recipeB.workflowVersionId,
      recipeId: recipeB.recipeId,
    });
  });

  it("preserves batch items when queue creation fails", async () => {
    mocks.createProductionQueue.mockRejectedValue(new Error("queue create failed"));
    const { result } = renderHook(() => useGenerationBatchController(options()));
    await importItems(result, [{ workflowVersionId: recipeA.workflowVersionId, recipeId: recipeA.recipeId, values }]);

    await act(async () => { await result.current.submitBatch(); });

    expect(result.current.batchItems).toHaveLength(1);
    expect(result.current.batchNotice).toBe("操作失败，请查看技术详情。");
    expect(mocks.startProductionQueue).not.toHaveBeenCalled();
  });

  it("blocks unsupported or incomplete inputs without writing a queue", async () => {
    const onValidationErrors = vi.fn();
    const { result } = renderHook(() => useGenerationBatchController(options({
      selectedWorkflow: { ...recipeA, outputTypes: ["video"] },
      onValidationErrors,
    })));

    act(() => result.current.addCurrentToBatch());
    expect(result.current.batchItems).toHaveLength(0);
    expect(onValidationErrors).not.toHaveBeenCalled();

    await importItems(result, [{ workflowVersionId: recipeA.workflowVersionId, recipeId: recipeA.recipeId, values: {} }]);
    await act(async () => { await result.current.submitBatch(); });
    expect(mocks.createProductionQueue).not.toHaveBeenCalled();
    expect(result.current.batchItems).toHaveLength(1);
  });

  it("supports blank cards, prompt splitting, editing, copy, move, remove, and clear", async () => {
    const { result } = renderHook(() => useGenerationBatchController(options()));

    act(() => result.current.addBlankPromptCard());
    expect(result.current.batchPrompt(result.current.batchItems[0])).toBe("");

    act(() => result.current.setBatchPasteText("first\n\nsecond"));
    act(() => result.current.splitPastedPrompts());
    expect(result.current.batchItems).toHaveLength(3);
    expect(result.current.batchPasteText).toBe("");

    const firstId = result.current.batchItems[0].id;
    act(() => result.current.updateBatchPrompt(firstId, "edited"));
    expect(result.current.batchPrompt(result.current.batchItems[0])).toBe("edited");
    act(() => result.current.copyBatchItem(firstId));
    expect(result.current.batchItems).toHaveLength(4);
    act(() => result.current.moveBatchItem(firstId, 1));
    expect(result.current.batchItems[1].id).toBe(firstId);
    act(() => result.current.removeBatchItem(firstId));
    expect(result.current.batchItems).toHaveLength(3);
    act(() => result.current.clearBatch());
    expect(result.current.batchItems).toHaveLength(0);
    expect(result.current.batchNotice).toBeUndefined();
  });

  it("blocks empty and over-limit pasted prompt lists", async () => {
    const { result } = renderHook(() => useGenerationBatchController(options()));
    act(() => result.current.splitPastedPrompts());
    expect(result.current.batchNotice).toContain("请先粘贴提示词");

    const hundred = Array.from({ length: 100 }, (_, index) => ({
      workflowVersionId: recipeA.workflowVersionId,
      recipeId: recipeA.recipeId,
      values: { prompt: { type: "string" as const, value: `prompt-${index}` } },
    }));
    await importItems(result, hundred);
    act(() => result.current.setBatchPasteText("one more"));
    act(() => result.current.splitPastedPrompts());
    expect(result.current.batchItems).toHaveLength(100);
    expect(result.current.batchNotice).toContain("超过 100 项上限");
  });

  it("enforces the 100 item limit for import, add, and copy", async () => {
    const { result } = renderHook(() => useGenerationBatchController(options()));
    const hundred = Array.from({ length: 100 }, (_, index) => ({
      workflowVersionId: recipeA.workflowVersionId,
      recipeId: recipeA.recipeId,
      values: { prompt: { type: "string" as const, value: `prompt-${index}` } },
    }));
    await importItems(result, hundred);

    act(() => result.current.addCurrentToBatch());
    expect(result.current.batchItems).toHaveLength(100);
    expect(result.current.batchNotice).toContain("达到批量任务上限");
    act(() => result.current.copyBatchItem(result.current.batchItems[0].id));
    expect(result.current.batchItems).toHaveLength(100);
    expect(result.current.batchNotice).toContain("达到图片批次上限");

    await act(async () => {
      await result.current.importBatchTaskList(taskListFile([{
        workflowVersionId: recipeA.workflowVersionId,
        recipeId: recipeA.recipeId,
        values,
      }]));
    });
    expect(result.current.batchItems).toHaveLength(100);
    expect(result.current.batchNotice).toContain("超过 100 项");
  });

  it("imports valid task lists and rejects invalid task lists", async () => {
    const { result } = renderHook(() => useGenerationBatchController(options()));
    await act(async () => { await result.current.importBatchTaskList(undefined); });
    expect(result.current.batchItems).toHaveLength(0);
    await importItems(result, [{ workflowVersionId: recipeA.workflowVersionId, recipeId: recipeA.recipeId, values }]);
    expect(result.current.batchNotice).toBe("已从 JSON 导入 1 个任务。");

    await act(async () => {
      await result.current.importBatchTaskList(new File(["not-json"], "bad.json"));
    });
    expect(result.current.batchItems).toHaveLength(1);
    expect(result.current.batchNotice).toBe("操作失败，请查看技术详情。");
  });

  it("resets only its own session when the project changes", async () => {
    const { result, rerender } = renderHook(
      (props: Parameters<typeof useGenerationBatchController>[0]) => useGenerationBatchController(props),
      { initialProps: options() },
    );
    await importItems(result, [{ workflowVersionId: recipeA.workflowVersionId, recipeId: recipeA.recipeId, values }]);
    act(() => result.current.setBatchPasteText("draft text"));

    rerender(options({ projectId: "project-b" }));

    await waitFor(() => {
      expect(result.current.batchItems).toHaveLength(0);
      expect(result.current.batchPasteText).toBe("");
      expect(result.current.batchNotice).toBeUndefined();
    });
  });

  it("keeps the batch draft when validation blocks submission", async () => {
    const { result } = renderHook(() => useGenerationBatchController(options()));
    await importItems(result, [{ workflowVersionId: recipeA.workflowVersionId, recipeId: recipeA.recipeId, values: {} }]);

    await act(async () => { await result.current.submitBatch(); });

    expect(mocks.createProductionQueue).not.toHaveBeenCalled();
    expect(result.current.batchItems).toHaveLength(1);
    expect(result.current.batchNotice).toContain("还未填写完整");
    expect(result.current.batchSubmitting).toBe(false);
  });

  it("creates, refreshes admission, starts, and clears a valid batch", async () => {
    const onBatchCreated = vi.fn();
    const onProductionAdmissionChanged = vi.fn().mockResolvedValue(undefined);
    const { result } = renderHook(() => useGenerationBatchController(options({ onBatchCreated, onProductionAdmissionChanged })));
    await importItems(result, [{ workflowVersionId: recipeA.workflowVersionId, recipeId: recipeA.recipeId, values }]);

    await act(async () => { await result.current.submitBatch(); });

    expect(mocks.createProductionQueue).toHaveBeenCalledWith(expect.objectContaining({
      projectId: "project-a",
      continueOnFailure: true,
      items: [{ workflowVersionId: recipeA.workflowVersionId, recipeId: recipeA.recipeId, values }],
    }));
    expect(onBatchCreated).toHaveBeenCalledWith("batch-created");
    expect(onProductionAdmissionChanged).toHaveBeenCalledTimes(1);
    expect(mocks.startProductionQueue).toHaveBeenCalledWith("project-a", "batch-created");
    expect(onProductionAdmissionChanged.mock.invocationCallOrder[0]).toBeLessThan(mocks.startProductionQueue.mock.invocationCallOrder[0]);
    expect(result.current.batchItems).toHaveLength(0);
    expect(result.current.batchNotice).toContain("已创建并开始执行");
    expect(result.current.batchSubmitting).toBe(false);
  });

  it("starts the persisted batch even when admission refresh fails", async () => {
    const onProductionAdmissionChanged = vi.fn().mockRejectedValue(new Error("refresh unavailable"));
    const { result } = renderHook(() => useGenerationBatchController(options({ onProductionAdmissionChanged })));
    await importItems(result, [{ workflowVersionId: recipeA.workflowVersionId, recipeId: recipeA.recipeId, values }]);

    await act(async () => { await result.current.submitBatch(); });

    expect(mocks.startProductionQueue).toHaveBeenCalledWith("project-a", "batch-created");
    expect(result.current.batchNotice).toContain("已创建并开始执行");
  });

  it("keeps the created batch manually startable when start fails", async () => {
    mocks.startProductionQueue.mockRejectedValue(new Error("start unavailable"));
    const { result } = renderHook(() => useGenerationBatchController(options()));
    await importItems(result, [{ workflowVersionId: recipeA.workflowVersionId, recipeId: recipeA.recipeId, values }]);

    await act(async () => { await result.current.submitBatch(); });

    expect(result.current.batchItems).toHaveLength(0);
    expect(result.current.batchNotice).toContain("可在队列中手动开始");
  });

  it("does not submit while production admission, ComfyUI, or task events are unavailable", async () => {
    const admissionBlocked = renderHook(() => useGenerationBatchController(options({ canSubmitLocalBatch: false })));
    await importItems(admissionBlocked.result, [{ workflowVersionId: recipeA.workflowVersionId, recipeId: recipeA.recipeId, values }]);
    await act(async () => { await admissionBlocked.result.current.submitBatch(); });
    expect(mocks.createProductionQueue).not.toHaveBeenCalled();

    cleanup();
    const runtimeBlocked = renderHook(() => useGenerationBatchController(options({ comfyConnected: false })));
    await importItems(runtimeBlocked.result, [{ workflowVersionId: recipeA.workflowVersionId, recipeId: recipeA.recipeId, values }]);
    await act(async () => { await runtimeBlocked.result.current.submitBatch(); });
    expect(mocks.createProductionQueue).not.toHaveBeenCalled();

    cleanup();
    const eventsBlocked = renderHook(() => useGenerationBatchController(options({ taskEventsReady: false })));
    await importItems(eventsBlocked.result, [{ workflowVersionId: recipeA.workflowVersionId, recipeId: recipeA.recipeId, values }]);
    await act(async () => { await eventsBlocked.result.current.submitBatch(); });
    expect(mocks.createProductionQueue).not.toHaveBeenCalled();
  });
});
