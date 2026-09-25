// @vitest-environment jsdom

import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
  WorkflowAutoIssueView,
  WorkflowAutoOnboardingPlanView,
  WorkflowOnboardingDraftView,
} from "../../../types/workflowOnboarding";
import { useWorkflowOnboardingStore } from "../../../stores/workflowOnboardingStore";
import { useWorkflowSmartImportController } from "./useWorkflowSmartImportController";

const serviceMocks = vi.hoisted(() => ({
  analyzeWorkflowImport: vi.fn(),
  reanalyzeWorkflowImport: vi.fn(),
  commitWorkflowImport: vi.fn(),
  discardOnboarding: vi.fn(),
  getOnboardingDraft: vi.fn(),
  rerecognizeWorkflow: vi.fn(),
  setOnboardingInputMapping: vi.fn(),
  setOnboardingMetadata: vi.fn(),
  setOnboardingOutputMapping: vi.fn(),
}));

vi.mock("../../../services/workflowClient", () => serviceMocks);

const draft: WorkflowOnboardingDraftView = {
  draftId: "draft-1",
  workflowSha256: "sha-1",
  originalFilename: "demo.json",
  nodeCount: 2,
  uniqueClassCount: 2,
  nodes: [{
    nodeId: "node-1",
    classType: "VideoNode",
    title: "视频",
    isOutputNode: true,
    inputs: [{
      name: "duration",
      kind: "number",
      currentValueSummary: "5",
      isLinked: false,
      bindable: true,
      suggestedType: "integer",
      suggestedSemanticKey: "duration_seconds",
      numericMin: "1",
      numericMax: "15",
      numericStep: "1",
      allowedOptions: [],
    }],
  }],
  capability: { state: "READY", issues: [] },
  inputMappings: [],
  outputMappings: [],
  manifest: {
    workflowId: "workflow-existing",
    name: "Demo Workflow",
    workflowVersion: "1.0.0",
    recipeVersion: "1.0.0",
    category: "video",
    mode: "CUSTOM_VIDEO",
    recipeId: "recipe-existing",
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
};

function plan(overrides: Partial<WorkflowAutoOnboardingPlanView> = {}): WorkflowAutoOnboardingPlanView {
  return {
    draftId: "draft-1",
    state: "NEEDS_REVIEW",
    commitRequired: true,
    workflowKind: "VIDEO",
    workflowSha256: "sha-1",
    originalFilename: "demo.json",
    nodeCount: 2,
    uniqueClassCount: 2,
    metadata: draft.manifest,
    semanticCapabilityStatus: "READY",
    runtimeImportStatus: "READY",
    runtimeImportBlockers: [],
    capability: { state: "READY", issues: [] },
    inputMappings: [],
    outputMappings: [],
    validation: draft.validation,
    inferences: [],
    issues: [],
    autoPublishable: true,
    message: "识别完成",
    ...overrides,
  };
}

function options(overrides: Partial<Parameters<typeof useWorkflowSmartImportController>[0]> = {}) {
  return {
    workspaceItems: [],
    importBusyRef: { current: false },
    onLoadWorkspace: vi.fn().mockResolvedValue(undefined),
    onCatalogChanged: vi.fn().mockResolvedValue(undefined),
    onDiscardReplacedDraft: vi.fn().mockResolvedValue(undefined),
    onResetImportView: vi.fn(),
    onCloseAdvanced: vi.fn(),
    onAdvancedRequested: vi.fn(),
    onPublished: vi.fn(),
    onOpenStudio: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

describe("useWorkflowSmartImportController", () => {
  afterEach(() => cleanup());

  beforeEach(() => {
    vi.resetAllMocks();
    useWorkflowOnboardingStore.getState().reset();
    serviceMocks.getOnboardingDraft.mockResolvedValue(draft);
    serviceMocks.setOnboardingInputMapping.mockResolvedValue(draft);
    serviceMocks.setOnboardingOutputMapping.mockResolvedValue(draft);
    serviceMocks.setOnboardingMetadata.mockResolvedValue(draft);
  });

  it("analyzes into a plan without implicitly committing", async () => {
    serviceMocks.analyzeWorkflowImport.mockResolvedValue(plan());
    const hookOptions = options();
    const { result } = renderHook(() => useWorkflowSmartImportController(hookOptions));

    await act(async () => { await result.current.smartImport(); });

    expect(serviceMocks.analyzeWorkflowImport).toHaveBeenCalledTimes(1);
    expect(serviceMocks.commitWorkflowImport).not.toHaveBeenCalled();
    expect(serviceMocks.getOnboardingDraft).toHaveBeenCalledWith("draft-1");
    expect(result.current.plan?.state).toBe("NEEDS_REVIEW");
    expect(useWorkflowOnboardingStore.getState().draft).toEqual(draft);
  });

  it("surfaces structured analyze errors and always clears loading", async () => {
    serviceMocks.analyzeWorkflowImport.mockRejectedValue({ code: "IMPORT_FAILED", message: "bad workflow" });
    const { result } = renderHook(() => useWorkflowSmartImportController(options()));

    await act(async () => { await result.current.smartImport(); });

    expect(result.current.importError).toMatchObject({ kind: "IMPORT_FAILED" });
    expect(useWorkflowOnboardingStore.getState().loading).toBe(false);
    expect(serviceMocks.commitWorkflowImport).not.toHaveBeenCalled();
  });

  it("keeps the active draft when replacing import is cancelled or fails", async () => {
    serviceMocks.analyzeWorkflowImport
      .mockResolvedValueOnce(plan())
      .mockResolvedValueOnce(null)
      .mockRejectedValueOnce({ code: "IMPORT_FAILED", message: "bad replacement" });
    const hookOptions = options();
    const { result } = renderHook(() => useWorkflowSmartImportController(hookOptions));

    await act(async () => { await result.current.smartImport(); });
    await act(async () => { await result.current.smartImport(); });
    expect(result.current.plan?.draftId).toBe("draft-1");
    expect(useWorkflowOnboardingStore.getState().draft?.draftId).toBe("draft-1");
    expect(hookOptions.onDiscardReplacedDraft).toHaveBeenCalledTimes(1);

    await act(async () => { await result.current.smartImport(); });
    expect(result.current.plan?.draftId).toBe("draft-1");
    expect(useWorkflowOnboardingStore.getState().draft?.draftId).toBe("draft-1");
    expect(result.current.importError).toMatchObject({ kind: "IMPORT_FAILED" });
    expect(hookOptions.onDiscardReplacedDraft).toHaveBeenCalledTimes(1);
  });

  it("resumes the current draft without reopening the file picker", async () => {
    serviceMocks.analyzeWorkflowImport
      .mockResolvedValueOnce(plan({
        state: "WAITING_FOR_COMFY_UI",
        capability: { state: "COMFY_OFFLINE", issues: [] },
      }));
    serviceMocks.reanalyzeWorkflowImport.mockResolvedValue(plan({ message: "已刷新" }));
    const { result } = renderHook(() => useWorkflowSmartImportController(options()));

    await act(async () => { await result.current.smartImport(); });
    await act(async () => { await result.current.resume(); });

    expect(serviceMocks.analyzeWorkflowImport).toHaveBeenCalledTimes(1);
    expect(serviceMocks.reanalyzeWorkflowImport).toHaveBeenCalledWith("draft-1");
    expect(result.current.plan?.message).toBe("已刷新");
    expect(useWorkflowOnboardingStore.getState().draft?.draftId).toBe("draft-1");
    expect(serviceMocks.commitWorkflowImport).not.toHaveBeenCalled();
  });

  it("retains a pending UI draft instead of discarding it as a format error", async () => {
    serviceMocks.analyzeWorkflowImport.mockResolvedValue(plan({
      sourceFormat: "UI",
      normalizationState: "UI_SOURCE_PENDING",
      state: "WAITING_FOR_COMFY_UI",
      capability: { state: "COMFY_OFFLINE", issues: [] },
    }));
    const { result } = renderHook(() => useWorkflowSmartImportController(options()));

    await act(async () => { await result.current.smartImport(); });

    expect(serviceMocks.getOnboardingDraft).toHaveBeenCalledWith("draft-1");
    expect(useWorkflowOnboardingStore.getState().draft).toEqual(draft);
    expect(result.current.plan?.normalizationState).toBe("UI_SOURCE_PENDING");
  });

  it("resolves an input ambiguity in place and refreshes the same draft", async () => {
    const issue: WorkflowAutoIssueView = {
      code: "AMBIGUOUS_INPUT",
      field: "duration_seconds",
      message: "choose",
      candidates: [{ label: "duration", nodeId: "node-1", inputName: "duration", fieldType: "integer" }],
    };
    const nextDraft: WorkflowOnboardingDraftView = {
      ...draft,
      inputMappings: [{
        semanticKey: "duration_seconds",
        fieldType: "integer",
        label: "Duration",
        required: true,
        targetNode: "node-1",
        targetInput: "duration",
      }],
    };
    const nextPlan = plan({ issues: [], inputMappings: nextDraft.inputMappings, message: "识别完成，可以添加工作流。" });
    serviceMocks.analyzeWorkflowImport.mockResolvedValue(plan({ issues: [issue], autoPublishable: false }));
    serviceMocks.setOnboardingInputMapping.mockResolvedValue(nextDraft);
    serviceMocks.reanalyzeWorkflowImport.mockResolvedValue(nextPlan);
    serviceMocks.getOnboardingDraft
      .mockResolvedValueOnce(draft)
      .mockResolvedValueOnce(nextDraft);
    const { result } = renderHook(() => useWorkflowSmartImportController(options()));

    await act(async () => { await result.current.smartImport(); });
    await act(async () => { await result.current.resolveIssue(issue, issue.candidates[0]); });

    expect(serviceMocks.setOnboardingInputMapping).toHaveBeenCalledWith("draft-1", expect.objectContaining({
       semanticKey: "duration_seconds",
       targetNode: "node-1",
       targetInput: "duration",
    }));
    expect(serviceMocks.reanalyzeWorkflowImport).toHaveBeenCalledWith("draft-1");
    expect(useWorkflowOnboardingStore.getState().draft).toEqual(nextDraft);
    expect(result.current.plan?.issues).toEqual([]);
    expect(useWorkflowOnboardingStore.getState().notice).toBe("识别完成，可以添加工作流。");
    expect(serviceMocks.commitWorkflowImport).not.toHaveBeenCalled();
  });

  it("resolves an output ambiguity in place and preserves the selected output", async () => {
    const issue: WorkflowAutoIssueView = {
      code: "AMBIGUOUS_OUTPUT",
      field: "output_1",
      message: "choose",
      candidates: [{ label: "video", nodeId: "node-1", outputId: "output_1", outputType: "video" }],
    };
    const nextDraft: WorkflowOnboardingDraftView = {
      ...draft,
      outputMappings: [{ outputId: "output_1", label: "video", type: "video", nodeId: "node-1", required: true }],
    };
    const nextPlan = plan({ issues: [], outputMappings: nextDraft.outputMappings, message: "识别完成，可以添加工作流。" });
    serviceMocks.analyzeWorkflowImport.mockResolvedValue(plan({ issues: [issue], autoPublishable: false }));
    serviceMocks.setOnboardingOutputMapping.mockResolvedValue(nextDraft);
    serviceMocks.reanalyzeWorkflowImport.mockResolvedValue(nextPlan);
    serviceMocks.getOnboardingDraft
      .mockResolvedValueOnce(draft)
      .mockResolvedValueOnce(nextDraft);
    const { result } = renderHook(() => useWorkflowSmartImportController(options()));

    await act(async () => { await result.current.smartImport(); });
    await act(async () => { await result.current.resolveIssue(issue, issue.candidates[0]); });

    expect(serviceMocks.setOnboardingOutputMapping).toHaveBeenCalledWith("draft-1", {
      outputId: "output_1",
      label: "video",
      type: "video",
      nodeId: "node-1",
      required: true,
    });
    expect(serviceMocks.reanalyzeWorkflowImport).toHaveBeenCalledWith("draft-1");
    expect(useWorkflowOnboardingStore.getState().draft).toEqual(nextDraft);
    expect(result.current.plan?.issues).toEqual([]);
  });

  it("commits NEW_VERSION with the exact existing workflow identity", async () => {
    serviceMocks.analyzeWorkflowImport.mockResolvedValue(plan({ existingWorkflowId: "workflow-existing" }));
    serviceMocks.commitWorkflowImport.mockResolvedValue({
      workflowId: "workflow-existing",
      workflowVersion: "2.0.0",
      recipeVersion: "1.0.0",
      recipeId: "recipe-new",
      packageName: "Demo Workflow",
      workflowSha256: "sha-1",
      refreshed: { packagesFound: 1, valid: 1, invalid: 0, inserted: 0, reused: 1, errors: [] },
    });
    const hookOptions = options();
    const { result } = renderHook(() => useWorkflowSmartImportController(hookOptions));

    await act(async () => { await result.current.smartImport(); });
    await act(async () => { await result.current.commit("NEW_VERSION"); });

    expect(serviceMocks.commitWorkflowImport).toHaveBeenCalledWith({
      draftId: "draft-1",
      action: "NEW_VERSION",
      workflowId: "workflow-existing",
      setCurrent: true,
    });
    expect(hookOptions.onPublished).toHaveBeenCalledWith({ workflowId: "workflow-existing", recipeId: "recipe-new" });
    expect(hookOptions.onCatalogChanged).toHaveBeenCalledTimes(1);
    expect(result.current.plan?.state).toBe("AUTO_PUBLISHED");
  });

  it("routes Advanced editing through the Workspace callback without owning its view state", async () => {
    serviceMocks.analyzeWorkflowImport.mockResolvedValue(plan({ existingWorkflowId: "workflow-existing", existingWorkflowVersion: "1.0.0" }));
    const hookOptions = options();
    const { result } = renderHook(() => useWorkflowSmartImportController(hookOptions));

    await act(async () => { await result.current.smartImport(); });
    await act(async () => { await result.current.openAdvanced(); });
    expect(hookOptions.onAdvancedRequested).toHaveBeenCalledWith(draft);

    await act(async () => { await result.current.openExistingVersion(); });
    expect(serviceMocks.setOnboardingMetadata).toHaveBeenCalledWith("draft-1", expect.objectContaining({
      workflowId: "workflow-existing",
      workflowVersion: "1.0.1",
    }));
    expect(hookOptions.onAdvancedRequested).toHaveBeenLastCalledWith(draft);
  });

  it("refreshes Smart Import from the same draft after Advanced editing", async () => {
    const nextDraft = { ...draft, manifest: { ...draft.manifest, name: "Edited Workflow" } };
    serviceMocks.analyzeWorkflowImport.mockResolvedValue(plan());
    serviceMocks.reanalyzeWorkflowImport.mockResolvedValue(plan({ metadata: nextDraft.manifest, message: "高级编辑已刷新" }));
    serviceMocks.getOnboardingDraft
      .mockResolvedValueOnce(draft)
      .mockResolvedValueOnce(nextDraft);
    const { result } = renderHook(() => useWorkflowSmartImportController(options()));

    await act(async () => { await result.current.smartImport(); });
    await act(async () => { await result.current.reanalyzeCurrentDraft(); });

    expect(serviceMocks.analyzeWorkflowImport).toHaveBeenCalledTimes(1);
    expect(serviceMocks.reanalyzeWorkflowImport).toHaveBeenCalledWith("draft-1");
    expect(result.current.plan?.metadata.name).toBe("Edited Workflow");
    expect(useWorkflowOnboardingStore.getState().draft).toEqual(nextDraft);
  });
});
