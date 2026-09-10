// @vitest-environment jsdom

import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
  WorkflowInputMappingView,
  WorkflowInputView,
  WorkflowOnboardingDraftView,
  WorkflowProductionWorkspaceView,
} from "../../../types/workflowOnboarding";
import { useWorkflowParameterExposureController, type MappingDraft, type ParameterMappingEdit } from "./useWorkflowParameterExposureController";

const serviceMocks = vi.hoisted(() => ({
  checkOnboardingCapability: vi.fn(),
  commitWorkflowImport: vi.fn(),
  discardOnboarding: vi.fn(),
  duplicateWorkflowRecipe: vi.fn(),
  getOnboardingDraft: vi.fn(),
  removeOnboardingInputMapping: vi.fn(),
  setOnboardingInputMapping: vi.fn(),
  validateOnboarding: vi.fn(),
}));

vi.mock("../../../services/workflowClient", () => serviceMocks);

function mapping(overrides: Partial<WorkflowInputMappingView> = {}): WorkflowInputMappingView {
  return {
    semanticKey: "steps",
    fieldType: "integer",
    label: "Steps",
    required: true,
    defaultValue: "20",
    minValue: "1",
    maxValue: "100",
    step: "1",
    targetNode: "88",
    targetInput: "steps",
    ...overrides,
  };
}

function draft(overrides: Partial<WorkflowOnboardingDraftView> = {}): WorkflowOnboardingDraftView {
  return {
    draftId: "draft-1",
    workflowSha256: "workflow-sha",
    originalFilename: "workflow.json",
    nodeCount: 1,
    uniqueClassCount: 1,
    nodes: [{
      nodeId: "88",
      classType: "KSampler",
      title: "采样器",
      isOutputNode: false,
      inputs: [{
        name: "steps",
        kind: "number",
        currentValueSummary: "20",
        isLinked: false,
        bindable: true,
        suggestedType: "integer",
        suggestedSemanticKey: "steps",
        numericMin: "1",
        numericMax: "100",
        numericStep: "1",
        allowedOptions: [],
      }],
    }],
    capability: { state: "READY", issues: [] },
    inputMappings: [mapping()],
    outputMappings: [],
    manifest: {
      workflowId: "workflow-1",
      name: "Demo Workflow",
      workflowVersion: "1.0.0",
      recipeVersion: "1.0.0",
      category: "video",
      mode: "CUSTOM_VIDEO",
      recipeId: "recipe-draft",
    },
    recipe: { inputs: [], bindings: [], outputs: [], valid: true, issues: [] },
    validation: {
      apiFormat: true,
      recipe: true,
      bindings: true,
      outputs: true,
      manifest: true,
      capability: true,
      dryRun: true,
      readyToPublish: true,
      issues: [],
    },
    ...overrides,
  };
}

function workflow(overrides: Partial<WorkflowProductionWorkspaceView> = {}): WorkflowProductionWorkspaceView {
  return {
    packageName: "demo-package",
    builtin: false,
    archived: false,
    packageStatus: "READY",
    workflowId: "workflow-1",
    workflowVersionId: "version-1",
    name: "Demo Workflow",
    workflowVersion: "1.0.0",
    enabled: true,
    capability: "READY",
    readiness: "READY",
    readinessReasons: [],
    capabilityIssues: [],
    nodeCount: 1,
    recipes: [
      { recipeId: "recipe-old", version: "1.0.0", inputCount: 1, outputCount: 1 },
      { recipeId: "recipe-source", version: "1.1.0", inputCount: 1, outputCount: 1 },
    ],
    activeTasks: 0,
    totalTasks: 0,
    hasSuccessfulRun: false,
    diagnostics: [],
    ...overrides,
  };
}

function input(overrides: Partial<WorkflowInputView> = {}): WorkflowInputView {
  return {
    name: "steps",
    kind: "number",
    currentValueSummary: "20",
    isLinked: false,
    bindable: true,
    suggestedType: "integer",
    suggestedSemanticKey: "steps",
    numericMin: "1",
    numericMax: "100",
    numericStep: "1",
    allowedOptions: [],
    ...overrides,
  };
}

function options() {
  return {
    onError: vi.fn(),
    onNotice: vi.fn(),
    onWorkspaceRefresh: vi.fn().mockResolvedValue(undefined),
    onCatalogChanged: vi.fn().mockResolvedValue(undefined),
    onBeforeOpen: vi.fn(),
  };
}

describe("useWorkflowParameterExposureController", () => {
  afterEach(() => cleanup());

  beforeEach(() => {
    vi.clearAllMocks();
    serviceMocks.checkOnboardingCapability.mockResolvedValue({ state: "READY", issues: [] });
    serviceMocks.commitWorkflowImport.mockResolvedValue({ recipeId: "recipe-new" });
    serviceMocks.discardOnboarding.mockResolvedValue(undefined);
    serviceMocks.duplicateWorkflowRecipe.mockResolvedValue(draft());
    serviceMocks.getOnboardingDraft.mockResolvedValue(draft());
    serviceMocks.removeOnboardingInputMapping.mockResolvedValue(draft({ inputMappings: [] }));
    serviceMocks.setOnboardingInputMapping.mockResolvedValue(draft());
    serviceMocks.validateOnboarding.mockResolvedValue(draft().validation);
  });

  it("opens the latest recipe with the exact workflow version identity", async () => {
    const hookOptions = options();
    const item = workflow();
    const { result } = renderHook(() => useWorkflowParameterExposureController(hookOptions));

    await act(async () => { await result.current.open(item); });

    expect(serviceMocks.duplicateWorkflowRecipe).toHaveBeenCalledWith("version-1", "recipe-source");
    expect(serviceMocks.checkOnboardingCapability).toHaveBeenCalledWith("draft-1");
    expect(serviceMocks.getOnboardingDraft).toHaveBeenCalledWith("draft-1");
    expect(result.current.item).toBe(item);
    expect(result.current.originalKeys).toEqual(["steps"]);
    expect(hookOptions.onBeforeOpen).toHaveBeenCalledTimes(1);
  });

  it.each([
    ["archived", workflow({ archived: true })],
    ["missing workflow version", workflow({ workflowVersionId: undefined })],
  ])("does not duplicate an %s workflow", async (_label, item) => {
    const { result } = renderHook(() => useWorkflowParameterExposureController(options()));

    await act(async () => { await result.current.open(item); });

    expect(serviceMocks.duplicateWorkflowRecipe).not.toHaveBeenCalled();
    expect(result.current.draft).toBeUndefined();
  });

  it("keeps the duplicated session when capability loading fails", async () => {
    serviceMocks.checkOnboardingCapability.mockRejectedValue({ code: "COMFY_OFFLINE" });
    const hookOptions = options();
    const item = workflow();
    const { result } = renderHook(() => useWorkflowParameterExposureController(hookOptions));

    await act(async () => { await result.current.open(item); });

    expect(result.current.draft?.draftId).toBe("draft-1");
    expect(result.current.item).toBe(item);
    expect(result.current.originalKeys).toEqual(["steps"]);
    expect(serviceMocks.getOnboardingDraft).not.toHaveBeenCalled();
    expect(hookOptions.onError).toHaveBeenLastCalledWith(expect.stringContaining("参数节点已加载"));
  });

  it("discards and clears the session even when discard fails", async () => {
    const hookOptions = options();
    const { result } = renderHook(() => useWorkflowParameterExposureController(hookOptions));
    await act(async () => { await result.current.open(workflow()); });
    serviceMocks.discardOnboarding.mockRejectedValueOnce(new Error("already consumed"));

    await act(async () => { await result.current.close(); });

    expect(serviceMocks.discardOnboarding).toHaveBeenCalledWith("draft-1");
    expect(result.current.draft).toBeUndefined();
    expect(result.current.item).toBeUndefined();
    expect(result.current.originalKeys).toEqual([]);
  });

  it("refreshes capability and then reads the authoritative draft", async () => {
    const refreshed = draft({ capability: { state: "MISSING_NODES", issues: [] } });
    const hookOptions = options();
    const { result } = renderHook(() => useWorkflowParameterExposureController(hookOptions));
    await act(async () => { await result.current.open(workflow()); });
    vi.clearAllMocks();
    serviceMocks.checkOnboardingCapability.mockResolvedValue({ state: "MISSING_NODES", issues: [] });
    serviceMocks.getOnboardingDraft.mockResolvedValue(refreshed);

    await act(async () => { await result.current.refreshCapability(); });

    expect(serviceMocks.checkOnboardingCapability).toHaveBeenCalledWith("draft-1");
    expect(serviceMocks.getOnboardingDraft).toHaveBeenCalledWith("draft-1");
    expect(result.current.draft?.capability.state).toBe("MISSING_NODES");
    expect(hookOptions.onNotice).toHaveBeenCalledWith("已刷新参数建议与 ComfyUI 输入范围。");
  });

  it("exposes a supported input with the complete mapping payload", async () => {
    const hookOptions = options();
    const { result } = renderHook(() => useWorkflowParameterExposureController(hookOptions));
    await act(async () => { await result.current.open(workflow()); });
    vi.clearAllMocks();

    await act(async () => { await result.current.exposeParameter("88", input()); });

    expect(serviceMocks.setOnboardingInputMapping).toHaveBeenCalledWith("draft-1", {
      semanticKey: "steps",
      fieldType: "integer",
      label: "Steps",
      required: true,
      defaultValue: "20",
      minValue: "1",
      maxValue: "100",
      step: "1",
      minItems: undefined,
      maxItems: undefined,
      targetNode: "88",
      targetInput: "steps",
    });
    expect(hookOptions.onNotice).toHaveBeenCalledWith("Steps 已加入新配方草稿。");
  });

  it("does not map unsupported or non-exposable inputs", async () => {
    const { result } = renderHook(() => useWorkflowParameterExposureController(options()));
    await act(async () => { await result.current.open(workflow()); });
    vi.clearAllMocks();

    await act(async () => {
      await result.current.exposeParameter("88", input({ suggestedType: "float" }));
      await result.current.exposeParameter("88", input({ bindable: false, isLinked: true, suggestedSemanticKey: "model" }));
    });

    expect(serviceMocks.setOnboardingInputMapping).not.toHaveBeenCalled();
  });

  it("saves every editable mapping field using the exact target", async () => {
    const { result } = renderHook(() => useWorkflowParameterExposureController(options()));
    await act(async () => { await result.current.open(workflow()); });
    vi.clearAllMocks();
    const edit: MappingDraft = {
      semanticKey: "strength",
      fieldType: "number",
      label: "Strength",
      required: false,
      defaultValue: "0.5",
      minValue: "0",
      maxValue: "1",
      minItems: "2",
      maxItems: "5",
      itemIndex: "3",
      step: "0.1",
    };

    await act(async () => { await result.current.saveMapping(edit, "88", "denoise"); });

    expect(serviceMocks.setOnboardingInputMapping).toHaveBeenCalledWith("draft-1", {
      semanticKey: "strength",
      fieldType: "number",
      label: "Strength",
      required: false,
      defaultValue: "0.5",
      minValue: "0",
      maxValue: "1",
      step: "0.1",
      minItems: 2,
      maxItems: 5,
      itemIndex: 3,
      targetNode: "88",
      targetInput: "denoise",
    });
  });

  it("removes a mapping with the exact semantic key and item index", async () => {
    const { result } = renderHook(() => useWorkflowParameterExposureController(options()));
    await act(async () => { await result.current.open(workflow()); });
    vi.clearAllMocks();
    const target = mapping({ semanticKey: "reference_images", itemIndex: 4 });

    await act(async () => { await result.current.removeMapping(target); });

    expect(serviceMocks.removeOnboardingInputMapping).toHaveBeenCalledWith("draft-1", {
      semanticKey: "reference_images",
      itemIndex: 4,
    });
  });

  it("applies publish edits in order and blocks commit when validation is not ready", async () => {
    const hookOptions = options();
    const { result } = renderHook(() => useWorkflowParameterExposureController(hookOptions));
    await act(async () => { await result.current.open(workflow()); });
    vi.clearAllMocks();
    const notReady = { ...draft().validation, readyToPublish: false, issues: ["binding invalid"] };
    serviceMocks.setOnboardingInputMapping
      .mockResolvedValueOnce(draft({ draftId: "draft-2" }))
      .mockResolvedValueOnce(draft({ draftId: "draft-3" }));
    serviceMocks.validateOnboarding.mockResolvedValue(notReady);
    const edits: ParameterMappingEdit[] = [
      { mapping: mapping({ semanticKey: "first" }), draft: { ...mappingDraft("first"), label: "First" } },
      { mapping: mapping({ semanticKey: "second", targetInput: "cfg" }), draft: { ...mappingDraft("second"), label: "Second" } },
    ];

    await act(async () => { await result.current.publish(edits); });

    expect(serviceMocks.setOnboardingInputMapping).toHaveBeenNthCalledWith(1, "draft-1", expect.objectContaining({ semanticKey: "first" }));
    expect(serviceMocks.setOnboardingInputMapping).toHaveBeenNthCalledWith(2, "draft-2", expect.objectContaining({ semanticKey: "second", targetInput: "cfg" }));
    expect(serviceMocks.validateOnboarding).toHaveBeenCalledWith("draft-3");
    expect(serviceMocks.commitWorkflowImport).not.toHaveBeenCalled();
    expect(hookOptions.onError).toHaveBeenLastCalledWith(expect.stringContaining("输入绑定校验未通过"));
  });

  it("publishes a new recipe without changing the current workflow and refreshes both views", async () => {
    const hookOptions = options();
    const { result } = renderHook(() => useWorkflowParameterExposureController(hookOptions));
    await act(async () => { await result.current.open(workflow()); });
    vi.clearAllMocks();
    serviceMocks.validateOnboarding.mockResolvedValue(draft().validation);
    serviceMocks.commitWorkflowImport.mockResolvedValue({ recipeId: "recipe-published" });
    const edits: ParameterMappingEdit[] = [{
      mapping: mapping(),
      draft: { ...mappingDraft("published"), label: "Published" },
    }];

    await act(async () => { await result.current.publish(edits); });

    expect(serviceMocks.validateOnboarding).toHaveBeenCalledWith("draft-1");
    expect(serviceMocks.commitWorkflowImport).toHaveBeenCalledWith({
      draftId: "draft-1",
      action: "NEW_RECIPE",
      workflowId: "workflow-1",
      setCurrent: false,
    });
    expect(hookOptions.onWorkspaceRefresh).toHaveBeenCalledTimes(1);
    expect(hookOptions.onCatalogChanged).toHaveBeenCalledTimes(1);
    expect(hookOptions.onNotice).toHaveBeenCalledWith("已保存为配方 recipe-published；工作流版本保持不变。");
    expect(result.current.draft).toBeUndefined();
    expect(result.current.item).toBeUndefined();
    expect(result.current.originalKeys).toEqual([]);
  });
});

function mappingDraft(semanticKey: string): MappingDraft {
  return {
    semanticKey,
    fieldType: "integer",
    label: semanticKey,
    required: true,
    defaultValue: "20",
    minValue: "1",
    maxValue: "100",
    minItems: "",
    maxItems: "",
    itemIndex: "",
    step: "1",
  };
}
