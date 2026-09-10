// @vitest-environment jsdom

import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
  WorkflowAutoOnboardingPlanView,
  WorkflowOnboardingDraftView,
} from "../../../types/workflowOnboarding";
import { useWorkflowOnboardingStore } from "../../../stores/workflowOnboardingStore";
import { useWorkflowSmartImportController } from "./useWorkflowSmartImportController";

const serviceMocks = vi.hoisted(() => ({
  analyzeWorkflowImport: vi.fn(),
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
    vi.clearAllMocks();
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

  it("keeps resume and issue resolution read-only until explicit commit", async () => {
    serviceMocks.analyzeWorkflowImport
      .mockResolvedValueOnce(plan({ existingWorkflowId: "workflow-existing" }))
      .mockResolvedValueOnce(plan({ message: "已刷新" }));
    const { result } = renderHook(() => useWorkflowSmartImportController(options()));

    await act(async () => { await result.current.smartImport(); });
    await act(async () => { await result.current.resume(); });
    await act(async () => {
      await result.current.resolveIssue(
        { code: "AMBIGUOUS_DURATION_SOURCE", field: "duration_seconds", message: "choose", candidates: [] },
        { label: "duration", nodeId: "node-1", inputName: "duration", fieldType: "integer" },
      );
    });

    expect(serviceMocks.analyzeWorkflowImport).toHaveBeenNthCalledWith(2, "workflow-existing");
    expect(serviceMocks.setOnboardingInputMapping).toHaveBeenCalledWith("draft-1", expect.objectContaining({
      semanticKey: "duration_seconds",
      targetNode: "node-1",
      targetInput: "duration",
    }));
    expect(serviceMocks.commitWorkflowImport).not.toHaveBeenCalled();
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
});
