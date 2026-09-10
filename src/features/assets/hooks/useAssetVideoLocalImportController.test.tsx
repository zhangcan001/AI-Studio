// @vitest-environment jsdom

import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RecipeViewModel } from "../../../types/generation";
import type { H3LocalImportInspection, H3ProjectSegment } from "../../../types/h3LocalImport";
import { MINIMAX_H3_FL2VA_WORKFLOW_ID } from "../../runtime/productRuntimeScope";
import { h3RecipeContract } from "../assetVideoBatch";
import { useAssetVideoLocalImportController } from "./useAssetVideoLocalImportController";

const mocks = vi.hoisted(() => ({
  commitH3LocalImport: vi.fn(),
  pickH3LocalImportDirectory: vi.fn(),
  rescanH3LocalImport: vi.fn(),
  updateH3ProjectSegmentDraft: vi.fn(),
}));

vi.mock("../../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../../services/tauriClient")>("../../../services/tauriClient");
  return {
    ...actual,
    commitH3LocalImport: mocks.commitH3LocalImport,
    pickH3LocalImportDirectory: mocks.pickH3LocalImportDirectory,
    rescanH3LocalImport: mocks.rescanH3LocalImport,
    updateH3ProjectSegmentDraft: mocks.updateH3ProjectSegmentDraft,
  };
});

const segment: H3ProjectSegment = {
  ordinal: 1,
  segmentId: "segment-1",
  folderName: "01-opening",
  generationMode: "FL2VA_IMAGE_TO_VIDEO",
  inferredMode: "FL2VA_IMAGE_TO_VIDEO",
  modeSource: "AUTO_INFERENCE",
  prompt: "original prompt",
  promptDisplayName: "prompt.txt",
  promptBytes: 16,
  width: 1344,
  height: 768,
  resolutionSource: "RECIPE_DEFAULT",
  durationSeconds: 5,
  durationSource: "RECIPE_DEFAULT",
  firstFrame: { id: "image-1", displayName: "image-1.png", kind: "image", sizeBytes: 10 },
  referenceImages: [],
  referenceAudios: [],
  referenceVideos: [],
  media: [],
  status: "READY",
  errors: [],
  warnings: [],
};

function inspection(overrides: Partial<H3LocalImportInspection> = {}): H3LocalImportInspection {
  return {
    sessionId: "session-1",
    displayRootName: "project-folder",
    mode: "PROJECT_FOLDER",
    detectedManifest: false,
    imageCount: 1,
    promptCount: 1,
    readyCount: 1,
    errorCount: 0,
    pairs: [],
    projectFolder: {
      displayRootName: "project-folder",
      segmentCount: 1,
      readyCount: 1,
      errorCount: 0,
      segments: [segment],
      errors: [],
      warnings: [],
    },
    errors: [],
    warnings: [],
    ...overrides,
  };
}

const recipe: RecipeViewModel = {
  workflowId: MINIMAX_H3_FL2VA_WORKFLOW_ID,
  workflowVersionId: "workflow-version-1",
  recipeId: "recipe-1",
  name: "MiniMax H3 FL2VA",
  category: "video",
  mode: "fl2va",
  fields: [
    { key: "duration_seconds", type: "integer", label: "时长", required: true, default: 5, min: 1, max: 15, step: 1 },
    { key: "width", type: "integer", label: "宽度", required: true, default: 1344, min: 32, max: 2048, step: 32 },
    { key: "height", type: "integer", label: "高度", required: true, default: 768, min: 32, max: 2048, step: 32 },
    { key: "prompt", type: "textarea", label: "提示词", required: true, default: "" },
    { key: "seed", type: "seed", label: "种子", defaultMode: "random" },
  ],
  outputTypes: ["video"],
};

function options(overrides: Partial<Parameters<typeof useAssetVideoLocalImportController>[0]> = {}) {
  return {
    projectId: "project-1",
    onBusyChange: vi.fn(),
    onNoticeChange: vi.fn(),
    onAdmissionChanged: vi.fn().mockResolvedValue(undefined),
    onCommittedBatch: vi.fn(),
    ...overrides,
  };
}

function commitOptions(overrides: Record<string, unknown> = {}) {
  return {
    catalog: [recipe],
    recipe,
    contract: h3RecipeContract(recipe),
    generationMode: "FL2VA_IMAGE_TO_VIDEO" as const,
    qualityProfile: "QUALITY" as const,
    durationSeconds: 6,
    width: 1344,
    height: 768,
    resolvedProjectRecipes: [],
    canCommit: true,
    ...overrides,
  };
}

describe("useAssetVideoLocalImportController", () => {
  afterEach(cleanup);

  beforeEach(() => {
    mocks.commitH3LocalImport.mockReset();
    mocks.pickH3LocalImportDirectory.mockReset();
    mocks.rescanH3LocalImport.mockReset();
    mocks.updateH3ProjectSegmentDraft.mockReset();
  });

  it("leaves the session unchanged when the directory picker is cancelled", async () => {
    mocks.pickH3LocalImportDirectory.mockResolvedValue(undefined);
    const busy = vi.fn();
    const { result } = renderHook(() => useAssetVideoLocalImportController(options({ onBusyChange: busy })));

    await act(async () => { await result.current.chooseDirectory(); });

    expect(mocks.pickH3LocalImportDirectory).toHaveBeenCalledWith("project-1", "PROJECT_FOLDER");
    expect(result.current.inspection).toBeUndefined();
    expect(result.current.segmentForms).toEqual({});
    expect(busy).toHaveBeenNthCalledWith(1, true);
    expect(busy).toHaveBeenLastCalledWith(false);
  });

  it("applies chosen inspection and rebuilds segment forms", async () => {
    mocks.pickH3LocalImportDirectory.mockResolvedValue(inspection());
    const { result } = renderHook(() => useAssetVideoLocalImportController(options()));

    await act(async () => { await result.current.chooseDirectory(); });

    expect(result.current.inspection?.sessionId).toBe("session-1");
    expect(result.current.segmentForms["segment-1"]).toMatchObject({
      mode: "FL2VA_IMAGE_TO_VIDEO",
      prompt: "original prompt",
      firstFrameId: "image-1",
    });
  });

  it("clears an invalid session after rescan failure", async () => {
    mocks.pickH3LocalImportDirectory.mockResolvedValue(inspection());
    mocks.rescanH3LocalImport.mockRejectedValue(new Error("session expired"));
    const notice = vi.fn();
    const { result } = renderHook(() => useAssetVideoLocalImportController(options({ onNoticeChange: notice })));

    await act(async () => { await result.current.chooseDirectory(); });
    await act(async () => { await result.current.rescan(); });

    expect(mocks.rescanH3LocalImport).toHaveBeenCalledWith("session-1", "PROJECT_FOLDER");
    expect(result.current.inspection).toBeUndefined();
    expect(result.current.segmentForms).toEqual({});
    expect(notice).toHaveBeenLastCalledWith("操作失败，请查看技术详情。");
  });

  it("preserves the segment save and reset command payloads", async () => {
    mocks.pickH3LocalImportDirectory.mockResolvedValue(inspection());
    mocks.updateH3ProjectSegmentDraft.mockResolvedValue(inspection({ sessionId: "session-2" }));
    const { result } = renderHook(() => useAssetVideoLocalImportController(options()));

    await act(async () => { await result.current.chooseDirectory(); });
    act(() => result.current.updateSegmentForm("segment-1", {
      prompt: "edited",
      referenceImageIds: ["image-2"],
    }));
    await act(async () => { await result.current.saveSegment(segment); });

    expect(mocks.updateH3ProjectSegmentDraft).toHaveBeenNthCalledWith(1, {
      sessionId: "session-1",
      segmentId: "segment-1",
      mode: "FL2VA_IMAGE_TO_VIDEO",
      prompt: "edited",
      durationSeconds: 5,
      width: 1344,
      height: 768,
      referenceImageIds: ["image-2"],
      referenceAudioIds: [],
      referenceVideoIds: [],
      firstFrameId: "image-1",
      lastFrameId: undefined,
    });

    await act(async () => { await result.current.resetSegment(segment); });
    expect(mocks.updateH3ProjectSegmentDraft).toHaveBeenNthCalledWith(2, {
      sessionId: "session-2",
      segmentId: "segment-1",
      resetAutoDetection: true,
    });
  });

  it("commits with exact workflow identities and clears the session on success", async () => {
    mocks.pickH3LocalImportDirectory.mockResolvedValue(inspection());
    mocks.commitH3LocalImport.mockResolvedValue({
      batchId: "batch-1",
      batchName: "custom batch",
      itemCount: 1,
      importedAssetCount: 1,
      autoStarted: false,
      warnings: ["warning"],
    });
    const onCommittedBatch = vi.fn();
    const onAdmissionChanged = vi.fn().mockResolvedValue(undefined);
    const { result } = renderHook(() => useAssetVideoLocalImportController(options({
      onCommittedBatch,
      onAdmissionChanged,
    })));

    await act(async () => { await result.current.chooseDirectory(); });
    act(() => {
      result.current.setBatchName(" custom batch ");
      result.current.setAutoStart(false);
    });
    await waitFor(() => expect(result.current.autoStart).toBe(false));
    await act(async () => { await result.current.commit(commitOptions()); });

    expect(mocks.commitH3LocalImport).toHaveBeenCalledWith(expect.objectContaining({
      sessionId: "session-1",
      batchName: "custom batch",
      workflowVersionId: "workflow-version-1",
      recipeId: "recipe-1",
      width: 1344,
      height: 768,
      durationSeconds: 6,
      autoStart: false,
      generationMode: "FL2VA_IMAGE_TO_VIDEO",
      qualityProfile: "QUALITY",
    }));
    expect(onCommittedBatch).toHaveBeenCalledWith({ batchId: "batch-1", autoStarted: false });
    expect(onAdmissionChanged).toHaveBeenCalledTimes(1);
    expect(result.current.inspection).toBeUndefined();
  });

  it("clears the local session when commit fails and does not retry", async () => {
    mocks.pickH3LocalImportDirectory.mockResolvedValue(inspection());
    mocks.commitH3LocalImport.mockRejectedValue(new Error("commit failed"));
    const notice = vi.fn();
    const { result } = renderHook(() => useAssetVideoLocalImportController(options({ onNoticeChange: notice })));

    await act(async () => { await result.current.chooseDirectory(); });
    await act(async () => { await result.current.commit(commitOptions()); });

    expect(mocks.commitH3LocalImport).toHaveBeenCalledTimes(1);
    expect(result.current.inspection).toBeUndefined();
    expect(result.current.segmentForms).toEqual({});
    expect(notice).toHaveBeenLastCalledWith("导入未完成，请重新选择项目文件夹。操作失败，请查看技术详情。");
  });
});
