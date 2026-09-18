// @vitest-environment jsdom

import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { GenerationValues, RecipeViewModel } from "../../../types/generation";
import type { ProductionBatchDetail } from "../../../types/productionQueue";
import type { TaskView } from "../../../types/task";
import { useTaskStore } from "../../../stores/taskStore";
import { useGenerationSubmissionController } from "./useGenerationSubmissionController";

const mocks = vi.hoisted(() => ({
  submitGeneration: vi.fn(),
  startProductionQueue: vi.fn(),
  cancelTask: vi.fn(),
}));

vi.mock("../../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../../services/tauriClient")>("../../../services/tauriClient");
  return {
    ...actual,
    submitGeneration: mocks.submitGeneration,
    startProductionQueue: mocks.startProductionQueue,
    cancelTask: mocks.cancelTask,
  };
});

const workflow: RecipeViewModel = {
  workflowId: "workflow-a",
  workflowVersionId: "workflow-version-a",
  recipeId: "recipe-a",
  name: "Workflow A",
  category: "image",
  mode: "text-to-image",
  fields: [{ key: "prompt", type: "textarea", label: "Prompt", required: true, default: "" }],
};

const values: GenerationValues = {
  prompt: { type: "string", value: "a test prompt" },
};

const task = (id = "task-a"): TaskView => ({
  id,
  projectId: "project-a",
  status: "CREATED",
  progress: { mode: "indeterminate" },
  createdAt: "2026-09-10T00:00:00Z",
  outputAssetIds: [],
});

const batch = (id = "batch-a"): ProductionBatchDetail => ({
  id,
  projectId: "project-a",
  name: "Generation",
  status: "READY",
  continueOnFailure: true,
  createdAt: "2026-09-10T00:00:00Z",
  updatedAt: "2026-09-10T00:00:00Z",
  total: 1,
  pending: 1,
  running: 0,
  succeeded: 0,
  failed: 0,
  cancelled: 0,
  skipped: 0,
  items: [{ id: "item-a", ordinal: 0, workflowVersionId: "workflow-version-a", recipeId: "recipe-a", status: "PENDING" }],
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function options(overrides: Partial<Parameters<typeof useGenerationSubmissionController>[0]> = {}) {
  return {
    projectId: "project-a",
    selectedWorkflow: workflow,
    values,
    productionAdmission: { busy: false },
    comfyConnected: true,
    taskEventsReady: true,
    missingAsset: false,
    unsupportedField: false,
    onValidationErrors: vi.fn(),
    onNotice: vi.fn(),
    ...overrides,
  };
}

describe("useGenerationSubmissionController", () => {
  beforeEach(() => {
    useTaskStore.getState().clear();
    mocks.submitGeneration.mockReset();
    mocks.startProductionQueue.mockReset();
    mocks.cancelTask.mockReset();
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("does not create without a selected workflow", async () => {
    const { result } = renderHook(() => useGenerationSubmissionController(options({ selectedWorkflow: undefined })));
    await act(async () => { await result.current.generate(); });
    expect(mocks.submitGeneration).not.toHaveBeenCalled();
  });

  it("blocks KREA/config errors before validation or creation", async () => {
    const onValidationErrors = vi.fn();
    const onNotice = vi.fn();
    const { result } = renderHook(() => useGenerationSubmissionController(options({
      configurationError: "KREA 配置错误",
      onValidationErrors,
      onNotice,
    })));
    await act(async () => { await result.current.generate(); });
    expect(onNotice).toHaveBeenCalledWith("KREA 配置错误");
    expect(onValidationErrors).not.toHaveBeenCalled();
    expect(mocks.submitGeneration).not.toHaveBeenCalled();
  });

  it("writes validation errors and does not create", async () => {
    const onValidationErrors = vi.fn();
    const { result } = renderHook(() => useGenerationSubmissionController(options({
      values: {},
      onValidationErrors,
    })));
    await act(async () => { await result.current.generate(); });
    expect(onValidationErrors).toHaveBeenCalledWith(expect.objectContaining({ prompt: expect.any(String) }));
    expect(mocks.submitGeneration).not.toHaveBeenCalled();
  });

  it.each([
    ["production busy", { productionAdmission: { busy: true } }],
    ["Comfy disconnected", { comfyConnected: false }],
    ["task events unavailable", { taskEventsReady: false, taskEventError: "任务事件不可用" }],
    ["missing asset", { missingAsset: true }],
    ["unsupported field", { unsupportedField: true }],
  ])("blocks when %s", async (_label, override) => {
    const onNotice = vi.fn();
    const { result } = renderHook(() => useGenerationSubmissionController(options({ ...override, onNotice })));
    await act(async () => { await result.current.generate(); });
    expect(onNotice).toHaveBeenCalledWith(expect.any(String));
    expect(mocks.submitGeneration).not.toHaveBeenCalled();
  });

  it("submits exact runtime identity and starts the persisted queue without adopting a Task", async () => {
    mocks.submitGeneration.mockResolvedValue(batch());
    const onNotice = vi.fn();
    const { result } = renderHook(() => useGenerationSubmissionController(options({ onNotice })));
    await act(async () => { await result.current.generate(); });
    expect(mocks.submitGeneration).toHaveBeenCalledWith({
      projectId: "project-a",
      workflowVersionId: "workflow-version-a",
      recipeId: "recipe-a",
      values,
      submissionIdempotencyKey: expect.any(String),
    });
    expect(mocks.startProductionQueue).toHaveBeenCalledWith("project-a", "batch-a");
    expect(useTaskStore.getState().currentTask).toBeUndefined();
    expect(result.current.creating).toBe(false);
    expect(onNotice).toHaveBeenLastCalledWith("已加入生产队列并开始处理。");
  });

  it("forwards an explicitly selected model version without inferring one", async () => {
    mocks.submitGeneration.mockResolvedValue(batch());
    const { result } = renderHook(() => useGenerationSubmissionController(options({ modelVersionId: "model-version-a" })));

    await act(async () => { await result.current.generate(); });

    expect(mocks.submitGeneration).toHaveBeenCalledWith(expect.objectContaining({
      modelVersionId: "model-version-a",
    }));
  });

  it("keeps creating true during a request and single-flights concurrent submissions", async () => {
    const pending = deferred<ProductionBatchDetail>();
    mocks.submitGeneration.mockReturnValue(pending.promise);
    const { result } = renderHook(() => useGenerationSubmissionController(options()));
    let first!: Promise<void>;
    let second!: Promise<void>;
    act(() => {
      first = result.current.generate();
      second = result.current.generate();
    });
    await waitFor(() => expect(result.current.creating).toBe(true));
    expect(mocks.submitGeneration).toHaveBeenCalledTimes(1);
    pending.resolve(batch());
    await act(async () => { await Promise.all([first, second]); });
    expect(result.current.creating).toBe(false);
  });

  it("uses a new idempotency key for a later independent submission", async () => {
    mocks.submitGeneration.mockResolvedValue(batch());
    const { result } = renderHook(() => useGenerationSubmissionController(options()));
    await act(async () => { await result.current.generate(); });
    await act(async () => { await result.current.generate(); });
    const firstKey = mocks.submitGeneration.mock.calls[0][0].submissionIdempotencyKey;
    const secondKey = mocks.submitGeneration.mock.calls[1][0].submissionIdempotencyKey;
    expect(secondKey).not.toBe(firstKey);
  });

  it("persists the queue item before official Queue Start and never adopts the submission as a Task", async () => {
    const order: string[] = [];
    mocks.submitGeneration.mockImplementation(async () => {
      order.push("enqueue");
      return batch();
    });
    mocks.startProductionQueue.mockImplementation(async () => {
      order.push("start");
      return batch();
    });
    const { result } = renderHook(() => useGenerationSubmissionController(options()));

    await act(async () => { await result.current.generate(); });

    expect(order).toEqual(["enqueue", "start"]);
    expect(mocks.submitGeneration).toHaveBeenCalledWith(expect.objectContaining({
      projectId: "project-a",
      workflowVersionId: "workflow-version-a",
      recipeId: "recipe-a",
      values,
      submissionIdempotencyKey: expect.any(String),
    }));
    expect(mocks.startProductionQueue).toHaveBeenCalledWith("project-a", "batch-a");
    expect(useTaskStore.getState().currentTask).toBeUndefined();
  });

  it("surfaces submission failures and clears creating", async () => {
    mocks.submitGeneration.mockRejectedValue(new Error("create failed"));
    const onNotice = vi.fn();
    const { result } = renderHook(() => useGenerationSubmissionController(options({ onNotice })));
    await act(async () => { await result.current.generate(); });
    expect(onNotice).toHaveBeenCalledWith(expect.any(String));
    expect(result.current.creating).toBe(false);
  });

  it("does not cancel when there is no current task", async () => {
    const { result } = renderHook(() => useGenerationSubmissionController(options()));
    await act(async () => { await result.current.cancelCurrentTask(); });
    expect(mocks.cancelTask).not.toHaveBeenCalled();
  });

  it("cancels the exact current task and upserts the returned task", async () => {
    const current = task("current-task");
    const cancelled = { ...current, status: "CANCELLED" as const };
    useTaskStore.getState().setCurrentTask(current);
    const pending = deferred<TaskView>();
    mocks.cancelTask.mockReturnValue(pending.promise);
    const onNotice = vi.fn();
    const { result } = renderHook(() => useGenerationSubmissionController(options({ onNotice })));
    let cancellation!: Promise<void>;
    act(() => {
      cancellation = result.current.cancelCurrentTask();
    });
    await waitFor(() => expect(result.current.cancelling).toBe(true));
    expect(mocks.cancelTask).toHaveBeenCalledWith("project-a", "current-task");
    pending.resolve(cancelled);
    await act(async () => { await cancellation; });
    expect(useTaskStore.getState().currentTask).toEqual(cancelled);
    expect(result.current.cancelling).toBe(false);
  });

  it("surfaces cancellation failures and always clears cancelling", async () => {
    useTaskStore.getState().setCurrentTask(task("current-task"));
    mocks.cancelTask.mockRejectedValue(new Error("cancel failed"));
    const onNotice = vi.fn();
    const { result } = renderHook(() => useGenerationSubmissionController(options({ onNotice })));
    await act(async () => { await result.current.cancelCurrentTask(); });
    expect(onNotice).toHaveBeenCalledWith(expect.any(String));
    expect(result.current.cancelling).toBe(false);
  });
});
