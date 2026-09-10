// @vitest-environment jsdom

import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useWorkflowOnboardingStore } from "../../../stores/workflowOnboardingStore";
import type { WorkflowInputView, WorkflowOnboardingDraftView } from "../../../types/workflowOnboarding";
import { mappingKey } from "../workflowParameterExposureModel";
import { useWorkflowAdvancedOnboardingController } from "./useWorkflowAdvancedOnboardingController";

const serviceMocks = vi.hoisted(() => ({
  checkOnboardingCapability: vi.fn(),
  commitWorkflowImport: vi.fn(),
  discardOnboarding: vi.fn(),
  getOnboardingDraft: vi.fn(),
  removeOnboardingInputMapping: vi.fn(),
  setOnboardingInputMapping: vi.fn(),
  setOnboardingMetadata: vi.fn(),
  setOnboardingOutputMapping: vi.fn(),
  validateOnboarding: vi.fn(),
}));

vi.mock("../../../services/workflowClient", () => serviceMocks);

function draft(overrides: Partial<WorkflowOnboardingDraftView> = {}): WorkflowOnboardingDraftView {
  return {
    draftId: "draft-advanced-1",
    workflowSha256: "workflow-sha",
    originalFilename: "workflow.json",
    nodeCount: 2,
    uniqueClassCount: 2,
    nodes: [
      {
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
      },
      {
        nodeId: "output-1",
        classType: "SaveImage",
        title: "输出",
        isOutputNode: true,
        inputs: [],
      },
    ],
    capability: { state: "READY", issues: [] },
    inputMappings: [],
    outputMappings: [],
    manifest: {
      workflowId: "workflow-advanced-1",
      name: "Advanced Workflow",
      workflowVersion: "1.0.0",
      recipeVersion: "1.0.0",
      category: "image",
      mode: "CUSTOM_IMAGE",
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
    onLoadWorkspace: vi.fn().mockResolvedValue(undefined),
    onCatalogChanged: vi.fn().mockResolvedValue(undefined),
    onResetSmartImport: vi.fn(),
  };
}

describe("useWorkflowAdvancedOnboardingController", () => {
  afterEach(() => cleanup());

  beforeEach(() => {
    vi.clearAllMocks();
    useWorkflowOnboardingStore.getState().reset();
    serviceMocks.checkOnboardingCapability.mockResolvedValue({ state: "READY", issues: [] });
    serviceMocks.commitWorkflowImport.mockResolvedValue({
      workflowId: "workflow-advanced-1",
      recipeId: "recipe-published",
      packageName: "Advanced Workflow",
    });
    serviceMocks.discardOnboarding.mockResolvedValue(undefined);
    serviceMocks.getOnboardingDraft.mockResolvedValue(draft());
    serviceMocks.removeOnboardingInputMapping.mockResolvedValue(draft());
    serviceMocks.setOnboardingInputMapping.mockResolvedValue(draft());
    serviceMocks.setOnboardingMetadata.mockResolvedValue(draft());
    serviceMocks.setOnboardingOutputMapping.mockResolvedValue(draft());
    serviceMocks.validateOnboarding.mockResolvedValue(draft().validation);
  });

  it("owns advanced session state and supports close/open", async () => {
    const currentDraft = draft();
    useWorkflowOnboardingStore.getState().setDraft(currentDraft);
    const { result } = renderHook(() => useWorkflowAdvancedOnboardingController(options()));

    expect(result.current.metadataDraft?.workflowId).toBe("workflow-advanced-1");
    expect(result.current.outputDraft.nodeId).toBe("output-1");

    await act(async () => { result.current.openAdvanced(); });
    expect(result.current.showAdvanced).toBe(true);
    act(() => { result.current.hideAdvanced(); });
    expect(result.current.showAdvanced).toBe(false);
  });

  it("checks capability and reads the authoritative draft", async () => {
    useWorkflowOnboardingStore.getState().setDraft(draft());
    const refreshed = draft({ capability: { state: "MISSING_NODES", issues: [] } });
    serviceMocks.getOnboardingDraft.mockResolvedValue(refreshed);
    const { result } = renderHook(() => useWorkflowAdvancedOnboardingController(options()));

    await act(async () => { await result.current.checkCapability(); });

    expect(serviceMocks.checkOnboardingCapability).toHaveBeenCalledWith("draft-advanced-1");
    expect(serviceMocks.getOnboardingDraft).toHaveBeenCalledWith("draft-advanced-1");
    expect(result.current.draft?.capability.state).toBe("MISSING_NODES");
    expect(useWorkflowOnboardingStore.getState().step).toBe("compatibility");
  });

  it("preserves input and output mapping payload semantics", async () => {
    useWorkflowOnboardingStore.getState().setDraft(draft());
    const { result } = renderHook(() => useWorkflowAdvancedOnboardingController(options()));

    act(() => {
      result.current.patchMapping(mappingKey("88", "steps"), {
        semanticKey: "sampling_steps",
        fieldType: "integer",
        label: "Sampling Steps",
        required: false,
        defaultValue: "24",
        minValue: "1",
        maxValue: "80",
        step: "1",
        minItems: "2",
        maxItems: "5",
        itemIndex: "3",
      });
    });
    await act(async () => { await result.current.bindInput("88", input()); });

    expect(serviceMocks.setOnboardingInputMapping).toHaveBeenCalledWith("draft-advanced-1", {
      semanticKey: "sampling_steps",
      fieldType: "integer",
      label: "Sampling Steps",
      required: false,
      defaultValue: "24",
      minValue: "1",
      maxValue: "80",
      step: "1",
      minItems: 2,
      maxItems: 5,
      itemIndex: 3,
      targetNode: "88",
      targetInput: "steps",
    });

    act(() => {
      result.current.setOutputDraft({
        outputId: "final",
        label: "Final Image",
        type: "image",
        nodeId: "output-1",
        required: true,
      });
    });
    await act(async () => { await result.current.addOutput(); });

    expect(serviceMocks.setOnboardingOutputMapping).toHaveBeenCalledWith("draft-advanced-1", {
      outputId: "final",
      label: "Final Image",
      type: "image",
      nodeId: "output-1",
      required: true,
    });
  });

  it("does not write mappings for unsupported inputs and removes by exact identity", async () => {
    useWorkflowOnboardingStore.getState().setDraft(draft({
      inputMappings: [{
        semanticKey: "references",
        fieldType: "images",
        label: "References",
        required: false,
        targetNode: "88",
        targetInput: "steps",
        itemIndex: 4,
      }],
    }));
    const { result } = renderHook(() => useWorkflowAdvancedOnboardingController(options()));

    await act(async () => {
      await result.current.bindInput("88", input({ bindable: false, isLinked: false, suggestedType: "float" }));
      await result.current.bindInput("88", input({ bindable: false, isLinked: true, suggestedSemanticKey: "model" }));
    });
    expect(serviceMocks.setOnboardingInputMapping).not.toHaveBeenCalled();

    await act(async () => {
      await result.current.removeInput({
        semanticKey: "references",
        fieldType: "images",
        label: "References",
        required: false,
        targetNode: "88",
        targetInput: "steps",
        itemIndex: 4,
      });
    });
    expect(serviceMocks.removeOnboardingInputMapping).toHaveBeenCalledWith("draft-advanced-1", {
      semanticKey: "references",
      itemIndex: 4,
    });
  });

  it("saves metadata and validates before publishing", async () => {
    useWorkflowOnboardingStore.getState().setDraft(draft());
    const hookOptions = options();
    const { result } = renderHook(() => useWorkflowAdvancedOnboardingController(hookOptions));

    act(() => {
      result.current.setMetadataDraft({ ...result.current.metadataDraft!, name: "Renamed Workflow" });
    });
    await act(async () => { await result.current.saveMetadata(); });
    expect(serviceMocks.setOnboardingMetadata).toHaveBeenCalledWith("draft-advanced-1", expect.objectContaining({
      workflowId: "workflow-advanced-1",
      name: "Renamed Workflow",
    }));

    const notReady = { ...draft().validation, readyToPublish: false, issues: ["binding invalid"] };
    serviceMocks.validateOnboarding.mockResolvedValue(notReady);
    await act(async () => { await result.current.validateDraft(); });
    expect(serviceMocks.validateOnboarding).toHaveBeenCalledWith("draft-advanced-1");
    expect(useWorkflowOnboardingStore.getState().step).toBe("validate");

    await act(async () => { await result.current.publishDraft(); });
    expect(serviceMocks.commitWorkflowImport).not.toHaveBeenCalled();
  });

  it("publishes a new workflow with exact draft identity and refreshes both views", async () => {
    useWorkflowOnboardingStore.getState().setDraft(draft());
    const hookOptions = options();
    const { result } = renderHook(() => useWorkflowAdvancedOnboardingController(hookOptions));

    await act(async () => { await result.current.publishDraft(); });

    expect(serviceMocks.commitWorkflowImport).toHaveBeenCalledWith({
      draftId: "draft-advanced-1",
      action: "NEW_WORKFLOW",
      setCurrent: false,
    });
    expect(hookOptions.onLoadWorkspace).toHaveBeenCalledWith("refresh");
    expect(hookOptions.onCatalogChanged).toHaveBeenCalledTimes(1);
    expect(result.current.published).toEqual({ workflowId: "workflow-advanced-1", recipeId: "recipe-published" });
    expect(useWorkflowOnboardingStore.getState().step).toBe("publish");
  });

  it("clears all local state when discard fails", async () => {
    useWorkflowOnboardingStore.getState().setDraft(draft());
    const hookOptions = options();
    const { result } = renderHook(() => useWorkflowAdvancedOnboardingController(hookOptions));
    act(() => { result.current.openAdvanced(); });
    serviceMocks.discardOnboarding.mockRejectedValueOnce(new Error("already consumed"));

    await act(async () => { await result.current.discardDraft(); });

    expect(serviceMocks.discardOnboarding).toHaveBeenCalledWith("draft-advanced-1");
    expect(useWorkflowOnboardingStore.getState().draft).toBeUndefined();
    expect(result.current.showAdvanced).toBe(false);
    expect(result.current.metadataDraft).toBeUndefined();
    expect(result.current.published).toBeUndefined();
    expect(useWorkflowOnboardingStore.getState().loading).toBe(false);
    expect(useWorkflowOnboardingStore.getState().error).toBeTruthy();
    expect(hookOptions.onResetSmartImport).toHaveBeenCalledTimes(1);
  });
});
