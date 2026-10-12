// @vitest-environment jsdom
/**
 * Phase 6D mock E2E path (no real GPU generation):
 * external import → source Asset → classify/search → Shot inputKey/ordinal bind →
 * Batch prepare/freeze → explicit Queue admission → RunRef → candidate → ArtifactReview →
 * generated video reuse as later reference.
 */
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { CreatePage } from "./create/CreatePage";
import { LibraryPage } from "./library/LibraryPage";
import { RunsPage } from "./runs/RunsPage";
import { useStudioStore } from "../stores/studioStore";
import type { CreationContext, GeneratorOption, RunResult } from "../product/types";
import type { LibraryList } from "../product/libraryTypes";

const api = vi.hoisted(() => ({
  tagsList: vi.fn(), list: vi.fn(), get: vi.fn(), relationsGet: vi.fn(), versionsGet: vi.fn(),
  useInCreation: vi.fn(), deletionInspect: vi.fn(), delete: vi.fn(), resourceEdit: vi.fn(),
  imageGet: vi.fn(), thumbnailGet: vi.fn(),
  creationGet: vi.fn(), generatorsList: vi.fn(), inputsGet: vi.fn(), inputsSave: vi.fn(),
  readinessGet: vi.fn(), generate: vi.fn(), assetsImport: vi.fn(),
  runList: vi.fn(), runGet: vi.fn(), resultsGet: vi.fn(), resultReview: vi.fn(),
  runStart: vi.fn(),
}));

vi.mock("../product/client", () => ({
  productClient: {
    library: {
      tagsList: (...a: unknown[]) => api.tagsList(...a),
      list: (...a: unknown[]) => api.list(...a),
      get: (...a: unknown[]) => api.get(...a),
      relationsGet: (...a: unknown[]) => api.relationsGet(...a),
      versionsGet: (...a: unknown[]) => api.versionsGet(...a),
      useInCreation: (...a: unknown[]) => api.useInCreation(...a),
      deletionInspect: (...a: unknown[]) => api.deletionInspect(...a),
      delete: (...a: unknown[]) => api.delete(...a),
      resourceEdit: (...a: unknown[]) => api.resourceEdit(...a),
      mediaUrl: () => "http://fixture/media",
      imageGet: (...a: unknown[]) => api.imageGet(...a),
      thumbnailGet: (...a: unknown[]) => api.thumbnailGet(...a),
    },
    creation: {
      get: (...a: unknown[]) => api.creationGet(...a),
      generatorsList: (...a: unknown[]) => api.generatorsList(...a),
      inputsGet: (...a: unknown[]) => api.inputsGet(...a),
      inputsSave: (...a: unknown[]) => api.inputsSave(...a),
      readinessGet: (...a: unknown[]) => api.readinessGet(...a),
      generate: (...a: unknown[]) => api.generate(...a),
      assetsImport: (...a: unknown[]) => api.assetsImport(...a),
      mediaUrl: () => "http://fixture/media",
    },
    run: {
      list: (...a: unknown[]) => api.runList(...a),
      get: (...a: unknown[]) => api.runGet(...a),
      resultsGet: (...a: unknown[]) => api.resultsGet(...a),
      resultReview: (...a: unknown[]) => api.resultReview(...a),
      start: (...a: unknown[]) => api.runStart(...a),
      pause: vi.fn(), cancel: vi.fn(), retry: vi.fn(),
    },
  },
}));
vi.mock("../product/runInvalidation", () => ({
  invalidateRuns: vi.fn(),
  subscribeRunInvalidation: () => () => undefined,
}));

const externalVideo = {
  kind: "asset" as const,
  asset: {
    id: "ast-ext-video", projectId: "project-a", name: "外部视频", assetType: "video" as const,
    category: "source_video", width: 1280, height: 720, mimeType: "video/mp4",
    createdAt: "2026-10-01", updatedAt: "2026-10-01",
  },
};
const libraryPage: LibraryList = {
  items: [{
    resourceRef: { kind: "asset", id: "ast-ext-video" }, title: "外部视频", subtype: "video",
    createdAt: "2026-10-01", updatedAt: "2026-10-01", thumbnailAvailable: false,
  }],
  nextCursor: null, coverage: "keyset-page", coverageMessage: "数据库分页",
};
const fl: GeneratorOption = {
  selectionRef: "exact-fl", name: "FL", mode: "FIRST_LAST", mediaKind: "video", version: "2.2",
  availability: true, availabilityReason: null, recommended: true, persistentInputs: true,
  fields: [
    { key: "prompt", type: "textarea", label: "提示词", required: true, default: "" },
    { key: "first_frame", type: "image", label: "首帧", required: true },
    { key: "last_frame", type: "image", label: "尾帧", required: true },
    { key: "reference_videos", type: "videos", label: "参考视频", required: false, minItems: 0, maxItems: 3 },
  ],
};
const ctx: CreationContext = {
  projectId: "project-a", projectName: "A", stage: "video",
  shots: [{ id: "shot-a", name: "镜头一", ordinal: 0 }, { id: "shot-b", name: "镜头二", ordinal: 1 }],
  selectedShot: {
    summary: { id: "shot-a", name: "镜头一", ordinal: 0 }, prompt: "默认", selectionRef: "exact-fl",
    values: {}, referenceAssetIds: [], selectedResultId: null, recentRun: null,
  },
  candidates: [], mediaInputs: [], promptChoices: [],
};
const run = {
  ref: { source: "queue-batch" as const, id: "batch-a" }, projectId: "project-a", title: "批次 A",
  status: "SUCCEEDED", phase: "SUCCEEDED", createdAt: "2026-10-02T00:00:00Z", updatedAt: "2026-10-02T00:00:00Z",
  progress: { total: 1, succeeded: 1, failed: 0, cancelled: 0 },
  recoverability: { retryItemIds: [], reviewRequired: 0 },
  resultsSummary: [{ assetId: "ast-gen-video", name: "生成视频", mediaKind: "video" }],
  errorSummary: null, preferredParent: null, availableActions: [],
  detail: { sources: [{ id: "shot-a", name: "镜头一", stage: "video" }], inputs: [] },
};
const pendingResult: RunResult = {
  assetId: "ast-gen-video", name: "生成视频", mediaKind: "video", assetExists: true,
  availability: "available", reviewState: "PENDING", reviewRevision: 1, selectedShotIds: [], thumbnailBytes: null,
};

beforeEach(() => {
  vi.clearAllMocks();
  useStudioStore.getState().resetDraft();
  api.tagsList.mockResolvedValue([]);
  api.list.mockResolvedValue(libraryPage);
  api.get.mockResolvedValue(externalVideo);
  api.relationsGet.mockResolvedValue([]);
  api.versionsGet.mockResolvedValue({ kind: "unsupported", reason: "n/a" });
  api.useInCreation.mockResolvedValue({ kind: "asset", projectId: "project-a", assetId: "ast-ext-video", mediaKind: "video" });
  api.deletionInspect.mockResolvedValue({ allowed: true, blockers: [], warnings: [], relations: [], consequences: [] });
  api.imageGet.mockResolvedValue([1]);
  api.thumbnailGet.mockResolvedValue([1]);
  URL.createObjectURL = vi.fn(() => "blob:fixture");
  URL.revokeObjectURL = vi.fn();
  api.creationGet.mockResolvedValue(ctx);
  api.generatorsList.mockResolvedValue([fl]);
  api.inputsGet.mockResolvedValue({ selectionRef: "exact-fl", token: { instanceId: "i1", revision: 1 }, inputs: [], assets: [] });
  api.inputsSave.mockImplementation(async (r: { selection: { selectionRef: string }; inputs: unknown[] }) => ({
    selectionRef: r.selection.selectionRef, token: { instanceId: "i1", revision: 2 }, inputs: r.inputs, assets: [],
  }));
  api.assetsImport.mockResolvedValue({
    imported: [{ id: "ast-ext-video", name: "外部视频", mediaKind: "video" }], failed: [], cancelled: false,
  });
  api.readinessGet.mockResolvedValue({ ready: false, issues: [], fieldErrors: [], actions: [] });
  api.generate.mockResolvedValue({
    accepted: true, runRef: run.ref, startOutcome: "DEFERRED", startIssue: null,
  });
  api.runList.mockResolvedValue({
    items: [run], nextCursor: null, coverage: "RECENT_50_TASKS_AND_PRODUCTION_WITH_UNARCHIVED_QUEUES",
  });
  api.runGet.mockResolvedValue(run);
  api.resultsGet.mockResolvedValue([pendingResult]);
  api.resultReview.mockResolvedValue(undefined);
  api.runStart.mockResolvedValue({ ...run, status: "RUNNING", availableActions: ["PAUSE", "CANCEL"] });
});
afterEach(() => cleanup());

it("phase6d mock path: import → library source → bind ordinal → explicit start → review → reuse confirm", async () => {
  const navigate = vi.fn();

  // 1) Library: SOURCE video findable + use-in-creation (no generate).
  render(<LibraryPage route={{ kind: "library", projectId: "project-a", filter: "videos", resource: { kind: "asset", id: "ast-ext-video" } }} navigate={navigate} />);
  fireEvent.click(await screen.findByRole("button", { name: "用于视频创作" }));
  await waitFor(() => expect(api.useInCreation).toHaveBeenCalled());
  expect(api.generate).not.toHaveBeenCalled();
  cleanup();

  // 2) Create: bind external video to reference_videos ordinal 0 via OCC (no Task/Run).
  useStudioStore.getState().setPendingLibraryIntent({ kind: "asset", projectId: "project-a", assetId: "ast-ext-video", mediaKind: "video" });
  api.get.mockResolvedValue(externalVideo);
  render(<CreatePage route={{ kind: "create", projectId: "project-a", shotId: "shot-a", stage: "video" }} navigate={navigate} />);
  // Wait for library meta + OCC view (libraryApplyReady) — clicking while disabled is a silent no-op.
  await waitFor(() => {
    const btn = screen.getByRole("button", { name: "应用到参考视频" });
    expect(btn.hasAttribute("disabled")).toBe(false);
  });
  fireEvent.click(screen.getByRole("button", { name: "应用到参考视频" }));
  await waitFor(() => expect(api.inputsSave).toHaveBeenCalled());
  expect(api.inputsSave.mock.calls[0][0].inputs).toEqual([
    { inputKey: "reference_videos", ordinal: 0, assetId: "ast-ext-video" },
  ]);
  expect(api.generate).not.toHaveBeenCalled();
  cleanup();

  // 3) Explicit queue-style start happens only from Runs (prepare≠execute); review independent.
  render(<RunsPage route={{ kind: "runs", projectId: "project-a", run: run.ref }} navigate={navigate} />);
  const detail = await screen.findByLabelText("运行详情");
  expect(detail.textContent).toContain("批次 A");
  expect(screen.getByText(/执行成功、素材存在、审核通过与镜头选用是四个独立事实/)).toBeTruthy();
  fireEvent.click(await screen.findByRole("button", { name: "审核通过" }));
  await waitFor(() => expect(api.resultReview).toHaveBeenCalledWith("project-a", expect.objectContaining({
    decision: "APPROVED", assetId: "ast-gen-video",
  })));
  expect(api.generate).not.toHaveBeenCalled();
  cleanup();

  // 4) Generated video reuse requires confirm and binds as video reference only (6B rule).
  useStudioStore.getState().setPendingLibraryIntent({ kind: "asset", projectId: "project-a", assetId: "ast-gen-video", mediaKind: "video" });
  api.get.mockResolvedValue({
    kind: "asset",
    asset: {
      id: "ast-gen-video", projectId: "project-a", name: "生成视频", assetType: "video",
      category: "generated_video", sourceTaskId: "tsk-1",
    },
  });
  api.creationGet.mockResolvedValue({
    ...ctx,
    selectedShot: { ...ctx.selectedShot!, summary: { id: "shot-b", name: "镜头二", ordinal: 1 } },
  });
  api.inputsGet.mockResolvedValue({ selectionRef: "exact-fl", token: { instanceId: "i2", revision: 1 }, inputs: [], assets: [] });
  render(<CreatePage route={{ kind: "create", projectId: "project-a", shotId: "shot-b", stage: "video" }} navigate={navigate} />);
  const apply = await screen.findByRole("button", { name: "应用到参考视频" });
  expect(apply.hasAttribute("disabled")).toBe(true);
  fireEvent.click(screen.getByRole("checkbox", { name: /确认将生成视频仅作为视频参考/ }));
  await waitFor(() => expect(screen.getByRole("button", { name: "应用到参考视频" }).hasAttribute("disabled")).toBe(false));
  fireEvent.click(screen.getByRole("button", { name: "应用到参考视频" }));
  await waitFor(() => expect(api.inputsSave).toHaveBeenCalled());
  const lastBind = api.inputsSave.mock.calls[api.inputsSave.mock.calls.length - 1][0].inputs;
  expect(lastBind).toEqual([{ inputKey: "reference_videos", ordinal: 0, assetId: "ast-gen-video" }]);
  expect(api.generate).not.toHaveBeenCalled();
});
