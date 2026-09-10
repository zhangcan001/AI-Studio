// @vitest-environment jsdom

import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { WorkflowBenchmarkView } from "../../../types/benchmark";
import type { GenerationValues, RecipeViewModel } from "../../../types/generation";
import type { ReusableGenerationDraft } from "../../../types/history";
import type { ProductionBatchDetail } from "../../../types/productionQueue";
import type { PromptVersionView } from "../../../types/prompt";
import type { ExperimentPlan } from "../../experiments/experimentPlanner";
import { useStudioStore } from "../../../stores/studioStore";
import { useGenerationExperimentController } from "./useGenerationExperimentController";

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
  recipeId: "recipe-a",
  name: "Workflow A",
  category: "image",
  mode: "text-to-image",
  fields: [{ key: "prompt", type: "textarea", label: "Prompt", required: true, default: "" }],
};

const recipeB: RecipeViewModel = {
  ...recipeA,
  workflowId: "workflow-b",
  workflowVersionId: "workflow-version-b",
  recipeId: "recipe-b",
  name: "Workflow B",
};

const values: GenerationValues = {
  prompt: { type: "string", value: "base prompt" },
};

function batchDetail(id = "experiment-batch", total = 2): ProductionBatchDetail {
  return {
    id,
    projectId: "project-a",
    name: "Experiment",
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

const plan: ExperimentPlan = {
  workflowVersionId: recipeA.workflowVersionId,
  recipeId: recipeA.recipeId,
  workflowName: recipeA.name,
  baseValues: values,
  dimensions: [{ fieldKey: "prompt", values: [{ type: "string", value: "variant" }] }],
  items: [{
    id: "experiment-item-1",
    ordinal: 0,
    values: { prompt: { type: "string", value: "variant" } },
    changes: [{ fieldKey: "prompt", fieldLabel: "Prompt", value: "variant" }],
  }],
  videoWarning: false,
  frozenAt: "2026-09-10T00:00:00Z",
};

function options(overrides: Partial<Parameters<typeof useGenerationExperimentController>[0]> = {}) {
  return {
    projectId: "project-a",
    selectedWorkflow: recipeA,
    productCatalog: [recipeA, recipeB],
    canExperimentBase: true,
    onNotice: vi.fn(),
    onProductionAdmissionChanged: vi.fn().mockResolvedValue(undefined),
    onStudioModeChange: vi.fn(),
    onClearMissingAssetFields: vi.fn(),
    ...overrides,
  };
}

function reusableDraft(overrides: Partial<ReusableGenerationDraft> = {}): ReusableGenerationDraft {
  return {
    projectId: "project-a",
    workflowVersionId: recipeB.workflowVersionId,
    recipeId: recipeB.recipeId,
    workflowName: recipeB.name,
    createdAt: "2026-09-10T00:00:00Z",
    values: { prompt: { type: "string", value: "winner" } },
    missingAssetIds: [],
    ...overrides,
  };
}

function benchmark(): WorkflowBenchmarkView {
  const summary = {
    id: "benchmark-a",
    projectId: "project-a",
    name: "Benchmark",
    mediaType: "IMAGE" as const,
    status: "QUEUED" as const,
    candidateCount: 2,
    succeededCount: 0,
    failedCount: 0,
    repeatCount: 1,
    seedStrategy: "FIXED_SEED",
    createdAt: "2026-09-10T00:00:00Z",
    updatedAt: "2026-09-10T00:00:00Z",
  };
  return {
    ...summary,
    productionBatchId: "benchmark-batch",
    baseValues: values,
    assetIds: [],
    candidates: [],
    summary,
    comparison: { directlyComparable: true, recommendations: [] },
  };
}

describe("useGenerationExperimentController", () => {
  beforeEach(() => {
    mocks.createProductionQueue.mockReset();
    mocks.startProductionQueue.mockReset();
    mocks.createProductionQueue.mockResolvedValue(batchDetail());
    mocks.startProductionQueue.mockResolvedValue(batchDetail());
    useStudioStore.getState().resetDraft();
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("blocks a plan when the base experiment admission is not ready", async () => {
    const onNotice = vi.fn();
    const { result } = renderHook(() => useGenerationExperimentController(options({
      canExperimentBase: false,
      blockedReason: "基础草稿无效",
      onNotice,
    })));

    await act(async () => { await result.current.submitExperimentPlan(plan); });

    expect(mocks.createProductionQueue).not.toHaveBeenCalled();
    expect(onNotice).toHaveBeenCalledWith("基础草稿无效");
  });

  it("preserves exact recipe identity, cloned values, context, focus, refresh order, and start", async () => {
    const onProductionAdmissionChanged = vi.fn().mockResolvedValue(undefined);
    const onStudioModeChange = vi.fn();
    const { result } = renderHook(() => useGenerationExperimentController(options({
      onProductionAdmissionChanged,
      onStudioModeChange,
    })));

    await act(async () => { await result.current.submitExperimentPlan(plan); });

    expect(mocks.createProductionQueue).toHaveBeenCalledWith(expect.objectContaining({
      projectId: "project-a",
      continueOnFailure: true,
      items: [{
        workflowVersionId: recipeA.workflowVersionId,
        recipeId: recipeA.recipeId,
        values: { prompt: { type: "string", value: "variant" } },
      }],
    }));
    const request = mocks.createProductionQueue.mock.calls[0][0];
    expect(request.items[0].values).not.toBe(plan.items[0].values);
    expect(result.current.experimentFocusBatchId).toBe("experiment-batch");
    expect(result.current.experimentContexts["experiment-batch"]).toMatchObject({
      recipe: recipeA,
      baseValues: values,
    });
    expect(onStudioModeChange).toHaveBeenCalledWith("batch");
    expect(onProductionAdmissionChanged).toHaveBeenCalledTimes(1);
    expect(mocks.startProductionQueue).toHaveBeenCalledWith("project-a", "experiment-batch");
    expect(onProductionAdmissionChanged.mock.invocationCallOrder[0]).toBeLessThan(mocks.startProductionQueue.mock.invocationCallOrder[0]);
  });

  it("still starts the persisted experiment when admission refresh fails", async () => {
    const onProductionAdmissionChanged = vi.fn().mockRejectedValue(new Error("refresh unavailable"));
    const { result } = renderHook(() => useGenerationExperimentController(options({ onProductionAdmissionChanged })));

    await act(async () => { await result.current.submitExperimentPlan(plan); });

    expect(mocks.startProductionQueue).toHaveBeenCalledWith("project-a", "experiment-batch");
    expect(result.current.experimentFocusBatchId).toBe("experiment-batch");
  });

  it("surfaces create failure without starting or focusing a queue", async () => {
    mocks.createProductionQueue.mockRejectedValue(new Error("create failed"));
    const onNotice = vi.fn();
    const { result } = renderHook(() => useGenerationExperimentController(options({ onNotice })));

    await act(async () => { await result.current.submitExperimentPlan(plan); });

    expect(mocks.startProductionQueue).not.toHaveBeenCalled();
    expect(result.current.experimentFocusBatchId).toBeUndefined();
    expect(onNotice).toHaveBeenLastCalledWith("操作失败，请查看技术详情。");
  });

  it("keeps focus and context when queue start fails and does not create a second queue", async () => {
    mocks.startProductionQueue.mockRejectedValue(new Error("start failed"));
    const onNotice = vi.fn();
    const { result } = renderHook(() => useGenerationExperimentController(options({ onNotice })));

    await act(async () => { await result.current.submitExperimentPlan(plan); });

    expect(mocks.createProductionQueue).toHaveBeenCalledTimes(1);
    expect(mocks.startProductionQueue).toHaveBeenCalledTimes(1);
    expect(result.current.experimentFocusBatchId).toBe("experiment-batch");
    expect(result.current.experimentContexts["experiment-batch"]).toBeDefined();
    expect(onNotice).toHaveBeenLastCalledWith(expect.stringContaining("可在生产队列中手动开始"));
  });

  it("builds prompt dimensions and handles benchmark focus", () => {
    const onStudioModeChange = vi.fn();
    const { result } = renderHook(() => useGenerationExperimentController(options({ onStudioModeChange })));
    const version: PromptVersionView = {
      id: "version-a",
      promptId: "prompt-a",
      version: 1,
      text: "prompt variant",
      createdAt: "2026-09-10T00:00:00Z",
    };

    act(() => result.current.usePromptVersionsForExperiment("prompt", [version]));
    expect(result.current.promptExperimentDimensions).toEqual([{
      fieldKey: "prompt",
      values: [{ type: "string", value: "prompt variant" }],
    }]);
    expect(onStudioModeChange).toHaveBeenCalledWith("experiment");

    act(() => result.current.handleBenchmarkCreated(benchmark()));
    expect(result.current.experimentFocusBatchId).toBe("benchmark-batch");
    expect(result.current.promptExperimentDimensions).toHaveLength(1);
    act(() => result.current.clearPromptExperimentDimensions());
    expect(result.current.promptExperimentDimensions).toHaveLength(0);
  });

  it("resets experiment state at the project boundary", async () => {
    const { result, rerender } = renderHook(
      (props: Parameters<typeof useGenerationExperimentController>[0]) => useGenerationExperimentController(props),
      { initialProps: options() },
    );
    await act(async () => { await result.current.submitExperimentPlan(plan); });
    expect(result.current.experimentFocusBatchId).toBe("experiment-batch");

    rerender(options({ projectId: "project-b" }));
    await waitFor(() => {
      expect(result.current.experimentFocusBatchId).toBeUndefined();
      expect(result.current.experimentContexts).toEqual({});
      expect(result.current.promptExperimentDimensions).toEqual([]);
    });
  });

  it("rejects foreign or unavailable winner results", async () => {
    const onNotice = vi.fn();
    const { result } = renderHook(() => useGenerationExperimentController(options({ onNotice })));

    await act(async () => {
      await result.current.promoteExperimentWinner(reusableDraft({ projectId: "project-b" }), { batchName: "Batch", taskId: "task" });
    });
    expect(onNotice).toHaveBeenLastCalledWith("生产结果属于其他项目，无法加载到当前创作。");
    await act(async () => {
      await result.current.promoteExperimentWinner(reusableDraft({ workflowVersionId: "missing" }), { batchName: "Batch", taskId: "task" });
    });
    expect(onNotice).toHaveBeenLastCalledWith("生产结果对应的工作流版本已不在运行目录中，请先刷新工作流列表。");
  });

  it("promotes an exact winner into Studio without auto-submitting", async () => {
    const onClearMissingAssetFields = vi.fn();
    const onStudioModeChange = vi.fn();
    const onNotice = vi.fn();
    const { result } = renderHook(() => useGenerationExperimentController(options({
      onClearMissingAssetFields,
      onStudioModeChange,
      onNotice,
    })));

    await act(async () => {
      await result.current.promoteExperimentWinner(reusableDraft({ missingAssetIds: ["missing-asset"] }), {
        batchName: "Experiment",
        taskId: "task-winner",
      });
    });

    expect(useStudioStore.getState().selectedWorkflow).toEqual(recipeB);
    expect(useStudioStore.getState().values).toEqual({ prompt: { type: "string", value: "winner" } });
    expect(useStudioStore.getState().reuseProvenance).toMatchObject({
      sourceBatchName: "Experiment",
      sourceTaskId: "task-winner",
    });
    expect(onClearMissingAssetFields).toHaveBeenCalledTimes(1);
    expect(onStudioModeChange).toHaveBeenCalledWith("single");
    expect(onNotice).toHaveBeenLastCalledWith(expect.stringContaining("未自动提交任务"));
    expect(mocks.createProductionQueue).not.toHaveBeenCalled();
  });
});
